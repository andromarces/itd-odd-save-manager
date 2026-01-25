// ITD ODD Save Manager by andromarces

use crate::backup::perform_backup_for_game;
use crate::filename_utils;
use log::{error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Default debounce duration to coalesce rapid writes
const DEBOUNCE_DURATION: Duration = Duration::from_secs(2);

/// Watches for file system changes in the save directory.
#[derive(Clone)]
pub struct FileWatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
}

impl FileWatcher {
    /// Creates a new, inactive FileWatcher instance.
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts watching the specified path.
    ///
    /// Expects a directory path usually, but handles file path by watching parent.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to watch.
    pub fn start(&self, path: PathBuf) -> Result<(), String> {
        // Stop existing if any
        self.stop();

        let (tx, rx) = channel();

        // Init watcher
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| e.to_string())?;

        let is_file_input = path.is_file();

        // We always watch the directory
        let watch_target = if is_file_input {
            path.parent().ok_or("Invalid file path")?.to_path_buf()
        } else {
            path.clone()
        };

        if !watch_target.exists() {
            return Err(format!("Watch target does not exist: {:?}", watch_target));
        }

        // Watch non-recursively
        if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            return Err(format!("Failed to watch path: {}", e));
        }

        // Store watcher
        *self.watcher.lock().unwrap() = Some(watcher);

        // Spawn debouncing thread
        thread::spawn(move || {
            debounce_loop(rx, watch_target);
        });

        info!("Started watching: {:?}", path);
        Ok(())
    }

    /// Stops the current watcher, if active.
    pub fn stop(&self) {
        let mut watcher_guard = self.watcher.lock().unwrap();
        if watcher_guard.is_some() {
            *watcher_guard = None;
            info!("Stopped watching");
        }
    }
}

/// Runs the debounce loop to process file system events.
///
/// It performs an initial scan for existing save files and then listens for
/// file system events, aggregating them to trigger backups.
///
/// # Arguments
///
/// * `rx` - Receiver for file system events.
/// * `save_dir` - The directory being watched.
fn debounce_loop(rx: Receiver<notify::Result<notify::Event>>, save_dir: PathBuf) {
    // Defer initial scan to improve startup responsiveness
    thread::sleep(Duration::from_secs(3));

    // Initial Scan: Check for existing saves that need backup
    info!("Performing initial scan of {:?}", save_dir);
    if let Ok(entries) = std::fs::read_dir(&save_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // We only care about main save files for the trigger,
            // perform_backup_for_game handles the rest.
            if let Some(info) = filename_utils::parse_path(&path) {
                if !info.is_bak {
                    if let Err(e) = perform_backup_for_game(&save_dir, info.game_number) {
                        error!("Initial backup failed for game {}: {}", info.game_number, e);
                    }
                }
            }
        }
    }

    let mut pending_games: HashSet<u32> = HashSet::new();
    let mut last_change_time = std::time::Instant::now();
    let mut pending_change = false;

    loop {
        // Calculate timeout
        let timeout = if pending_change {
            let elapsed = last_change_time.elapsed();
            if elapsed >= DEBOUNCE_DURATION {
                // Trigger Backups
                info!(
                    "Debounce timeout. Backing up {} games.",
                    pending_games.len()
                );

                for game_number in pending_games.iter() {
                    if let Err(e) = perform_backup_for_game(&save_dir, *game_number) {
                        error!("Backup failed for game {}: {}", game_number, e);
                    }
                }

                pending_games.clear();
                pending_change = false;
                Duration::from_secs(60) // Idle wait
            } else {
                DEBOUNCE_DURATION - elapsed
            }
        } else {
            Duration::from_secs(3600) // Long wait
        };

        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
                // Check for relevance
                let mut relevant_event = false;
                for path in event.paths {
                    if let Some(info) = filename_utils::parse_path(&path) {
                        if !info.is_bak {
                            pending_games.insert(info.game_number);
                            relevant_event = true;
                        }
                    }
                }

                if relevant_event {
                    pending_change = true;
                    last_change_time = std::time::Instant::now();
                }
            }
            Ok(Err(e)) => error!("Watch error: {:?}", e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Loop continues
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Tests that the watcher starts and stops without error.
    #[test]
    fn test_file_watcher_lifecycle() {
        let watcher = FileWatcher::new();
        let dir = tempdir().unwrap();
        // Just verify it doesn't panic on start/stop
        assert!(watcher.start(dir.path().to_path_buf()).is_ok());
        watcher.stop();
    }

    /// Tests the path parsing logic that underpins the debounce loop's filtering.
    /// The debounce_loop (private) uses `!info.is_bak` to ignore backup files.
    #[test]
    fn test_filename_parsing_for_filtering() {
        let path_valid = PathBuf::from("gamesave_1.sav");
        let path_bak = PathBuf::from("gamesave_1.sav.bak");
        let path_invalid = PathBuf::from("other.txt");
        let path_invalid_fmt = PathBuf::from("gamesave_abc.sav");

        let valid_info = filename_utils::parse_path(&path_valid).unwrap();
        assert!(
            !valid_info.is_bak,
            "Main save file should not be marked as bak"
        );

        let bak_info = filename_utils::parse_path(&path_bak).unwrap();
        assert!(bak_info.is_bak, "Backup file should be marked as bak");

        // These return None, so they are filtered out implicitly by the `if let Some` check
        assert!(filename_utils::parse_path(&path_invalid).is_none());
        assert!(filename_utils::parse_path(&path_invalid_fmt).is_none());
    }
}
