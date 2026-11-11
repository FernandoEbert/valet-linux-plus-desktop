//! Read-only access to the nginx logs in `~/.config/valet/Log`.
//!
//! Only plain file names listed in that directory are accepted, and only the tail of the file
//! is read (site logs can grow to many MB).

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use thiserror::Error;

use super::env::HostEnv;

pub const DEFAULT_LINES: usize = 200;
pub const MAX_LINES: usize = 2000;
/// Upper bound read from the end of the file, whatever the line count.
const MAX_TAIL_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogFile {
    pub name: String,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogTail {
    pub name: String,
    pub size: u64,
    /// Last lines, oldest first.
    pub lines: Vec<String>,
    /// More content exists before the returned lines.
    pub truncated: bool,
}

#[derive(Debug, Error)]
pub enum LogError {
    #[error("invalid log name '{0}'")]
    InvalidName(String),
    #[error("could not read {path}: {source}")]
    Io { path: String, source: std::io::Error },
}

fn log_dir(env: &HostEnv) -> PathBuf {
    env.valet_home().join("Log")
}

/// `*.log` files, the global `nginx-error.log` first, then by most recent change.
pub fn list_logs(env: &HostEnv) -> Result<Vec<LogFile>, LogError> {
    let dir = log_dir(env);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(LogError::Io { path: dir.to_string_lossy().into(), source }),
    };

    let mut logs: Vec<LogFile> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().ok().filter(|m| m.is_file())?;
            name.ends_with(".log").then(|| LogFile {
                name,
                size: meta.len(),
                modified: meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()),
            })
        })
        .collect();

    logs.sort_by(|a, b| {
        (b.name == "nginx-error.log")
            .cmp(&(a.name == "nginx-error.log"))
            .then(b.modified.cmp(&a.modified))
            .then(a.name.cmp(&b.name))
    });
    Ok(logs)
}

pub fn tail_log(env: &HostEnv, name: &str, lines: usize) -> Result<LogTail, LogError> {
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', '\0']) || !name.ends_with(".log") {
        return Err(LogError::InvalidName(name.to_owned()));
    }
    let path = log_dir(env).join(name);
    let lines = lines.clamp(1, MAX_LINES);

    let io_err = |source| LogError::Io { path: path.to_string_lossy().into(), source };
    let (size, mut content, cut_start) = read_tail(&path).map_err(io_err)?;

    // Drop the partial first line when we started mid-file.
    if cut_start && let Some(idx) = content.find('\n') {
        content.drain(..=idx);
    }

    let all: Vec<&str> = content.lines().collect();
    let skip = all.len().saturating_sub(lines);
    Ok(LogTail {
        name: name.to_owned(),
        size,
        lines: all[skip..].iter().map(|l| (*l).to_owned()).collect(),
        truncated: cut_start || skip > 0,
    })
}

fn read_tail(path: &Path) -> std::io::Result<(u64, String, bool)> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(MAX_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity((size - start) as usize);
    file.read_to_end(&mut buf)?;
    Ok((size, String::from_utf8_lossy(&buf).into_owned(), start > 0))
}

#[tauri::command]
pub async fn get_logs() -> Result<Vec<LogFile>, String> {
    tauri::async_runtime::spawn_blocking(|| list_logs(&HostEnv::from_process()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_log(name: String, lines: Option<usize>) -> Result<LogTail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        tail_log(&HostEnv::from_process(), &name, lines.unwrap_or(DEFAULT_LINES))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(root: &Path) -> HostEnv {
        HostEnv {
            home: root.to_path_buf(),
            path: None,
            usr_bin: root.join("usr/bin"),
            force_files: false,
            helper: PathBuf::from("/nonexistent/valet-helper"),
        }
    }

    fn write_log(env: &HostEnv, name: &str, content: &str) {
        let dir = log_dir(env);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn lists_logs_with_nginx_error_first() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        write_log(&env, "shop.test-error.log", "x");
        write_log(&env, "nginx-error.log", "");
        write_log(&env, "notes.txt", "ignored");
        fs::create_dir_all(log_dir(&env).join("dir.log")).unwrap();

        let names: Vec<String> = list_logs(&env).unwrap().into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["nginx-error.log", "shop.test-error.log"]);
    }

    #[test]
    fn missing_log_dir_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_logs(&env(tmp.path())).unwrap().is_empty());
    }

    #[test]
    fn tails_the_last_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        let content: String = (1..=10).map(|i| format!("line {i}\n")).collect();
        write_log(&env, "nginx-error.log", &content);

        let tail = tail_log(&env, "nginx-error.log", 3).unwrap();
        assert_eq!(tail.lines, ["line 8", "line 9", "line 10"]);
        assert!(tail.truncated);

        let all = tail_log(&env, "nginx-error.log", 50).unwrap();
        assert_eq!(all.lines.len(), 10);
        assert!(!all.truncated);
    }

    #[test]
    fn reads_only_the_end_of_big_files() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        let line = "x".repeat(99) + "\n";
        write_log(&env, "big.log", &line.repeat(20_000)); // ~2 MB

        let tail = tail_log(&env, "big.log", MAX_LINES).unwrap();
        assert_eq!(tail.lines.len(), MAX_LINES);
        assert!(tail.lines.iter().all(|l| l.len() == 99), "no partial first line");
        assert!(tail.truncated);
    }

    #[test]
    fn rejects_paths_outside_the_log_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env(tmp.path());
        for name in ["../config.json", "/etc/passwd", "..log", "a/b.log", "config.json", ""] {
            assert!(matches!(tail_log(&env, name, 10), Err(LogError::InvalidName(_))), "{name}");
        }
    }
}
