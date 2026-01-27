// ITD ODD Save Manager by andromarces

mod backup;
mod commands;
mod config;
pub mod filename_utils;
mod game_manager;
mod save_paths;
mod tray;
mod watcher;
mod window;
mod wrapper_launch;

use config::{AppConfig, ConfigState};
use std::path::Path;
use tauri::{async_runtime, Manager};
use tauri_plugin_notification::NotificationExt;
use watcher::FileWatcher;
use window::show_main_window;

/// Initializes the configuration, performing auto-detection if necessary.
fn bootstrap_config(config_path: &Path) -> AppConfig {
    #[cfg(target_os = "windows")]
    let mut config = config::load_config_from_path(config_path);
    #[cfg(not(target_os = "windows"))]
    let config = config::load_config_from_path(config_path);

    // Auto-detect save path if not set (Windows only)
    #[cfg(target_os = "windows")]
    if config.save_path.is_none() {
        if let Some(path) = save_paths::detect_windows_local_save_path() {
            let path_str = path.to_string_lossy().to_string();
            log::info!("Auto-detected save path: {}", path_str);
            config.save_path = Some(path_str);
            if let Err(e) = config::save_config_to_path(&config, config_path) {
                log::error!("Failed to save auto-detected config: {}", e);
            }
        }
    }

    config
}

/// Runs the Tauri application entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = config::get_config_path();
    let initial_config = bootstrap_config(&config_path);
    let watcher = FileWatcher::new();

    // Check for wrapper mode (Steam Launch Options: "Manager.exe" %command%)
    let launched_via_wrapper = wrapper_launch::maybe_launch_from_wrapper_args();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app, true);
        }))
        .manage(ConfigState(std::sync::Mutex::new(initial_config.clone())))
        .manage(watcher)
        .setup(move |app| {
            // Logger setup
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Tray setup
            tray::create_tray(app.handle())?;

            // Start Game Monitor
            game_manager::start_monitor(app.handle().clone());

            // Auto Launch Game if enabled (skip if already launched via wrapper)
            if initial_config.auto_launch_game && !launched_via_wrapper {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let _ = async_runtime::block_on(game_manager::launch_game(handle));
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();

                match window.hide() {
                    Ok(_) => {
                        let _ = window
                            .app_handle()
                            .notification()
                            .builder()
                            .title("ITD ODD Save Manager")
                            .body("App minimized into the tray")
                            .show();
                    }
                    Err(e) => {
                        log::error!("Failed to hide window: {}", e);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            save_paths::detect_steam_save_paths,
            save_paths::is_auto_detection_supported,
            config::get_config,
            config::set_save_path,
            config::set_game_settings,
            config::validate_path,
            commands::get_backups_command,
            commands::restore_backup_command,
            commands::toggle_backup_lock_command,
            commands::set_backup_note_command,
            commands::delete_backup_command,
            commands::batch_delete_backups_command,
            commands::init_watcher,
            game_manager::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[cfg(target_os = "windows")]
    #[test]
    fn bootstrap_config_detects_and_saves_path() {
        use std::env;
        use std::fs;

        static ENV_MUTEX: Mutex<()> = Mutex::new(());

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let mock_profile = temp_dir.path().join("UserProfile");
        let config_path = temp_dir.path().join("config.json");

        let expected_save_path = mock_profile
            .join("AppData")
            .join("LocalLow")
            .join("PikPok")
            .join("IntoTheDeadOurDarkestDays");
        fs::create_dir_all(&expected_save_path).expect("failed to create mock save dir");

        let _guard = ENV_MUTEX.lock().expect("env mutex locked");
        let original_profile = env::var_os("USERPROFILE");
        env::set_var("USERPROFILE", &mock_profile);

        let config = bootstrap_config(&config_path);

        if let Some(val) = original_profile {
            env::set_var("USERPROFILE", val);
        } else {
            env::remove_var("USERPROFILE");
        }

        assert_eq!(
            config.save_path,
            Some(expected_save_path.to_string_lossy().to_string())
        );

        let saved_config = config::load_config_from_path(&config_path);
        assert_eq!(
            saved_config.save_path,
            Some(expected_save_path.to_string_lossy().to_string())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn bootstrap_config_does_not_override_existing_path() {
        use std::env;
        use std::fs;

        static ENV_MUTEX: Mutex<()> = Mutex::new(());

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        let existing_config = AppConfig {
            save_path: Some("C:\\Custom\\Path".to_string()),
            ..Default::default()
        };
        config::save_config_to_path(&existing_config, &config_path)
            .expect("failed to save setup config");

        let mock_profile = temp_dir.path().join("UserProfile");
        let detected_path = mock_profile.join("AppData/LocalLow/PikPok/IntoTheDeadOurDarkestDays");
        fs::create_dir_all(&detected_path).expect("failed to create mock save dir");

        let _guard = ENV_MUTEX.lock().expect("env mutex locked");
        let original = env::var_os("USERPROFILE");
        env::set_var("USERPROFILE", &mock_profile);

        let config = bootstrap_config(&config_path);

        if let Some(val) = original {
            env::set_var("USERPROFILE", val);
        } else {
            env::remove_var("USERPROFILE");
        }

        assert_eq!(config.save_path, Some("C:\\Custom\\Path".to_string()));
    }
}
