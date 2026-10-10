// ITD ODD Save Manager by andromarces

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Returns a sibling path of `path` with `suffix` appended to the file name.
fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Writes `contents` to `path` so that readers see either the old or the new file, never a partial one.
///
/// Data goes to a sibling `<name>.tmp` file (same directory, so the rename stays on one
/// filesystem), is flushed to disk, then renamed over `path`. The temporary file is removed
/// if any step fails. The parent directory is not fsynced.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp_path = sibling_with_suffix(path, ".tmp");
    let result = (|| {
        let mut file = File::create(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

/// Renames a damaged file to a sibling `<name>.corrupt-<timestamp>` so it is not overwritten.
///
/// Returns the new path, or the rename error. Callers log the outcome.
pub(crate) fn quarantine_corrupt(path: &Path) -> std::io::Result<PathBuf> {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f");
    let target = sibling_with_suffix(path, &format!(".corrupt-{stamp}"));
    fs::rename(path, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Verifies a write creates the file and leaves no temporary sibling.
    #[test]
    fn test_write_atomic_creates_and_replaces() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert!(!dir.path().join("data.json.tmp").exists());
    }

    /// Verifies a failed write leaves the target intact and removes the temporary file.
    #[test]
    fn test_write_atomic_failure_preserves_target() {
        let dir = tempdir().unwrap();
        // A directory at the target path makes the final rename fail.
        let path = dir.path().join("data.json");
        fs::create_dir(&path).unwrap();
        assert!(write_atomic(&path, b"x").is_err());
        assert!(path.is_dir());
        assert!(!dir.path().join("data.json.tmp").exists());
    }

    /// Verifies quarantine moves the file to a sibling and keeps its contents.
    #[test]
    fn test_quarantine_corrupt_preserves_contents() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"{trunc").unwrap();
        let moved = quarantine_corrupt(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(moved.parent(), path.parent());
        assert_eq!(fs::read(&moved).unwrap(), b"{trunc");
    }
}
