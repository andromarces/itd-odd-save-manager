mod backup;
mod config;
mod save_paths;
mod watcher;

use config::ConfigState;
use std::path::PathBuf;
use watcher::FileWatcher;

/// Runs the Tauri application entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_config = config::load_initial_config();
    let watcher = FileWatcher::new();

    // Auto-start watcher if path exists
    if let Some(path_str) = &initial_config.save_path {
        let path = PathBuf::from(path_str);
        // Basic check, more validation happens in start()
        if path.exists() {
            let _ = watcher.start(path);
        }
    }

    tauri::Builder::default()
        .manage(ConfigState(std::sync::Mutex::new(initial_config)))
        .manage(watcher)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            save_paths::detect_steam_save_paths,
            config::get_config,
            config::set_save_path,
            config::validate_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
