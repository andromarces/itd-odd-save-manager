use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

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
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S_%3f");

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
        // Note: fs mtime granularity can be low, so we need sleeps or just trust OS handles it if we create them fast?
        // Better to explicitly set mtime if possible, but that's complex in std.
        // We'll just rely on creating them. If they have same timestamp, sort is unstable but that's fine for prune count.

        for i in 0..5 {
            let p = backup_dir.join(format!("backup_{}.sav", i));
            File::create(&p).unwrap();
            // Sleep a bit to ensure mtime diff
            thread::sleep(Duration::from_millis(100));
        }

        // We have 5 files. Keep 2.
        let deleted = prune_backups(&backup_dir, 2).unwrap();
        assert_eq!(deleted, 3);

        let remaining = fs::read_dir(&backup_dir).unwrap().count();
        assert_eq!(remaining, 2);
    }
}
