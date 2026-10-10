use super::common::{BACKUP_DIR_NAME, HASH_FILE_NAME};
use super::data::{build_save_paths, read_source_metadata};
use super::hashing::calculate_hash;
use super::index::{BackupStore, IndexEntry};
use crate::filename_utils;
use std::fs;
use std::path::{Path, PathBuf};

/// Resolves the canonical backup folder and checks it against the canonical `.backups` root of
/// the target. The folder must be a direct child of that root with a valid backup folder name.
/// Both the pre-copy guard and the index update use this check, so they cannot disagree.
fn validate_backup_source(
    backup_folder_path: &Path,
    target_save_dir: &Path,
) -> Result<(PathBuf, String, filename_utils::BackupFolderInfo), String> {
    let source = backup_folder_path
        .canonicalize()
        .map_err(|_| "Backup folder does not exist".to_string())?;
    let backup_root = target_save_dir
        .join(BACKUP_DIR_NAME)
        .canonicalize()
        .map_err(|_| "Target .backups directory not found".to_string())?;
    if source.parent() != Some(backup_root.as_path()) {
        return Err("Backup folder is not under the target .backups directory".to_string());
    }
    let folder_name = source
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Backup folder name is invalid".to_string())?
        .to_string();
    let info = filename_utils::parse_backup_folder_name(&folder_name)
        .ok_or_else(|| "Backup folder name did not match expected format".to_string())?;
    Ok((source, folder_name, info))
}

/// Restores a backup folder to the save directory.
pub fn restore_backup(backup_folder_path: &Path, target_save_dir: &Path) -> Result<(), String> {
    if !backup_folder_path.exists() {
        return Err("Backup folder does not exist".to_string());
    }
    if !target_save_dir.exists() {
        return Err("Target save directory does not exist".to_string());
    }
    // Validate before any file is copied.
    let (backup_folder_path, _, _) = validate_backup_source(backup_folder_path, target_save_dir)?;
    let backup_folder_path = backup_folder_path.as_path();

    let mut restored_any = false;

    for entry in fs::read_dir(backup_folder_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            if let Some(_info) = filename_utils::parse_path(&path) {
                // It is a valid save file or bak
                let file_name = path.file_name().unwrap();
                let target_file = target_save_dir.join(file_name);

                fs::copy(&path, &target_file).map_err(|e| e.to_string())?;
                restored_any = true;
            }
        }
    }

    if restored_any {
        log::info!(
            "Restored backup from {:?} to {:?}",
            backup_folder_path,
            target_save_dir
        );
        update_index_after_restore(backup_folder_path, target_save_dir)
    } else {
        Err("No valid save files found in backup folder to restore".to_string())
    }
}

/// Updates the backup index after a successful restore when possible.
fn update_index_after_restore(
    backup_folder_path: &Path,
    target_save_dir: &Path,
) -> Result<(), String> {
    let (_, folder_name, info) = validate_backup_source(backup_folder_path, target_save_dir)?;

    let paths = build_save_paths(target_save_dir, info.game_number);
    if !paths.main_path.exists() {
        return Err("Restored main save file was not found after restore".to_string());
    }
    let source = read_source_metadata(&paths.main_path)?;

    let hash_path = backup_folder_path.join(HASH_FILE_NAME);
    let hash = if hash_path.exists() {
        fs::read_to_string(&hash_path)
            .map_err(|e| e.to_string())?
            .trim()
            .to_string()
    } else {
        String::new()
    };
    let hash = if hash.is_empty() {
        calculate_hash(&paths.main_path)?
    } else {
        hash
    };

    let mut store = BackupStore::new(target_save_dir)?;
    store.index.games.insert(
        info.game_number,
        IndexEntry::from_source(hash, &source, folder_name),
    );
    store.save()
}
