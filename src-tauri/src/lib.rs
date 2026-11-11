pub mod valet;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            valet::state::get_state,
            valet::actions::run_valet_action,
            valet::actions::privileged_session_active,
            valet::actions::end_privileged_session,
            valet::logs::get_logs,
            valet::logs::read_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
