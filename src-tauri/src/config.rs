// ITD ODD Save Manager by andromarces

use crate::watcher::FileWatcher;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

/// Configuration structure for the application.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    /// The user-configured path to the game's save directory or file.
    pub save_path: Option<String>,
    /// Whether to automatically launch the game when the app starts.
    #[serde(default)]
    pub auto_launch_game: bool,
    /// Whether to automatically close the app when the game exits.
    #[serde(default)]
    pub auto_close: bool,
    /// Maximum number of backups to keep per game.
    #[serde(default = "default_max_backups")]
    pub max_backups_per_game: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: None,
            auto_launch_game: false,
            auto_close: false,
            max_backups_per_game: default_max_backups(),
        }
    }
}

fn default_max_backups() -> usize {
    100
}

/// State wrapper for the application configuration.
pub struct ConfigState(pub Mutex<AppConfig>);

/// Resolves the path to the configuration file.
pub(crate) fn get_config_path() -> PathBuf {
    std::env::current_exe()
        .map(|p| config_path_for_exe(&p))
        .unwrap_or_else(|_| PathBuf::from("config.json"))
}

/// Derives the configuration file path from an executable path.
fn config_path_for_exe(exe_path: &Path) -> PathBuf {
    let config_name = exe_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| format!("{stem}.config.json"))
        .unwrap_or_else(|| "config.json".to_string());

    exe_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(config_name)
}

/// Loads configuration from a specific file path.
pub fn load_config_from_path(path: &Path) -> AppConfig {
    log::info!("Loading configuration from: {:?}", path);
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => {
                    log::info!("Configuration loaded successfully");
                    return config;
                }
                Err(e) => log::error!("Failed to parse configuration: {}", e),
            },
            Err(e) => log::error!("Failed to read configuration file: {}", e),
        }
    } else {
        log::info!("Configuration file not found, using defaults");
    }
    AppConfig::default()
}

/// Validates if the provided string is a valid path (file or directory).
///
/// If the path itself does not exist, it only accepts it if it looks like a file
/// (has an extension) and its parent directory exists.
fn is_valid_path(path: &str) -> bool {
    let p = Path::new(path);
    if p.exists() {
        return true;
    }
    // Only accept non-existent file paths if parent exists
    if let Some(parent) = p.parent() {
        if parent.exists() {
            // Heuristic: if it has an extension, treat as file path
            return p.extension().is_some();
        }
    }
    false
}

/// Retrieves the current application configuration.
///
/// # Returns
///
/// * `Result<AppConfig, String>` - The current configuration or an error message.
#[tauri::command(rename_all = "snake_case")]
pub async fn get_config(state: State<'_, ConfigState>) -> Result<AppConfig, String> {
    log::info!("Retrieving configuration");
    state.0.lock().map(|config| config.clone()).map_err(|e| {
        log::error!("Failed to access configuration state: {}", e);
        "Failed to access configuration state".to_string()
    })
}

/// Locks, clones, mutates, saves to the given path, then updates in-memory state.
///
/// Memory is only updated after a successful disk write, so a write failure
/// leaves both disk and memory at the previous value.
pub(crate) fn update_config_with_path(
    config_state: &ConfigState,
    config_path: &Path,
    mutator: impl FnOnce(&mut AppConfig),
) -> Result<(), String> {
    let mut config_guard = config_state.0.lock().map_err(|e| {
        log::error!("Failed to acquire lock on configuration state: {}", e);
        "Failed to acquire lock on configuration state".to_string()
    })?;

    let mut new_config = config_guard.clone();
    mutator(&mut new_config);

    save_config_to_path(&new_config, config_path)?;

    *config_guard = new_config;
    Ok(())
}

/// Thin wrapper around `update_config_with_path` using the default config path.
fn update_config(
    config_state: &State<'_, ConfigState>,
    mutator: impl FnOnce(&mut AppConfig),
) -> Result<(), String> {
    update_config_with_path(config_state, &get_config_path(), mutator)
}

/// Restores the previous watcher state after a failed path swap.
///
/// If the old path exists, restarts the watcher on it. If that also fails,
/// stops the watcher and clears `save_path` in both memory and on disk.
fn rollback_watcher(
    config_state: &ConfigState,
    watcher: &FileWatcher,
    old_save_path: &Option<String>,
    max_backups: usize,
    config_path: &Path,
) {
    match old_save_path {
        Some(old_path) => {
            if let Err(re) = watcher.start(PathBuf::from(old_path), max_backups, None) {
                log::error!("Failed to restore previous watcher: {}", re);
                watcher.stop();
                let _ = update_config_with_path(config_state, config_path, |c| {
                    c.save_path = None;
                });
            }
            // else: old watcher restored; disk and memory config already hold old path.
        }
        None => watcher.stop(),
    }
}

/// Restarts the watcher with a new backup limit, rolling back on failure.
///
/// On restart failure, attempts to restore the watcher with `old_limit`. Memory is
/// always updated to match actual watcher state before a best-effort disk sync,
/// so runtime config remains consistent even when the disk write fails.
pub(crate) fn restart_watcher_with_limit(
    config_state: &ConfigState,
    watcher: &FileWatcher,
    path: PathBuf,
    new_limit: usize,
    old_limit: usize,
    config_path: &Path,
) -> Result<(), String> {
    if let Err(e) = watcher.start(path.clone(), new_limit, None) {
        log::error!("Failed to restart watcher with new limit: {}", e);

        let restore_failed = watcher.start(path, old_limit, None).is_err();
        if restore_failed {
            log::error!("Failed to restore previous watcher after limit change");
        }

        // Update memory unconditionally so runtime config always reflects actual
        // watcher state, then attempt disk sync (best-effort).
        match config_state.0.lock() {
            Ok(mut guard) => {
                guard.max_backups_per_game = old_limit;
                if restore_failed {
                    guard.save_path = None;
                }
                let _ = save_config_to_path(&*guard, config_path);
            }
            Err(e) => log::error!("Failed to acquire lock for rollback config update: {}", e),
        }

        return Err(format!(
            "Failed to restart watcher with new backup limit. Error: {}",
            e
        ));
    }

    Ok(())
}

/// Atomically replaces the watched path and persists the new value to `config_path`.
///
/// Step 1: starts the new watcher. Step 2: persists the config. If either step
/// fails, `rollback_watcher` is called to restore the previous watcher state
/// before returning the error.
pub(crate) fn replace_watcher_path(
    config_state: &ConfigState,
    watcher: &FileWatcher,
    new_path: PathBuf,
    new_path_str: String,
    config_path: &Path,
) -> Result<(), String> {
    let (old_save_path, max_backups) = {
        let guard = config_state.0.lock().map_err(|e| {
            log::error!("Failed to acquire lock on configuration state: {}", e);
            "Failed to acquire lock on configuration state".to_string()
        })?;
        (guard.save_path.clone(), guard.max_backups_per_game)
    };

    if let Err(e) = watcher.start(new_path, max_backups, None) {
        log::error!("Failed to start watcher for new path: {}", e);
        rollback_watcher(
            config_state,
            watcher,
            &old_save_path,
            max_backups,
            config_path,
        );
        return Err(format!(
            "Configuration path accepted, but failed to start monitoring. Error: {}",
            e
        ));
    }

    if let Err(e) = update_config_with_path(config_state, config_path, |config| {
        config.save_path = Some(new_path_str);
    }) {
        log::error!("Failed to persist new save path: {}", e);
        rollback_watcher(
            config_state,
            watcher,
            &old_save_path,
            max_backups,
            config_path,
        );
        return Err(e);
    }

    Ok(())
}

/// Sets the save path in the configuration, persists it, and updates the watcher.
///
/// Normalizes the input path to a directory. If a file path is provided,
/// its parent directory is used.
///
/// # Arguments
///
/// * `path` - The user-provided path string.
///
/// # Returns
///
/// * `Result<String, String>` - The normalized path string on success, or an error message.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_save_path(
    config_state: State<'_, ConfigState>,
    watcher: State<'_, FileWatcher>,
    path: String,
) -> Result<String, String> {
    log::info!("Attempting to set save path to: {}", path);

    // Validate using the refined rule (path exists OR non-existent file with existing parent)
    if !is_valid_path(&path) {
        log::warn!("Validation failed: Path (or its parent) does not exist or is an invalid directory entry");
        return Err(
            "The provided path must exist, or be a new file path within an existing directory."
                .to_string(),
        );
    }

    let path_buf = PathBuf::from(&path);

    // Normalize to directory using shared helper
    let final_path = crate::filename_utils::normalize_to_directory(&path_buf)
        .map_err(|e| format!("Invalid path: {}", e))?;

    let final_path_str = final_path.to_string_lossy().to_string();
    log::info!("Normalized save path to: {}", final_path_str);

    replace_watcher_path(
        &config_state,
        &watcher,
        final_path,
        final_path_str.clone(),
        &get_config_path(),
    )?;

    Ok(final_path_str)
}

/// Sets the game launch and auto-close settings.
///
/// # Arguments
///
/// * `auto_launch_game` - Enable/disable auto-launch.
/// * `auto_close` - Enable/disable auto-close.
/// * `max_backups_per_game` - The limit for backups per game.
#[tauri::command(rename_all = "snake_case")]
pub async fn set_game_settings(
    config_state: State<'_, ConfigState>,
    watcher: State<'_, FileWatcher>,
    auto_launch_game: bool,
    auto_close: bool,
    max_backups_per_game: usize,
) -> Result<(), String> {
    log::info!(
        "Setting game settings: auto_launch={}, auto_close={}, max_backups={}",
        auto_launch_game,
        auto_close,
        max_backups_per_game
    );

    let (limit_changed, old_limit) = {
        let guard = config_state.0.lock().map_err(|e| e.to_string())?;
        (
            guard.max_backups_per_game != max_backups_per_game,
            guard.max_backups_per_game,
        )
    };

    update_config(&config_state, |config| {
        config.auto_launch_game = auto_launch_game;
        config.auto_close = auto_close;
        config.max_backups_per_game = max_backups_per_game;
    })?;

    if limit_changed {
        // Extract the watched path while holding the lock, then release it so
        // watcher operations (which join threads) do not block under the lock.
        let path_buf = {
            let config = config_state.0.lock().map_err(|e| e.to_string())?;
            config.save_path.as_deref().map(PathBuf::from)
        };

        if let Some(path_buf) = path_buf {
            restart_watcher_with_limit(
                &config_state,
                &watcher,
                path_buf,
                max_backups_per_game,
                old_limit,
                &get_config_path(),
            )?;
        }
    }

    Ok(())
}

/// Serializes and writes the configuration to disk.
pub(crate) fn save_config(config: &AppConfig) -> Result<(), String> {
    save_config_to_path(config, &get_config_path())
}

/// Serializes and writes the configuration to a specific path.
pub(crate) fn save_config_to_path(config: &AppConfig, path: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config).map_err(|e| {
        log::error!("Failed to serialize config: {}", e);
        format!("Failed to serialize config: {}", e)
    })?;

    fs::write(path, json).map_err(|e| {
        log::error!("Failed to write config file: {}", e);
        format!("Failed to write config file: {}", e)
    })?;

    log::info!("Configuration saved successfully to {:?}", path);
    Ok(())
}

/// Checks if a path is valid.
///
/// Returns true if the path exists OR if the parent directory exists and looks like a file path.
#[tauri::command(rename_all = "snake_case")]
pub async fn validate_path(path: String) -> bool {
    let is_valid = is_valid_path(&path);
    log::info!("Validating path '{}': {}", path, is_valid);
    is_valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::sync::Mutex;
    use tempfile::tempdir;

    fn make_config_state(save_path: Option<&str>) -> ConfigState {
        ConfigState(Mutex::new(AppConfig {
            save_path: save_path.map(|s| s.to_string()),
            ..AppConfig::default()
        }))
    }

    /// Tests that the AppConfig struct serializes to the expected JSON format.
    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig {
            save_path: Some("C:\\Test".to_string()),
            auto_launch_game: true,
            auto_close: true,
            max_backups_per_game: 50,
        };
        let json = serde_json::to_string(&config).unwrap();
        // Field order depends on struct definition or serde implementation.
        // Check if it contains the fields.
        assert!(json.contains(r#""save_path":"C:\\Test""#));
        assert!(json.contains(r#""auto_launch_game":true"#));
        assert!(json.contains(r#""auto_close":true"#));
        assert!(json.contains(r#""max_backups_per_game":50"#));
    }

    /// Tests that the default configuration has expected values.
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(config.save_path.is_none());
        assert!(!config.auto_launch_game);
        assert!(!config.auto_close);
        assert_eq!(config.max_backups_per_game, 100);
    }

    /// Tests that an invalid path string returns false.
    #[test]
    fn test_validate_path_invalid() {
        assert!(!is_valid_path("::invalid::path::??"));
    }

    /// Tests that a valid directory path returns true.
    #[test]
    fn test_validate_path_valid_dir() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let path_str = temp_dir.path().to_string_lossy().to_string();
        assert!(is_valid_path(&path_str));
    }

    /// Tests that a non-existent file path with an existing parent returns true.
    #[test]
    fn test_validate_path_non_existent_file_with_parent() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let future_file = temp_dir.path().join("future.sav");
        let path_str = future_file.to_string_lossy().to_string();
        assert!(is_valid_path(&path_str));
    }

    /// Tests that a non-existent directory path returns false.
    #[test]
    fn test_validate_path_non_existent_dir_with_parent() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let future_dir = temp_dir.path().join("future_dir");
        // No extension, should be treated as directory
        let path_str = future_dir.to_string_lossy().to_string();
        assert!(!is_valid_path(&path_str));
    }

    /// Tests that a valid file path returns true.
    #[test]
    fn test_validate_path_valid_file() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let file_path = temp_dir.path().join("test.sav");
        File::create(&file_path).unwrap();
        let path_str = file_path.to_string_lossy().to_string();
        assert!(is_valid_path(&path_str));
    }

    /// Tests loading configuration from an existing file.
    #[test]
    fn test_load_config_from_path_existing() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config = AppConfig {
            save_path: Some("TestPath".to_string()),
            auto_launch_game: true,
            auto_close: false,
            max_backups_per_game: 200,
        };
        let json = serde_json::to_string(&config).unwrap();
        fs::write(&config_path, json).expect("failed to write config");

        let loaded = load_config_from_path(&config_path);
        assert_eq!(loaded.save_path, Some("TestPath".to_string()));
        assert!(loaded.auto_launch_game);
        assert!(!loaded.auto_close);
        assert_eq!(loaded.max_backups_per_game, 200);
    }

    /// Tests loading configuration from a missing file returns default.
    #[test]
    fn test_load_config_from_path_missing() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("missing.json");

        let loaded = load_config_from_path(&config_path);
        assert!(loaded.save_path.is_none());
        assert!(!loaded.auto_launch_game);
        assert!(!loaded.auto_close);
        assert_eq!(loaded.max_backups_per_game, 100);
    }

    /// Tests that the config path mirrors the executable name.
    #[test]
    fn test_config_path_for_exe_mirrors_name() {
        let exe_path = PathBuf::from("C:/Apps/ITD ODD Save Manager.exe");
        let expected = exe_path
            .parent()
            .expect("missing parent")
            .join("ITD ODD Save Manager.config.json");

        let resolved = config_path_for_exe(&exe_path);

        assert_eq!(resolved, expected);
    }

    /// Verifies a successful replace_watcher_path call updates config and starts the watcher.
    #[test]
    fn test_replace_watcher_path_success() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().join("saves");
        fs::create_dir_all(&save_dir).unwrap();
        let config_path = temp.path().join("config.json");

        let cs = make_config_state(None);
        let watcher = FileWatcher::new();

        let result = replace_watcher_path(
            &cs,
            &watcher,
            save_dir.clone(),
            save_dir.to_string_lossy().to_string(),
            &config_path,
        );

        assert!(result.is_ok());
        let persisted = load_config_from_path(&config_path);
        assert_eq!(
            persisted.save_path,
            Some(save_dir.to_string_lossy().to_string())
        );

        watcher.stop();
    }

    /// Verifies that replace_watcher_path returns an error and leaves config unchanged
    /// when the new path does not exist and there was no previous watcher.
    #[test]
    fn test_replace_watcher_path_fails_no_prior_path() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.json");
        let nonexistent = temp.path().join("does_not_exist");

        let cs = make_config_state(None);
        let watcher = FileWatcher::new();

        let result = replace_watcher_path(
            &cs,
            &watcher,
            nonexistent,
            "does_not_exist".to_string(),
            &config_path,
        );

        assert!(result.is_err());
        // Config file should not have been written
        assert!(!config_path.exists());
        // In-memory save_path should remain None
        assert!(cs.0.lock().unwrap().save_path.is_none());
    }

    /// Verifies that restart_watcher_with_limit succeeds and does not modify config
    /// when the path exists and the new limit is valid.
    ///
    /// The helper does not update config on success; that is the caller's
    /// responsibility (set_game_settings calls update_config before this helper).
    #[test]
    fn test_restart_watcher_with_limit_success() {
        let temp = tempdir().unwrap();
        let save_dir = temp.path().join("saves");
        fs::create_dir_all(&save_dir).unwrap();
        let config_path = temp.path().join("config.json");

        let save_dir_str = save_dir.to_string_lossy().to_string();
        // Simulate the state after set_game_settings has already persisted new_limit=100.
        let cs = ConfigState(Mutex::new(AppConfig {
            save_path: Some(save_dir_str.clone()),
            max_backups_per_game: 100,
            ..AppConfig::default()
        }));
        let watcher = FileWatcher::new();
        watcher.start(save_dir.clone(), 50, None).unwrap();

        let result = restart_watcher_with_limit(&cs, &watcher, save_dir, 100, 50, &config_path);

        assert!(result.is_ok());
        // Helper does not touch config on success.
        assert_eq!(cs.0.lock().unwrap().max_backups_per_game, 100);
        watcher.stop();
    }

    /// Verifies that restart_watcher_with_limit returns Err and rolls back
    /// max_backups_per_game to old_limit in memory when the start fails.
    /// Using the same non-existent path for both start and restore means both
    /// fail, so save_path is also cleared.
    #[test]
    fn test_restart_watcher_with_limit_both_fail_clears_save_path() {
        let temp = tempdir().unwrap();
        let config_path = temp.path().join("config.json");
        let nonexistent = temp.path().join("no_such_dir");

        let cs = ConfigState(Mutex::new(AppConfig {
            save_path: Some(nonexistent.to_string_lossy().to_string()),
            max_backups_per_game: 100, // new_limit that was already persisted by caller
            ..AppConfig::default()
        }));
        let watcher = FileWatcher::new();

        let result = restart_watcher_with_limit(&cs, &watcher, nonexistent, 100, 50, &config_path);

        assert!(result.is_err());
        let guard = cs.0.lock().unwrap();
        // Memory must reflect stopped-watcher state: old_limit, no save_path.
        assert_eq!(guard.max_backups_per_game, 50);
        assert!(
            guard.save_path.is_none(),
            "save_path must be cleared when watcher is stopped"
        );
    }

    /// Verifies that replace_watcher_path restores the previous watcher when the new
    /// path does not exist, leaving config on-disk and in-memory at the old value.
    #[test]
    fn test_replace_watcher_path_rolls_back_to_previous_watcher() {
        let temp = tempdir().unwrap();
        let old_save_dir = temp.path().join("old_saves");
        let nonexistent = temp.path().join("new_saves_missing");
        fs::create_dir_all(&old_save_dir).unwrap();
        let config_path = temp.path().join("config.json");

        // Seed the config file with the old path
        let initial_config = AppConfig {
            save_path: Some(old_save_dir.to_string_lossy().to_string()),
            ..AppConfig::default()
        };
        save_config_to_path(&initial_config, &config_path).unwrap();

        let cs = make_config_state(Some(&old_save_dir.to_string_lossy()));
        let watcher = FileWatcher::new();
        // Start the watcher on the old path so there is a prior active watcher
        watcher
            .start(old_save_dir.clone(), 100, None)
            .expect("initial watcher start failed");

        let result = replace_watcher_path(
            &cs,
            &watcher,
            nonexistent,
            "new_saves_missing".to_string(),
            &config_path,
        );

        assert!(result.is_err());
        // On-disk config should still hold the old path
        let persisted = load_config_from_path(&config_path);
        assert_eq!(
            persisted.save_path,
            Some(old_save_dir.to_string_lossy().to_string())
        );
        // In-memory config should still hold the old path
        assert_eq!(
            cs.0.lock().unwrap().save_path,
            Some(old_save_dir.to_string_lossy().to_string())
        );

        watcher.stop();
    }
}
