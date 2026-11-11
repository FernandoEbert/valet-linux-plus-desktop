//! User preferences of the desktop app (not Valet's), stored as JSON in the app config dir
//! (`~/.config/com.ebertsystem.ValetLinuxPlus/settings.json`).
//!
//! Start-at-login is not stored here: the autostart plugin's `.desktop` file is the source of truth.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Desktop notification when a running service stops.
    pub notify_service_down: bool,
    /// Closing the window keeps the app in the tray (only when a tray host exists).
    pub close_to_tray: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { notify_service_down: true, close_to_tray: true }
    }
}

impl Settings {
    /// Defaults when the file is missing or unreadable: preferences must never block the app.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).ok().and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // Write then rename, so a crash never leaves a truncated file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        fs::rename(tmp, path)
    }
}

pub fn settings_path(config_dir: &Path) -> PathBuf {
    config_dir.join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_missing_or_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = settings_path(tmp.path());
        assert_eq!(Settings::load(&path), Settings::default());

        fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn keeps_defaults_for_missing_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let path = settings_path(tmp.path());
        fs::write(&path, r#"{"close_to_tray": false, "unknown": 1}"#).unwrap();

        assert_eq!(Settings::load(&path), Settings { notify_service_down: true, close_to_tray: false });
    }

    #[test]
    fn roundtrips_and_creates_the_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = settings_path(&tmp.path().join("nested/dir"));
        let settings = Settings { notify_service_down: false, close_to_tray: true };

        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
    }
}
