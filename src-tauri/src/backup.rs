// ITD ODD Save Manager by andromarces

use crate::filename_utils;
use chrono::{DateTime, Local};
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
const LOCKED_FILE_NAME: &str = ".locked";

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
    /// Whether the backup is locked (preventing auto-deletion).
    pub locked: bool,
    /// The SHA-256 hash of the main save file.
    pub hash: String,
    /// An optional user-provided note.
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct BackupIndex {
    pub(crate) games: HashMap<u32, IndexEntry>,
    #[serde(default)]
    pub(crate) notes: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct IndexEntry {
    pub(crate) last_hash: String,
    pub(crate) last_source_size: u64,
    pub(crate) last_source_modified: u128, // Unix timestamp in nanoseconds
    pub(crate) last_backup_path: String,   // Relative folder name of the last backup
}

/// Loads the backup index from disk.
pub(crate) fn load_index(backup_root: &Path) -> BackupIndex {
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
pub(crate) fn save_index(backup_root: &Path, index: &BackupIndex) {
    let index_path = backup_root.join(INDEX_FILE_NAME);
    if let Ok(content) = serde_json::to_string(index) {
        let _ = fs::write(index_path, content);
    }
}

/// Conventional save file paths for a specific game slot.
#[derive(Debug, Clone)]
struct SavePaths {
    main_filename: String,
    main_path: PathBuf,
    bak_filename: String,
    bak_path: PathBuf,
}

/// Builds the conventional save file paths for a game slot.
fn build_save_paths(save_dir: &Path, game_number: u32) -> SavePaths {
    let main_filename = format!("gamesave_{}.sav", game_number);
    let bak_filename = format!("gamesave_{}.sav.bak", game_number);
    SavePaths {
        main_path: save_dir.join(&main_filename),
        bak_path: save_dir.join(&bak_filename),
        main_filename,
        bak_filename,
    }
}

/// Metadata needed for backup naming and deduplication.
#[derive(Debug, Clone)]
struct SourceMetadata {
    size: u64,
    modified_nanos: u128,
    modified_dt: DateTime<Local>,
}

/// Reads metadata needed for deduplication and folder naming.
fn read_source_metadata(main_path: &Path) -> Result<SourceMetadata, String> {
    let metadata = fs::metadata(main_path).map_err(|e| e.to_string())?;
    let modified_time = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let modified_nanos = modified_time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    Ok(SourceMetadata {
        size: metadata.len(),
        modified_nanos,
        modified_dt: modified_time.into(),
    })
}

/// Ensures the backup root directory exists and returns its path.
pub(crate) fn ensure_backup_root(save_dir: &Path) -> Result<PathBuf, String> {
    let backup_root = save_dir.join(BACKUP_DIR_NAME);
    if !backup_root.exists() {
        fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    }
    Ok(backup_root)
}

/// Resolves the content hash, short circuiting when index metadata matches.
fn resolve_hash(
    index: &BackupIndex,
    game_number: u32,
    source: &SourceMetadata,
    main_path: &Path,
) -> Result<(String, bool), String> {
    if let Some(entry) = index.games.get(&game_number) {
        if entry.last_source_size == source.size
            && entry.last_source_modified == source.modified_nanos
        {
            log::debug!(
                "Metadata match for game {}: skipping hash calculation.",
                game_number
            );
            return Ok((entry.last_hash.clone(), false));
        }
    }

    let hash = calculate_hash(main_path)?;
    Ok((hash, true))
}

/// Checks for duplicate backups and refreshes stale index metadata when safe.
fn should_skip_duplicate(
    index: &mut BackupIndex,
    backup_root: &Path,
    save_dir: &Path,
    game_number: u32,
    hash: &str,
    calculated: bool,
    source: &SourceMetadata,
) -> bool {
    if let Some(entry) = index.games.get(&game_number).cloned() {
        if entry.last_hash == hash {
            let last_backup_full_path = backup_root.join(&entry.last_backup_path);
            if last_backup_full_path.exists() {
                if calculated
                    && (entry.last_source_size != source.size
                        || entry.last_source_modified != source.modified_nanos)
                {
                    index.games.insert(
                        game_number,
                        IndexEntry {
                            last_hash: hash.to_string(),
                            last_source_size: source.size,
                            last_source_modified: source.modified_nanos,
                            last_backup_path: entry.last_backup_path.clone(),
                        },
                    );
                    // Disk write deferred to caller for batch optimization
                }

                log::info!(
                    "Duplicate backup found for game {} (index match), skipping.",
                    game_number
                );
                return true;
            }

            log::warn!(
                "Index pointed to missing backup {:?}, forcing new backup.",
                last_backup_full_path
            );
        }
    }

    // Fallback: Check all existing backups for this game to prevent duplicates after restore
    if let Ok(backups) = get_backups(save_dir) {
        for backup in backups {
            if backup.game_number == game_number && backup.hash == hash {
                log::info!(
                    "Duplicate backup found for game {} in existing backup {}, skipping.",
                    game_number,
                    backup.filename
                );

                // Update index to point to this backup for future fast-path metadata matches
                index.games.insert(
                    game_number,
                    IndexEntry {
                        last_hash: hash.to_string(),
                        last_source_size: source.size,
                        last_source_modified: source.modified_nanos,
                        last_backup_path: backup.filename.clone(),
                    },
                );
                return true;
            }
        }
    }

    false
}

/// Creates the target backup directory and returns its path.
fn create_target_dir(backup_root: &Path, folder_name: &str) -> Result<PathBuf, String> {
    let target_dir = backup_root.join(folder_name);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    Ok(target_dir)
}

/// Copies the relevant save files into the target directory.
fn copy_save_files(paths: &SavePaths, target_dir: &Path) -> Result<(), String> {
    fs::copy(&paths.main_path, target_dir.join(&paths.main_filename)).map_err(|e| e.to_string())?;
    if paths.bak_path.exists() {
        fs::copy(&paths.bak_path, target_dir.join(&paths.bak_filename))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Writes the hash marker file into the target directory.
fn write_hash_file(target_dir: &Path, hash: &str) -> Result<(), String> {
    fs::write(target_dir.join(HASH_FILE_NAME), hash).map_err(|e| e.to_string())
}

/// Updates the backup index after a successful backup.
fn update_index_after_backup(
    index: &mut BackupIndex,
    game_number: u32,
    hash: String,
    source: &SourceMetadata,
    folder_name: String,
) {
    index.games.insert(
        game_number,
        IndexEntry {
            last_hash: hash,
            last_source_size: source.size,
            last_source_modified: source.modified_nanos,
            last_backup_path: folder_name,
        },
    );
    // Disk write deferred to caller for batch optimization
}

/// Builds a BackupInfo from a backup folder if it matches the naming contract.
fn backup_info_from_folder(
    path: &Path,
    folder_name: &str,
    save_dir: &Path,
) -> Result<Option<BackupInfo>, String> {
    let Some(info) = filename_utils::parse_backup_folder_name(folder_name) else {
        return Ok(None);
    };

    let main_filename = format!("gamesave_{}.sav", info.game_number);
    let main_file_path = path.join(&main_filename);
    if !main_file_path.exists() {
        log::warn!(
            "Skipping backup folder {:?} because main save is missing.",
            path
        );
        return Ok(None);
    }

    let size = fs::metadata(&main_file_path)
        .map_err(|e| e.to_string())?
        .len();

    let locked = path.join(LOCKED_FILE_NAME).exists();
    let hash = fs::read_to_string(path.join(HASH_FILE_NAME))
        .map(|h| h.trim().to_string())
        .unwrap_or_default();

    Ok(Some(BackupInfo {
        path: path.to_string_lossy().to_string(),
        filename: folder_name.to_string(),
        original_filename: main_filename.clone(),
        original_path: save_dir
            .join(format!("gamesave_{}.sav", info.game_number))
            .to_string_lossy()
            .to_string(),
        size,
        modified: info.timestamp.to_rfc3339(),
        game_number: info.game_number,
        locked,
        hash,
        note: None,
    }))
}

/// Backs up a specific game slot by directory and game number.
///
/// Creates a backup folder named "Game {N+1} - {Timestamp}" to be user-friendly.
///
/// # Arguments
///
/// * `save_dir` - The directory containing the save files.
/// * `game_number` - The 0-based game number index.
/// * `limit` - The maximum number of backups to keep for this game.
///
/// # Returns
///
/// * `Result<Option<PathBuf>, String>` - The path to the created backup folder, or None if skipped.
pub fn perform_backup_for_game(
    save_dir: &Path,
    game_number: u32,
    limit: usize,
) -> Result<Option<PathBuf>, String> {
    if !save_dir.exists() {
        return Err(format!("Save directory does not exist: {:?}", save_dir));
    }

    let backup_root = ensure_backup_root(save_dir)?;
    let mut index = load_index(&backup_root);

    let result =
        perform_backup_for_game_internal(save_dir, &backup_root, game_number, &mut index, limit)?;

    save_index(&backup_root, &index);

    Ok(result)
}

/// Internal implementation of perform_backup_for_game that accepts a mutable index.
/// This allows for batching backups without repeated index I/O.
pub(crate) fn perform_backup_for_game_internal(
    save_dir: &Path,
    backup_root: &Path,
    game_number: u32,
    index: &mut BackupIndex,
    limit: usize,
) -> Result<Option<PathBuf>, String> {
    let paths = build_save_paths(save_dir, game_number);
    if !paths.main_path.exists() {
        // Skip if main save is missing, even if .bak exists (requirement)
        if paths.bak_path.exists() {
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

    let source = read_source_metadata(&paths.main_path)?;
    let (hash, calculated) = resolve_hash(index, game_number, &source, &paths.main_path)?;
    if should_skip_duplicate(
        index,
        backup_root,
        save_dir,
        game_number,
        &hash,
        calculated,
        &source,
    ) {
        return Ok(None);
    }

    // Enforce limit BEFORE creating new backup
    if let Err(e) = enforce_backup_limit(save_dir, game_number, limit) {
        log::error!(
            "Failed to enforce backup limit for game {}: {}",
            game_number,
            e
        );
    }

    let folder_name = filename_utils::format_backup_folder_name(game_number, source.modified_dt);
    let target_dir = create_target_dir(backup_root, &folder_name)?;
    copy_save_files(&paths, &target_dir)?;
    write_hash_file(&target_dir, &hash)?;
    update_index_after_backup(index, game_number, hash, &source, folder_name);

    Ok(Some(target_dir))
}

/// Enforces the backup limit for a specific game.
///
/// Deletes oldest backups if the count meets or exceeds the limit, ensuring space for one new backup.
fn enforce_backup_limit(save_dir: &Path, game_number: u32, limit: usize) -> Result<(), String> {
    // 0 means no limit
    if limit == 0 {
        return Ok(());
    }

    let mut backups = get_backups(save_dir)?
        .into_iter()
        .filter(|b| b.game_number == game_number && !b.locked)
        .collect::<Vec<_>>();

    // Backups are sorted by modified desc (newest first).
    // If we have N backups and limit is L.
    // If N >= L, we need to remove (N - L + 1) backups to make room for the new one?
    // Or just strictly ensure we have < L before adding?
    // "delete the oldest one before creating a new backup"
    // implies if count == limit, delete 1.
    // If count > limit (maybe user changed config), delete enough to be limit - 1.

    if backups.len() >= limit {
        // We want to keep (limit - 1) backups so that adding 1 results in 'limit' backups.
        // So we keep the first (limit - 1) items.
        // We delete everything from index (limit - 1) onwards.

        // Example: Limit 3. Backups: [A, B, C]. Len 3.
        // We want to add D. Result should be [D, A, B]. C deleted.
        // Keep: limit - 1 = 2. Keep [A, B]. Delete C.

        let keep_count = if limit > 0 { limit - 1 } else { 0 };

        if backups.len() > keep_count {
            let to_delete = backups.split_off(keep_count);
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

    let index = load_index(&backup_root);
    let mut backups = Vec::new();

    for entry in fs::read_dir(&backup_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            if let Some(mut info) = backup_info_from_folder(&path, &folder_name, save_dir)? {
                if let Some(note) = index.notes.get(&info.filename) {
                    info.note = Some(note.clone());
                }
                backups.push(info);
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
        if let Err(e) = update_index_after_restore(backup_folder_path, target_save_dir) {
            log::warn!("Failed to update backup index after restore: {}", e);
        }
        Ok(())
    } else {
        Err("No valid save files found in backup folder to restore".to_string())
    }
}

/// Updates the backup index after a successful restore when possible.
fn update_index_after_restore(
    backup_folder_path: &Path,
    target_save_dir: &Path,
) -> Result<(), String> {
    let folder_name = backup_folder_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Backup folder name is invalid".to_string())?;
    let info = filename_utils::parse_backup_folder_name(folder_name)
        .ok_or_else(|| "Backup folder name did not match expected format".to_string())?;
    let backup_root = target_save_dir.join(BACKUP_DIR_NAME);
    if !backup_folder_path.starts_with(&backup_root) {
        return Err("Backup folder is not under the target .backups directory".to_string());
    }

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

    let mut index = load_index(&backup_root);
    index.games.insert(
        info.game_number,
        IndexEntry {
            last_hash: hash,
            last_source_size: source.size,
            last_source_modified: source.modified_nanos,
            last_backup_path: folder_name.to_string(),
        },
    );
    save_index(&backup_root, &index);
    Ok(())
}

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
    let backup_root = ensure_backup_root(save_dir)?;
    let mut index = load_index(&backup_root);

    if let Some(n) = note {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            index.notes.remove(folder_name);
        } else {
            index
                .notes
                .insert(folder_name.to_string(), trimmed.to_string());
        }
    } else {
        index.notes.remove(folder_name);
    }

    save_index(&backup_root, &index);
    Ok(())
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

    /// Tests that save paths are constructed consistently for a game slot.
    #[test]
    fn test_build_save_paths() {
        let dir = tempdir().unwrap();
        let paths = build_save_paths(dir.path(), 2);

        assert_eq!(paths.main_filename, "gamesave_2.sav");
        assert_eq!(paths.bak_filename, "gamesave_2.sav.bak");
        assert_eq!(paths.main_path, dir.path().join("gamesave_2.sav"));
        assert_eq!(paths.bak_path, dir.path().join("gamesave_2.sav.bak"));
    }

    /// Tests that invalid backup folder names are ignored during listing.
    #[test]
    fn test_backup_info_from_folder_ignores_invalid_name() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let backup_root = save_dir.join(BACKUP_DIR_NAME);
        fs::create_dir_all(&backup_root).unwrap();

        let invalid_folder = backup_root.join("not-a-backup");
        fs::create_dir_all(&invalid_folder).unwrap();

        let result = backup_info_from_folder(&invalid_folder, "not-a-backup", save_dir).unwrap();
        assert!(result.is_none());
    }

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
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
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
        let result_dup = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result_dup.is_none());

        // 3. Modify save and backup (should succeed)
        // Sleep to ensure timestamp changes (folder name resolution is seconds)
        std::thread::sleep(std::time::Duration::from_secs(2));
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "new data").unwrap();
        }
        let result_new = perform_backup_for_game(save_dir, game_number, 100).unwrap();
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

        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
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
        let backup_folder = perform_backup_for_game(save_dir, game_number, 100)
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

        let backup_folder = perform_backup_for_game(save_dir, game_number, 100)
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
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        let backup_folder = result.unwrap();

        // 2. Simulate user deleting the backup folder manually, but index remains
        fs::remove_dir_all(&backup_folder).unwrap();

        // 3. Perform backup again - should detect missing folder and recreate
        let result_retry = perform_backup_for_game(save_dir, game_number, 100).unwrap();
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

    /// Tests that the backup limit is enforced.
    #[test]
    fn test_backup_limit_enforcement() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 3;
        let main_sav = save_dir.join("gamesave_3.sav");

        // Limit to 2 backups
        let limit = 2;

        for i in 0..4 {
            {
                let mut f = File::create(&main_sav).unwrap();
                writeln!(f, "data {}", i).unwrap();
            }
            // Sleep to ensure distinct timestamps
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            perform_backup_for_game(save_dir, game_number, limit).unwrap();
        }

        let backups = get_backups(save_dir).unwrap();
        assert_eq!(backups.len(), 2);

        // The newest backups should be retained (data 3 and data 2)
        // Since we slept, timestamps should be ordered.
        // Backup 0 (data 0) and Backup 1 (data 1) should be gone.
        // But we need to verify CONTENT to be sure which ones are left.

        // Helper to check content of a backup
        let check_content = |backup: &BackupInfo, expected: &str| {
            let path = PathBuf::from(&backup.path).join("gamesave_3.sav");
            let content = fs::read_to_string(path).unwrap();
            assert_eq!(content.trim(), expected);
        };

        // backups[0] is newest

        check_content(&backups[0], "data 3");

        check_content(&backups[1], "data 2");
    }

    /// Tests that locked backups are excluded from the limit count and not deleted.

    #[test]

    fn test_backup_lock_enforcement() {
        let dir = tempdir().unwrap();

        let save_dir = dir.path();

        let game_number = 4;

        let main_sav = save_dir.join("gamesave_4.sav");

        let limit = 2;

        // 1. Create first backup

        {
            let mut f = File::create(&main_sav).unwrap();

            writeln!(f, "data 1").unwrap();
        }

        let backup1_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 2. Lock the first backup

        set_backup_lock(&backup1_path, true).unwrap();

        // 3. Create second backup

        {
            let mut f = File::create(&main_sav).unwrap();

            writeln!(f, "data 2").unwrap();
        }

        let _backup2_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 4. Create third backup

        {
            let mut f = File::create(&main_sav).unwrap();

            writeln!(f, "data 3").unwrap();
        }

        let _backup3_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        // Check backups

        let backups = get_backups(save_dir).unwrap();

        // Should have 3 backups total:

        // - data 3 (newest, unlocked)

        // - data 2 (unlocked)

        // - data 1 (oldest, locked)

        // Limit is 2 unlocked. We have 2 unlocked. Locking saved data 1 from being deleted?

        // Wait, if limit is 2 unlocked.

        // Step 1: data 1 (locked) -> 0 unlocked.

        // Step 3: data 2 (unlocked) -> 1 unlocked.

        // Step 4: data 3 (unlocked) -> 2 unlocked.

        // Total backups: 3.

        assert_eq!(backups.len(), 3);

        // Verify content

        let check_content = |backup: &BackupInfo, expected: &str| {
            let path = PathBuf::from(&backup.path).join("gamesave_4.sav");

            let content = fs::read_to_string(path).unwrap();

            assert_eq!(content.trim(), expected);
        };

        // Sorted by modified desc: data 3, data 2, data 1

        check_content(&backups[0], "data 3");

        check_content(&backups[1], "data 2");

        check_content(&backups[2], "data 1");

        assert!(backups[2].locked);

        // 5. Create fourth backup -> limit 2 exceeded for unlocked?

        // Current unlocked: data 3, data 2. Count 2.

        // Adding data 4 -> Count 3 unlocked.

        // Should delete oldest unlocked (data 2).

        // Result: data 4, data 3, data 1 (locked).

        std::thread::sleep(std::time::Duration::from_secs(2));

        {
            let mut f = File::create(&main_sav).unwrap();

            writeln!(f, "data 4").unwrap();
        }

        perform_backup_for_game(save_dir, game_number, limit).unwrap();

        let backups_final = get_backups(save_dir).unwrap();

        assert_eq!(backups_final.len(), 3);

        check_content(&backups_final[0], "data 4");

        check_content(&backups_final[1], "data 3");

        check_content(&backups_final[2], "data 1");
    }

    /// Tests that restoring an older backup does not result in a new duplicate backup.
    #[test]
    fn test_no_duplicate_after_restore() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 0;
        let main_sav = save_dir.join("gamesave_0.sav");

        // 1. Create first backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "version 1").unwrap();
        }
        let backup1_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 2. Modify and create second backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "version 2").unwrap();
        }
        let _backup2_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();

        // 3. Restore first backup
        restore_backup(&backup1_path, save_dir).unwrap();

        // 4. Try to backup again - it should be skipped because it matches backup 1
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(
            result.is_none(),
            "Backup should have been skipped as it matches an existing backup (v1)"
        );

        // 5. Verify index was updated to point to backup 1
        let backup_root = save_dir.join(".backups");
        let index = load_index(&backup_root);
        let entry = index.games.get(&game_number).unwrap();
        assert_eq!(
            entry.last_backup_path,
            backup1_path.file_name().unwrap().to_string_lossy()
        );
    }

    /// Tests note persistence and management.
    #[test]
    fn test_note_persistence() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1;

        // 1. Create a backup
        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data").unwrap();
        }
        let backup_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();
        let folder_name = backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        // 2. Set a note
        set_backup_note(save_dir, &folder_name, Some("My Note".to_string())).unwrap();

        // 3. Verify in index
        let backup_root = save_dir.join(BACKUP_DIR_NAME);
        let index = load_index(&backup_root);
        assert_eq!(index.notes.get(&folder_name).unwrap(), "My Note");

        // 4. Update note
        set_backup_note(save_dir, &folder_name, Some("Updated Note".to_string())).unwrap();
        let index2 = load_index(&backup_root);
        assert_eq!(index2.notes.get(&folder_name).unwrap(), "Updated Note");

        // 5. Verify get_backups retrieves it
        let backups = get_backups(save_dir).unwrap();
        assert_eq!(backups[0].note.as_deref(), Some("Updated Note"));

        // 6. Remove note
        set_backup_note(save_dir, &folder_name, None).unwrap();
        let index3 = load_index(&backup_root);
        assert!(!index3.notes.contains_key(&folder_name));

        let backups_empty = get_backups(save_dir).unwrap();
        assert!(backups_empty[0].note.is_none());
    }
}
