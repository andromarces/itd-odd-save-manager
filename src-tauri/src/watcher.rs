use crate::backup::{perform_backup, prune_backups};
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
const MAX_BACKUPS: usize = 50;

#[derive(Clone)]
pub struct FileWatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    current_path: Arc<Mutex<Option<PathBuf>>>,
}

impl FileWatcher {
    /// Creates a new, inactive FileWatcher instance.
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            current_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts watching the specified path (file or directory).
    ///
    /// If `path` is a file, watches its parent directory and filters for changes to that specific file.
    /// If `path` is a directory, watches the directory and filters for changes to any `.sav` files.
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
            // We ignore send errors because if the channel is closed, the watcher is stopping anyway
            let _ = tx.send(res);
        })
        .map_err(|e| e.to_string())?;

        // Determine what to watch
        let is_watching_file = path.is_file();
        let watch_target = if is_watching_file {
            path.parent().ok_or("Invalid file path")?.to_path_buf()
        } else {
            path.clone()
        };

        // Watch non-recursively
        if let Err(e) = watcher.watch(&watch_target, RecursiveMode::NonRecursive) {
            return Err(format!("Failed to watch path: {}", e));
        }

        // Store watcher
        *self.watcher.lock().unwrap() = Some(watcher);
        *self.current_path.lock().unwrap() = Some(path.clone());

        // Spawn debouncing thread
        let path_clone = path.clone();
        thread::spawn(move || {
            debounce_loop(rx, path_clone, is_watching_file);
        });

        info!("Started watching: {:?}", path);
        Ok(())
    }

    /// Stops the current watcher, if active.
    pub fn stop(&self) {
        let mut watcher_guard = self.watcher.lock().unwrap();
        if watcher_guard.is_some() {
            // Dropping the watcher stops it.
            *watcher_guard = None;
            *self.current_path.lock().unwrap() = None;
            info!("Stopped watching");
        }
    }
}

/// Runs the debounce loop to process file system events.
///
/// Coalesces rapid events and triggers backups for changed files.
///
/// # Arguments
///
/// * `rx` - The receiver channel for notify events.
/// * `target_path` - The user-configured path (file or directory).
/// * `is_watching_file` - True if `target_path` refers to a specific file, False if it's a directory.
fn debounce_loop(
    rx: Receiver<notify::Result<notify::Event>>,
    target_path: PathBuf,
    is_watching_file: bool,
) {
    let mut pending_files: HashSet<PathBuf> = HashSet::new();
    let mut last_change_time = std::time::Instant::now();
    let mut pending_change = false;

    loop {
        // Calculate timeout
        let timeout = if pending_change {
            let elapsed = last_change_time.elapsed();
            if elapsed >= DEBOUNCE_DURATION {
                // Trigger Backups
                info!(
                    "Debounce timeout reached. Backing up {} files.",
                    pending_files.len()
                );

                for file_path in &pending_files {
                    if let Err(e) = perform_backup(file_path) {
                        error!("Backup failed for {:?}: {}", file_path, e);
                    } else {
                        // Prune
                        if let Some(parent) = file_path.parent() {
                            let backup_dir = parent.join(".backups");
                            let _ = prune_backups(&backup_dir, MAX_BACKUPS);
                        }
                    }
                }

                pending_files.clear();
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
                for path in event.paths {
                    let is_relevant = if is_watching_file {
                        // If watching a specific file, only that file is relevant (case-insensitive)
                        path.file_name().map(|s| s.to_string_lossy().to_lowercase())
                            == target_path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_lowercase())
                    } else {
                        // If watching a directory, any .sav file is relevant (case-insensitive)
                        path.extension()
                            .is_some_and(|ext| ext.to_string_lossy().to_lowercase() == "sav")
                    };

                    if is_relevant {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                // If target is a file, we track the target path (canonical)
                                // If target is a dir, we track the specific file that changed
                                let path_to_backup = if is_watching_file {
                                    target_path.clone()
                                } else {
                                    path.clone()
                                };

                                pending_files.insert(path_to_backup);
                                pending_change = true;
                                last_change_time = std::time::Instant::now();
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Err(e)) => error!("Watch error: {:?}", e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Loop continues, processing pending backups if any
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
    use std::fs::File;
    use tempfile::tempdir;

    /// Tests that a new FileWatcher is initialized with no active watcher or path.
    #[test]
    fn test_file_watcher_initialization() {
        let watcher = FileWatcher::new();
        assert!(watcher.watcher.lock().unwrap().is_none());
    }

    /// Tests the lifecycle of starting and stopping the watcher.
    #[test]
    fn test_file_watcher_start_stop() {
        let watcher = FileWatcher::new();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.sav");
        File::create(&file_path).unwrap();

        assert!(watcher.start(file_path.clone()).is_ok());
        assert!(watcher.watcher.lock().unwrap().is_some());

        watcher.stop();
        assert!(watcher.watcher.lock().unwrap().is_none());
    }

    /// Tests the relevance logic for file watching mode, including case insensitivity.
    #[test]
    fn test_is_relevant_logic_file() {
        // Logic extraction for testing without full notify integration
        let target_path = PathBuf::from("C:\\Games\\Save.sav");
        let _is_watching_file = true;

        // Exact match
        let event_path = PathBuf::from("C:\\Games\\Save.sav");
        let is_relevant = event_path
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            == target_path
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase());
        assert!(is_relevant);

        // Case difference
        let event_path_case = PathBuf::from("C:\\Games\\save.sav");
        let is_relevant_case = event_path_case
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            == target_path
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase());
        assert!(is_relevant_case);

        // Different file
        let event_path_other = PathBuf::from("C:\\Games\\Other.sav");
        let is_relevant_other = event_path_other
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            == target_path
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase());
        assert!(!is_relevant_other);
    }

    /// Tests the relevance logic for directory watching mode, ensuring only .sav files are matched case-insensitively.
    #[test]
    fn test_is_relevant_logic_dir_case_insensitive() {
        // Logic extraction for directory mode
        let _target_path = PathBuf::from("C:\\Games\\");
        let _is_watching_file = false;

        let check = |p: &str| -> bool {
            PathBuf::from(p)
                .extension()
                .is_some_and(|ext| ext.to_string_lossy().to_lowercase() == "sav")
        };

        assert!(check("save.sav"));
        assert!(check("SAVE.SAV"));
        assert!(check("Mixed.SaV"));
        assert!(!check("image.png"));
        assert!(!check("no_ext"));
    }
}
