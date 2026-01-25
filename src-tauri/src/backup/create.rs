use super::cleanup::enforce_backup_limit;
use super::common::HASH_FILE_NAME;
use super::data::{build_save_paths, read_source_metadata, BackupInfo, SavePaths, SourceMetadata};
use super::hashing::calculate_hash;
use super::index::{BackupIndex, BackupStore, IndexEntry};
use super::listing::get_backups;
use crate::filename_utils;
use std::fs;
use std::path::{Path, PathBuf};

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

/// Checks if the current save matches the index metadata (fast deduplication).
fn is_duplicate_by_index(
    index: &mut BackupIndex,
    backup_root: &Path,
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
    false
}

/// Checks if the current save matches any existing backup content (fallback deduplication).
fn is_duplicate_by_content(
    index: &mut BackupIndex,
    game_number: u32,
    hash: &str,
    source: &SourceMetadata,
    backups: &[BackupInfo],
) -> bool {
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
    false
}

/// Internal implementation of perform_backup_for_game that accepts a mutable index.
pub(crate) fn perform_backup_for_game_internal(
    save_dir: &Path,
    backup_root: &Path,
    game_number: u32,
    index: &mut BackupIndex,
    limit: usize,
) -> Result<Option<PathBuf>, String> {
    let paths = build_save_paths(save_dir, game_number);
    if !paths.main_path.exists() {
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

    // 1. Optimistic duplicate check (Index only)
    if is_duplicate_by_index(index, backup_root, game_number, &hash, calculated, &source) {
        return Ok(None);
    }

    // 2. Fetch full backup list (once) for fallback checks and limit enforcement
    let backups = get_backups(save_dir, true).unwrap_or_default();

    // 3. Fallback duplicate check (Content scan)
    if is_duplicate_by_content(index, game_number, &hash, &source, &backups) {
        return Ok(None);
    }

    // 4. Enforce limit
    if let Err(e) = enforce_backup_limit(game_number, limit, &backups) {
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

/// Backs up a specific game slot by directory and game number.
#[allow(dead_code)]
pub fn perform_backup_for_game(
    save_dir: &Path,
    game_number: u32,
    limit: usize,
) -> Result<Option<PathBuf>, String> {
    if !save_dir.exists() {
        return Err(format!("Save directory does not exist: {:?}", save_dir));
    }

    let mut store = BackupStore::new(save_dir)?;

    let result = perform_backup_for_game_internal(
        save_dir,
        &store.root,
        game_number,
        &mut store.index,
        limit,
    )?;

    store.save();

    Ok(result)
}
