use crate::backup::common::BACKUP_DIR_NAME;
use crate::backup::index::BackupStore;
use crate::backup::{self, BackupInfo};
use crate::config::ConfigState;
use crate::watcher::FileWatcher;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{async_runtime, Emitter, Manager, State};

/// Extracts the configured save path without holding the mutex across blocking work.
fn extract_save_path(state: &State<'_, ConfigState>) -> Result<Option<PathBuf>, String> {
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

/// Verifies that a backup path is a canonical direct child of the `.backups` root.
///
/// Rejects the root itself, nested descendants, paths outside the root (including symlink
/// escapes, because the path is canonicalized first), and folder names that do not match
/// the backup folder naming contract.
fn verify_backup_path(save_path: &Path, backup_path: &Path) -> Result<PathBuf, String> {
    let backup_root = save_path.join(BACKUP_DIR_NAME);
    let canonical_target = backup_path
        .canonicalize()
        .map_err(|_| "Invalid backup path".to_string())?;
    let canonical_root = backup_root
        .canonicalize()
        .map_err(|_| "Backup directory not found".to_string())?;

    if canonical_target.parent() != Some(canonical_root.as_path()) {
        return Err(
            "Security violation: Path is not a backup folder in the backup directory".to_string(),
        );
    }

    let folder_name = canonical_target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid backup folder name".to_string())?;
    if crate::filename_utils::parse_backup_folder_name(folder_name).is_none() {
        return Err("Invalid backup folder name".to_string());
    }

    Ok(canonical_target)
}

/// Tauri command to list available backups for the configured save path.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_backups_command(state: State<'_, ConfigState>) -> Result<Vec<BackupInfo>, String> {
    if let Some(path) = extract_save_path(&state)? {
        run_blocking(move || backup::get_backups(&path, false, None)).await
    } else {
        Ok(Vec::new())
    }
}

/// Tauri command to restore a specific backup to the configured save directory.
#[tauri::command(rename_all = "snake_case")]
pub async fn restore_backup_command(
    state: State<'_, ConfigState>,
    backup_path: String,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let backup = verify_backup_path(&save_path, Path::new(&backup_path))?;
    // Canonical form keeps the `.backups` containment check in the restore index update consistent.
    let target_dir = save_path
        .canonicalize()
        .map_err(|_| "Invalid save path".to_string())?;

    run_blocking(move || backup::restore_backup(&backup, &target_dir)).await
}

/// Tauri command to toggle the lock status of a backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_backup_lock_command(
    state: State<'_, ConfigState>,
    backup_path: String,
    locked: bool,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let path = PathBuf::from(&backup_path);

    let verified_path = verify_backup_path(&save_path, &path)?;

    run_blocking(move || backup::set_backup_lock(&verified_path, locked)).await
}

/// Tauri command to set or update a note for a backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_backup_note_command(
    state: State<'_, ConfigState>,
    backup_filename: String,
    note: Option<String>,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;

    run_blocking(move || backup::set_backup_note(&save_path, &backup_filename, note)).await
}

/// Tauri command to delete a specific backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_backup_command(
    state: State<'_, ConfigState>,
    backup_path: String,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let path = PathBuf::from(&backup_path);

    let verified_path = verify_backup_path(&save_path, &path)?;

    run_blocking(move || {
        let folder_name = verified_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        backup::delete_backup_folder(&verified_path)?;

        if let Some(mut store) = BackupStore::load_if_exists(&save_path)? {
            store.index.prune_deleted(&folder_name);
            store.save()?;
        }

        Ok(())
    })
    .await
}

/// Tauri command to batch delete backups.
#[tauri::command(rename_all = "snake_case")]
pub async fn batch_delete_backups_command(
    state: State<'_, ConfigState>,
    game_numbers: Vec<u32>,
    keep_latest: bool,
    delete_locked: bool,
) -> Result<usize, String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;

    run_blocking(move || {
        backup::delete_backups_batch(&save_path, &game_numbers, keep_latest, delete_locked)
    })
    .await
}

/// Command to initialize the watcher from the frontend.
#[tauri::command(rename_all = "snake_case")]
pub async fn init_watcher(
    app: tauri::AppHandle,
    state: State<'_, ConfigState>,
) -> Result<(), String> {
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
            let app_handle = app.clone();
            let on_backup = Arc::new(move || {
                if let Err(e) = app_handle.emit("backups-updated", ()) {
                    log::error!("Failed to emit backups-updated event: {}", e);
                }
            });
            watcher.start(path, config.max_backups_per_game, Some(on_backup))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Verifies that blocking work is dispatched to a different thread than the caller.
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

    const VALID_FOLDER: &str = "Game 1 - 01-Jan-2024 10-00-00 AM";

    /// Creates a directory symlink. Returns false when the platform denies it.
    fn try_symlink_dir(original: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(original, link);
        #[cfg(windows)]
        let result = std::os::windows::fs::symlink_dir(original, link);
        result.is_ok()
    }

    /// Verifies that the backup root, nested descendants, and malformed folder names are rejected.
    #[test]
    fn test_verify_backup_path_rejects_root_nested_and_malformed() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let nested = backup_root.join(VALID_FOLDER).join("child");
        let malformed = backup_root.join("not-a-backup");

        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::create_dir_all(&malformed).expect("failed to create malformed dir");

        assert!(verify_backup_path(&save_path, &backup_root).is_err());
        assert!(verify_backup_path(&save_path, &nested).is_err());
        assert!(verify_backup_path(&save_path, &malformed).is_err());
        assert!(verify_backup_path(&save_path, &save_path).is_err());
    }

    /// Verifies that a symlink inside the backup root that points outside it is rejected.
    #[test]
    fn test_verify_backup_path_rejects_symlink_escape() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let outside = temp_dir.path().join("outside");
        fs::create_dir_all(&backup_root).expect("failed to create backup root");
        fs::create_dir_all(&outside).expect("failed to create outside dir");

        let link = backup_root.join(VALID_FOLDER);
        if !try_symlink_dir(&outside, &link) {
            eprintln!("symlink creation unavailable; skipping");
            return;
        }

        assert!(verify_backup_path(&save_path, &link).is_err());
    }

    /// Verifies the security logic of the verify_backup_path helper.
    #[test]
    fn test_verify_backup_path_security() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let game_backup = backup_root.join(VALID_FOLDER);

        fs::create_dir_all(&game_backup).expect("failed to create mock backup dir");

        assert!(verify_backup_path(&save_path, &game_backup).is_ok());

        let malicious = temp_dir.path().join("malicious.exe");
        fs::File::create(&malicious).expect("failed to create mock malicious file");
        assert!(verify_backup_path(&save_path, &malicious).is_err());

        let other = save_path.join("unauthorized_file.txt");
        fs::File::create(&other).expect("failed to create mock unauthorized file");
        assert!(verify_backup_path(&save_path, &other).is_err());
    }
}
