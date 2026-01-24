// ITD ODD Save Manager by andromarces

use crate::config::ConfigState;
use std::thread;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

const GAME_APP_ID: &str = "2239710";
// Matches "IntoTheDead" case-insensitively. This is a heuristic and relies on the
// executable name containing this substring. If the game executable is renamed
// or differs significantly, detection will fail.
const PROCESS_NAME_PART: &str = "intothedead"; // Lowercase match

/// Initiates game launch via Steam protocol.
#[tauri::command(rename_all = "snake_case")]
pub fn launch_game<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    log::info!("Launching game via Steam...");
    match app
        .opener()
        .open_url(format!("steam://run/{}", GAME_APP_ID), None::<&str>)
    {
        Ok(_) => {
            log::info!("Game launch command sent successfully.");
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to launch game: {}", e);
            Err(format!("Failed to launch game: {}", e))
        }
    }
}

/// Initiates the background process monitor.
pub fn start_monitor<R: Runtime>(app: AppHandle<R>) {
    log::info!("Starting game process monitor...");
    thread::spawn(move || {
        let mut sys = System::new_all();
        let mut game_was_running = false;

        loop {
            // Check config to see if auto-close is enabled
            let should_auto_close = {
                let state = app.state::<ConfigState>();
                state.0.lock().map(|c| c.auto_close).unwrap_or(false)
            };

            // Check if game is running
            sys.refresh_processes(ProcessesToUpdate::All, true);
            let processes = sys.processes();
            let game_running = processes.values().any(|p| {
                if let Some(name) = p.name().to_str() {
                    name.to_lowercase().contains(PROCESS_NAME_PART)
                } else {
                    false
                }
            });

            if game_running {
                if !game_was_running {
                    log::info!("Game process detected: {}", PROCESS_NAME_PART);
                }
                game_was_running = true;
            } else if game_was_running {
                log::info!("Game process exited.");
                game_was_running = false;

                if should_auto_close {
                    log::info!("Auto-close enabled. Exiting application.");
                    app.exit(0);
                    break;
                }
            }

            thread::sleep(Duration::from_secs(5));
        }
    });
}
