//! Main window visibility and tray-host detection.

use tauri::{AppHandle, Manager};

use super::Shared;
use super::monitor::Trigger;
use crate::valet::runner::HostRunner;

pub const MAIN: &str = "main";

pub fn is_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// Shows, restores and focuses the main window, and refreshes the state right away
/// (it may be up to 30 s old while hidden).
pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    if let Some(shared) = app.try_state::<Shared>() {
        shared.trigger(Trigger::Refresh);
    }
}

/// Whether a StatusNotifier host is running, i.e. a tray icon would actually be visible
/// (GNOME without the AppIndicator extension has none). Without it, the window must never be
/// hidden, or the app would become unreachable.
pub fn tray_host_available(runner: &dyn HostRunner) -> bool {
    runner
        .run(
            "gdbus",
            &[
                "call",
                "--session",
                "--dest",
                "org.freedesktop.DBus",
                "--object-path",
                "/org/freedesktop/DBus",
                "--method",
                "org.freedesktop.DBus.NameHasOwner",
                "org.kde.StatusNotifierWatcher",
            ],
        )
        .is_ok_and(|out| out.success() && parse_name_has_owner(&out.stdout))
}

/// `gdbus` prints a GVariant tuple: `(true,)` or `(false,)`.
fn parse_name_has_owner(stdout: &str) -> bool {
    stdout.trim() == "(true,)"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::runner::fake::FakeRunner;

    const CMD: &str = "gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
                       --method org.freedesktop.DBus.NameHasOwner org.kde.StatusNotifierWatcher";

    #[test]
    fn detects_a_status_notifier_host() {
        assert!(tray_host_available(&FakeRunner::new().ok(CMD, "(true,)\n")));
        assert!(!tray_host_available(&FakeRunner::new().ok(CMD, "(false,)\n")));
        assert!(!tray_host_available(&FakeRunner::new().respond(CMD, 1, "")));
        assert!(!tray_host_available(&FakeRunner::new()), "gdbus missing");
    }
}
