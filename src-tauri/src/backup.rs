// ITD ODD Save Manager by andromarces

use crate::filename_utils;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BACKUP_DIR_NAME: &str = ".backups";
const HASH_FILE_NAME: &str = ".hash";
const INDEX_FILE_NAME: &str = "index.json";

/// Represents metadata for a backup entry.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupInfo {
    /// The absolute path to the backup folder.
    pub path: String,
    /// The display name of the backup folder (e.g., "Game 1 - ...").
    pub filename: String,
    /// The name of the original save file (e.g., "gamesave_0.sav").
    pub original_filename: String,
    /// The original path where the save file resides.
    pub original_path: String,
    /// The size of the save file in bytes.
    pub size: u64,
    /// The modification timestamp (ISO 8601).
    pub modified: String,
    /// The game number (0-based for internal logic).
    pub game_number: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct BackupIndex {
    games: HashMap<u32, IndexEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct IndexEntry {
    last_hash: String,
    last_source_size: u64,
    last_source_modified: u128, // Unix timestamp in nanoseconds
    last_backup_path: String,   // Relative folder name of the last backup
}

/// Loads the backup index from disk.
fn load_index(backup_root: &Path) -> BackupIndex {
    let index_path = backup_root.join(INDEX_FILE_NAME);
    if index_path.exists() {
        if let Ok(content) = fs::read_to_string(&index_path) {
            if let Ok(index) = serde_json::from_str(&content) {
                return index;
            }
        }
    }
    BackupIndex::default()
}

/// Saves the backup index to disk.
fn save_index(backup_root: &Path, index: &BackupIndex) {
    let index_path = backup_root.join(INDEX_FILE_NAME);
    if let Ok(content) = serde_json::to_string_pretty(index) {
        let _ = fs::write(index_path, content);
    }
}

/// Backs up a specific game slot by directory and game number.
///
/// Creates a backup folder named "Game {N+1} - {Timestamp}" to be user-friendly.
///
/// # Arguments
///
/// * `save_dir` - The directory containing the save files.
/// * `game_number` - The 0-based game number index.
///
/// # Returns
///
/// * `Result<Option<PathBuf>, String>` - The path to the created backup folder, or None if skipped.
pub fn perform_backup_for_game(
    save_dir: &Path,
    game_number: u32,
) -> Result<Option<PathBuf>, String> {
    if !save_dir.exists() {
        return Err(format!("Save directory does not exist: {:?}", save_dir));
    }

    let main_filename = format!("gamesave_{}.sav", game_number);
    let main_path = save_dir.join(&main_filename);
    let bak_filename = format!("gamesave_{}.sav.bak", game_number);
    let bak_path = save_dir.join(&bak_filename);

    if !main_path.exists() {
        // Skip if main save is missing, even if .bak exists (requirement)
        if bak_path.exists() {
            log::info!(
                "Only .bak exists for game {}, skipping backup.",
                game_number
            );
        } else {
            log::info!(
                "Main save file not found for game {}, skipping backup.",
                game_number
            );
        }
        return Ok(None);
    }

    let backup_root = save_dir.join(BACKUP_DIR_NAME);
    if !backup_root.exists() {
        fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    }

    let mut index = load_index(&backup_root);

    // Get metadata of main save
    let metadata = fs::metadata(&main_path).map_err(|e| e.to_string())?;
    let size = metadata.len();
    let mtime = metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut hash = String::new();
    let mut calculated = false;

    // Check index for short-circuiting
    if let Some(entry) = index.games.get(&game_number) {
        if entry.last_source_size == size && entry.last_source_modified == mtime {
            // Metadata matches, assume hash is same
            hash = entry.last_hash.clone();
            log::debug!(
                "Metadata match for game {}: skipping hash calculation.",
                game_number
            );
        }
    }

    if hash.is_empty() {
        hash = calculate_hash(&main_path)?;
        calculated = true;
    }

    // Check for duplicates using index
    if let Some(entry) = index.games.get(&game_number) {
        if entry.last_hash == hash {
            // Check if the referenced backup folder actually exists
            let last_backup_full_path = backup_root.join(&entry.last_backup_path);
            if last_backup_full_path.exists() {
                // If we calculated it, update metadata in index if it was stale
                if calculated
                    && (entry.last_source_size != size || entry.last_source_modified != mtime)
                {
                    index.games.insert(
                        game_number,
                        IndexEntry {
                            last_hash: hash,
                            last_source_size: size,
                            last_source_modified: mtime,
                            last_backup_path: entry.last_backup_path.clone(),
                        },
                    );
                    save_index(&backup_root, &index);
                }

                log::info!(
                    "Duplicate backup found for game {} (index match), skipping.",
                    game_number
                );
                return Ok(None);
            } else {
                log::warn!(
                    "Index pointed to missing backup {:?}, forcing new backup.",
                    last_backup_full_path
                );
                // Proceed to create new backup
            }
        }
    }

    // Proceed with backup
    let display_number = game_number + 1;
    let folder_prefix = format!("Game {} -", display_number);

    let modified_dt: DateTime<Local> = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();
    // Format: Game N - dd-MMM-yyyy hh-mm-ss AM
    let timestamp_str = modified_dt.format("%d-%b-%Y %I-%M-%S %p").to_string();
    let folder_name = format!("{} {}", folder_prefix.trim(), timestamp_str);

    let target_dir = backup_root.join(&folder_name);
    // If target dir already exists (e.g. fast updates within same second), we might overwrite or fail.
    // fs::create_dir_all succeeds if exists.
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

    // Copy files
    fs::copy(&main_path, target_dir.join(&main_filename)).map_err(|e| e.to_string())?;
    if bak_path.exists() {
        fs::copy(&bak_path, target_dir.join(&bak_filename)).map_err(|e| e.to_string())?;
    }

    // Write hash
    fs::write(target_dir.join(HASH_FILE_NAME), &hash).map_err(|e| e.to_string())?;

    // Update Index
    index.games.insert(
        game_number,
        IndexEntry {
            last_hash: hash,
            last_source_size: size,
            last_source_modified: mtime,
            last_backup_path: folder_name,
        },
    );
    save_index(&backup_root, &index);

    log::info!(
        "Backup created for game {} at {:?}",
        game_number,
        target_dir
    );
    Ok(Some(target_dir))
}

/// Lists all backups available in the .backups directory.
///
/// Parses the backup folder name to derive the timestamp and game number.
///
/// # Arguments
///
/// * `save_dir` - The directory containing the `.backups` folder.
///
/// # Returns
///
/// * `Result<Vec<BackupInfo>, String>` - A list of available backups.
pub fn get_backups(save_dir: &Path) -> Result<Vec<BackupInfo>, String> {
    let backup_root = save_dir.join(BACKUP_DIR_NAME);
    if !backup_root.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();

    for entry in fs::read_dir(&backup_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();

            // Expected Format: "Game {N} - {Timestamp}"
            // where Timestamp is "dd-MMM-yyyy hh-mm-ss AM"
            if let Some((prefix, date_part)) = folder_name.split_once(" - ") {
                // Parse "Game {N}"
                if let Some(stripped_prefix) = prefix.strip_prefix("Game ") {
                    if let Ok(display_number) = stripped_prefix.parse::<u32>() {
                        // Internal game number is 0-based
                        let game_number = display_number.saturating_sub(1);

                        // Parse timestamp using NaiveDateTime and Local timezone to avoid deprecation
                        let modified_str = if let Ok(naive_dt) =
                            NaiveDateTime::parse_from_str(date_part, "%d-%b-%Y %I-%M-%S %p")
                        {
                            match Local.from_local_datetime(&naive_dt) {
                                chrono::LocalResult::Single(dt) => dt.to_rfc3339(),
                                chrono::LocalResult::Ambiguous(dt1, _) => dt1.to_rfc3339(),
                                chrono::LocalResult::None => {
                                    // Fallback if conversion fails
                                    let main_file_path =
                                        path.join(format!("gamesave_{}.sav", game_number));
                                    if let Ok(meta) = fs::metadata(&main_file_path) {
                                        let m: DateTime<Local> = meta
                                            .modified()
                                            .unwrap_or(SystemTime::UNIX_EPOCH)
                                            .into();
                                        m.to_rfc3339()
                                    } else {
                                        let dt: DateTime<Local> = SystemTime::UNIX_EPOCH.into();
                                        dt.to_rfc3339()
                                    }
                                }
                            }
                        } else {
                            // Fallback to file metadata if parsing fails
                            let main_file_path = path.join(format!("gamesave_{}.sav", game_number));
                            if let Ok(meta) = fs::metadata(&main_file_path) {
                                let m: DateTime<Local> =
                                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();
                                m.to_rfc3339()
                            } else {
                                // Default to epoch if all else fails
                                let dt: DateTime<Local> = SystemTime::UNIX_EPOCH.into();
                                dt.to_rfc3339()
                            }
                        };

                        // We still need file size from the actual file
                        let main_filename = format!("gamesave_{}.sav", game_number);
                        let main_file_path = path.join(&main_filename);
                        let size = if main_file_path.exists() {
                            fs::metadata(&main_file_path).map(|m| m.len()).unwrap_or(0)
                        } else {
                            0
                        };

                        if main_file_path.exists() {
                            backups.push(BackupInfo {
                                path: path.to_string_lossy().to_string(),
                                filename: folder_name,
                                original_filename: main_filename,
                                original_path: save_dir
                                    .join(format!("gamesave_{}.sav", game_number))
                                    .to_string_lossy()
                                    .to_string(),
                                size,
                                modified: modified_str,
                                game_number,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by modified desc
    backups.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(backups)
}

/// Restores a backup folder to the save directory.
/// Copies gamesave_{n}.sav and gamesave_{n}.sav.bak from backup folder to target dir.
///
/// # Arguments
///
/// * `backup_folder_path` - The path to the specific backup folder.
/// * `target_save_dir` - The directory to restore files into.
pub fn restore_backup(backup_folder_path: &Path, target_save_dir: &Path) -> Result<(), String> {
    if !backup_folder_path.exists() {
        return Err("Backup folder does not exist".to_string());
    }
    if !target_save_dir.exists() {
        return Err("Target save directory does not exist".to_string());
    }

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
        Ok(())
    } else {
        Err("No valid save files found in backup folder to restore".to_string())
    }
}

/// Calculates the SHA-256 hash of a file.
fn calculate_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    /// Tests the complete backup flow including folder creation, deduplication, and file copying.
    #[test]
    fn test_backup_flow() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1; // Internal 1 -> Display "Game 2"

        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "save data").unwrap();
        }

        // 1. Perform backup
        let result = perform_backup_for_game(save_dir, game_number).unwrap();
        assert!(result.is_some());
        let backup_folder = result.unwrap();

        assert!(backup_folder.exists());
        // Verify naming: "Game 2 - ..."
        let folder_name = backup_folder.file_name().unwrap().to_string_lossy();
        assert!(folder_name.starts_with("Game 2 -"));

        assert!(backup_folder.join("gamesave_1.sav").exists());
        assert!(backup_folder.join(".hash").exists());

        // Verify index was created
        let index_path = save_dir.join(".backups").join("index.json");
        assert!(index_path.exists());

        // 2. Perform duplicate backup (should skip)
        let result_dup = perform_backup_for_game(save_dir, game_number).unwrap();
        assert!(result_dup.is_none());

        // 3. Modify save and backup (should succeed)
        // Sleep to ensure timestamp changes (folder name resolution is seconds)
        std::thread::sleep(std::time::Duration::from_secs(2));
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "new data").unwrap();
        }
        let result_new = perform_backup_for_game(save_dir, game_number).unwrap();
        assert!(result_new.is_some());
        assert_ne!(result_new.unwrap(), backup_folder); // Different timestamp folder

        // 4. List backups
        let backups = get_backups(save_dir).unwrap();
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].game_number, 1);
        // Verify folder name logic in listing
        assert!(backups[0].filename.starts_with("Game 2 -"));
    }

    /// Tests that backup is skipped if only the .bak file exists.
    #[test]
    fn test_skip_if_only_bak() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 0;

        let bak_sav = save_dir.join("gamesave_0.sav.bak");
        File::create(&bak_sav).unwrap();

        let result = perform_backup_for_game(save_dir, game_number).unwrap();
        assert!(result.is_none());
    }

    /// Tests the restore functionality.
    #[test]
    fn test_restore() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 2;

        let main_sav = save_dir.join("gamesave_2.sav");
        let bak_sav = save_dir.join("gamesave_2.sav.bak");

        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "original").unwrap();
            let mut f2 = File::create(&bak_sav).unwrap();
            writeln!(f2, "original bak").unwrap();
        }

        // Backup
        let backup_folder = perform_backup_for_game(save_dir, game_number)
            .unwrap()
            .unwrap();

        // Modify
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "corrupted").unwrap();
        }

        // Restore
        restore_backup(&backup_folder, save_dir).unwrap();

        let content = fs::read_to_string(&main_sav).unwrap();
        assert_eq!(content.trim(), "original");

        let content_bak = fs::read_to_string(&bak_sav).unwrap();
        assert_eq!(content_bak.trim(), "original bak");
    }

    /// Tests that the hash file contains only the hex digest.
    #[test]
    fn test_hash_content() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 5;
        let main_sav = save_dir.join("gamesave_5.sav");

        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "hash test").unwrap();
        }

        let backup_folder = perform_backup_for_game(save_dir, game_number)
            .unwrap()
            .unwrap();
        let hash_file = backup_folder.join(".hash");

        let content = fs::read_to_string(hash_file).unwrap();
        // Hex digest of "hash test\n" (on linux/mac) or "hash test\r\n" (windows)
        // Just verify it's a hex string.
        assert!(content.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!content.contains("Sha256")); // Should not contain struct debug info
    }

    /// Tests robustness against missing backup folders when index exists.
    #[test]
    fn test_missing_backup_folder_recovery() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1;

        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "important data").unwrap();
        }

        // 1. Create initial backup
        let result = perform_backup_for_game(save_dir, game_number).unwrap();
        let backup_folder = result.unwrap();

        // 2. Simulate user deleting the backup folder manually, but index remains
        fs::remove_dir_all(&backup_folder).unwrap();

        // 3. Perform backup again - should detect missing folder and recreate
        let result_retry = perform_backup_for_game(save_dir, game_number).unwrap();
        assert!(result_retry.is_some());
        let new_backup_folder = result_retry.unwrap();

        assert!(new_backup_folder.exists());
        let restored_sav = new_backup_folder.join("gamesave_1.sav");
        assert!(restored_sav.exists());
        assert!(new_backup_folder.join(".hash").exists());

        // Verify content matches original
        let content = fs::read_to_string(restored_sav).unwrap();
        assert_eq!(content.trim(), "important data");
    }
}
