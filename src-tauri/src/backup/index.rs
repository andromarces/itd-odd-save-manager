use super::common::{BACKUP_DIR_NAME, INDEX_FILE_NAME};
use crate::atomic_file::{load_json, write_atomic};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub(crate) struct BackupIndex {
    pub(crate) games: HashMap<u32, IndexEntry>,
    #[serde(default)]
    pub(crate) notes: HashMap<String, String>,
}

impl BackupIndex {
    /// Removes all index entries associated with a deleted backup folder.
    pub(crate) fn prune_deleted(&mut self, folder_name: &str) {
        self.notes.remove(folder_name);
        self.games
            .retain(|_, entry| entry.last_backup_path != folder_name);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct IndexEntry {
    pub(crate) last_hash: String,
    pub(crate) last_source_size: u64,
    pub(crate) last_source_modified: u128, // Unix timestamp in nanoseconds
    pub(crate) last_backup_path: String,   // Relative folder name of the last backup
}

/// Ensures the backup root directory exists and returns its path.
pub(crate) fn ensure_backup_root(save_dir: &Path) -> Result<PathBuf, String> {
    let backup_root = save_dir.join(BACKUP_DIR_NAME);
    if !backup_root.exists() {
        fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;
    }
    Ok(backup_root)
}

/// Wrapper for backup index operations.
pub(crate) struct BackupStore {
    pub(crate) root: PathBuf,
    pub(crate) index: BackupIndex,
}

impl BackupStore {
    /// Initializes the backup store, creating the backup directory if it does not exist.
    pub(crate) fn new(save_dir: &Path) -> Result<Self, String> {
        let root = ensure_backup_root(save_dir)?;
        let index = load_index(&root);
        Ok(Self { root, index })
    }

    /// Loads the backup store if the backup directory exists.
    /// Does NOT create the directory if it's missing.
    pub(crate) fn load_if_exists(save_dir: &Path) -> Result<Option<Self>, String> {
        let root = save_dir.join(BACKUP_DIR_NAME);
        if !root.exists() {
            return Ok(None);
        }
        let index = load_index(&root);
        Ok(Some(Self { root, index }))
    }

    /// Saves the current index to the backup directory.
    pub(crate) fn save(&self) -> Result<(), String> {
        save_index(&self.root, &self.index)
    }
}

/// Loads the backup index from the given backup root directory.
/// Returns a default index if the file is missing or damaged. A damaged file is logged at error
/// level and preserved (see `load_json`).
pub(crate) fn load_index(backup_root: &Path) -> BackupIndex {
    load_json(&backup_root.join(INDEX_FILE_NAME), "Backup index").unwrap_or_default()
}

/// Saves the given index to the backup root directory using an atomic write.
pub(crate) fn save_index(backup_root: &Path, index: &BackupIndex) -> Result<(), String> {
    let index_path = backup_root.join(INDEX_FILE_NAME);
    let content = serde_json::to_string(index).map_err(|e| e.to_string())?;
    write_atomic(&index_path, content.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Verifies a saved index loads back unchanged.
    #[test]
    fn test_index_round_trip() {
        let dir = tempdir().unwrap();
        let mut index = BackupIndex::default();
        index.notes.insert("folder".to_string(), "note".to_string());
        save_index(dir.path(), &index).unwrap();

        let loaded = load_index(dir.path());
        assert_eq!(loaded.notes.get("folder").map(String::as_str), Some("note"));
        assert!(!dir.path().join("index.json.tmp").exists());
    }

    /// Verifies a missing index loads as empty and creates no quarantine file.
    #[test]
    fn test_load_index_missing() {
        let dir = tempdir().unwrap();
        let loaded = load_index(dir.path());
        assert!(loaded.games.is_empty() && loaded.notes.is_empty());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    /// Verifies a corrupt index is moved aside with its contents intact.
    #[test]
    fn test_load_index_corrupt_is_preserved() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(INDEX_FILE_NAME), b"{\"games\": {").unwrap();

        let loaded = load_index(dir.path());
        assert!(loaded.games.is_empty() && loaded.notes.is_empty());
        assert!(!dir.path().join(INDEX_FILE_NAME).exists());

        let preserved: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(preserved.len(), 1);
        assert_eq!(fs::read(&preserved[0]).unwrap(), b"{\"games\": {");
    }
}
