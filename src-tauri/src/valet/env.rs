//! Host environment the backend depends on, injected so tests never touch the real machine.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct HostEnv {
    /// User home directory (`$HOME`).
    pub home: PathBuf,
    /// Raw `$PATH`.
    pub path: Option<OsString>,
    /// Directory scanned for versioned PHP binaries (`/usr/bin`).
    pub usr_bin: PathBuf,
    /// Skip `valet state --json` and always read files (debug builds only, for parity checks).
    pub force_files: bool,
}

impl HostEnv {
    pub fn from_process() -> Self {
        Self {
            home: std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default(),
            path: std::env::var_os("PATH"),
            usr_bin: PathBuf::from("/usr/bin"),
            force_files: cfg!(debug_assertions) && std::env::var_os("VALET_UI_FORCE_FILES").is_some(),
        }
    }

    /// `VALET_HOME_PATH` of the CLI (`~/.config/valet`).
    pub fn valet_home(&self) -> PathBuf {
        self.home.join(".config").join("valet")
    }

    /// First executable named `name` in `$PATH`, skipping the paths in `ignored`.
    pub fn which(&self, name: &str, ignored: &[&str]) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        std::env::split_paths(path)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(|dir| dir.join(name))
            .filter(|candidate| !ignored.iter().any(|i| Path::new(i) == candidate))
            .find(|candidate| is_executable(candidate))
    }
}

pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
