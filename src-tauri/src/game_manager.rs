// ITD ODD Save Manager by andromarces

use crate::config::ConfigState;
use crate::MonitorInvalidator;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

const GAME_APP_ID: &str = "2239710";
// Matches "IntoTheDead" case-insensitively. This is a heuristic and relies on the
// executable name containing this substring. If the game executable is renamed
// or differs significantly, detection will fail.
const PROCESS_NAME_PART: &str = "intothedead"; // Lowercase match

/// Checks if the process name matches the game we are looking for.
///
/// This is a heuristic that checks if the process name contains the target string (case-insensitive).
fn is_game_process(name: &str) -> bool {
    name.to_ascii_lowercase().contains(PROCESS_NAME_PART)
}

/// Initiates game launch via Steam protocol.
#[tauri::command(rename_all = "snake_case")]
pub async fn launch_game<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::{apply_monitor_tick, is_game_process, MonitorAction};
    use crate::config::signal_invalidator_if_disabled;
    use crate::MonitorInvalidator;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn make_inv() -> MonitorInvalidator {
        MonitorInvalidator(Arc::new(AtomicBool::new(false)))
    }

    /// Tests that process matching is case-insensitive.
    #[test]
    fn test_is_game_process_case_insensitive() {
        assert!(is_game_process("IntoTheDead.exe"));
        assert!(is_game_process("intothedead"));
    }

    /// Tests that unrelated processes are rejected.
    #[test]
    fn test_is_game_process_rejects_unrelated() {
        assert!(!is_game_process("notepad.exe"));
    }

    /// Game first detected returns GameDetected and sets game_was_running.
    #[test]
    fn test_apply_tick_game_detected() {
        let inv = make_inv();
        let mut gwr = false;
        let action = apply_monitor_tick(&mut gwr, true, &inv.0);
        assert!(gwr);
        assert!(matches!(action, MonitorAction::GameDetected));
    }

    /// Game already running returns NoChange.
    #[test]
    fn test_apply_tick_game_already_running() {
        let inv = make_inv();
        let mut gwr = true;
        let action = apply_monitor_tick(&mut gwr, true, &inv.0);
        assert!(gwr);
        assert!(matches!(action, MonitorAction::NoChange));
    }

    /// Game exits after being detected returns GameExited and clears game_was_running.
    #[test]
    fn test_apply_tick_game_exited() {
        let inv = make_inv();
        let mut gwr = true;
        let action = apply_monitor_tick(&mut gwr, false, &inv.0);
        assert!(!gwr);
        assert!(matches!(action, MonitorAction::GameExited));
    }

    /// Game not running and was not running returns NoChange.
    #[test]
    fn test_apply_tick_no_game_no_change() {
        let inv = make_inv();
        let mut gwr = false;
        let action = apply_monitor_tick(&mut gwr, false, &inv.0);
        assert!(!gwr);
        assert!(matches!(action, MonitorAction::NoChange));
    }

    /// Integration: disabling auto_close clears stale game_was_running before the next scan,
    /// preventing a false GameExited action.
    #[test]
    fn test_disable_auto_close_clears_stale_game_was_running() {
        let inv = make_inv();
        let mut gwr = true;

        // Config setter disables auto_close (true -> false transition)
        signal_invalidator_if_disabled(true, false, &inv);

        // Monitor tick: game is no longer running, but invalidation signal must clear
        // gwr before the exit check, so the result must be NoChange not GameExited.
        let action = apply_monitor_tick(&mut gwr, false, &inv.0);

        assert!(
            !gwr,
            "game_was_running must be cleared by the invalidation signal"
        );
        assert!(
            matches!(action, MonitorAction::NoChange),
            "stale game_was_running must not produce a GameExited action"
        );
        // Confirm the signal was consumed
        assert!(
            !inv.0.load(Ordering::Relaxed),
            "invalidator must be cleared after tick"
        );
    }

    /// Integration: without a disable signal, a game exit correctly produces GameExited.
    #[test]
    fn test_no_disable_signal_game_exit_produces_game_exited() {
        let inv = make_inv();
        let mut gwr = true;

        // No signal set (auto_close remains enabled throughout)
        let action = apply_monitor_tick(&mut gwr, false, &inv.0);

        assert!(!gwr);
        assert!(matches!(action, MonitorAction::GameExited));
    }

    /// Integration: re-enabling auto_close after a disable does not suppress a genuine exit
    /// that occurs after the signal was consumed.
    #[test]
    fn test_reenable_then_game_exit_produces_game_exited() {
        let inv = make_inv();
        let mut gwr = false;

        // Disable signal fired, then auto_close re-enabled
        signal_invalidator_if_disabled(true, false, &inv);

        // First tick: game still running, signal consumed and clears gwr (was already false)
        apply_monitor_tick(&mut gwr, true, &inv.0);
        // gwr is now true (game detected)
        assert!(gwr);
        assert!(
            !inv.0.load(Ordering::Relaxed),
            "signal consumed after first tick"
        );

        // Second tick: game exits with no outstanding signal
        let action = apply_monitor_tick(&mut gwr, false, &inv.0);
        assert!(!gwr);
        assert!(matches!(action, MonitorAction::GameExited));
    }
}

/// Outcome of a single enabled monitor iteration.
pub(crate) enum MonitorAction {
    /// Game process newly observed for the first time.
    GameDetected,
    /// Game process was running and has now exited; application should exit.
    GameExited,
    /// No state change this iteration.
    NoChange,
}

/// Applies invalidation and game-state tracking for one enabled monitor iteration.
///
/// Clears `game_was_running` if the invalidator flag was set (indicating `auto_close`
/// was disabled since the previous scan), then derives the appropriate `MonitorAction`
/// from the current `game_running` observation.
pub(crate) fn apply_monitor_tick(
    game_was_running: &mut bool,
    game_running: bool,
    invalidator: &Arc<AtomicBool>,
) -> MonitorAction {
    if invalidator.swap(false, Ordering::Relaxed) {
        *game_was_running = false;
    }

    if game_running {
        let first_detection = !*game_was_running;
        *game_was_running = true;
        if first_detection {
            MonitorAction::GameDetected
        } else {
            MonitorAction::NoChange
        }
    } else if *game_was_running {
        *game_was_running = false;
        MonitorAction::GameExited
    } else {
        MonitorAction::NoChange
    }
}

/// Initiates the background process monitor.
///
/// The process scan only executes when `auto_close` is enabled. When disabled,
/// the thread sleeps at a reduced rate without incurring scan cost.
///
/// `game_was_running` is cleared atomically whenever `set_game_settings` disables
/// `auto_close`, including fast disable/re-enable cycles that complete entirely
/// within the 5-second sleep. The signal is written by the config path and read
/// via `MonitorInvalidator` at the start of each enabled iteration.
pub fn start_monitor<R: Runtime>(app: AppHandle<R>) {
    log::info!("Starting game process monitor...");
    let invalidator: Arc<_> = {
        let state = app.state::<MonitorInvalidator>();
        Arc::clone(&state.0)
    };

    thread::spawn(move || {
        let mut sys = System::new();
        let mut game_was_running = false;

        loop {
            let should_auto_close = {
                let state = app.state::<ConfigState>();
                state.0.lock().map(|c| c.auto_close).unwrap_or(false)
            };

            if should_auto_close {
                sys.refresh_processes_specifics(
                    ProcessesToUpdate::All,
                    true,
                    ProcessRefreshKind::nothing(),
                );
                let game_running = sys
                    .processes()
                    .values()
                    .any(|p| p.name().to_str().map(is_game_process).unwrap_or(false));

                match apply_monitor_tick(&mut game_was_running, game_running, &invalidator) {
                    MonitorAction::GameDetected => {
                        log::info!("Game process detected: {}", PROCESS_NAME_PART);
                    }
                    MonitorAction::GameExited => {
                        log::info!("Game process exited.");
                        log::info!("Auto-close enabled. Exiting application.");
                        app.exit(0);
                        break;
                    }
                    MonitorAction::NoChange => {}
                }

                thread::sleep(Duration::from_secs(5));
            } else {
                game_was_running = false;
                thread::sleep(Duration::from_secs(30));
            }
        }
    });
}
