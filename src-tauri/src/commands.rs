use crate::backup::common::BACKUP_DIR_NAME;
use crate::backup::index::BackupStore;
use crate::backup::{self, BackupInfo};
use crate::config::ConfigState;
use crate::watcher::FileWatcher;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{async_runtime, Emitter, Manager, State};

/// Extracts the configured save path without holding the mutex across blocking work.
fn extract_save_path(state: &State<'_, ConfigState>) -> Result<Option<PathBuf>, String> {
    let save_path = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?
        .save_path
        .clone();
    Ok(save_path.map(PathBuf::from))
}

/// Runs blocking work on the blocking thread pool and surfaces join errors.
async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| format!("Blocking task join error: {}", e))?
}

/// Verifies that a backup path is a canonical direct child of the `.backups` root.
///
/// Rejects the root itself, nested descendants, paths outside the root (including symlink
/// escapes, because the path is canonicalized first), and folder names that do not match
/// the backup folder naming contract.
fn verify_backup_path(save_path: &Path, backup_path: &Path) -> Result<PathBuf, String> {
    let backup_root = save_path.join(BACKUP_DIR_NAME);
    let canonical_target = backup_path
        .canonicalize()
        .map_err(|_| "Invalid backup path".to_string())?;
    let canonical_root = backup_root
        .canonicalize()
        .map_err(|_| "Backup directory not found".to_string())?;

    if canonical_target.parent() != Some(canonical_root.as_path()) {
        return Err(
            "Security violation: Path is not a backup folder in the backup directory".to_string(),
        );
    }

    let folder_name = canonical_target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid backup folder name".to_string())?;
    if crate::filename_utils::parse_backup_folder_name(folder_name).is_none() {
        return Err("Invalid backup folder name".to_string());
    }

    Ok(canonical_target)
}

/// Tauri command to list available backups for the configured save path.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_backups_command(state: State<'_, ConfigState>) -> Result<Vec<BackupInfo>, String> {
    if let Some(path) = extract_save_path(&state)? {
        run_blocking(move || backup::get_backups(&path, false, None)).await
    } else {
        Ok(Vec::new())
    }
}

/// Tauri command to restore a specific backup to the configured save directory.
#[tauri::command(rename_all = "snake_case")]
pub async fn restore_backup_command(
    state: State<'_, ConfigState>,
    backup_path: String,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let backup = verify_backup_path(&save_path, Path::new(&backup_path))?;
    // Canonical form keeps the `.backups` containment check in the restore index update consistent.
    let target_dir = save_path
        .canonicalize()
        .map_err(|_| "Invalid save path".to_string())?;

    run_blocking(move || backup::restore_backup(&backup, &target_dir)).await
}

/// Tauri command to toggle the lock status of a backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_backup_lock_command(
    state: State<'_, ConfigState>,
    backup_path: String,
    locked: bool,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let path = PathBuf::from(&backup_path);

    let verified_path = verify_backup_path(&save_path, &path)?;

    run_blocking(move || backup::set_backup_lock(&verified_path, locked)).await
}

/// Tauri command to set or update a note for a backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_backup_note_command(
    state: State<'_, ConfigState>,
    backup_filename: String,
    note: Option<String>,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;

    run_blocking(move || backup::set_backup_note(&save_path, &backup_filename, note)).await
}

/// Tauri command to delete a specific backup.
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_backup_command(
    state: State<'_, ConfigState>,
    backup_path: String,
) -> Result<(), String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;
    let path = PathBuf::from(&backup_path);

    let verified_path = verify_backup_path(&save_path, &path)?;

    run_blocking(move || {
        let folder_name = verified_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        backup::delete_backup_folder(&verified_path)?;

        if let Some(mut store) = BackupStore::load_if_exists(&save_path)? {
            store.index.prune_deleted(&folder_name);
            store.save()?;
        }

        Ok(())
    })
    .await
}

/// Tauri command to batch delete backups.
#[tauri::command(rename_all = "snake_case")]
pub async fn batch_delete_backups_command(
    state: State<'_, ConfigState>,
    game_numbers: Vec<u32>,
    keep_latest: bool,
    delete_locked: bool,
) -> Result<usize, String> {
    let save_path =
        extract_save_path(&state)?.ok_or_else(|| "Save path not configured".to_string())?;

    run_blocking(move || {
        backup::delete_backups_batch(&save_path, &game_numbers, keep_latest, delete_locked)
    })
    .await
}

/// Command to initialize the watcher from the frontend.
#[tauri::command(rename_all = "snake_case")]
pub async fn init_watcher(
    app: tauri::AppHandle,
    state: State<'_, ConfigState>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        if !window.is_visible().unwrap_or(false) {
            return Err("Watcher initialization deferred: window not yet visible".to_string());
        }
    }

    let config = state
        .0
        .lock()
        .map_err(|e| format!("Failed to lock config: {}", e))?;
    if let Some(path_str) = &config.save_path {
        let path = PathBuf::from(path_str);
        if path.exists() {
            let watcher = app.state::<FileWatcher>();
            let app_handle = app.clone();
            let on_backup = Arc::new(move || {
                if let Err(e) = app_handle.emit("backups-updated", ()) {
                    log::error!("Failed to emit backups-updated event: {}", e);
                }
            });
            watcher.start(path, config.max_backups_per_game, Some(on_backup))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Verifies that blocking work is dispatched to a different thread than the caller.
    #[test]
    fn run_blocking_executes_on_different_thread() {
        let caller_thread = thread::current().id();
        let worker_thread = tauri::async_runtime::block_on(async {
            run_blocking(|| Ok(thread::current().id())).await
        })
        .expect("Blocking task should complete successfully");

        assert_ne!(
            caller_thread, worker_thread,
            "Blocking work should not execute on the caller thread"
        );
    }

    const VALID_FOLDER: &str = "Game 1 - 01-Jan-2024 10-00-00 AM";

    /// Creates a directory link: a symlink, or on Windows a junction when symlinks are denied.
    /// Returns false when the platform denies both. CI always sets `CI`, and a denied link there
    /// fails the test instead of skipping it.
    fn try_link_dir(original: &Path, link: &Path) -> bool {
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(original, link).is_ok();
        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_dir(original, link).is_ok()
            || std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(original)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        if !created {
            assert!(
                std::env::var_os("CI").is_none(),
                "directory link creation must work on CI"
            );
            eprintln!("SKIPPED: directory link creation denied on this machine; CI runs this test");
        }
        created
    }

    /// Verifies that the backup root, nested descendants, and malformed folder names are rejected.
    #[test]
    fn test_verify_backup_path_rejects_root_nested_and_malformed() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let nested = backup_root.join(VALID_FOLDER).join("child");
        let malformed = backup_root.join("not-a-backup");

        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::create_dir_all(&malformed).expect("failed to create malformed dir");

        assert!(verify_backup_path(&save_path, &backup_root).is_err());
        assert!(verify_backup_path(&save_path, &nested).is_err());
        assert!(verify_backup_path(&save_path, &malformed).is_err());
        assert!(verify_backup_path(&save_path, &save_path).is_err());
    }

    /// Verifies that a link inside the backup root that points outside it is rejected by the
    /// containment guard alone: the link target carries a valid backup folder name.
    #[test]
    fn test_verify_backup_path_rejects_symlink_escape() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let outside = temp_dir.path().join("outside").join(VALID_FOLDER);
        fs::create_dir_all(&backup_root).expect("failed to create backup root");
        fs::create_dir_all(&outside).expect("failed to create outside dir");

        let link = backup_root.join(VALID_FOLDER);
        if !try_link_dir(&outside, &link) {
            return;
        }

        let err = verify_backup_path(&save_path, &link).expect_err("escape must be rejected");
        assert!(
            err.contains("Security violation"),
            "unexpected error: {err}"
        );
    }

    /// Verifies that a linked `.backups` root is accepted by validation and restore alike, so the
    /// up-front guard and the index update agree.
    #[test]
    fn test_symlinked_backup_root_validates_and_restores() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let real_root = temp_dir.path().join("real_backups");
        let folder = real_root.join(VALID_FOLDER);
        fs::create_dir_all(&save_path).expect("failed to create save dir");
        fs::create_dir_all(&folder).expect("failed to create backup folder");
        fs::write(folder.join("gamesave_0.sav"), b"data").expect("failed to write backup save");

        if !try_link_dir(&real_root, &save_path.join(BACKUP_DIR_NAME)) {
            return;
        }

        let verified = verify_backup_path(
            &save_path,
            &save_path.join(BACKUP_DIR_NAME).join(VALID_FOLDER),
        )
        .expect("linked root must validate");
        let target = save_path
            .canonicalize()
            .expect("failed to canonicalize save dir");
        backup::restore_backup(&verified, &target).expect("restore must succeed");
        assert!(save_path.join("gamesave_0.sav").exists());
    }

    /// Verifies that restore rejects a source outside the backup root before copying any file.
    #[test]
    fn test_restore_rejects_outside_source_before_copy() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let outside = temp_dir.path().join("outside").join(VALID_FOLDER);
        fs::create_dir_all(save_path.join(BACKUP_DIR_NAME)).expect("failed to create backup root");
        fs::create_dir_all(&outside).expect("failed to create outside dir");
        fs::write(outside.join("gamesave_0.sav"), b"data").expect("failed to write save");

        assert!(backup::restore_backup(&outside, &save_path).is_err());
        assert!(!save_path.join("gamesave_0.sav").exists());
    }

    /// Verifies the security logic of the verify_backup_path helper.
    #[test]
    fn test_verify_backup_path_security() {
        use std::fs;
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("saves");
        let backup_root = save_path.join(BACKUP_DIR_NAME);
        let game_backup = backup_root.join(VALID_FOLDER);

        fs::create_dir_all(&game_backup).expect("failed to create mock backup dir");

        assert!(verify_backup_path(&save_path, &game_backup).is_ok());

        let malicious = temp_dir.path().join("malicious.exe");
        fs::File::create(&malicious).expect("failed to create mock malicious file");
        assert!(verify_backup_path(&save_path, &malicious).is_err());

        let other = save_path.join("unauthorized_file.txt");
        fs::File::create(&other).expect("failed to create mock unauthorized file");
        assert!(verify_backup_path(&save_path, &other).is_err());
    }

    /// Windows path forms that reach `verify_backup_path` from the frontend and the config.
    #[cfg(windows)]
    mod windows_path_forms {
        use super::*;
        use std::fs;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        const RESTORED_FILE: &str = "gamesave_0.sav";
        const ROOT_BYTES: &[u8] = b"root";
        const BACKUP_BYTES: &[u8] = b"backup";
        const OUTSIDE_BYTES: &[u8] = b"outside";

        struct Fixture {
            _temp_dir: tempfile::TempDir,
            save_path: PathBuf,
            backup_root: PathBuf,
            backup: PathBuf,
            outside: PathBuf,
        }

        fn fixture() -> Fixture {
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            let save_path = temp_dir.path().join("saves");
            let backup_root = save_path.join(BACKUP_DIR_NAME);
            let backup = backup_root.join(VALID_FOLDER);
            let outside = temp_dir.path().join("outside").join(VALID_FOLDER);
            fs::create_dir_all(&backup).expect("failed to create backup dir");
            fs::create_dir_all(&outside).expect("failed to create outside dir");
            // Distinct save files in every candidate folder make a bypassed restore guard
            // observable, and show which folder a restore copied from.
            for (dir, bytes) in [
                (&backup_root, ROOT_BYTES),
                (&backup, BACKUP_BYTES),
                (&outside, OUTSIDE_BYTES),
            ] {
                fs::write(dir.join(RESTORED_FILE), bytes).expect("failed to write save file");
            }
            Fixture {
                _temp_dir: temp_dir,
                save_path,
                backup_root,
                backup,
                outside,
            }
        }

        fn flip_drive_case(path: &Path) -> PathBuf {
            let text = path.to_str().expect("temp path must be UTF-8");
            let mut chars = text.chars();
            let drive = chars.next().expect("path must not be empty");
            let flipped = if drive.is_ascii_uppercase() {
                drive.to_ascii_lowercase()
            } else {
                drive.to_ascii_uppercase()
            };
            PathBuf::from(format!("{flipped}{}", chars.as_str()))
        }

        fn forward_slashes(path: &Path) -> PathBuf {
            PathBuf::from(
                path.to_str()
                    .expect("temp path must be UTF-8")
                    .replace('\\', "/"),
            )
        }

        fn trailing_separator(path: &Path) -> PathBuf {
            let mut text = path.as_os_str().to_owned();
            text.push("\\");
            PathBuf::from(text)
        }

        fn verbatim(path: &Path) -> PathBuf {
            let canonical = path.canonicalize().expect("failed to canonicalize path");
            assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
            canonical
        }

        /// Returns the 8.3 form of an existing path, or `None` when the volume generates no
        /// short name for it. Panics when `GetShortPathNameW` fails, so an API error never reads
        /// as a missing alias.
        fn short_form(path: &Path) -> Option<PathBuf> {
            #[link(name = "kernel32")]
            extern "system" {
                fn GetShortPathNameW(long: *const u16, short: *mut u16, len: u32) -> u32;
            }
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let mut buf = vec![0u16; 1024];
            let written = loop {
                // SAFETY: `wide` is NUL-terminated and `buf` is writable for `buf.len()` units.
                let written =
                    unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) }
                        as usize;
                if written == 0 {
                    panic!(
                        "GetShortPathNameW failed for {path:?}: {}",
                        std::io::Error::last_os_error()
                    );
                }
                // A result above the buffer length is the required size, NUL included.
                if written <= buf.len() {
                    break written;
                }
                buf.resize(written, 0);
            };
            let short = PathBuf::from(std::ffi::OsString::from_wide(&buf[..written]));
            let unchanged = short
                .to_string_lossy()
                .eq_ignore_ascii_case(&path.to_string_lossy());
            (!unchanged).then_some(short)
        }

        type Form = fn(&Path) -> PathBuf;
        const FORMS: [(&str, Form); 4] = [
            ("drive-letter case", flip_drive_case),
            ("forward slashes", forward_slashes),
            ("trailing separator", trailing_separator),
            ("verbatim", verbatim),
        ];

        /// Verifies that a valid backup folder is accepted in every Windows path form, whether the
        /// form applies to the backup path, the configured save path, or both.
        #[test]
        fn test_verify_backup_path_accepts_windows_forms_of_valid_backup() {
            let fx = fixture();
            let expected = fx.backup.canonicalize().expect("failed to canonicalize");
            for (name, form) in FORMS {
                let cases = [
                    ("backup path", fx.save_path.clone(), form(&fx.backup)),
                    ("save path", form(&fx.save_path), fx.backup.clone()),
                    ("both", form(&fx.save_path), form(&fx.backup)),
                ];
                for (side, save_path, backup_path) in cases {
                    let verified = verify_backup_path(&save_path, &backup_path)
                        .unwrap_or_else(|e| panic!("{name} on {side} rejected: {e}"));
                    assert_eq!(verified, expected, "{name} on {side}");
                }
            }
        }

        /// Verifies that the backup root and a path outside it stay rejected in every Windows
        /// path form, so no form defeats the containment check.
        #[test]
        fn test_verify_backup_path_rejects_windows_forms_of_root_and_outside() {
            let fx = fixture();
            for (name, form) in FORMS {
                for (label, target) in [("root", &fx.backup_root), ("outside", &fx.outside)] {
                    let cases = [
                        (fx.save_path.clone(), form(target)),
                        (form(&fx.save_path), form(target)),
                    ];
                    for (save_path, backup_path) in cases {
                        assert!(
                            verify_backup_path(&save_path, &backup_path).is_err(),
                            "{name} form of {label} was accepted"
                        );
                    }
                }
            }
        }

        /// Asserts that restore succeeds and the destination holds the backup bytes.
        fn assert_restore_accepted(fx: &Fixture, save_path: &Path, backup_path: &Path, case: &str) {
            let target = fx.save_path.join(RESTORED_FILE);
            let _ = fs::remove_file(&target);
            let result = backup::restore_backup(backup_path, save_path);
            let contents = fs::read(&target);
            let _ = fs::remove_file(&target);
            result.unwrap_or_else(|e| panic!("{case}: restore failed: {e}"));
            assert_eq!(
                contents.unwrap_or_default(),
                BACKUP_BYTES,
                "{case}: destination does not hold the backup bytes"
            );
        }

        /// Asserts that restore returns an error and, independently, that no file was copied,
        /// so a guard that copies before failing is caught.
        fn assert_restore_rejected(fx: &Fixture, save_path: &Path, backup_path: &Path, case: &str) {
            let target = fx.save_path.join(RESTORED_FILE);
            let _ = fs::remove_file(&target);
            let result = backup::restore_backup(backup_path, save_path);
            let copied = target.exists();
            let _ = fs::remove_file(&target);
            assert!(result.is_err(), "{case}: restore returned success");
            assert!(!copied, "{case}: a file was copied before rejection");
        }

        /// Verifies that restore (through `validate_backup_source`) accepts a valid backup folder
        /// in every Windows path form.
        #[test]
        fn test_restore_accepts_windows_forms_of_valid_backup() {
            let fx = fixture();
            for (name, form) in FORMS {
                let cases = [
                    ("backup path", fx.save_path.clone(), form(&fx.backup)),
                    ("save path", form(&fx.save_path), fx.backup.clone()),
                    ("both", form(&fx.save_path), form(&fx.backup)),
                ];
                for (side, save_path, backup_path) in cases {
                    assert_restore_accepted(
                        &fx,
                        &save_path,
                        &backup_path,
                        &format!("{name} on {side}"),
                    );
                }
            }
        }

        /// Verifies that restore rejects the backup root and a path outside it in every Windows
        /// path form, and copies nothing.
        #[test]
        fn test_restore_rejects_windows_forms_of_root_and_outside() {
            let fx = fixture();
            for (name, form) in FORMS {
                for (label, target) in [("root", &fx.backup_root), ("outside", &fx.outside)] {
                    let cases = [
                        (fx.save_path.clone(), form(target)),
                        (form(&fx.save_path), form(target)),
                    ];
                    for (save_path, backup_path) in cases {
                        assert_restore_rejected(
                            &fx,
                            &save_path,
                            &backup_path,
                            &format!("{name} form of {label}"),
                        );
                    }
                }
            }
        }

        /// Verifies the 8.3 short-name forms of the save path and backup folder are accepted and
        /// the short-name forms of the root and an outside path are rejected, by both
        /// `verify_backup_path` and restore. Skips only when the call succeeds and the volume made
        /// no short name for the backup folder; an API failure panics.
        #[test]
        fn test_short_name_forms_are_contained() {
            let fx = fixture();
            let Some(short_backup) = short_form(&fx.backup) else {
                eprintln!("SKIPPED: volume generates no 8.3 short name for the backup folder");
                return;
            };
            let short_save = short_form(&fx.save_path).unwrap_or_else(|| fx.save_path.clone());
            let short_root = short_form(&fx.backup_root).expect("root short name missing");
            let short_outside = short_form(&fx.outside).expect("outside short name missing");
            let expected = fx.backup.canonicalize().expect("failed to canonicalize");

            for (save_path, backup_path) in [
                (&short_save, &fx.backup),
                (&fx.save_path, &short_backup),
                (&short_save, &short_backup),
            ] {
                assert_eq!(
                    verify_backup_path(save_path, backup_path).expect("short form rejected"),
                    expected
                );
                assert_restore_accepted(&fx, save_path, backup_path, "short-name form");
            }
            for (save_path, backup_path) in [
                (&fx.save_path, &short_root),
                (&short_save, &short_root),
                (&fx.save_path, &short_outside),
                (&short_save, &short_outside),
            ] {
                assert!(verify_backup_path(save_path, backup_path).is_err());
                assert_restore_rejected(&fx, save_path, backup_path, "short-name form");
            }
        }
    }
}
