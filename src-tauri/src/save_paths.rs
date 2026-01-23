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
}
