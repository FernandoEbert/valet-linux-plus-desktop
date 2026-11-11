//! systemd unit status for the files fallback (`valet state` reports it itself).
//!
//! One `systemctl show` call for every candidate unit; no root needed.

use super::model::Service;
use super::runner::HostRunner;

/// A logical service and the unit names it may have depending on the distro, in priority order.
#[derive(Debug, Clone)]
pub struct ServiceSpec {
    pub name: String,
    pub candidates: Vec<String>,
}

impl ServiceSpec {
    pub fn new(name: impl Into<String>, candidates: &[&str]) -> Self {
        Self { name: name.into(), candidates: candidates.iter().map(|c| (*c).to_owned()).collect() }
    }

    /// PHP-FPM unit names used by the package managers supported by the CLI
    /// (`php8.3-fpm` on apt/dnf/yum, `php83-fpm` on pacman, `php-fpm` for the distro default).
    pub fn php_fpm(version: &str) -> Self {
        let no_dot = version.replace('.', "");
        Self {
            name: format!("php{version}"),
            candidates: vec![format!("php{version}-fpm"), format!("php{no_dot}-fpm"), "php-fpm".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitStatus {
    loaded: bool,
    active: bool,
}

/// Resolves each spec to its first installed unit and whether it is running.
/// When systemctl is unavailable every service comes back as `active: None`.
pub fn services_status(runner: &dyn HostRunner, specs: &[ServiceSpec]) -> (Vec<Service>, Option<String>) {
    let mut units: Vec<&str> = Vec::new();
    for unit in specs.iter().flat_map(|s| s.candidates.iter().map(String::as_str)) {
        if !units.contains(&unit) {
            units.push(unit);
        }
    }

    let (statuses, warning) = match query(runner, &units) {
        Ok(statuses) => (statuses, None),
        Err(message) => (Vec::new(), Some(message)),
    };
    let lookup = |unit: &str| units.iter().position(|u| *u == unit).and_then(|i| statuses.get(i));

    let services = specs
        .iter()
        .map(|spec| {
            let installed =
                spec.candidates.iter().find_map(|unit| lookup(unit).filter(|s| s.loaded).map(|s| (unit, s)));

            match installed {
                Some((unit, status)) => {
                    Service { name: spec.name.clone(), unit: unit.clone(), active: Some(status.active) }
                }
                None => Service {
                    name: spec.name.clone(),
                    unit: spec.candidates.first().cloned().unwrap_or_default(),
                    active: None,
                },
            }
        })
        .collect();

    (services, warning)
}

fn query(runner: &dyn HostRunner, units: &[&str]) -> Result<Vec<UnitStatus>, String> {
    if units.is_empty() {
        return Ok(Vec::new());
    }

    let mut args = vec!["show", "--property=LoadState,ActiveState", "--"];
    args.extend_from_slice(units);

    let out = runner.run("systemctl", &args).map_err(|e| format!("could not run systemctl: {e}"))?;
    let statuses = parse_show(&out.stdout);
    if statuses.len() != units.len() {
        return Err(format!("unexpected systemctl output (exit code {:?})", out.code));
    }
    Ok(statuses)
}

/// `systemctl show` prints one `Key=Value` block per unit, in argument order, separated by blank lines.
fn parse_show(stdout: &str) -> Vec<UnitStatus> {
    stdout
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .map(|block| {
            let value =
                |key: &str| block.lines().find_map(|l| l.trim().strip_prefix(key).and_then(|v| v.strip_prefix('=')));
            UnitStatus {
                loaded: value("LoadState").is_some_and(|v| v != "not-found" && v != "masked"),
                active: value("ActiveState") == Some("active"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::runner::fake::FakeRunner;

    const CMD: &str = "systemctl show --property=LoadState,ActiveState -- nginx mariadb mysql redis-server redis";

    fn specs() -> Vec<ServiceSpec> {
        vec![
            ServiceSpec::new("nginx", &["nginx"]),
            ServiceSpec::new("mysql", &["mariadb", "mysql"]),
            ServiceSpec::new("redis", &["redis-server", "redis"]),
        ]
    }

    #[test]
    fn maps_load_and_active_state() {
        let stdout = "LoadState=loaded\nActiveState=failed\n\n\
                      LoadState=not-found\nActiveState=inactive\n\n\
                      LoadState=loaded\nActiveState=active\n\n\
                      LoadState=not-found\nActiveState=inactive\n\n\
                      LoadState=not-found\nActiveState=inactive\n";
        let runner = FakeRunner::new().ok(CMD, stdout);

        let (services, warning) = services_status(&runner, &specs());

        assert_eq!(warning, None);
        assert_eq!(services[0], Service { name: "nginx".into(), unit: "nginx".into(), active: Some(false) });
        assert_eq!(services[1], Service { name: "mysql".into(), unit: "mysql".into(), active: Some(true) });
        assert_eq!(services[2], Service { name: "redis".into(), unit: "redis-server".into(), active: None });
    }

    #[test]
    fn unknown_when_systemctl_is_unavailable() {
        let (services, warning) = services_status(&FakeRunner::new(), &specs());

        assert!(warning.is_some());
        assert!(services.iter().all(|s| s.active.is_none()));
    }

    #[test]
    fn php_fpm_candidates() {
        assert_eq!(ServiceSpec::php_fpm("8.3").candidates, ["php8.3-fpm", "php83-fpm", "php-fpm"]);
    }
}
