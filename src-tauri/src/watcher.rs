// ITD ODD Save Manager by andromarces

use crate::backup::{ensure_backup_root, load_index, perform_backup_for_game_internal, save_index};
use crate::filename_utils;
use log::{error, info};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    shutdown: Arc<Mutex<Arc<AtomicBool>>>,
}

impl FileWatcher {
    /// Creates a new, inactive FileWatcher instance.
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(Mutex::new(Arc::new(AtomicBool::new(false)))),
        }
    }

    /// Starts watching the specified path.
    ///
    /// * `on_backup` - Optional callback invoked when one or more backups are successfully created.
    pub fn start(
        &self,
        path: PathBuf,
        limit: usize,
        on_backup: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    ) -> Result<(), String> {
        self.stop();

        let (tx, rx) = channel();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .map_err(|e| e.to_string())?;

        let watch_target = crate::filename_utils::normalize_to_directory(&path)
            .map_err(|e| format!("Invalid watch target: {}", e))?;

        if !watch_target.exists() {
            return Err(format!("Watch target does not exist: {:?}", watch_target));
        }

        if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            return Err(format!("Failed to watch path: {}", e));
        }

        let mut watcher_guard = self
            .watcher
            .lock()
            .map_err(|_| "Failed to lock watcher state".to_string())?;
        *watcher_guard = Some(watcher);

        let shutdown_token = Arc::new(AtomicBool::new(false));
        {
            let mut shutdown_guard = self
                .shutdown
                .lock()
                .map_err(|_| "Failed to lock shutdown state".to_string())?;
            *shutdown_guard = shutdown_token.clone();
        }

        thread::spawn(move || {
            debounce_loop(rx, watch_target, shutdown_token, limit, on_backup);
        });

        info!("Started watching: {:?} (limit: {})", path, limit);
        Ok(())
    }

    /// Stops the current watcher, if active.
    pub fn stop(&self) {
        match self.shutdown.lock() {
            Ok(shutdown_guard) => {
                shutdown_guard.store(true, Ordering::SeqCst);
            }
            Err(e) => error!("Failed to lock shutdown state during stop: {}", e),
        }

        match self.watcher.lock() {
            Ok(mut watcher_guard) => {
                if watcher_guard.is_some() {
                    *watcher_guard = None;
                    info!("Stopped watching");
                }
            }
            Err(e) => error!("Failed to lock watcher state during stop: {}", e),
        }
    }

    /// Returns the current shutdown token for test assertions.
    #[cfg(test)]
    fn debug_shutdown_token(&self) -> Arc<AtomicBool> {
        match self.shutdown.lock() {
            Ok(shutdown_guard) => shutdown_guard.clone(),
            Err(poisoned) => {
                error!(
                    "Shutdown state lock poisoned during debug access: {}",
                    poisoned
                );
                poisoned.into_inner().clone()
            }
        }
    }
}

/// Executes backups for a set of games with a shared index load and save.
///
/// Returns `true` if at least one backup was successfully created.
fn perform_batch_backups(save_dir: &Path, game_numbers: &HashSet<u32>, limit: usize) -> bool {
    if game_numbers.is_empty() {
        return false;
    }

    let mut backups_created = false;
    if let Ok(backup_root) = ensure_backup_root(save_dir) {
        let mut index = load_index(&backup_root);
        let filter = if game_numbers.len() == 1 {
            game_numbers.iter().next().cloned()
        } else {
            None
        };
        let backups = crate::backup::get_backups(save_dir, true, filter).unwrap_or_default();

        for &game_number in game_numbers {
            match perform_backup_for_game_internal(
                save_dir,
                &backup_root,
                game_number,
                &mut index,
                limit,
                &backups,
            ) {
                Ok(Some(_)) => backups_created = true,
                Ok(None) => {}
                Err(e) => error!("Backup failed for game {}: {}", game_number, e),
            }
        }
        save_index(&backup_root, &index);
    }
    backups_created
}

/// Performs an immediate scan of the directory and backs up any existing save files.
///
/// Returns `true` if at least one backup was successfully created during the scan.
pub(crate) fn scan_and_backup_existing(save_dir: &Path, limit: usize) -> bool {
    info!("Performing initial scan of {:?}", save_dir);
    if let Ok(entries) = std::fs::read_dir(save_dir) {
        let mut pending_games = HashSet::new();
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if !name.starts_with("gamesave_") {
                    continue;
                }
            }

            if let Some(info) = filename_utils::parse_path(&entry.path()) {
                if !info.is_bak {
                    pending_games.insert(info.game_number);
                }
            }
        }
        return perform_batch_backups(save_dir, &pending_games, limit);
    }
    false
}

/// Runs the debounce loop to process file system events.
fn debounce_loop(
    rx: Receiver<notify::Result<notify::Event>>,
    save_dir: PathBuf,
    shutdown: Arc<AtomicBool>,
    limit: usize,
    on_backup: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
) {
    // Initial Scan: Check for existing saves that need backup
    if scan_and_backup_existing(&save_dir, limit) {
        if let Some(cb) = &on_backup {
            cb();
        }
    }

    let mut pending_games: HashSet<u32> = HashSet::new();
    let mut last_change_time = std::time::Instant::now();
    let mut pending_change = false;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Calculate timeout
        let timeout = if pending_change {
            let elapsed = last_change_time.elapsed();
            if elapsed >= DEBOUNCE_DURATION {
                info!(
                    "Debounce timeout. Backing up {} games.",
                    pending_games.len()
                );
                if perform_batch_backups(&save_dir, &pending_games, limit) {
                    if let Some(cb) = &on_backup {
                        cb();
                    }
                }
                pending_games.clear();
                pending_change = false;
                Duration::from_secs(60)
            } else {
                DEBOUNCE_DURATION - elapsed
            }
        } else {
            Duration::from_secs(60)
        };

        match rx.recv_timeout(timeout) {
            Ok(Ok(event)) => {
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
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use tempfile::tempdir;

    /// Verifies that FileWatcher can start and stop without error.
    #[test]
    fn test_file_watcher_lifecycle() {
        let watcher = FileWatcher::new();
        let dir = tempdir().unwrap();
        assert!(watcher.start(dir.path().to_path_buf(), 100, None).is_ok());
        watcher.stop();
    }

    /// Ensures that a previous shutdown token remains signaled after restarting the watcher.
    #[test]
    fn test_shutdown_signal_not_cleared_by_restart() {
        let watcher = FileWatcher::new();
        let dir = tempdir().unwrap();

        assert!(watcher.start(dir.path().to_path_buf(), 100, None).is_ok());
        let shutdown_token = watcher.debug_shutdown_token();

        watcher.stop();
        assert!(
            shutdown_token.load(Ordering::SeqCst),
            "Shutdown token should be true after stop"
        );

        assert!(watcher.start(dir.path().to_path_buf(), 100, None).is_ok());
        assert!(
            shutdown_token.load(Ordering::SeqCst),
            "Shutdown token from the previous thread should remain true after restart"
        );
    }

    /// Confirms that debug_shutdown_token does not panic when the mutex is poisoned.
    #[test]
    fn test_debug_shutdown_token_handles_poisoned_lock() {
        let watcher = FileWatcher::new();
        let shutdown_arc = watcher.shutdown.clone();

        let poison_result = std::panic::catch_unwind(move || {
            let _guard = shutdown_arc.lock().unwrap();
            panic!("poison shutdown mutex");
        });
        assert!(poison_result.is_err(), "Expected a poisoned shutdown mutex");

        let debug_result = std::panic::catch_unwind(|| watcher.debug_shutdown_token());
        assert!(
            debug_result.is_ok(),
            "debug_shutdown_token should not panic on poisoned mutex"
        );
    }

    /// Validates that filename parsing correctly identifies save files and backup files.
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

        assert!(filename_utils::parse_path(&path_invalid).is_none());
        assert!(filename_utils::parse_path(&path_invalid_fmt).is_none());
    }

    /// Checks that scan_and_backup_existing backs up multiple save files correctly.
    #[test]
    fn test_initial_scan_batch_processing() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path().to_path_buf();

        let save1 = save_dir.join("gamesave_1.sav");
        let save2 = save_dir.join("gamesave_2.sav");
        std::fs::write(&save1, "data1").unwrap();
        std::fs::write(&save2, "data2").unwrap();

        scan_and_backup_existing(&save_dir, 100);

        let backups_dir = save_dir.join(".backups");
        assert!(backups_dir.exists());
        let index_path = backups_dir.join("index.json");
        assert!(index_path.exists());

        let backups = crate::backup::get_backups(&save_dir, true, None).unwrap();
        assert_eq!(backups.len(), 2);

        let games: std::collections::HashSet<u32> = backups.iter().map(|b| b.game_number).collect();
        assert!(games.contains(&1));
        assert!(games.contains(&2));
    }

    /// Ensures the initial scan runs promptly after starting the watcher.
    #[test]
    fn test_initial_scan_is_prompt() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path().to_path_buf();
        let save1 = save_dir.join("gamesave_1.sav");
        std::fs::write(&save1, "data1").unwrap();

        let watcher = FileWatcher::new();
        let start_time = std::time::Instant::now();

        watcher.start(save_dir.clone(), 100, None).unwrap();

        let mut found = false;
        for _ in 0..15 {
            if save_dir.join(".backups").exists() {
                found = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        let elapsed = start_time.elapsed();
        watcher.stop();

        assert!(found, "Backup folder should be created by initial scan");
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "Initial scan should happen promptly, but took {:?}",
            elapsed
        );
    }
}
