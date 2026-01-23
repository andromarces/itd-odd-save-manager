// ITD ODD Save Manager by andromarces

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const STEAM_APP_ID: &str = "2239710";
const STEAM_SAVE_FILE: &str = "gamesave_0.sav";

/// Adds a candidate path to the list when it exists as a directory.
fn push_if_dir(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_dir() {
        paths.push(path);
    }
}

/// Returns candidate Steam install roots for a Windows system.
pub(crate) fn candidate_steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        push_if_dir(&mut roots, PathBuf::from(program_files_x86).join("Steam"));
    }

    if let Some(program_files) = env::var_os("ProgramFiles") {
        push_if_dir(&mut roots, PathBuf::from(program_files).join("Steam"));
    }

    let mut unique = Vec::new();
    for root in roots {
        if !unique.iter().any(|item: &PathBuf| item == &root) {
            unique.push(root);
        }
    }

    unique
}

/// Finds save directories for the game under the given Steam install root.
pub(crate) fn find_steam_save_dirs(steam_root: &Path) -> Vec<PathBuf> {
    let userdata = steam_root.join("userdata");
    let mut matches = Vec::new();

    let entries = match fs::read_dir(&userdata) {
        Ok(entries) => entries,
        Err(_) => return matches,
    };

    for entry in entries.flatten() {
        let user_dir = entry.path();
        if !user_dir.is_dir() {
            continue;
        }

        let remote_dir = user_dir.join(STEAM_APP_ID).join("remote");
        let save_file = remote_dir.join(STEAM_SAVE_FILE);
        if save_file.is_file() {
            matches.push(remote_dir);
        }
    }

    matches
}

/// Detects Steam save directories for the game and returns them as strings.
#[tauri::command]
pub(crate) fn detect_steam_save_paths() -> Vec<String> {
    log::info!("Steam save detection started");

    if !cfg!(target_os = "windows") {
        log::warn!("Steam save detection is limited to Windows in this build");
        return Vec::new();
    }

    let mut results = Vec::new();
    for root in candidate_steam_roots() {
        results.extend(find_steam_save_dirs(&root));
    }

    results.sort();
    results.dedup();

    log::info!(
        "Steam save detection completed with {} result(s)",
        results.len()
    );

    results
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect()
}

/// Checks if a given path appears to be a Steam Cloud path.
///
/// Returns true if the path contains 'userdata' and the App ID '2239710'.
/// Comparison is case-insensitive to handle Windows paths correctly.
pub(crate) fn is_steam_cloud_path(path: &Path) -> bool {
    // Simple heuristic: check for app id and userdata in the path.
    // Normalized to handle both slash types if needed, but path components are safer.
    // Use to_string_lossy() and eq_ignore_ascii_case for case-insensitive comparison on Windows.
    let has_app_id = path.iter().any(|p| p.to_string_lossy() == STEAM_APP_ID);
    let has_userdata = path
        .iter()
        .any(|p| p.to_string_lossy().eq_ignore_ascii_case("userdata"));

    has_app_id && has_userdata
}

/// Tauri command to check if the provided path is a Steam Cloud path.
#[tauri::command]
pub(crate) fn check_steam_cloud_path(path: String) -> bool {
    is_steam_cloud_path(Path::new(&path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies that a Steam save folder is detected when the structure exists.
    #[test]
    fn find_steam_save_dirs_detects_expected_path() {
        let temp_dir = tempfile::tempdir().expect("temp directory created");
        let steam_root = temp_dir.path().join("Steam");
        let save_dir = steam_root
            .join("userdata")
            .join("123456789")
            .join(STEAM_APP_ID)
            .join("remote");
        fs::create_dir_all(&save_dir).expect("save directory created");
        fs::write(save_dir.join(STEAM_SAVE_FILE), "test").expect("save file created");

        let matches = find_steam_save_dirs(&steam_root);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], save_dir);
    }

    /// Verifies that no results are returned when the user data folder is missing.
    #[test]
    fn find_steam_save_dirs_returns_empty_when_missing() {
        let temp_dir = tempfile::tempdir().expect("temp directory created");
        let steam_root = temp_dir.path().join("Steam");

        let matches = find_steam_save_dirs(&steam_root);

        assert!(matches.is_empty());
    }

    /// Tests the `is_steam_cloud_path` heuristic with various path combinations, including case sensitivity.
    #[test]
    fn test_is_steam_cloud_path() {
        let cloud_path =
            PathBuf::from("C:/Program Files (x86)/Steam/userdata/12345/2239710/remote");
        let local_path = PathBuf::from("C:/Games/IntoTheDead/Saves");
        let partial_path = PathBuf::from("C:/Steam/userdata/12345/999999/remote");
        let mixed_case_path = PathBuf::from("C:/Steam/UserDATA/12345/2239710/remote");

        assert!(is_steam_cloud_path(&cloud_path));
        assert!(!is_steam_cloud_path(&local_path));
        assert!(!is_steam_cloud_path(&partial_path));
        assert!(is_steam_cloud_path(&mixed_case_path));
    }
}
