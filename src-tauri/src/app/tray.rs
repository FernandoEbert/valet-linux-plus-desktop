//! Tray icon (AppIndicator on Linux). On Linux clicks don't reach the app, so everything is in
//! the menu: status, open, restart all, sites and quit. Rebuilt whenever the state changes.

use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuEvent, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_opener::OpenerExt;

use super::{monitor, window};
use crate::valet::actions::{Action, run_in_session};
use crate::valet::model::ValetState;

const TRAY_ID: &str = "main";
const SITE_PREFIX: &str = "site:";
/// Long menus are unusable; the full list is in the app.
const MAX_SITES: usize = 40;

const ICON: &[u8] = include_bytes!("../../icons/tray/tray.png");
const ICON_ALERT: &[u8] = include_bytes!("../../icons/tray/tray-alert.png");

/// Status line and whether something needs attention (an installed service is stopped).
pub fn tray_summary(state: Option<&ValetState>) -> (String, bool) {
    let Some(state) = state else { return ("Reading Valet state…".into(), false) };
    let stopped: Vec<&str> =
        state.services.iter().filter(|s| s.active == Some(false)).map(|s| s.name.as_str()).collect();
    if stopped.is_empty() {
        ("All services running".into(), false)
    } else {
        (format!("Stopped: {}", stopped.join(", ")), true)
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .tooltip("Valet Linux+")
        .menu(&build_menu(app, None)?)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event)
        .build(app)?;
    Ok(())
}

pub fn update(app: &AppHandle, state: &ValetState) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let (summary, alert) = tray_summary(Some(state));
    if let Ok(menu) = build_menu(app, Some(state)) {
        let _ = tray.set_menu(Some(menu));
    }
    if let Ok(icon) = Image::from_bytes(if alert { ICON_ALERT } else { ICON }) {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(format!("Valet Linux+ · {summary}")));
}

fn build_menu(app: &AppHandle, state: Option<&ValetState>) -> tauri::Result<Menu<Wry>> {
    let (summary, _) = tray_summary(state);
    let status = MenuItemBuilder::with_id("status", summary).enabled(false).build(app)?;

    let mut sites = SubmenuBuilder::new(app, "Sites");
    let all = state.map(|s| s.sites.as_slice()).unwrap_or_default();
    for site in all.iter().take(MAX_SITES) {
        sites = sites.text(format!("{SITE_PREFIX}{}", site.url), &site.domain);
    }
    if all.is_empty() {
        sites = sites.item(&MenuItemBuilder::with_id("no-sites", "No sites").enabled(false).build(app)?);
    } else if all.len() > MAX_SITES {
        sites = sites.separator().text("open", format!("{} more in the app…", all.len() - MAX_SITES));
    }

    MenuBuilder::new(app)
        .item(&status)
        .separator()
        .text("open", "Open Valet Linux+")
        .text("restart-all", "Restart all services")
        .item(&sites.build()?)
        .separator()
        .text("quit", "Quit")
        .build()
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        "open" => window::show(app),
        "quit" => app.exit(0),
        "restart-all" => restart_all(app.clone()),
        id => {
            if let Some(url) = id.strip_prefix(SITE_PREFIX) {
                let _ = app.opener().open_url(url, None::<&str>);
            }
        }
    }
}

/// Same privileged session as the UI: asks for the password only if none is open yet.
fn restart_all(app: AppHandle) {
    std::thread::spawn(move || {
        let (title, body) = match run_in_session(&Action::Restart { service: None }) {
            Ok(result) if result.success => ("Valet services restarted", result.stdout),
            Ok(result) => ("Restart failed", if result.stderr.is_empty() { result.stdout } else { result.stderr }),
            Err(err) => ("Restart failed", err.to_string()),
        };
        monitor::notify(&app, title, &body);
        app.state::<super::Shared>().trigger(monitor::Trigger::Refresh);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::valet::model::Service;

    fn with_services(services: &[(&str, Option<bool>)]) -> ValetState {
        let mut state: ValetState =
            serde_json::from_str(include_str!("../../tests/fixtures/state_v1.json")).expect("fixture");
        state.services = services
            .iter()
            .map(|(name, active)| Service { name: (*name).into(), unit: (*name).into(), active: *active })
            .collect();
        state
    }

    #[test]
    fn summarizes_service_status() {
        assert_eq!(tray_summary(None), ("Reading Valet state…".into(), false));
        assert_eq!(
            tray_summary(Some(&with_services(&[("nginx", Some(true)), ("redis", None)]))),
            ("All services running".into(), false)
        );
        assert_eq!(
            tray_summary(Some(&with_services(&[
                ("nginx", Some(false)),
                ("mysql", Some(false)),
                ("php8.3", Some(true))
            ]))),
            ("Stopped: nginx, mysql".into(), true)
        );
        assert_eq!(tray_summary(Some(&with_services(&[]))), ("All services running".into(), false));
    }
}
