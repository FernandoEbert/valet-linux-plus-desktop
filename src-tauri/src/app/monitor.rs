//! Background state monitor: reads the Valet state right away on a trigger (the inotify watcher
//! on `~/.config/valet` and the parked paths, the UI or the tray), pushes it to the UI, updates
//! the tray and notifies when a running service stops.
//!
//! The periodic read is only a fallback in case a file event is missed (or a service stops,
//! which changes no file): every 3 min while the window is visible and every 10 min while
//! hidden. Every read runs `valet`, so polling more often only costs CPU and disk I/O.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use super::{Shared, tray, window};
use crate::valet::model::{Service, ValetState};
use crate::valet::state::load_host_state;

pub const VISIBLE_INTERVAL: Duration = Duration::from_secs(3 * 60);
pub const HIDDEN_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// Event with the full state after every read.
pub const STATE_EVENT: &str = "valet://state";
/// Event with the error message when a read fails.
pub const STATE_ERROR_EVENT: &str = "valet://state-error";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// `~/.config/valet` or a parked path changed.
    FilesChanged,
    /// The UI or the tray asked for a fresh read.
    Refresh,
}

/// Services that were running in `prev` and are stopped in `next`. Nothing on the first read,
/// and services that are not installed (`None`) never count.
pub fn stopped_services<'a>(prev: Option<&ValetState>, next: &'a ValetState) -> Vec<&'a Service> {
    let Some(prev) = prev else { return Vec::new() };
    next.services
        .iter()
        .filter(|s| s.active == Some(false))
        .filter(|s| prev.services.iter().any(|p| p.name == s.name && p.active == Some(true)))
        .collect()
}

pub fn spawn(app: AppHandle, triggers: Receiver<Trigger>) {
    std::thread::Builder::new()
        .name("valet-monitor".into())
        .spawn(move || {
            loop {
                refresh(&app);
                let interval = if window::is_visible(&app) { VISIBLE_INTERVAL } else { HIDDEN_INTERVAL };
                match triggers.recv_timeout(interval) {
                    Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                // Coalesce a burst of triggers into a single read.
                while triggers.try_recv().is_ok() {}
            }
        })
        .expect("spawn monitor thread");
}

/// Reads the state and publishes it (blocking).
pub fn refresh(app: &AppHandle) -> Option<ValetState> {
    match load_host_state() {
        Ok(state) => {
            observe(app, &state);
            Some(state)
        }
        Err(err) => {
            let _ = app.emit(STATE_ERROR_EVENT, err.to_string());
            None
        }
    }
}

/// Publishes a fresh state: UI event, tray, watcher paths and service-down notifications.
pub fn observe(app: &AppHandle, state: &ValetState) {
    let shared = app.state::<Shared>();
    let (changed, stopped): (bool, Vec<String>) = {
        let mut last = shared.last_state.lock().unwrap_or_else(|p| p.into_inner());
        let changed = last.as_ref() != Some(state);
        let stopped = stopped_services(last.as_ref(), state).into_iter().map(|s| s.name.clone()).collect();
        *last = Some(state.clone());
        (changed, stopped)
    };

    let _ = app.emit(STATE_EVENT, state);

    if changed {
        tray::update(app, state);
        shared.sync_watcher(state);
    }

    if !stopped.is_empty() && shared.settings().notify_service_down {
        notify(app, "Valet service down", &format!("{} stopped.", stopped.join(", ")));
    }
}

pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(services: &[(&str, Option<bool>)]) -> ValetState {
        let mut state: ValetState =
            serde_json::from_str(include_str!("../../tests/fixtures/state_v1.json")).expect("fixture");
        state.services = services
            .iter()
            .map(|(name, active)| Service { name: (*name).into(), unit: (*name).into(), active: *active })
            .collect();
        state
    }

    fn names(services: Vec<&Service>) -> Vec<&str> {
        services.into_iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn reports_services_that_went_down() {
        let prev = state(&[("nginx", Some(true)), ("redis", Some(true)), ("mysql", Some(false))]);
        let next = state(&[("nginx", Some(false)), ("redis", Some(true)), ("mysql", Some(false))]);
        assert_eq!(names(stopped_services(Some(&prev), &next)), ["nginx"]);
    }

    #[test]
    fn ignores_first_read_not_installed_and_new_services() {
        let next = state(&[("nginx", Some(false))]);
        assert!(stopped_services(None, &next).is_empty(), "first read");

        let prev = state(&[("redis", None)]);
        let next = state(&[("redis", Some(false)), ("php8.3", Some(false))]);
        assert!(stopped_services(Some(&prev), &next).is_empty(), "was not running / is new");
    }

    #[test]
    fn ignores_services_that_disappeared() {
        let prev = state(&[("php8.2", Some(true))]);
        let next = state(&[]);
        assert!(stopped_services(Some(&prev), &next).is_empty());
    }
}
