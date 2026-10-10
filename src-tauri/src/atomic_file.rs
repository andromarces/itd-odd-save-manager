// ITD ODD Save Manager by andromarces

use serde::de::DeserializeOwned;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Counter that makes temporary file names unique within the process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a sibling path of `path` with `suffix` appended to the file name.
fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Returns a process-unique sibling temporary path for `path`.
fn unique_temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    sibling_with_suffix(path, &format!(".{}.{}.tmp", std::process::id(), n))
}

/// Creates a new sibling temporary file. `create_new` guarantees that this call owns the file.
fn create_temp(path: &Path) -> io::Result<(File, PathBuf)> {
    loop {
        let tmp_path = unique_temp_path(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)
        {
            Ok(file) => return Ok((file, tmp_path)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

fn timestamp() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%S%3f").to_string()
}

/// Copies `bytes` to a new sibling `<name>.corrupt-<stamp>[-n]`.
/// `create_new` with a retry on the next suffix guarantees that an existing file is never replaced.
fn preserve_copy(path: &Path, bytes: &[u8], stamp: &str) -> io::Result<PathBuf> {
    let mut n = 0u32;
    loop {
        let suffix = if n == 0 {
            format!(".corrupt-{stamp}")
        } else {
            format!(".corrupt-{stamp}-{n}")
        };
        let target = sibling_with_suffix(path, &suffix);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut dest) => {
                let written = dest.write_all(bytes).and_then(|()| dest.sync_all());
                if written.is_err() {
                    let _ = fs::remove_file(&target);
                }
                return written.map(|()| target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(e),
        }
    }
}

/// Writes `contents` to `path` so that readers see either the old or the new file, never a partial one.
///
/// If `path` already holds a file that does not parse as `T`, the bytes are first copied to
/// `<name>.corrupt-<timestamp>[-n]`. If that copy fails, or the file cannot be read, the save
/// returns an error and leaves `path` untouched.
///
/// Data then goes to a unique sibling `<name>.<pid>.<n>.tmp` file (same directory, so the rename
/// stays on one filesystem), is flushed to disk, then renamed over `path`. Only the temporary file
/// that this call created is removed on failure. The parent directory is not fsynced.
// known-limit: the target can change between the validity read and the rename
pub(crate) fn write_atomic<T: DeserializeOwned>(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_atomic_with_stamp::<T>(path, contents, &timestamp())
}

fn write_atomic_with_stamp<T: DeserializeOwned>(
    path: &Path,
    contents: &[u8],
    stamp: &str,
) -> io::Result<()> {
    match fs::read(path) {
        Ok(existing) => {
            if serde_json::from_slice::<T>(&existing).is_err() {
                let preserved = preserve_copy(path, &existing, stamp).inspect_err(|e| {
                    log::error!("Failed to preserve damaged file {:?}: {}", path, e)
                })?;
                log::error!("Damaged file {:?} preserved at {:?}", path, preserved);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
            log::error!("Failed to read {:?} before overwrite: {}", path, e);
            return Err(e);
        }
    }

    let (mut file, tmp_path) = create_temp(path)?;
    let result = file
        .write_all(contents)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&tmp_path, path)
        });
    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

/// Loads and parses a JSON file. Returns `None` if the file is missing or damaged.
///
/// A damaged file (parse error, invalid UTF-8, or a read error other than not found) is logged at
/// error level and left in place. The next `write_atomic` preserves it before replacing it.
pub(crate) fn load_json<T: DeserializeOwned>(path: &Path, label: &str) -> Option<T> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            log::info!("{} file not found at {:?}", label, path);
            return None;
        }
        Err(e) => {
            log::error!("Failed to read {} file {:?}: {}", label, path, e);
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(e) => {
            log::error!("Failed to parse {} file {:?}: {}", label, path, e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use tempfile::tempdir;

    fn dir_entries(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    /// Returns every entry in `dir` other than `path`.
    fn others(dir: &Path, path: &Path) -> Vec<PathBuf> {
        dir_entries(dir).into_iter().filter(|p| p != path).collect()
    }

    /// Verifies a write creates the file, replaces valid content, and leaves no sibling.
    #[test]
    fn test_write_atomic_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic::<Value>(&path, br#"{"a":1}"#).unwrap();
        write_atomic::<Value>(&path, br#"{"a":2}"#).unwrap();
        assert_eq!(load_json::<Value>(&path, "test").unwrap()["a"], 2);
        assert_eq!(dir_entries(dir.path()), vec![path]);
    }

    /// Verifies a missing file loads as None without creating anything.
    #[test]
    fn test_load_json_missing() {
        let dir = tempdir().unwrap();
        assert!(load_json::<Value>(&dir.path().join("data.json"), "test").is_none());
        assert!(dir_entries(dir.path()).is_empty());
    }

    /// Verifies a failed write leaves the target intact and creates no temporary file.
    #[test]
    fn test_write_atomic_failure_preserves_target() {
        let dir = tempdir().unwrap();
        // A directory at the target path cannot be read or replaced.
        let path = dir.path().join("data.json");
        fs::create_dir(&path).unwrap();
        assert!(write_atomic::<Value>(&path, b"{}").is_err());
        assert!(path.is_dir());
        assert_eq!(dir_entries(dir.path()), vec![path]);
    }

    /// Verifies temporary names never collide.
    #[test]
    fn test_temp_names_are_unique() {
        let path = Path::new("data.json");
        assert_ne!(unique_temp_path(path), unique_temp_path(path));

        let dir = tempdir().unwrap();
        let target = dir.path().join("data.json");
        let (_file, first) = create_temp(&target).unwrap();
        let (_file2, second) = create_temp(&target).unwrap();
        assert_ne!(first, second);
    }

    /// Verifies loading a corrupt file returns None and leaves the file untouched.
    #[test]
    fn test_load_json_corrupt_is_left_in_place() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"{trunc").unwrap();
        assert!(load_json::<Value>(&path, "test").is_none());
        assert_eq!(fs::read(&path).unwrap(), b"{trunc");
        assert_eq!(dir_entries(dir.path()), vec![path]);
    }

    /// Verifies a normal save after a corrupt load succeeds and leaves one preserved copy.
    #[test]
    fn test_save_after_corrupt_load_preserves_copy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"{trunc").unwrap();
        assert!(load_json::<Value>(&path, "test").is_none());

        write_atomic::<Value>(&path, b"{}").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{}");
        let preserved = others(dir.path(), &path);
        assert_eq!(preserved.len(), 1);
        assert_eq!(fs::read(&preserved[0]).unwrap(), b"{trunc");
    }

    /// Verifies invalid UTF-8 is preserved byte for byte before the save replaces it.
    #[test]
    fn test_invalid_utf8_is_preserved() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        assert!(load_json::<Value>(&path, "test").is_none());

        write_atomic::<Value>(&path, b"{}").unwrap();
        let preserved = others(dir.path(), &path);
        assert_eq!(preserved.len(), 1);
        assert_eq!(fs::read(&preserved[0]).unwrap(), [0xff, 0xfe, 0xfd]);
    }

    /// Verifies a destination collision keeps both copies.
    #[test]
    fn test_preserve_collision_keeps_both_copies() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        let first = preserve_copy(&path, b"first", "stamp").unwrap();
        let second = preserve_copy(&path, b"second", "stamp").unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&second).unwrap(), b"second");
    }

    /// Verifies a failed preservation returns an error and leaves the original bytes.
    #[test]
    fn test_failed_preservation_leaves_original() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"{trunc").unwrap();
        // A stamp with a path separator points into a missing directory, so the copy fails.
        let result = write_atomic_with_stamp::<Value>(&path, b"{}", "no/such/dir");
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{trunc");
        assert_eq!(dir_entries(dir.path()), vec![path]);
    }
}
