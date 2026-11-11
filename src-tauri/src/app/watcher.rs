//! Reacts to changes in `~/.config/valet` and in the parked paths, so the UI updates as soon as
//! `valet link`, `park`, `secure`… run in a terminal, instead of waiting for the next poll.
//!
//! Read-only: it only watches (inotify) and asks the monitor for a refresh.

use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

use super::monitor::Trigger;

const DEBOUNCE: Duration = Duration::from_millis(500);

/// Whether a change at `path` can affect the Valet state.
pub fn is_relevant(path: &Path, valet_home: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(valet_home) else {
        // Inside a parked path: a site folder was added, removed or renamed.
        return true;
    };
    let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned());
    // nginx writes logs continuously; FPM sockets change on every restart.
    !matches!(first.as_deref(), Some("Log")) && path.extension().is_none_or(|ext| ext != "sock")
}

/// Hash of `config.json`: every valet command (even `valet state`) rewrites it with the same
/// content, which must not count as a change or each refresh would trigger the next one.
fn content_hash(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

/// Directories to watch, all non-recursively. Never recursive: `Sites/` holds symlinks to whole
/// projects (node_modules, vendor…), which a recursive watch would follow, exhausting the
/// inotify limit and refreshing on every file saved in any project.
pub fn watch_set(valet_home: &Path, parked: &[String]) -> BTreeSet<PathBuf> {
    let sites_dir = valet_home.join("Sites");
    [valet_home.to_path_buf(), sites_dir.clone(), valet_home.join("Nginx"), valet_home.join("Certificates")]
        .into_iter()
        .chain(parked.iter().map(PathBuf::from).filter(|p| *p != sites_dir))
        .filter(|p| p.is_dir())
        .collect()
}

/// Live inotify watches; recreated when the set of directories to watch changes.
pub struct ValetWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
    watched: BTreeSet<PathBuf>,
}

impl ValetWatcher {
    pub fn start(valet_home: &Path, parked: &[String], trigger: Sender<Trigger>) -> notify::Result<Self> {
        let home = valet_home.to_path_buf();
        let config = home.join("config.json");
        let mut config_hash = content_hash(&config);

        let mut debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let relevant = events.iter().filter(|e| is_relevant(&e.path, &home)).any(|e| {
                if e.path != config {
                    return true;
                }
                let hash = content_hash(&config);
                let changed = hash != config_hash;
                config_hash = hash;
                changed
            });
            if relevant {
                let _ = trigger.send(Trigger::FilesChanged);
            }
        })?;

        let watched = watch_set(valet_home, parked);
        let watcher = debouncer.watcher();
        watcher.watch(valet_home, RecursiveMode::NonRecursive)?;
        for dir in watched.iter().filter(|d| d.as_path() != valet_home) {
            // A folder may vanish or be unreadable; the rest keeps working.
            let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
        }

        Ok(Self { _debouncer: debouncer, watched })
    }

    /// Whether the directories to watch changed (parked paths, or Sites/Nginx/Certificates
    /// created after the app started): then the watcher must be recreated.
    pub fn needs_restart(&self, valet_home: &Path, paths: &[String]) -> bool {
        watch_set(valet_home, paths) != self.watched
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn filters_noise_inside_valet_home() {
        let home = Path::new("/home/dev/.config/valet");
        assert!(is_relevant(&home.join("config.json"), home));
        assert!(is_relevant(&home.join("Sites/shop"), home));
        assert!(is_relevant(&home.join("Nginx/shop.test"), home));
        assert!(is_relevant(&home.join("Certificates/shop.test.crt"), home));
        assert!(!is_relevant(&home.join("Log/nginx-error.log"), home));
        assert!(!is_relevant(&home.join("Log"), home));
        assert!(!is_relevant(&home.join("valet83.sock"), home));
        assert!(is_relevant(Path::new("/home/dev/sites/new-project"), home));
    }

    #[test]
    fn notifies_real_changes_but_not_identical_rewrites() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("valet");
        let parked = tmp.path().join("sites");
        std::fs::create_dir_all(home.join("Log")).unwrap();
        std::fs::create_dir_all(&parked).unwrap();
        std::fs::write(home.join("config.json"), "{}").unwrap();

        let (tx, rx) = mpsc::channel();
        let _watcher = ValetWatcher::start(&home, &[parked.to_string_lossy().into()], tx).unwrap();
        let wait = || rx.recv_timeout(Duration::from_secs(3)).is_ok();
        let drain = || while rx.recv_timeout(Duration::from_millis(900)).is_ok() {};

        std::fs::write(home.join("config.json"), "{}").unwrap(); // same content
        std::fs::write(home.join("Log/nginx-error.log"), "noise").unwrap();
        assert!(!wait(), "identical rewrite and logs must be ignored");

        std::fs::write(home.join("config.json"), r#"{"domain":"test"}"#).unwrap();
        assert!(wait(), "config change");
        drain();

        std::fs::create_dir(parked.join("new-site")).unwrap();
        assert!(wait(), "new folder in a parked path");
    }

    #[test]
    fn does_not_follow_site_symlinks_into_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("valet");
        let project = tmp.path().join("project");
        std::fs::create_dir_all(home.join("Sites")).unwrap();
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::os::unix::fs::symlink(&project, home.join("Sites/project")).unwrap();

        let (tx, rx) = mpsc::channel();
        let _watcher = ValetWatcher::start(&home, &[], tx).unwrap();

        std::fs::write(project.join("node_modules/a.js"), "x").unwrap();
        std::fs::write(project.join("index.php"), "x").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_err(), "project files are not Valet state");

        std::os::unix::fs::symlink(&project, home.join("Sites/other")).unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(3)).is_ok(), "new link");
    }

    #[test]
    fn watches_only_the_relevant_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("valet");
        let parked = tmp.path().join("sites");
        for dir in [home.join("Sites"), home.join("Nginx"), home.join("Log"), parked.clone()] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let sites = home.join("Sites").to_string_lossy().into_owned();
        let set = watch_set(&home, &[sites, parked.to_string_lossy().into(), "/missing".into()]);
        assert_eq!(set, [home.clone(), parked, home.join("Nginx"), home.join("Sites")].into_iter().collect());
    }

    #[test]
    fn restarts_when_parked_paths_change() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("valet");
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        for dir in [&home, &a, &b] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let (tx, _rx) = mpsc::channel();
        let sites = home.join("Sites").to_string_lossy().into_owned();
        let watcher = ValetWatcher::start(&home, &[sites.clone(), a.to_string_lossy().into()], tx).unwrap();

        assert!(!watcher.needs_restart(&home, &[a.to_string_lossy().into(), sites]));
        assert!(watcher.needs_restart(&home, &[a.to_string_lossy().into(), b.to_string_lossy().into()]));
    }
}
