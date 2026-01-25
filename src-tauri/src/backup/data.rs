use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// Conventional save file paths for a specific game slot.
#[derive(Debug, Clone)]
pub(crate) struct SavePaths {
    pub(crate) main_filename: String,
    pub(crate) main_path: PathBuf,
    pub(crate) bak_filename: String,
    pub(crate) bak_path: PathBuf,
}

/// Builds the conventional save file paths for a game slot.
pub(crate) fn build_save_paths(save_dir: &Path, game_number: u32) -> SavePaths {
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
pub(crate) struct SourceMetadata {
    pub(crate) size: u64,
    pub(crate) modified_nanos: u128,
    pub(crate) modified_dt: DateTime<Local>,
}

/// Reads metadata needed for deduplication and folder naming.
pub(crate) fn read_source_metadata(main_path: &Path) -> Result<SourceMetadata, String> {
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
