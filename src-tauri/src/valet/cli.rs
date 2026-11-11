//! Reads the state from `valet state --json` (valet-linux-plus >= 2.1.3).

use std::path::Path;

use super::error::StateError;
use super::model::{SUPPORTED_SCHEMA_VERSION, Source, ValetState, clean_php_version};
use super::runner::HostRunner;

pub fn from_cli(runner: &dyn HostRunner, valet: &Path) -> Result<ValetState, StateError> {
    let out = runner.run(&valet.to_string_lossy(), &["state", "--json"])?;
    if !out.success() {
        let message = first_line(&out.stderr).or_else(|| first_line(&out.stdout)).unwrap_or_default();
        return Err(StateError::CommandFailed { code: out.code, message });
    }

    let mut state = parse(&out.stdout)?;
    let cli = &mut state.php.cli;
    cli.version = clean_php_version(cli.version.as_deref(), cli.binary.as_deref());
    state.source = Source::Cli;
    state.valet_path = Some(valet.to_string_lossy().into_owned());
    Ok(state)
}

/// Parses the payload and checks `schema_version` before mapping it to the struct.
pub fn parse(stdout: &str) -> Result<ValetState, StateError> {
    let value: serde_json::Value = serde_json::from_str(json_body(stdout))?;

    let found = value.get("schema_version").and_then(serde_json::Value::as_u64);
    if found != Some(SUPPORTED_SCHEMA_VERSION) {
        return Err(StateError::UnsupportedSchema { found, supported: SUPPORTED_SCHEMA_VERSION });
    }

    Ok(serde_json::from_value(value)?)
}

/// PHP may print deprecation notices before the JSON; start at the first line opening an object.
fn json_body(stdout: &str) -> &str {
    let trimmed = stdout.trim_start();
    if trimmed.starts_with('{') {
        return trimmed;
    }
    stdout.find("\n{").map_or(stdout, |i| &stdout[i + 1..])
}

fn first_line(text: &str) -> Option<String> {
    text.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::model::SiteType;
    use crate::valet::runner::fake::FakeRunner;

    const V1: &str = include_str!("../../tests/fixtures/state_v1.json");
    const V2: &str = include_str!("../../tests/fixtures/state_v2.json");

    #[test]
    fn parses_schema_v1() {
        let state = parse(V1).unwrap();

        assert_eq!(state.schema_version, 1);
        assert_eq!(state.valet_version.as_deref(), Some("2.1.3"));
        assert_eq!(state.domain, "test");
        assert_eq!(state.port, "80");
        assert_eq!(state.php.global.as_deref(), Some("8.5"));
        assert_eq!(state.php.cli.binary.as_deref(), Some("/usr/bin/php8.3"));
        assert_eq!(state.php.installed, ["8.1", "8.2", "8.3", "8.4", "8.5"]);
        assert_eq!(state.paths.len(), 2);
        assert_eq!(state.source, Source::Cli);

        let isolated = state.sites.iter().find(|s| s.name == "shop").unwrap();
        assert_eq!(isolated.site_type, SiteType::Linked);
        assert!(isolated.secure);
        assert_eq!(isolated.isolated_php.as_deref(), Some("8.3"));
        assert_eq!(isolated.php.as_deref(), Some("8.3"));

        let proxy = state.sites.iter().find(|s| s.site_type == SiteType::Proxy).unwrap();
        assert_eq!(proxy.path, None);
        assert_eq!(proxy.php, None);
        assert_eq!(proxy.proxy_host.as_deref(), Some("http://127.0.0.1:8080"));

        let redis = state.services.iter().find(|s| s.name == "redis").unwrap();
        assert_eq!(redis.active, None);
        let nginx = state.services.iter().find(|s| s.name == "nginx").unwrap();
        assert_eq!(nginx.active, Some(true));
    }

    #[test]
    fn tolerates_unknown_fields_and_site_types() {
        let mut value: serde_json::Value = serde_json::from_str(V1).unwrap();
        value["new_top_level"] = serde_json::json!({"x": 1});
        value["sites"][0]["type"] = serde_json::json!("worktree");
        value["sites"][0]["driver"] = serde_json::json!("laravel");

        let state = parse(&value.to_string()).unwrap();
        assert_eq!(state.sites[0].site_type, SiteType::Other);
    }

    #[test]
    fn rejects_other_schema_versions() {
        let err = parse(V2).unwrap_err();
        assert!(matches!(err, StateError::UnsupportedSchema { found: Some(2), .. }));

        let err = parse(r#"{"domain":"test"}"#).unwrap_err();
        assert!(matches!(err, StateError::UnsupportedSchema { found: None, .. }));
    }

    #[test]
    fn skips_noise_before_the_json() {
        let noisy = format!("PHP Deprecated: something in foo.php\n{V1}");
        assert!(parse(&noisy).is_ok());
    }

    #[test]
    fn from_cli_fails_when_command_is_missing() {
        let runner = FakeRunner::new().respond("/bin/valet state --json", 1, "");
        let err = from_cli(&runner, Path::new("/bin/valet")).unwrap_err();
        assert!(matches!(err, StateError::CommandFailed { code: Some(1), .. }));
    }

    #[test]
    fn from_cli_fails_on_non_json_output() {
        let runner = FakeRunner::new().ok("/bin/valet state --json", "Command \"state\" is not defined.");
        assert!(matches!(from_cli(&runner, Path::new("/bin/valet")).unwrap_err(), StateError::InvalidJson(_)));
    }

    #[test]
    fn cleans_cli_php_version_noise() {
        let mut value: serde_json::Value = serde_json::from_str(V1).unwrap();
        value["php"]["cli"]["version"] = serde_json::json!("8.5.12Xdebug: [Step Debug] Could not connect");
        let runner = FakeRunner::new().ok("/bin/valet state --json", &value.to_string());
        let state = from_cli(&runner, Path::new("/bin/valet")).unwrap();
        assert_eq!(state.php.cli.version.as_deref(), Some("8.5.12"));

        value["php"]["cli"]["version"] = serde_json::Value::Null;
        let runner = FakeRunner::new().ok("/bin/valet state --json", &value.to_string());
        let state = from_cli(&runner, Path::new("/bin/valet")).unwrap();
        assert_eq!(state.php.cli.version.as_deref(), Some("8.3"), "falls back to the binary name");
    }

    #[test]
    fn from_cli_sets_source_and_path() {
        let runner = FakeRunner::new().ok("/bin/valet state --json", V1);
        let state = from_cli(&runner, Path::new("/bin/valet")).unwrap();
        assert_eq!(state.source, Source::Cli);
        assert_eq!(state.valet_path.as_deref(), Some("/bin/valet"));
    }
}
