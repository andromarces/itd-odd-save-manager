use super::common::LOCKED_FILE_NAME;
use super::index::BackupStore;
use std::fs;
use std::path::Path;

/// Sets or unsets the lock status for a backup folder.
pub fn set_backup_lock(backup_folder_path: &Path, locked: bool) -> Result<(), String> {
    if !backup_folder_path.exists() {
        return Err("Backup folder does not exist".to_string());
    }

    let lock_file = backup_folder_path.join(LOCKED_FILE_NAME);

    if locked {
        if !lock_file.exists() {
            fs::write(&lock_file, "").map_err(|e| e.to_string())?;
        }
    } else if lock_file.exists() {
        fs::remove_file(&lock_file).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Sets or updates a note for a specific backup.
pub fn set_backup_note(
    save_dir: &Path,
    folder_name: &str,
    note: Option<String>,
) -> Result<(), String> {
    let mut store = BackupStore::new(save_dir)?;

    if let Some(n) = note {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            store.index.notes.remove(folder_name);
        } else {
            store
                .index
                .notes
                .insert(folder_name.to_string(), trimmed.to_string());
        }
    } else {
        store.index.notes.remove(folder_name);
    }

    store.save();
    Ok(())
}
