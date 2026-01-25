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
use tauri::{async_runtime, Manager};
use tauri_plugin_notification::NotificationExt;
use watcher::FileWatcher;

use config::AppConfig;
use std::path::Path;

// Store tray icon to prevent it from being dropped
struct TrayState(#[allow(dead_code)] tauri::tray::TrayIcon);

/// Initializes the configuration, performing auto-detection if necessary.
///
/// If `save_path` is not set and the application is running on Windows,
/// it attempts to detect the standard save location. If successful,
/// the configuration is updated and saved to the specified path.
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

/// Extracts the configured save path without holding the mutex across blocking work.
fn extract_save_path(state: &tauri::State<'_, ConfigState>) -> Result<Option<PathBuf>, String> {
    let save_path = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?
        .save_path
        .clone();
    Ok(save_path.map(PathBuf::from))
}

/// Runs blocking work on the blocking thread pool and surfaces join errors.
async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("Blocking task join error: {}", e))?
}

/// Tauri command to list available backups for the configured save path.
#[tauri::command(rename_all = "snake_case")]
async fn get_backups_command(
    state: tauri::State<'_, ConfigState>,
) -> Result<Vec<BackupInfo>, String> {
    if let Some(path) = extract_save_path(&state)? {
        run_blocking(move || backup::get_backups(&path)).await
    } else {
        Ok(Vec::new())
    }
}

/// Tauri command to restore a specific backup to a target location.
#[tauri::command(rename_all = "snake_case")]
async fn restore_backup_command(backup_path: String, target_path: String) -> Result<(), String> {
    let backup = PathBuf::from(backup_path);
    let target = PathBuf::from(target_path);

    // The backup::restore_backup function expects a target DIRECTORY.
    // If target_path points to a file (e.g. gamesave_0.sav), we use its parent.
    let target_dir = crate::filename_utils::normalize_to_directory(&target)
        .map_err(|_| "Invalid target path".to_string())?;

    run_blocking(move || backup::restore_backup(&backup, &target_dir)).await
}

/// Command to initialize the watcher from the frontend.
/// This ensures the watcher starts strictly after the UI is shown.
#[tauri::command(rename_all = "snake_case")]
async fn init_watcher(
    app: tauri::AppHandle,
    state: tauri::State<'_, ConfigState>,
) -> Result<(), String> {
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
    let config_path = config::get_config_path();
    let initial_config = bootstrap_config(&config_path);
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
                                let app_handle = app.clone();
                                async_runtime::spawn(async move {
                                    let _ = game_manager::launch_game(app_handle).await;
                                });
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
    use std::thread;
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

    /// Verifies that blocking work is dispatched to a blocking thread.
    #[test]
    fn run_blocking_executes_on_different_thread() {
        let caller_thread = thread::current().id();
        let worker_thread = tauri::async_runtime::block_on(async {
            run_blocking(|| Ok(thread::current().id())).await
        })
        .expect("Blocking task should complete successfully");

        assert_ne!(
            caller_thread, worker_thread,
            "Blocking work should not execute on the caller thread"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn bootstrap_config_detects_and_saves_path() {
        use std::env;
        use std::fs;
        use std::sync::Mutex;

        // Mutex to protect environment variable access
        static ENV_MUTEX: Mutex<()> = Mutex::new(());

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let mock_profile = temp_dir.path().join("UserProfile");
        let config_path = temp_dir.path().join("config.json");

        // Setup mock save directory structure
        let expected_save_path = mock_profile
            .join("AppData")
            .join("LocalLow")
            .join("PikPok")
            .join("IntoTheDeadOurDarkestDays");
        fs::create_dir_all(&expected_save_path).expect("failed to create mock save dir");

        // Run within mutex lock
        let _guard = ENV_MUTEX.lock().expect("env mutex locked");
        let original_profile = env::var_os("USERPROFILE");
        env::set_var("USERPROFILE", &mock_profile);

        // Execute bootstrap with no existing config file
        let config = bootstrap_config(&config_path);

        // Restore env
        if let Some(val) = original_profile {
            env::set_var("USERPROFILE", val);
        } else {
            env::remove_var("USERPROFILE");
        }

        // Verify in-memory config
        assert_eq!(
            config.save_path,
            Some(expected_save_path.to_string_lossy().to_string())
        );

        // Verify persisted config
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
        use std::sync::Mutex;

        static ENV_MUTEX: Mutex<()> = Mutex::new(());

        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");

        // Create existing config with a custom path
        let existing_config = AppConfig {
            save_path: Some("C:\\Custom\\Path".to_string()),
            ..Default::default()
        };
        config::save_config_to_path(&existing_config, &config_path)
            .expect("failed to save setup config");

        // Even if we have a valid detection candidate, it should be ignored
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
