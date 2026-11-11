//! Finds the `valet` binary installed by Composer.
//!
//! Order: `$PATH` → `~/.config/composer/vendor/bin/valet` → `~/.composer/vendor/bin/valet`
//! → `composer global config bin-dir --absolute`. A GUI session often has a shorter `$PATH`
//! than the user's shell, hence the Composer fallbacks.

use std::path::PathBuf;

use super::env::{HostEnv, is_executable};
use super::runner::HostRunner;

pub fn locate_valet(runner: &dyn HostRunner, env: &HostEnv) -> Option<PathBuf> {
    if let Some(found) = env.which("valet", &[]) {
        return Some(found);
    }

    let composer_homes =
        [env.home.join(".config/composer/vendor/bin/valet"), env.home.join(".composer/vendor/bin/valet")];
    if let Some(found) = composer_homes.into_iter().find(|p| is_executable(p)) {
        return Some(found);
    }

    composer_bin_dir(runner).map(|dir| dir.join("valet")).filter(|p| is_executable(p))
}

fn composer_bin_dir(runner: &dyn HostRunner) -> Option<PathBuf> {
    let out = runner.run("composer", &["global", "config", "bin-dir", "--absolute"]).ok()?;
    if !out.success() {
        return None;
    }
    // Composer may print "Changed current directory to ..." before the value.
    out.stdout.lines().map(str::trim).rfind(|l| l.starts_with('/')).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::runner::fake::FakeRunner;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn env(root: &Path, path_dirs: &[&Path]) -> HostEnv {
        HostEnv {
            home: root.join("home"),
            path: Some(std::env::join_paths(path_dirs).unwrap()),
            usr_bin: root.join("usr/bin"),
            force_files: false,
        }
    }

    #[test]
    fn prefers_path() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        executable(&bin.join("valet"));
        executable(&tmp.path().join("home/.config/composer/vendor/bin/valet"));

        let found = locate_valet(&FakeRunner::new(), &env(tmp.path(), &[&bin]));
        assert_eq!(found, Some(bin.join("valet")));
    }

    #[test]
    fn ignores_non_executable_files_in_path() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("valet"), "").unwrap();
        executable(&tmp.path().join("home/.config/composer/vendor/bin/valet"));

        let found = locate_valet(&FakeRunner::new(), &env(tmp.path(), &[&bin]));
        assert_eq!(found, Some(tmp.path().join("home/.config/composer/vendor/bin/valet")));
    }

    #[test]
    fn falls_back_to_legacy_composer_home() {
        let tmp = tempfile::tempdir().unwrap();
        executable(&tmp.path().join("home/.composer/vendor/bin/valet"));

        let found = locate_valet(&FakeRunner::new(), &env(tmp.path(), &[]));
        assert_eq!(found, Some(tmp.path().join("home/.composer/vendor/bin/valet")));
    }

    #[test]
    fn falls_back_to_composer_bin_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let custom = tmp.path().join("custom/bin");
        executable(&custom.join("valet"));
        let runner = FakeRunner::new().ok(
            "composer global config bin-dir --absolute",
            &format!("Changed current directory to /x\n{}\n", custom.display()),
        );

        let found = locate_valet(&runner, &env(tmp.path(), &[]));
        assert_eq!(found, Some(custom.join("valet")));
    }

    #[test]
    fn returns_none_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(locate_valet(&FakeRunner::new(), &env(tmp.path(), &[])), None);
    }
}
