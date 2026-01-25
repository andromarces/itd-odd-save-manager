// ITD ODD Save Manager by andromarces

use std::env;
use std::path::{Path, PathBuf};

const LOCAL_SAVE_DIR_COMPONENTS: [&str; 4] =
    ["AppData", "LocalLow", "PikPok", "IntoTheDeadOurDarkestDays"];

/// Builds the expected local save path from a user profile directory.
fn local_save_path_from_profile(user_profile: &Path) -> PathBuf {
    let mut path = PathBuf::from(user_profile);
    for component in LOCAL_SAVE_DIR_COMPONENTS {
        path = path.join(component);
    }
    path
}

/// Detects the local save path using the USERPROFILE environment variable.
fn detect_windows_local_save_path() -> Option<PathBuf> {
    let user_profile = PathBuf::from(env::var_os("USERPROFILE")?);
    let path = local_save_path_from_profile(&user_profile);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// Detects the save directory for the game and returns it as a string.
#[tauri::command(rename_all = "snake_case")]
pub(crate) async fn detect_steam_save_paths() -> Vec<String> {
    log::info!("Save path detection started");

    if !cfg!(target_os = "windows") {
        log::warn!("Save path detection is only supported on Windows in this build");
        return Vec::new();
    }

    let results = detect_windows_local_save_path()
        .map(|path| vec![path])
        .unwrap_or_default();

    log::info!(
        "Save path detection completed with {} result(s)",
        results.len()
    );

    results
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

/// Reports whether auto-detection is supported on this platform.
#[tauri::command(rename_all = "snake_case")]
pub(crate) fn is_auto_detection_supported() -> bool {
    cfg!(target_os = "windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// Runs a closure with USERPROFILE set, restoring the original value afterward.
    fn with_userprofile<T>(value: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = ENV_MUTEX.lock().expect("env mutex locked");
        let original = env::var_os("USERPROFILE");
        env::set_var("USERPROFILE", value);
        let result = f();
        match original {
            Some(value) => env::set_var("USERPROFILE", value),
            None => env::remove_var("USERPROFILE"),
        }
        result
    }

    /// Verifies that the expected local save path is returned when it exists.
    #[test]
    fn detect_windows_local_save_path_returns_expected_path() {
        let temp_dir = tempfile::tempdir().expect("temp directory created");
        let expected = local_save_path_from_profile(temp_dir.path());
        std::fs::create_dir_all(&expected).expect("save directory created");

        let detected = with_userprofile(temp_dir.path(), detect_windows_local_save_path);

        assert_eq!(detected.as_deref(), Some(expected.as_path()));
    }

    /// Verifies that detection fails when the expected local save path is missing.
    #[test]
    fn detect_windows_local_save_path_returns_none_when_missing() {
        let temp_dir = tempfile::tempdir().expect("temp directory created");
        let detected = with_userprofile(temp_dir.path(), detect_windows_local_save_path);

        assert!(detected.is_none());
    }

    /// Verifies that detection is disabled for non-Windows builds.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn detect_steam_save_paths_returns_empty_on_non_windows() {
        let results = detect_steam_save_paths();

        assert!(results.is_empty());
    }

    /// Verifies that auto-detection is flagged as supported on Windows.
    #[cfg(target_os = "windows")]
    #[test]
    fn is_auto_detection_supported_returns_true_on_windows() {
        assert!(is_auto_detection_supported());
    }

    /// Verifies that auto-detection is flagged as unsupported on non-Windows builds.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn is_auto_detection_supported_returns_false_on_non_windows() {
        assert!(!is_auto_detection_supported());
    }
}
