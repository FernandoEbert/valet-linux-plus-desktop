//! Desktop shell around the valet layer: background monitor, inotify watcher, tray,
//! close-to-tray, start hidden and app preferences.

pub mod monitor;
pub mod settings;
pub mod tray;
pub mod watcher;
pub mod window;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender};

use serde::Serialize;
use tauri::{AppHandle, Manager, WindowEvent};

use crate::valet::env::HostEnv;
use crate::valet::model::ValetState;
use crate::valet::runner::NativeRunner;
use monitor::Trigger;
use settings::{Settings, settings_path};
use watcher::ValetWatcher;

/// `--hidden`: start in the tray without showing the window (used by autostart).
pub const HIDDEN_FLAG: &str = "--hidden";

/// State shared by the monitor, the tray and the commands.
pub struct Shared {
    pub tray_available: bool,
    settings_file: PathBuf,
    settings: Mutex<Settings>,
    last_state: Mutex<Option<ValetState>>,
    triggers: Mutex<Sender<Trigger>>,
    watcher: Mutex<Option<ValetWatcher>>,
    valet_home: PathBuf,
}

impl Shared {
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn trigger(&self, trigger: Trigger) {
        let _ = self.triggers.lock().unwrap_or_else(|p| p.into_inner()).send(trigger);
    }

    /// Starts the watcher, or recreates it when the parked paths changed.
    fn sync_watcher(&self, state: &ValetState) {
        let mut watcher = self.watcher.lock().unwrap_or_else(|p| p.into_inner());
        if watcher.as_ref().is_some_and(|w| !w.needs_restart(&self.valet_home, &state.paths)) {
            return;
        }
        let sender = self.triggers.lock().unwrap_or_else(|p| p.into_inner()).clone();
        // Without inotify (limits reached, unsupported FS) polling still keeps the UI current.
        *watcher = ValetWatcher::start(&self.valet_home, &state.paths, sender).ok();
    }

    /// Close-to-tray is only honoured when a tray icon is actually visible.
    fn close_to_tray(&self) -> bool {
        self.tray_available && self.settings().close_to_tray
    }
}

/// Wires the tray, monitor and window behaviour. Called from `tauri::Builder::setup`.
pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let tray_available = window::tray_host_available(&NativeRunner::default());
    let settings_file = settings_path(&app.path().app_config_dir()?);
    let (tx, rx) = mpsc::channel();

    app.manage(Shared {
        tray_available,
        settings: Mutex::new(Settings::load(&settings_file)),
        settings_file,
        last_state: Mutex::new(None),
        triggers: Mutex::new(tx),
        watcher: Mutex::new(None),
        valet_home: HostEnv::from_process().valet_home(),
    });

    if tray_available {
        tray::create(&handle)?;
    }

    let main = app.get_webview_window(window::MAIN).ok_or("main window missing")?;
    let close_handle = handle.clone();
    main.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event
            && close_handle.state::<Shared>().close_to_tray()
            && let Some(window) = close_handle.get_webview_window(window::MAIN)
        {
            api.prevent_close();
            let _ = window.hide();
        }
    });

    // Without a visible tray icon `--hidden` would leave the app unreachable: show it anyway.
    let start_hidden = tray_available && std::env::args().any(|a| a == HIDDEN_FLAG);
    if !start_hidden {
        main.show()?;
    }

    monitor::spawn(handle, rx);
    Ok(())
}

/// A second launch (e.g. from the app menu while it sits in the tray) focuses the running one.
pub fn on_second_instance(app: &AppHandle) {
    window::show(app);
}

// Commands

/// Reads the state now (also publishes it to the tray and listeners).
#[tauri::command]
pub async fn get_state(app: AppHandle) -> Result<ValetState, String> {
    tauri::async_runtime::spawn_blocking(move || match crate::valet::state::load_host_state() {
        Ok(state) => {
            monitor::observe(&app, &state);
            Ok(state)
        }
        Err(err) => Err(err.to_string()),
    })
    .await
    .map_err(|e| format!("state task failed: {e}"))?
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    pub settings: Settings,
    /// A tray icon is visible on this desktop (close-to-tray and `--hidden` depend on it).
    pub tray_available: bool,
}

#[tauri::command]
pub fn get_settings(shared: tauri::State<'_, Shared>) -> SettingsView {
    SettingsView { settings: shared.settings(), tray_available: shared.tray_available }
}

#[tauri::command]
pub fn set_settings(shared: tauri::State<'_, Shared>, settings: Settings) -> Result<SettingsView, String> {
    settings.save(&shared.settings_file).map_err(|e| format!("could not save settings: {e}"))?;
    *shared.settings.lock().unwrap_or_else(|p| p.into_inner()) = settings;
    Ok(get_settings(shared))
}

/// How this build was packaged: `appimage`, `deb` or `rpm`, or `None` for `tauri dev` and plain
/// binaries. The updater can only install itself over a packaged build.
#[tauri::command]
pub fn install_kind() -> Option<String> {
    use tauri::utils::config::BundleType;
    tauri::utils::platform::bundle_type().and_then(|kind| match kind {
        BundleType::AppImage => Some("appimage".to_owned()),
        BundleType::Deb => Some("deb".to_owned()),
        BundleType::Rpm => Some("rpm".to_owned()),
        _ => None,
    })
}
