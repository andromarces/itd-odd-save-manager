// ITD ODD Save Manager by andromarces

use serde::de::DeserializeOwned;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Counter that makes temporary file names unique within the process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Paths of damaged files that could be neither moved nor copied aside.
/// `write_atomic` refuses to replace them, so the only remaining copy is never lost.
static UNPRESERVED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

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

fn is_unpreserved(path: &Path) -> bool {
    UNPRESERVED
        .lock()
        .map(|list| list.iter().any(|p| p == path))
        .unwrap_or(false)
}

fn set_unpreserved(path: &Path, damaged: bool) {
    if let Ok(mut list) = UNPRESERVED.lock() {
        list.retain(|p| p != path);
        if damaged {
            list.push(path.to_path_buf());
        }
    }
}

/// Writes `contents` to `path` so that readers see either the old or the new file, never a partial one.
///
/// Data goes to a unique sibling `<name>.<pid>.<n>.tmp` file (same directory, so the rename stays on
/// one filesystem), is flushed to disk, then renamed over `path`. Only the temporary file that this
/// call created is removed on failure. The parent directory is not fsynced.
/// Fails without writing if `path` holds a damaged file that could not be preserved.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    if is_unpreserved(path) {
        return Err(io::Error::other(
            "refusing to overwrite a damaged file that could not be preserved",
        ));
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

/// Returns a sibling path `<name>.corrupt-<timestamp>[-n]` that does not exist yet.
// known-limit: the existence check is not atomic against another process preserving the same file
fn unique_corrupt_path(path: &Path) -> PathBuf {
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f");
    let mut n = 0u32;
    loop {
        let suffix = if n == 0 {
            format!(".corrupt-{stamp}")
        } else {
            format!(".corrupt-{stamp}-{n}")
        };
        let candidate = sibling_with_suffix(path, &suffix);
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// Renames a damaged file to a unique sibling `<name>.corrupt-<timestamp>`. Never replaces a file.
pub(crate) fn quarantine_corrupt(path: &Path) -> io::Result<PathBuf> {
    let target = unique_corrupt_path(path);
    fs::rename(path, &target)?;
    Ok(target)
}

/// Copies a damaged file to a unique sibling `<name>.corrupt-<timestamp>`, leaving the original in
/// place. Never replaces a file.
pub(crate) fn copy_corrupt(path: &Path) -> io::Result<PathBuf> {
    let mut source = File::open(path)?;
    loop {
        let target = unique_corrupt_path(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut dest) => {
                let copied = io::copy(&mut source, &mut dest).and_then(|_| dest.sync_all());
                if copied.is_err() {
                    let _ = fs::remove_file(&target);
                }
                return copied.map(|()| target);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Preserves a damaged file: tries `rename_step`, then `copy_step`. If both fail, the path is
/// marked so `write_atomic` refuses to overwrite it. Logs every outcome at error level.
fn preserve_with(
    path: &Path,
    rename_step: impl Fn(&Path) -> io::Result<PathBuf>,
    copy_step: impl Fn(&Path) -> io::Result<PathBuf>,
) {
    match rename_step(path) {
        Ok(moved) => log::error!("Damaged file {:?} preserved at {:?}", path, moved),
        Err(re) => {
            log::error!("Failed to move damaged file {:?} aside: {}", path, re);
            match copy_step(path) {
                Ok(copy) => log::error!("Damaged file {:?} copied to {:?}", path, copy),
                Err(ce) => {
                    log::error!(
                        "Failed to preserve damaged file {:?}: {}. Overwrites are blocked until it parses",
                        path,
                        ce
                    );
                    set_unpreserved(path, true);
                }
            }
        }
    }
}

/// Loads and parses a JSON file. Returns `None` if the file is missing or damaged.
///
/// A damaged file (any read error other than not found, invalid UTF-8, or a parse error) is logged
/// at error level and preserved, so a later save cannot silently discard it.
pub(crate) fn load_json<T: DeserializeOwned>(path: &Path, label: &str) -> Option<T> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            log::info!("{} file not found at {:?}", label, path);
            return None;
        }
        Err(e) => {
            log::error!("Failed to read {} file {:?}: {}", label, path, e);
            preserve_with(path, quarantine_corrupt, copy_corrupt);
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => {
            set_unpreserved(path, false);
            Some(value)
        }
        Err(e) => {
            log::error!("Failed to parse {} file {:?}: {}", label, path, e);
            preserve_with(path, quarantine_corrupt, copy_corrupt);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn dir_entries(dir: &Path) -> Vec<PathBuf> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    /// Verifies a write creates the file and leaves no temporary sibling.
    #[test]
    fn test_write_atomic_creates_and_replaces() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(dir_entries(dir.path()), vec![path]);
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
        assert_eq!(dir_entries(dir.path()), vec![path]);
    }

    /// Verifies temporary names never collide and a failed write spares other temporary files.
    #[test]
    fn test_temp_names_are_unique() {
        let path = Path::new("data.json");
        assert_ne!(unique_temp_path(path), unique_temp_path(path));

        let dir = tempdir().unwrap();
        let target = dir.path().join("data.json");
        let (_file, other_tmp) = create_temp(&target).unwrap();
        let (_file2, own_tmp) = create_temp(&target).unwrap();
        assert_ne!(other_tmp, own_tmp);

        fs::create_dir(&target).unwrap();
        assert!(write_atomic(&target, b"x").is_err());
        assert!(other_tmp.exists() && own_tmp.exists());
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

    /// Verifies a colliding destination is skipped and the earlier preserved file is untouched.
    #[test]
    fn test_quarantine_destination_collision() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"second").unwrap();
        // Occupy the first candidate name.
        let first = unique_corrupt_path(&path);
        fs::write(&first, b"first").unwrap();

        let moved = quarantine_corrupt(&path).unwrap();
        assert_ne!(moved, first);
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&moved).unwrap(), b"second");
    }

    /// Verifies the copy fallback keeps the original and never replaces an existing file.
    #[test]
    fn test_copy_corrupt_keeps_original() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"damaged").unwrap();
        let first = copy_corrupt(&path).unwrap();
        let second = copy_corrupt(&path).unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        assert_eq!(fs::read(&first).unwrap(), b"damaged");
    }

    /// Verifies a failed move falls back to a copy.
    #[test]
    fn test_preserve_falls_back_to_copy() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"damaged").unwrap();
        preserve_with(
            &path,
            |_| Err(io::Error::other("rename blocked")),
            copy_corrupt,
        );
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        assert_eq!(dir_entries(dir.path()).len(), 2);
        // The copy exists, so a save may replace the original.
        write_atomic(&path, b"new").unwrap();
    }

    /// Verifies that when move and copy both fail, the original survives and saves are refused.
    #[test]
    fn test_failed_quarantine_does_not_lose_original() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, b"damaged").unwrap();
        preserve_with(
            &path,
            |_| Err(io::Error::other("rename blocked")),
            |_| Err(io::Error::other("copy blocked")),
        );
        assert!(write_atomic(&path, b"new").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"damaged");
        assert_eq!(dir_entries(dir.path()), vec![path.clone()]);

        // A later valid load lifts the block.
        fs::write(&path, b"{}").unwrap();
        assert!(load_json::<serde_json::Value>(&path, "test").is_some());
        write_atomic(&path, b"new").unwrap();
    }

    /// Verifies invalid UTF-8 is treated as damage and preserved.
    #[test]
    fn test_load_json_invalid_utf8_is_preserved() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("data.json");
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        assert!(load_json::<serde_json::Value>(&path, "test").is_none());
        assert!(!path.exists());
        let preserved = dir_entries(dir.path());
        assert_eq!(preserved.len(), 1);
        assert_eq!(fs::read(&preserved[0]).unwrap(), [0xff, 0xfe, 0xfd]);
    }
}
