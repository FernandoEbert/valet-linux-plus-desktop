pub mod app;
pub mod valet;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch only focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| app::on_second_instance(app)))
        .plugin(tauri_plugin_autostart::Builder::new().args([app::HIDDEN_FLAG]).app_name("Valet Linux+").build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(app::setup)
        .invoke_handler(tauri::generate_handler![
            app::get_state,
            app::get_settings,
            app::set_settings,
            valet::actions::run_valet_action,
            valet::actions::privileged_session_active,
            valet::actions::end_privileged_session,
            valet::logs::get_logs,
            valet::logs::read_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
