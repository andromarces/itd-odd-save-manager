use super::data::BackupInfo;
use super::listing::get_backups;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Deletes a specific backup folder.
pub fn delete_backup_folder(backup_folder_path: &Path) -> Result<(), String> {
    if !backup_folder_path.exists() {
        return Err("Backup folder does not exist".to_string());
    }

    // Safety check: Ensure we are deleting a directory
    if !backup_folder_path.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    fs::remove_dir_all(backup_folder_path).map_err(|e| e.to_string())?;
    log::info!("Deleted backup folder: {:?}", backup_folder_path);

    Ok(())
}

/// Batch deletes backups based on criteria.
pub fn delete_backups_batch(
    save_dir: &Path,
    target_games: &[u32],
    keep_latest: bool,
    delete_locked: bool,
) -> Result<usize, String> {
    let mut backups = get_backups(save_dir, false, None)?;
    let mut deleted_count = 0;

    // Group backups by game number
    // backups are already sorted by modified desc (newest first)
    let mut games_backups: HashMap<u32, Vec<BackupInfo>> = HashMap::new();
    for backup in backups.drain(..) {
        games_backups
            .entry(backup.game_number)
            .or_default()
            .push(backup);
    }

    for &game_number in target_games {
        if let Some(game_list) = games_backups.get(&game_number) {
            let candidates = if keep_latest {
                // Skip the first one (newest), consider the rest
                if game_list.is_empty() {
                    &[]
                } else {
                    &game_list[1..]
                }
            } else {
                // Consider all
                &game_list[..]
            };

            for backup in candidates {
                if backup.locked && !delete_locked {
                    continue;
                }

                let path = PathBuf::from(&backup.path);
                if let Err(e) = delete_backup_folder(&path) {
                    log::error!("Failed to delete backup {:?}: {}", path, e);
                } else {
                    deleted_count += 1;
                }
            }
        }
    }

    Ok(deleted_count)
}

/// Enforces the backup limit for a specific game.
pub(crate) fn enforce_backup_limit(
    game_number: u32,
    limit: usize,
    all_backups: &[BackupInfo],
) -> Result<(), String> {
    // 0 means no limit
    if limit == 0 {
        return Ok(());
    }

    let mut game_backups = all_backups
        .iter()
        .filter(|b| b.game_number == game_number && !b.locked)
        .cloned()
        .collect::<Vec<_>>();

    if game_backups.len() >= limit {
        let keep_count = if limit > 0 { limit - 1 } else { 0 };

        if game_backups.len() > keep_count {
            let to_delete = game_backups.split_off(keep_count);
            log::info!(
                "Enforcing limit ({}): Deleting {} old backups for game {} to make room.",
                limit,
                to_delete.len(),
                game_number
            );

            for backup in to_delete {
                let path = PathBuf::from(&backup.path);
                if path.exists() {
                    fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}
