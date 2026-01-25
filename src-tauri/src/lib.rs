// ITD ODD Save Manager by andromarces

mod backup;
mod config;
pub mod filename_utils;
mod game_manager;
mod save_paths;
mod watcher;

use backup::BackupInfo;
use config::ConfigState;
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
#[cfg(target_os = "linux")]
use tauri_plugin_notification::NotificationExt;
use watcher::FileWatcher;

// Store tray icon to prevent it from being dropped
struct TrayState(#[allow(dead_code)] tauri::tray::TrayIcon);

/// Tauri command to list available backups for the configured save path.
#[tauri::command(rename_all = "snake_case")]
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
#[tauri::command(rename_all = "snake_case")]
fn restore_backup_command(backup_path: String, target_path: String) -> Result<(), String> {
    let backup = PathBuf::from(backup_path);
    let target = PathBuf::from(target_path);

    // The backup::restore_backup function expects a target DIRECTORY.
    // If target_path points to a file (e.g. gamesave_0.sav), we use its parent.
    let target_dir = if target.extension().is_some() {
        target.parent().ok_or("Invalid target path")?.to_path_buf()
    } else {
        target
    };

    backup::restore_backup(&backup, &target_dir)
}

/// Command to initialize the watcher from the frontend.
/// This ensures the watcher starts strictly after the UI is shown.
#[tauri::command(rename_all = "snake_case")]
fn init_watcher(app: tauri::AppHandle, state: tauri::State<ConfigState>) -> Result<(), String> {
    // Hard guarantee: Ensure the window is actually visible before starting
    if let Some(window) = app.get_webview_window("main") {
        if !window.is_visible().unwrap_or(false) {
            return Err("Watcher initialization deferred: window not yet visible".to_string());
        }
    }

    let config = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;
    if let Some(path_str) = &config.save_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            let watcher = app.state::<FileWatcher>();
            watcher.start(path)?;
        }
    }
    Ok(())
}

/// Helper to show and focus the main window.
fn show_main_window(app: &tauri::AppHandle, _from_second_instance: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();

        // Foregrounding reliable on Windows and macOS, inconsistent on Linux
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let _ = window.set_focus();
        }

        #[cfg(target_os = "linux")]
        {
            if _from_second_instance {
                let _ = app
                    .notification()
                    .builder()
                    .title("Already Running")
                    .body("ITD ODD Save Manager is already active.")
                    .show();
            }
        }
    }
}

/// Determines whether a tray icon event should show and focus the main window.
fn should_show_main_window_from_tray_event(event: &tauri::tray::TrayIconEvent) -> bool {
    match event {
        tauri::tray::TrayIconEvent::Click { button, .. } => {
            matches!(button, tauri::tray::MouseButton::Left)
        }
        tauri::tray::TrayIconEvent::DoubleClick { button, .. } => {
            matches!(button, tauri::tray::MouseButton::Left)
        }
        _ => false,
    }
}

/// Runs the Tauri application entry point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let initial_config = config::load_initial_config();
    let watcher = FileWatcher::new();

    // Note: Watcher auto-start is deferred to the frontend init_watcher command
    // to ensure it runs strictly after the UI is shown.

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
                                show_main_window(app, false);
                            }
                            "launch" => {
                                let _ = game_manager::launch_game(app.clone());
                            }
                            _ => {}
                        }
                    })
                    .on_tray_icon_event(|tray, event: tauri::tray::TrayIconEvent| {
                        if should_show_main_window_from_tray_event(&event) {
                            show_main_window(tray.app_handle(), false);
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
            save_paths::is_auto_detection_supported,
            config::get_config,
            config::set_save_path,
            config::set_game_settings,
            config::validate_path,
            get_backups_command,
            restore_backup_command,
            init_watcher,
            game_manager::launch_game
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent, TrayIconId};

    /// Builds a tray click event with the provided mouse button.
    fn make_click_event(button: MouseButton) -> TrayIconEvent {
        TrayIconEvent::Click {
            button,
            button_state: MouseButtonState::Down,
            id: TrayIconId::new("test"),
            position: tauri::PhysicalPosition::default(),
            rect: tauri::Rect::default(),
        }
    }

    /// Builds a tray double click event with the provided mouse button.
    fn make_double_click_event(button: MouseButton) -> TrayIconEvent {
        TrayIconEvent::DoubleClick {
            button,
            id: TrayIconId::new("test"),
            position: tauri::PhysicalPosition::default(),
            rect: tauri::Rect::default(),
        }
    }

    /// Verifies that a left click triggers the main window behavior.
    #[test]
    fn tray_left_click_shows_main_window() {
        let event = make_click_event(MouseButton::Left);
        assert!(should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a right click does not trigger the main window behavior.
    #[test]
    fn tray_right_click_does_not_show_main_window() {
        let event = make_click_event(MouseButton::Right);
        assert!(!should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a left double click triggers the main window behavior.
    #[test]
    fn tray_left_double_click_shows_main_window() {
        let event = make_double_click_event(MouseButton::Left);
        assert!(should_show_main_window_from_tray_event(&event));
    }

    /// Verifies that a right double click does not trigger the main window behavior.
    #[test]
    fn tray_right_double_click_does_not_show_main_window() {
        let event = make_double_click_event(MouseButton::Right);
        assert!(!should_show_main_window_from_tray_event(&event));
    }
}
