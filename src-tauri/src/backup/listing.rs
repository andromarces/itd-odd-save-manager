use super::common::{HASH_FILE_NAME, LOCKED_FILE_NAME};
use super::data::BackupInfo;
use super::index::BackupStore;
use crate::filename_utils;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[cfg(test)]
thread_local! {
    /// Games whose backup folders fail the metadata read, simulating a per-folder I/O error.
    pub(crate) static FAILING_GAMES: std::cell::RefCell<HashSet<u32>> =
        std::cell::RefCell::new(HashSet::new());
}

/// Lists all backups available in the .backups directory.
pub fn get_backups(
    save_dir: &Path,
    include_hash: bool,
    game_filter: Option<u32>,
) -> Result<Vec<BackupInfo>, String> {
    let games = game_filter.map(|game| HashSet::from([game]));
    list_backups(save_dir, include_hash, games.as_ref())
}

/// Lists backups of the given games with a single enumeration of the .backups directory.
///
/// Each entry still gets a directory check. Folders of other games are then rejected before the
/// per-folder reads: main-save existence check, metadata read, locked-marker check, and hash read.
pub fn get_backups_for_games(
    save_dir: &Path,
    include_hash: bool,
    games: &HashSet<u32>,
) -> Result<Vec<BackupInfo>, String> {
    list_backups(save_dir, include_hash, Some(games))
}

/// Shared listing; `None` lists every game.
fn list_backups(
    save_dir: &Path,
    include_hash: bool,
    game_filter: Option<&HashSet<u32>>,
) -> Result<Vec<BackupInfo>, String> {
    let store = match BackupStore::load_if_exists(save_dir)? {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let mut backups = Vec::new();

    for entry in fs::read_dir(&store.root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            if let Some(mut info) =
                backup_info_from_folder(&path, &folder_name, save_dir, include_hash, game_filter)?
            {
                if let Some(note) = store.index.notes.get(&info.filename) {
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

/// Builds a BackupInfo from a backup folder if it matches the naming contract.
pub(crate) fn backup_info_from_folder(
    path: &Path,
    folder_name: &str,
    save_dir: &Path,
    include_hash: bool,
    game_filter: Option<&HashSet<u32>>,
) -> Result<Option<BackupInfo>, String> {
    let Some(info) = filename_utils::parse_backup_folder_name(folder_name) else {
        return Ok(None);
    };

    if let Some(filter) = game_filter {
        if !filter.contains(&info.game_number) {
            return Ok(None);
        }
    }

    let main_filename = format!("gamesave_{}.sav", info.game_number);
    let main_file_path = path.join(&main_filename);
    if !main_file_path.exists() {
        log::warn!(
            "Skipping backup folder {:?} because main save is missing.",
            path
        );
        return Ok(None);
    }

    #[cfg(test)]
    if FAILING_GAMES.with(|games| games.borrow().contains(&info.game_number)) {
        return Err(format!("injected metadata failure for {:?}", path));
    }

    let size = fs::metadata(&main_file_path)
        .map_err(|e| e.to_string())?
        .len();

    let locked = path.join(LOCKED_FILE_NAME).exists();
    let hash = if include_hash {
        fs::read_to_string(path.join(HASH_FILE_NAME))
            .map(|h| h.trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    Ok(Some(BackupInfo {
        path: path.to_string_lossy().to_string(),
        filename: folder_name.to_string(),
        original_filename: main_filename.clone(),
        original_path: save_dir.join(&main_filename).to_string_lossy().to_string(),
        size,
        modified: info.timestamp.to_rfc3339(),
        game_number: info.game_number,
        locked,
        hash,
        note: None,
    }))
}
