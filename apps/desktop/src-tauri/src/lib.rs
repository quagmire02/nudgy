use nudgy_shared::Settings;

/// Placeholder until settings storage lands (milestone 2): returns the defaults.
#[tauri::command]
fn get_settings() -> Settings {
    Settings::default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_settings])
        .run(tauri::generate_context!())
        .expect("error while running Nudgy");
}
