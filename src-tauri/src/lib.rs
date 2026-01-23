mod backup;
mod config;
mod save_paths;
mod watcher;

use backup::BackupInfo;
use config::ConfigState;
use std::path::PathBuf;
use watcher::FileWatcher;

/// Tauri command to list available backups for the configured save path.
///
/// Returns a list of `BackupInfo` objects.
#[tauri::command]
fn get_backups_command(state: tauri::State<ConfigState>) -> Result<Vec<BackupInfo>, String> {
    let config = state.0.lock().unwrap();
    if let Some(path_str) = &config.save_path {
        let path = PathBuf::from(path_str);
        backup::get_backups(&path)
    } else {
        Ok(Vec::new())
    }
}

/// Tauri command to restore a specific backup to a target location.
///
/// # Arguments
///
/// * `backup_path` - The absolute path to the backup file.
/// * `target_path` - The absolute path where the file should be restored.
#[tauri::command]
fn restore_backup_command(backup_path: String, target_path: String) -> Result<(), String> {
    let backup = PathBuf::from(backup_path);
    let target = PathBuf::from(target_path);
    backup::restore_backup(&backup, &target)
}

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
            config::validate_path,
            get_backups_command,
            restore_backup_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
