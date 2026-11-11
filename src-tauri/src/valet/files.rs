//! Read-only fallback that rebuilds [`ValetState`] straight from `~/.config/valet`, for Valet
//! versions without `valet state --json` (< 2.1.3) or when that command fails.
//!
//! It mirrors `cli/Valet/StateReporter.php` and the helpers it relies on (`Site::servedSites`,
//! `SiteSecure::secured`, `SiteIsolate::isolatedPhpVersion`, `SiteProxy::proxies`,
//! `SiteNginxOverride::isActive`). Only the config keys listed in [`ConfigFile`] are read, so
//! secrets such as `mysql.password` never leave the file.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use super::env::HostEnv;
use super::error::StateError;
use super::model::{CliPhp, PhpInfo, SUPPORTED_SCHEMA_VERSION, Site, SiteType, Source, ValetState, scalar_string};
use super::runner::HostRunner;
use super::services::{ServiceSpec, services_status};

static PHP_VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:php[@-]?)?(\d).?(\d)$").expect("valid regex"));
static ISOLATED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^# ISOLATED_PHP_VERSION=(.*?)\r?$").expect("valid regex"));
static PROXY_PASS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"proxy_pass\s+(?P<host>https?://.*)\s*;").expect("valid regex"));
static SEMVER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+\.\d+\.\d+").expect("valid regex"));
static PHP_BIN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^php(\d)\.?(\d)$").expect("valid regex"));

/// The subset of `config.json` the UI needs. Everything else (credentials included) is ignored.
#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    domain: Option<String>,
    #[serde(default, deserialize_with = "optional_scalar")]
    port: Option<String>,
    #[serde(default, deserialize_with = "optional_scalar")]
    https_port: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    php_version: Option<String>,
    #[serde(default)]
    fallback_binary: Option<String>,
    #[serde(default)]
    custom_nginx: Vec<String>,
}

fn optional_scalar<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    scalar_string(d).map(Some)
}

pub fn from_files(runner: &dyn HostRunner, env: &HostEnv, valet: Option<&Path>) -> Result<ValetState, StateError> {
    let home = env.valet_home();
    let config = read_config(&home.join("config.json"))?;

    let domain = config.domain.clone().filter(|d| !d.is_empty()).unwrap_or_else(|| "test".to_owned());
    let port = config.port.clone().unwrap_or_else(|| "80".to_owned());
    let https_port = config.https_port.clone().unwrap_or_else(|| "443".to_owned());

    let cli = cli_php(runner, env);
    let global_php =
        config.php_version.as_deref().and_then(normalize_php).or_else(|| cli.version.as_deref().and_then(major_minor));

    let layout = Layout { home: &home, domain: &domain, port: &port, https_port: &https_port };
    let sites = sites(&layout, &config, global_php.as_deref());

    let (services, services_warning) = services_status(runner, &service_specs(global_php.as_deref(), &sites));

    Ok(ValetState {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        valet_version: valet.and_then(|v| valet_version(runner, v)),
        home: home.to_string_lossy().into_owned(),
        domain: domain.clone(),
        port: port.clone(),
        https_port: https_port.clone(),
        php: PhpInfo {
            global: global_php,
            cli,
            fallback_binary: config.fallback_binary.clone(),
            installed: installed_php_versions(&env.usr_bin),
        },
        paths: config.paths.clone(),
        sites,
        services,
        source: Source::Files,
        valet_path: valet.map(|v| v.to_string_lossy().into_owned()),
        warnings: services_warning.into_iter().collect(),
    })
}

fn read_config(path: &Path) -> Result<ConfigFile, StateError> {
    let display = path.to_string_lossy().into_owned();
    let raw = fs::read_to_string(path).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => StateError::NotInstalled(display.clone()),
        _ => StateError::Read { path: display.clone(), source },
    })?;
    serde_json::from_str(&raw).map_err(|source| StateError::InvalidConfig { path: display, source })
}

struct Layout<'a> {
    home: &'a Path,
    domain: &'a str,
    port: &'a str,
    https_port: &'a str,
}

impl Layout<'_> {
    fn sites_dir(&self) -> PathBuf {
        self.home.join("Sites")
    }

    fn nginx_file(&self, domain: &str) -> PathBuf {
        self.home.join("Nginx").join(domain)
    }

    fn url(&self, domain: &str, secure: bool) -> String {
        let (scheme, default, port) =
            if secure { ("https", "443", self.https_port) } else { ("http", "80", self.port) };
        let suffix = if port == default { String::new() } else { format!(":{port}") };
        format!("{scheme}://{domain}{suffix}")
    }
}

fn sites(layout: &Layout, config: &ConfigFile, global_php: Option<&str>) -> Vec<Site> {
    let secured = secured(&layout.home.join("Certificates"));
    let custom_nginx: HashSet<&str> = config.custom_nginx.iter().map(String::as_str).collect();
    let (served, linked) = served_sites(layout, &config.paths);

    let mut sites: Vec<Site> = served
        .iter()
        .map(|(name, path)| {
            let domain = format!("{name}.{}", layout.domain);
            let secure = secured.contains(&domain);
            let isolated_php = isolated_php(&layout.nginx_file(&domain));
            let phprc_php = path.as_deref().and_then(phprc_php);
            let php = isolated_php.clone().or_else(|| phprc_php.clone()).or_else(|| global_php.map(str::to_owned));

            Site {
                name: name.clone(),
                url: layout.url(&domain, secure),
                domain,
                path: path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                site_type: if linked.contains(name) { SiteType::Linked } else { SiteType::Parked },
                secure,
                php,
                isolated_php,
                phprc_php,
                custom_nginx: custom_nginx.contains(name.as_str()),
                proxy_host: None,
            }
        })
        .collect();

    sites.extend(proxies(layout, &served, &secured));
    sites.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    sites
}

/// Parked sites (sub-directories of each parked path, except `Sites/`) and linked sites
/// (entries of `Sites/`, resolved with realpath). A link wins over a parked site of the same name.
fn served_sites(layout: &Layout, paths: &[String]) -> (BTreeMap<String, Option<PathBuf>>, BTreeSet<String>) {
    let sites_dir = layout.sites_dir();
    let mut served = BTreeMap::new();

    for parked in paths.iter().map(PathBuf::from) {
        if parked.as_path() == sites_dir.as_path() {
            continue;
        }
        for name in scandir(&parked) {
            let dir = parked.join(&name);
            if dir.is_dir() {
                served.insert(name, Some(dir));
            }
        }
    }

    let mut linked = BTreeSet::new();
    for name in scandir(&sites_dir) {
        let link = sites_dir.join(&name);
        let target = fs::canonicalize(&link).ok().or_else(|| fs::read_link(&link).ok());
        served.insert(name.clone(), target);
        linked.insert(name);
    }

    (served, linked)
}

/// Directory entries like `Filesystem::scandir()` (skips `.keep`), empty when it can't be read.
fn scandir(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != ".keep")
        .collect();
    names.sort();
    names
}

/// Domains with any certificate file in `Certificates/`.
fn secured(certificates: &Path) -> HashSet<String> {
    scandir(certificates)
        .into_iter()
        .map(|file| {
            [".key", ".csr", ".crt", ".conf"]
                .iter()
                .fold(file, |acc, ext| acc.strip_suffix(ext).map(str::to_owned).unwrap_or(acc))
        })
        .collect()
}

fn isolated_php(nginx_file: &Path) -> Option<String> {
    let conf = fs::read_to_string(nginx_file).ok()?;
    if !conf.contains("# ISOLATED_PHP_VERSION") {
        return None;
    }
    ISOLATED_RE.captures(&conf).and_then(|c| normalize_php(c[1].trim()))
}

fn phprc_php(site_path: &Path) -> Option<String> {
    fs::read_to_string(site_path.join(".valetphprc")).ok().and_then(|v| normalize_php(v.trim()))
}

fn proxies(layout: &Layout, served: &BTreeMap<String, Option<PathBuf>>, secured: &HashSet<String>) -> Vec<Site> {
    let suffix = format!(".{}", layout.domain);

    scandir(&layout.home.join("Nginx"))
        .into_iter()
        .filter_map(|domain| {
            let name = domain.strip_suffix(&suffix)?.to_owned();
            if served.contains_key(&name) {
                return None;
            }
            let conf = fs::read_to_string(layout.nginx_file(&domain)).ok()?;
            let host = PROXY_PASS_RE.captures(&conf)?["host"].trim().to_owned();
            let secure = secured.contains(&domain);

            Some(Site {
                name,
                url: layout.url(&domain, secure),
                domain,
                path: None,
                site_type: SiteType::Proxy,
                secure,
                php: None,
                isolated_php: None,
                phprc_php: None,
                custom_nginx: false,
                proxy_host: Some(host),
            })
        })
        .collect()
}

/// Same list and order as `StateReporter::services()`.
fn service_specs(global_php: Option<&str>, sites: &[Site]) -> Vec<ServiceSpec> {
    let mut php_versions: Vec<&str> =
        sites.iter().filter_map(|s| s.isolated_php.as_deref()).chain(global_php).collect();
    php_versions.sort_by(|a, b| natural_cmp(a, b));
    php_versions.dedup();

    let mut specs = vec![ServiceSpec::new("nginx", &["nginx"]), ServiceSpec::new("dnsmasq", &["dnsmasq"])];
    specs.extend(php_versions.into_iter().map(ServiceSpec::php_fpm));
    specs.extend([
        ServiceSpec::new("mysql", &["mariadb", "mysql"]),
        ServiceSpec::new("redis", &["redis-server", "redis"]),
        ServiceSpec::new("mailpit", &["mailpit"]),
    ]);
    specs
}

/// PHP resolved by the CLI: first `php` in `$PATH`, ignoring `/usr/local/bin/php`
/// like `PhpFpm::getPhpExecutablePath()`.
fn cli_php(runner: &dyn HostRunner, env: &HostEnv) -> CliPhp {
    let Some(path) = env.which("php", &["/usr/local/bin/php"]) else {
        return CliPhp::default();
    };
    let path_str = path.to_string_lossy().into_owned();
    let version = runner
        .run(&path_str, &["-r", "echo PHP_VERSION;"])
        .ok()
        .filter(|o| o.success())
        .and_then(|o| SEMVER_RE.find(o.stdout.trim()).map(|m| m.as_str().to_owned()));

    CliPhp {
        binary: fs::canonicalize(&path).ok().map(|p| p.to_string_lossy().into_owned()),
        path: Some(path_str),
        version,
    }
}

fn valet_version(runner: &dyn HostRunner, valet: &Path) -> Option<String> {
    let out = runner.run(&valet.to_string_lossy(), &["--version"]).ok()?;
    SEMVER_RE.find(&out.stdout).map(|m| m.as_str().to_owned())
}

/// Versioned PHP binaries in `/usr/bin` (`php8.3`, `php83`).
fn installed_php_versions(usr_bin: &Path) -> Vec<String> {
    let mut versions: Vec<String> = scandir(usr_bin)
        .iter()
        .filter_map(|name| PHP_BIN_RE.captures(name).map(|c| format!("{}.{}", &c[1], &c[2])))
        .collect();
    versions.sort_by(|a, b| natural_cmp(a, b));
    versions.dedup();
    versions
}

/// `PhpFpm::normalizePhpVersion()`: `php8.3`, `php@8.3`, `83`, `8.3` → `8.3`.
fn normalize_php(version: &str) -> Option<String> {
    PHP_VERSION_RE.captures(version).map(|c| format!("{}.{}", &c[1], &c[2]))
}

fn major_minor(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    Some(format!("{}.{}", parts.next()?, parts.next()?))
}

/// Case-insensitive natural order like PHP's `strnatcasecmp` (`SORT_NATURAL | SORT_FLAG_CASE`),
/// which folds to upper case: `erpMaciel` sorts before `erp_ebertsystem`.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.to_uppercase(), b.to_uppercase());
    let (mut a, mut b) = (a.as_str(), b.as_str());

    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
                let nb = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
                let (da, db) = (a[..na].trim_start_matches('0'), b[..nb].trim_start_matches('0'));
                let ord = da.len().cmp(&db.len()).then_with(|| da.cmp(db));
                if ord != Ordering::Equal {
                    return ord;
                }
                (a, b) = (&a[na..], &b[nb..]);
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::runner::fake::FakeRunner;
    use std::os::unix::fs::{PermissionsExt, symlink};

    const SYSTEMCTL: &str = "systemctl show --property=LoadState,ActiveState -- \
        nginx dnsmasq php8.3-fpm php83-fpm php-fpm php8.5-fpm php85-fpm mariadb mysql redis-server redis mailpit";

    struct Fixture {
        _tmp: tempfile::TempDir,
        env: HostEnv,
        root: PathBuf,
    }

    impl Fixture {
        fn valet_home(&self) -> PathBuf {
            self.env.valet_home()
        }
        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    /// Fake machine:
    /// - parked path `sites/` with `blog` (custom nginx), `legacy` (.valetphprc 7.4) and `shop`
    ///   (shadowed by a link of the same name), plus a plain file that must be ignored;
    /// - `Sites/shop` → `projects/shop`, secured and isolated to 8.3;
    /// - proxy `api.test` (secured) and a non-proxy nginx stub;
    /// - `/usr/bin` with php8.3, php8.5 and unrelated binaries; `php` on PATH.
    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let env = HostEnv {
            home: root.join("home"),
            path: Some(std::env::join_paths([root.join("bin")]).unwrap()),
            usr_bin: root.join("usr/bin"),
            force_files: false,
        };
        let valet = env.valet_home();

        write(
            &valet.join("config.json"),
            &serde_json::json!({
                "domain": "test",
                "paths": [valet.join("Sites"), root.join("sites")],
                "port": 80,
                "mysql": {"user": "valet", "password": "s3cr3t"},
                "fallback_binary": "/usr/bin/php8.3",
                "php_version": "8.5",
                "isolated_versions": {"shop": "/usr/bin/php8.3"},
                "custom_nginx": ["blog"]
            })
            .to_string(),
        );

        fs::create_dir_all(root.join("sites/blog")).unwrap();
        write(&root.join("sites/legacy/.valetphprc"), "php7.4\n");
        fs::create_dir_all(root.join("sites/shop")).unwrap();
        write(&root.join("sites/README.md"), "not a site");
        fs::create_dir_all(root.join("projects/shop")).unwrap();
        fs::create_dir_all(valet.join("Sites")).unwrap();
        symlink(root.join("projects/shop"), valet.join("Sites/shop")).unwrap();

        for ext in ["crt", "key", "csr", "conf"] {
            write(&valet.join(format!("Certificates/shop.test.{ext}")), "");
            write(&valet.join(format!("Certificates/api.test.{ext}")), "");
        }
        write(&valet.join("Nginx/shop.test"), "# ISOLATED_PHP_VERSION=php8.3\nserver {}\n");
        write(
            &valet.join("Nginx/api.test"),
            "server {\n  location / {\n    proxy_pass http://127.0.0.1:8080;\n  }\n}\n",
        );
        write(&valet.join("Nginx/other.test"), "server { listen 443; }\n");

        for bin in ["php8.3", "php8.5", "phpize8.3", "php-config"] {
            write(&root.join("usr/bin").join(bin), "");
        }
        let php = root.join("bin/php");
        write(&php, "#!/bin/sh\n");
        fs::set_permissions(&php, fs::Permissions::from_mode(0o755)).unwrap();

        Fixture { _tmp: tmp, env, root }
    }

    fn runner(fx: &Fixture) -> FakeRunner {
        FakeRunner::new()
            .ok(&format!("{} -r echo PHP_VERSION;", fx.path("bin/php").display()), "8.3.36")
            .ok("/opt/valet --version", "ValetLinux+ 2.1.2\n")
            .ok(
                SYSTEMCTL,
                &[
                    "LoadState=loaded\nActiveState=active",      // nginx
                    "LoadState=loaded\nActiveState=active",      // dnsmasq
                    "LoadState=loaded\nActiveState=active",      // php8.3-fpm
                    "LoadState=not-found\nActiveState=inactive", // php83-fpm
                    "LoadState=not-found\nActiveState=inactive", // php-fpm
                    "LoadState=loaded\nActiveState=failed",      // php8.5-fpm
                    "LoadState=not-found\nActiveState=inactive", // php85-fpm
                    "LoadState=loaded\nActiveState=active",      // mariadb
                    "LoadState=loaded\nActiveState=active",      // mysql (alias)
                    "LoadState=not-found\nActiveState=inactive", // redis-server
                    "LoadState=not-found\nActiveState=inactive", // redis
                    "LoadState=not-found\nActiveState=inactive", // mailpit
                ]
                .join("\n\n"),
            )
    }

    fn site<'a>(state: &'a ValetState, name: &str) -> &'a Site {
        state.sites.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("site {name} missing"))
    }

    #[test]
    fn builds_state_from_files() {
        let fx = fixture();
        let state = from_files(&runner(&fx), &fx.env, Some(Path::new("/opt/valet"))).unwrap();

        assert_eq!(state.source, Source::Files);
        assert_eq!(state.schema_version, 1);
        assert_eq!(state.valet_version.as_deref(), Some("2.1.2"));
        assert_eq!(state.home, fx.valet_home().to_string_lossy());
        assert_eq!(state.domain, "test");
        assert_eq!(state.port, "80");
        assert_eq!(state.https_port, "443");
        assert_eq!(state.php.global.as_deref(), Some("8.5"));
        assert_eq!(state.php.cli.version.as_deref(), Some("8.3.36"));
        assert_eq!(state.php.fallback_binary.as_deref(), Some("/usr/bin/php8.3"));
        assert_eq!(state.php.installed, ["8.3", "8.5"]);
        assert_eq!(state.paths.len(), 2);

        let names: Vec<&str> = state.sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["api", "blog", "legacy", "shop"]);
    }

    #[test]
    fn resolves_site_types_ssl_and_php_priority() {
        let fx = fixture();
        let state = from_files(&runner(&fx), &fx.env, None).unwrap();

        let shop = site(&state, "shop");
        assert_eq!(shop.site_type, SiteType::Linked);
        assert_eq!(shop.path.as_deref(), Some(fs::canonicalize(fx.path("projects/shop")).unwrap().to_str().unwrap()));
        assert!(shop.secure);
        assert_eq!(shop.url, "https://shop.test");
        assert_eq!(shop.isolated_php.as_deref(), Some("8.3"));
        assert_eq!(shop.php.as_deref(), Some("8.3"));

        let legacy = site(&state, "legacy");
        assert_eq!(legacy.site_type, SiteType::Parked);
        assert_eq!(legacy.phprc_php.as_deref(), Some("7.4"));
        assert_eq!(legacy.php.as_deref(), Some("7.4"));
        assert_eq!(legacy.url, "http://legacy.test");

        let blog = site(&state, "blog");
        assert_eq!(blog.php.as_deref(), Some("8.5"));
        assert!(blog.custom_nginx);
        assert!(!blog.secure);

        let api = site(&state, "api");
        assert_eq!(api.site_type, SiteType::Proxy);
        assert_eq!(api.proxy_host.as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(api.path, None);
        assert_eq!(api.php, None);
        assert_eq!(api.url, "https://api.test");
    }

    #[test]
    fn isolated_php_wins_over_phprc() {
        let fx = fixture();
        write(&fx.path("projects/shop/.valetphprc"), "7.4");
        let state = from_files(&runner(&fx), &fx.env, None).unwrap();

        let shop = site(&state, "shop");
        assert_eq!(shop.phprc_php.as_deref(), Some("7.4"));
        assert_eq!(shop.php.as_deref(), Some("8.3"));
    }

    #[test]
    fn reports_services_like_the_cli() {
        let fx = fixture();
        let state = from_files(&runner(&fx), &fx.env, None).unwrap();

        let services: Vec<(&str, &str, Option<bool>)> =
            state.services.iter().map(|s| (s.name.as_str(), s.unit.as_str(), s.active)).collect();
        assert_eq!(
            services,
            [
                ("nginx", "nginx", Some(true)),
                ("dnsmasq", "dnsmasq", Some(true)),
                ("php8.3", "php8.3-fpm", Some(true)),
                ("php8.5", "php8.5-fpm", Some(false)),
                ("mysql", "mariadb", Some(true)),
                ("redis", "redis-server", None),
                ("mailpit", "mailpit", None),
            ]
        );
        assert!(state.warnings.is_empty());
    }

    #[test]
    fn never_exposes_config_secrets() {
        let fx = fixture();
        let state = from_files(&runner(&fx), &fx.env, None).unwrap();
        let json = serde_json::to_string(&state).unwrap();

        assert!(!json.contains("s3cr3t"));
        assert!(!json.contains("mysql\":{"));
    }

    #[test]
    fn uses_non_default_ports_in_urls() {
        let fx = fixture();
        let config = fx.valet_home().join("config.json");
        let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
        value["port"] = serde_json::json!("8080");
        value["https_port"] = serde_json::json!(8443);
        fs::write(&config, value.to_string()).unwrap();

        let state = from_files(&runner(&fx), &fx.env, None).unwrap();
        assert_eq!(site(&state, "blog").url, "http://blog.test:8080");
        assert_eq!(site(&state, "shop").url, "https://shop.test:8443");
    }

    #[test]
    fn tolerates_minimal_install() {
        let tmp = tempfile::tempdir().unwrap();
        let env = HostEnv {
            home: tmp.path().join("home"),
            path: None,
            usr_bin: tmp.path().join("missing"),
            force_files: false,
        };
        write(&env.valet_home().join("config.json"), r#"{"domain":"local","paths":[]}"#);

        let state = from_files(&FakeRunner::new(), &env, None).unwrap();

        assert_eq!(state.domain, "local");
        assert!(state.sites.is_empty());
        assert_eq!(state.php.global, None);
        assert_eq!(state.php.cli, CliPhp::default());
        assert!(state.services.iter().all(|s| s.active.is_none()));
        assert_eq!(state.warnings.len(), 1, "systemctl failure must be reported");
    }

    #[test]
    fn errors_when_valet_is_not_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let env =
            HostEnv { home: tmp.path().to_path_buf(), path: None, usr_bin: tmp.path().into(), force_files: false };
        assert!(matches!(from_files(&FakeRunner::new(), &env, None), Err(StateError::NotInstalled(_))));
    }

    #[test]
    fn normalizes_php_versions() {
        for input in ["8.3", "php8.3", "php@8.3", "PHP-8.3", "83", "php83"] {
            assert_eq!(normalize_php(input).as_deref(), Some("8.3"), "{input}");
        }
        assert_eq!(normalize_php("8.10"), None);
        assert_eq!(normalize_php("latest"), None);
    }

    #[test]
    fn sorts_naturally_ignoring_case() {
        let mut names = vec!["site10", "Site2", "alpha", "site1", "erp_ebertsystem", "erpMaciel"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["alpha", "erpMaciel", "erp_ebertsystem", "site1", "Site2", "site10"]);
    }
}
