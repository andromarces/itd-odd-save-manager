// ITD ODD Save Manager by andromarces

mod backup;
mod config;
mod game_manager;
mod save_paths;
mod watcher;

use backup::BackupInfo;
use config::ConfigState;
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use watcher::FileWatcher;

// Store tray icon to prevent it from being dropped
struct TrayState(#[allow(dead_code)] tauri::tray::TrayIcon);

/// Tauri command to list available backups for the configured save path.
#[tauri::command]
fn get_backups_command(state: tauri::State<ConfigState>) -> Result<Vec<BackupInfo>, String> {
    let config = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;
    if let Some(path_str) = &config.save_path {
        let path = PathBuf::from(path_str);
        backup::get_backups(&path)
    } else {
        Ok(Vec::new())
    }
}

/// Tauri command to restore a specific backup to a target location.
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
        if path.exists() {
            let _ = watcher.start(path);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            let status_i =
                MenuItem::with_id(app, "status", "Status: Monitoring", false, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let open_i = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let launch_i = MenuItem::with_id(app, "launch", "Launch Game", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status_i, &open_i, &launch_i, &quit_i])?;

            // Load application icon
            let icon_bytes = include_bytes!("../icons/32x32.png");
            let icon = tauri::image::Image::from_bytes(icon_bytes).expect("Failed to load icon");

            let tray =
                TrayIconBuilder::new()
                    .menu(&menu)
                    .on_menu_event(|app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                        match event.id().as_ref() {
                            "quit" => {
                                app.exit(0);
                            }
                            "open" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "launch" => {
                                let _ = game_manager::launch_game(app.clone());
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event: tauri::tray::TrayIconEvent| {
                        if let tauri::tray::TrayIconEvent::Click { .. } = event {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .icon(icon)
                    .build(app)?;

            app.manage(TrayState(tray));

            // Start Game Monitor
            game_manager::start_monitor(app.handle().clone());

            // Auto Launch Game if enabled
            if initial_config.auto_launch_game {
                let handle = app.handle().clone();
                // Use standard thread to avoid async runtime dependency issues for sleep
                std::thread::spawn(move || {
                    // Give app a moment to settle
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let _ = game_manager::launch_game(handle);
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Err(e) = window.hide() {
                    log::error!("Failed to hide window: {}", e);
                }
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            save_paths::detect_steam_save_paths,
            save_paths::check_steam_cloud_path,
            config::get_config,
            config::set_save_path,
            config::set_game_settings,
            config::validate_path,
            get_backups_command,
            restore_backup_command,
            game_manager::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
