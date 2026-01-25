use super::common::{BACKUP_DIR_NAME, INDEX_FILE_NAME};
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
    pub(crate) fn save(&self) {
        save_index(&self.root, &self.index);
    }
}

/// Loads the backup index from the given backup root directory.
/// Returns a default index if the file is missing or invalid.
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

/// Saves the given index to the backup root directory.
pub(crate) fn save_index(backup_root: &Path, index: &BackupIndex) {
    let index_path = backup_root.join(INDEX_FILE_NAME);
    if let Ok(content) = serde_json::to_string(index) {
        let _ = fs::write(index_path, content);
    }
}
