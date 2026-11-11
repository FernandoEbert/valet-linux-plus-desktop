//! Mirror of the `valet state --json` contract (schema v1, `cli/Valet/StateReporter.php`).
//!
//! The same struct is filled by the files fallback, so the frontend only knows one shape.
//! Unknown fields are ignored and non-essential ones default, so additive changes to v1
//! don't break the app.

use serde::{Deserialize, Deserializer, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Read from `valet state --json`.
    #[default]
    Cli,
    /// Rebuilt from `~/.config/valet` (valet < 2.1.3 or the command failed).
    Files,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValetState {
    pub schema_version: u64,
    #[serde(default)]
    pub valet_version: Option<String>,
    pub home: String,
    pub domain: String,
    #[serde(default = "default_port", deserialize_with = "scalar_string")]
    pub port: String,
    #[serde(default = "default_https_port", deserialize_with = "scalar_string")]
    pub https_port: String,
    pub php: PhpInfo,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub sites: Vec<Site>,
    #[serde(default)]
    pub services: Vec<Service>,

    // Added by the app, not part of the CLI payload.
    #[serde(default)]
    pub source: Source,
    #[serde(default)]
    pub valet_path: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PhpInfo {
    #[serde(default)]
    pub global: Option<String>,
    #[serde(default)]
    pub cli: CliPhp,
    #[serde(default)]
    pub fallback_binary: Option<String>,
    #[serde(default)]
    pub installed: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CliPhp {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub binary: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SiteType {
    Parked,
    Linked,
    Proxy,
    /// Any type a future CLI may add.
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub domain: String,
    pub url: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "type")]
    pub site_type: SiteType,
    #[serde(default)]
    pub secure: bool,
    /// Effective PHP: isolated → `.valetphprc` → global. `None` for proxies.
    #[serde(default)]
    pub php: Option<String>,
    #[serde(default)]
    pub isolated_php: Option<String>,
    #[serde(default)]
    pub phprc_php: Option<String>,
    #[serde(default)]
    pub custom_nginx: bool,
    #[serde(default)]
    pub proxy_host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub unit: String,
    /// `true` running, `false` stopped/failed, `None` not installed.
    #[serde(default)]
    pub active: Option<bool>,
}

/// Clean PHP version for display. `valet state` <= 2.1.3 may glue extension noise to it
/// (e.g. `8.5.12Xdebug: [Step Debug] Could not connect…`) or drop it when the noise comes first;
/// keep the `x.y.z` part, or fall back to the version in the binary name (`php8.5` → `8.5`).
pub fn clean_php_version(raw: Option<&str>, binary: Option<&str>) -> Option<String> {
    static SEMVER: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"\d+\.\d+\.\d+").expect("valid regex"));
    static BINARY: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"php(\d)\.?(\d+)$").expect("valid regex"));

    raw.and_then(|v| SEMVER.find(v))
        .map(|m| m.as_str().to_owned())
        .or_else(|| binary.and_then(|b| BINARY.captures(b)).map(|c| format!("{}.{}", &c[1], &c[2])))
}

fn default_port() -> String {
    "80".to_owned()
}

fn default_https_port() -> String {
    "443".to_owned()
}

/// Accepts `"80"` or `80` (older configs may store ports as numbers).
pub fn scalar_string<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    use serde::de::Error;
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(D::Error::custom(format!("expected string or number, got {other}"))),
    }
}
