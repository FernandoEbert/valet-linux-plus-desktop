//! Reads the Valet state: CLI first, files fallback otherwise.

use super::cli::from_cli;
use super::env::HostEnv;
use super::error::StateError;
use super::files::from_files;
use super::locate::locate_valet;
use super::model::ValetState;
use super::runner::{HostRunner, NativeRunner};

/// Reads the state with `valet state --json`; on any failure (binary not found, valet < 2.1.3,
/// unsupported schema, invalid output) falls back to the files and records why in `warnings`.
pub fn load_state(runner: &dyn HostRunner, env: &HostEnv) -> Result<ValetState, StateError> {
    let valet = locate_valet(runner, env);

    let reason = match (&valet, env.force_files) {
        (_, true) => "files source forced by VALET_UI_FORCE_FILES".to_owned(),
        (None, false) => "valet binary not found in PATH or Composer bin-dir".to_owned(),
        (Some(path), false) => match from_cli(runner, path) {
            Ok(state) => return Ok(state),
            Err(err) => err.to_string(),
        },
    };

    let mut state = from_files(runner, env, valet.as_deref())?;
    state.warnings.insert(0, reason);
    Ok(state)
}

/// [`load_state`] against the real host.
pub fn load_host_state() -> Result<ValetState, StateError> {
    load_state(&NativeRunner::default(), &HostEnv::from_process())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::model::Source;
    use crate::valet::runner::fake::FakeRunner;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    const V1: &str = include_str!("../../tests/fixtures/state_v1.json");
    const V2: &str = include_str!("../../tests/fixtures/state_v2.json");

    fn env_with_valet(root: &Path) -> (HostEnv, String) {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let valet = bin.join("valet");
        fs::write(&valet, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&valet, fs::Permissions::from_mode(0o755)).unwrap();

        let env = HostEnv {
            home: root.join("home"),
            path: Some(std::env::join_paths([&bin]).unwrap()),
            usr_bin: root.join("usr/bin"),
            force_files: false,
            helper: PathBuf::from("/nonexistent/valet-helper"),
        };
        fs::create_dir_all(env.valet_home()).unwrap();
        fs::write(env.valet_home().join("config.json"), r#"{"domain":"test","paths":[]}"#).unwrap();
        (env, valet.to_string_lossy().into_owned())
    }

    #[test]
    fn uses_cli_when_available() {
        let tmp = tempfile::tempdir().unwrap();
        let (env, valet) = env_with_valet(tmp.path());
        let runner = FakeRunner::new().ok(&format!("{valet} state --json"), V1);

        let state = load_state(&runner, &env).unwrap();

        assert_eq!(state.source, Source::Cli);
        assert_eq!(state.sites.len(), 4);
        assert!(state.warnings.is_empty());
    }

    #[test]
    fn falls_back_when_command_does_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let (env, valet) = env_with_valet(tmp.path());
        let runner = FakeRunner::new()
            .respond(&format!("{valet} state --json"), 1, "")
            .ok(&format!("{valet} --version"), "ValetLinux+ 2.0.4");

        let state = load_state(&runner, &env).unwrap();

        assert_eq!(state.source, Source::Files);
        assert_eq!(state.valet_version.as_deref(), Some("2.0.4"));
        assert!(state.warnings[0].contains("exited with code"));
    }

    #[test]
    fn falls_back_on_unsupported_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let (env, valet) = env_with_valet(tmp.path());
        let runner = FakeRunner::new().ok(&format!("{valet} state --json"), V2);

        let state = load_state(&runner, &env).unwrap();

        assert_eq!(state.source, Source::Files);
        assert!(state.warnings[0].contains("schema_version"));
    }

    #[test]
    fn falls_back_when_valet_is_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut env, _) = env_with_valet(tmp.path());
        env.path = None;

        let state = load_state(&FakeRunner::new(), &env).unwrap();

        assert_eq!(state.source, Source::Files);
        assert_eq!(state.valet_path, None);
        assert!(state.warnings[0].contains("not found"));
    }

    #[test]
    fn force_files_skips_the_cli() {
        let tmp = tempfile::tempdir().unwrap();
        let (mut env, valet) = env_with_valet(tmp.path());
        env.force_files = true;
        let runner = FakeRunner::new();

        let state = load_state(&runner, &env).unwrap();

        assert_eq!(state.source, Source::Files);
        assert!(!runner.called(&format!("{valet} state --json")));
    }

    /// Parity check against the real machine: `cargo test -- --ignored real_machine`.
    #[test]
    #[ignore = "needs valet >= 2.1.3 installed on this machine"]
    fn real_machine_cli_and_files_agree() {
        let runner = NativeRunner::default();
        let env = HostEnv {
            force_files: false,
            helper: PathBuf::from("/nonexistent/valet-helper"),
            ..HostEnv::from_process()
        };
        let cli = load_state(&runner, &env).unwrap();
        assert_eq!(cli.source, Source::Cli, "warnings: {:?}", cli.warnings);

        let files = load_state(&runner, &HostEnv { force_files: true, ..env }).unwrap();

        assert_eq!(files.domain, cli.domain);
        assert_eq!(files.paths, cli.paths);
        assert_eq!(files.php.global, cli.php.global);
        assert_eq!(files.php.cli, cli.php.cli);
        assert_eq!(files.php.installed, cli.php.installed);
        assert_eq!(files.valet_version, cli.valet_version);
        assert_eq!(files.sites, cli.sites);
        assert_eq!(files.services, cli.services);
    }
}
