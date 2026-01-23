// ITD ODD Save Manager by andromarces

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d_%H-%M-%S_%3f";
// Length of "YYYY-MM-DD_HH-MM-SS_MS" is 23 chars.
// Plus the preceding underscore separator is 24 chars.
const TIMESTAMP_SUFFIX_LEN: usize = 24;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupInfo {
    pub path: String,
    pub filename: String,
    pub original_filename: String,
    pub original_path: String,
    pub size: u64,
    pub modified: String, // ISO 8601
}

/// Copies the save file to a timestamped backup in a .backups subdirectory.
///
/// # Arguments
///
/// * `source_path` - The path to the source file to backup.
///
/// # Returns
///
/// * `Result<PathBuf, String>` - The path to the created backup file on success, or an error message.
pub fn perform_backup(source_path: &Path) -> Result<PathBuf, String> {
    // Check if source exists
    if !source_path.exists() {
        return Err(format!("Source file does not exist: {:?}", source_path));
    }

    // Determine directories
    let parent_dir = source_path
        .parent()
        .ok_or("Could not get parent directory")?;
    let backup_dir = parent_dir.join(".backups");

    // Create backup dir if needed
    if !backup_dir.exists() {
        fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    }

    // Generate filename
    let file_stem = source_path
        .file_stem()
        .ok_or("Invalid filename")?
        .to_string_lossy();
    let extension = source_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy();
    let timestamp = Local::now().format(TIMESTAMP_FORMAT);

    let backup_filename = if extension.is_empty() {
        format!("{}_{}", file_stem, timestamp)
    } else {
        format!("{}_{}.{}", file_stem, timestamp, extension)
    };

    let backup_path = backup_dir.join(backup_filename);

    // Copy file
    fs::copy(source_path, &backup_path).map_err(|e| e.to_string())?;

    log::info!("Backup successful: {:?}", backup_path);

    Ok(backup_path)
}

/// Keeps only the N most recent backups in the directory.
/// Returns the number of files deleted.
///
/// # Arguments
///
/// * `backup_dir` - The directory containing backup files.
/// * `retention_count` - The number of most recent backups to keep.
pub fn prune_backups(backup_dir: &Path, retention_count: usize) -> Result<usize, String> {
    if !backup_dir.exists() {
        return Ok(0);
    }

    let mut entries = fs::read_dir(backup_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .collect::<Vec<_>>();

    // Sort by modification time, newest first
    entries.sort_by(|a, b| {
        let meta_a = a
            .metadata()
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let meta_b = b
            .metadata()
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        // Reverse order (newest first)
        meta_b.cmp(&meta_a)
    });

    let mut deleted_count = 0;
    if entries.len() > retention_count {
        for entry in entries.iter().skip(retention_count) {
            if fs::remove_file(entry.path()).is_ok() {
                deleted_count += 1;
            }
        }
    }

    if deleted_count > 0 {
        log::info!("Pruned {} old backups", deleted_count);
    }

    Ok(deleted_count)
}

/// Lists all backups available for a given configuration path.
///
/// # Arguments
///
/// * `config_path` - The configured save path (file or directory).
///
/// # Returns
///
/// * `Result<Vec<BackupInfo>, String>` - List of available backups.
pub fn get_backups(config_path: &Path) -> Result<Vec<BackupInfo>, String> {
    // Determine if we should treat the config_path as a file or a directory.
    // If the path exists, we use the filesystem metadata.
    // If it doesn't exist (e.g., save file deleted), we infer from the extension.
    let is_file_mode = if config_path.exists() {
        config_path.is_file()
    } else {
        // Only treat as file if it has a specific extension known for save files (e.g. .sav)
        // to avoid treating directories with dots (e.g. "My.Saves") as files.
        config_path
            .extension()
            .map(|ext| ext.to_string_lossy().eq_ignore_ascii_case("sav"))
            .unwrap_or(false)
    };

    let (backup_dir, target_filename) = if is_file_mode {
        (
            config_path
                .parent()
                .unwrap_or(Path::new("."))
                .join(".backups"),
            Some(
                config_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            ),
        )
    } else {
        (config_path.join(".backups"), None)
    };

    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    let entries = fs::read_dir(&backup_dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            let original_filename = derive_original_filename(&path);

            // If filtering by specific file, ensure original filename matches exact target
            if let Some(ref target) = target_filename {
                if original_filename != *target {
                    continue;
                }
            }

            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            let modified: DateTime<Local> =
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH).into();

            // Calculate original path
            let original_path = if is_file_mode {
                config_path.to_path_buf()
            } else {
                config_path.join(&original_filename)
            };

            backups.push(BackupInfo {
                path: path.to_string_lossy().into_owned(),
                filename,
                original_filename,
                original_path: original_path.to_string_lossy().into_owned(),
                size: metadata.len(),
                modified: modified.to_rfc3339(),
            });
        }
    }

    // Sort by modified desc
    backups.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(backups)
}

/// Derives the original filename from a backup filename by stripping the timestamp suffix.
///
/// Expected backup format: `{original_stem}_{timestamp}.{ext}`
/// where timestamp is `%Y-%m-%d_%H-%M-%S_%3f` (23 chars).
///
/// If the filename is shorter than expected or doesn't follow the pattern, returns the backup filename itself.
fn derive_original_filename(backup_path: &Path) -> String {
    let stem = backup_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let extension = backup_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy();

    let stem_str = stem.as_ref();
    if stem_str.len() > TIMESTAMP_SUFFIX_LEN {
        let original_stem = &stem_str[..stem_str.len() - TIMESTAMP_SUFFIX_LEN];
        if extension.is_empty() {
            original_stem.to_string()
        } else {
            format!("{}.{}", original_stem, extension)
        }
    } else {
        // Fallback if format doesn't match expected length
        if extension.is_empty() {
            stem.to_string()
        } else {
            format!("{}.{}", stem, extension)
        }
    }
}

/// Restores a backup file to the target location.
///
/// # Arguments
///
/// * `backup_path` - The path to the backup file.
/// * `target_path` - The path where the file should be restored to.
pub fn restore_backup(backup_path: &Path, target_path: &Path) -> Result<(), String> {
    if !backup_path.exists() {
        return Err("Backup file does not exist".to_string());
    }

    // Ensure target parent exists
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Copy (overwrite)
    fs::copy(backup_path, target_path).map_err(|e| e.to_string())?;
    log::info!("Restored {:?} to {:?}", backup_path, target_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    /// Tests that a backup is correctly created in the .backups subdirectory with a timestamped name.
    #[test]
    fn test_perform_backup() {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("savegame.sav");
        let mut file = File::create(&source_path).unwrap();
        writeln!(file, "dummy content").unwrap();

        let backup_path = perform_backup(&source_path).unwrap();

        assert!(backup_path.exists());
        assert_eq!(
            backup_path.parent().unwrap().file_name().unwrap(),
            ".backups"
        );
        assert!(backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("savegame_"));
    }

    /// Tests that the pruning logic correctly removes older backups, keeping only the specified retention count.
    #[test]
    fn test_prune_backups() {
        let dir = tempdir().unwrap();
        let backup_dir = dir.path().join(".backups");
        fs::create_dir_all(&backup_dir).unwrap();

        // Create 5 dummy backup files with different timestamps (simulated by sleep)
        for i in 0..5 {
            let p = backup_dir.join(format!("backup_{}.sav", i));
            File::create(&p).unwrap();
            thread::sleep(Duration::from_millis(100));
        }

        // We have 5 files. Keep 2.
        let deleted = prune_backups(&backup_dir, 2).unwrap();
        assert_eq!(deleted, 3);

        let remaining = fs::read_dir(&backup_dir).unwrap().count();
        assert_eq!(remaining, 2);
    }

    #[test]
    fn test_get_and_restore_backup() {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("save.sav");
        {
            let mut f = File::create(&source_path).unwrap();
            writeln!(f, "original").unwrap();
        }

        // Make a backup
        let backup_path = perform_backup(&source_path).unwrap();

        // List backups
        let backups = get_backups(dir.path()).unwrap(); // using dir path
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].path, backup_path.to_string_lossy());

        // Modify source
        {
            let mut f = File::create(&source_path).unwrap();
            writeln!(f, "modified").unwrap();
        }

        // Restore
        restore_backup(&PathBuf::from(&backups[0].path), &source_path).unwrap();

        // Check content
        let content = fs::read_to_string(&source_path).unwrap();
        assert_eq!(content.trim(), "original");
    }

    #[test]
    fn test_derive_original_filename() {
        // Mock path logic using PathBuf
        let p = PathBuf::from("savegame_2026-01-23_14-05-01_123.sav");
        assert_eq!(derive_original_filename(&p), "savegame.sav");

        let p2 = PathBuf::from("my_save_2026-01-23_14-05-01_123.sav");
        assert_eq!(derive_original_filename(&p2), "my_save.sav");

        let p3 = PathBuf::from("short.sav");
        assert_eq!(derive_original_filename(&p3), "short.sav");
    }

    #[test]
    fn test_get_backups_filtering() {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("save.sav");
        let unrelated_path = dir.path().join("save_other.sav"); // Prefix collision candidate

        {
            File::create(&source_path).unwrap();
            File::create(&unrelated_path).unwrap();
        }

        let backup1 = perform_backup(&source_path).unwrap();
        let backup2 = perform_backup(&unrelated_path).unwrap();

        // Get backups strictly for "save.sav"
        let backups = get_backups(&source_path).unwrap();
        assert_eq!(backups.len(), 1);
        assert_eq!(backups[0].path, backup1.to_string_lossy());

        // Get backups strictly for "save_other.sav"
        let backups_other = get_backups(&unrelated_path).unwrap();
        assert_eq!(backups_other.len(), 1);
        assert_eq!(backups_other[0].path, backup2.to_string_lossy());
    }

    #[test]
    fn test_get_backups_missing_file_heuristic() {
        let dir = tempdir().unwrap();

        // Case 1: Missing file with .sav extension -> Should be treated as file
        // Expected behavior: Look in parent/.backups
        let missing_file = dir.path().join("missing_save.sav");
        let backup_dir_for_file = dir.path().join(".backups");
        fs::create_dir_all(&backup_dir_for_file).unwrap();

        // Create a dummy backup that matches the missing file pattern
        let timestamp = Local::now().format(TIMESTAMP_FORMAT);
        let backup_name = format!("missing_save_{}.sav", timestamp);
        let backup_path = backup_dir_for_file.join(&backup_name);
        File::create(&backup_path).unwrap();

        let backups = get_backups(&missing_file).unwrap();
        assert_eq!(backups.len(), 1, "Should find backup for missing .sav file");
        assert_eq!(backups[0].filename, backup_name);

        // Case 2: Missing path without .sav extension -> Should be treated as directory
        // Expected behavior: Look in path/.backups
        let missing_dir = dir.path().join("missing_dir");
        let backup_dir_for_dir = missing_dir.join(".backups");
        fs::create_dir_all(&backup_dir_for_dir).unwrap(); // create the backup dir structure

        // Create a dummy backup inside this directory structure
        let dir_backup_name = format!("save_{}.sav", timestamp);
        let dir_backup_path = backup_dir_for_dir.join(&dir_backup_name);
        File::create(&dir_backup_path).unwrap();

        let backups_dir = get_backups(&missing_dir).unwrap();
        assert_eq!(
            backups_dir.len(),
            1,
            "Should find backup for directory mode"
        );
        assert_eq!(backups_dir[0].filename, dir_backup_name);
    }
}
