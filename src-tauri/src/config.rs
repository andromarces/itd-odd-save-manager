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

/// Helper to safely update configuration: locks, clones, mutates, saves to disk, then updates memory.
fn update_config(
    config_state: &State<'_, ConfigState>,
    mutator: impl FnOnce(&mut AppConfig),
) -> Result<(), String> {
    let mut config_guard = config_state.0.lock().map_err(|e| {
        log::error!("Failed to acquire lock on configuration state: {}", e);
        "Failed to acquire lock on configuration state".to_string()
    })?;

    let mut new_config = config_guard.clone();
    mutator(&mut new_config);

    save_config(&new_config)?;

    *config_guard = new_config;
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

    // Update Watcher
    let max_backups = config_state.0.lock().unwrap().max_backups_per_game;
    if let Err(e) = watcher.start(final_path, max_backups) {
        log::error!("Failed to start watcher: {}", e);

        // Disable auto-backup on failure
        let _ = update_config(&config_state, |config| {
            config.save_path = None;
        });

        return Err(format!(
            "Configuration path accepted, but failed to start monitoring. Auto-backup has been disabled. Error: {}",
            e
        ));
    }

    // Success -> persist normalized path
    update_config(&config_state, |config| {
        config.save_path = Some(final_path_str.clone());
    })?;

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

    update_config(&config_state, |config| {
        config.auto_launch_game = auto_launch_game;
        config.auto_close = auto_close;
        config.max_backups_per_game = max_backups_per_game;
    })?;

    // Restart watcher with new limit if active
    let config = config_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(path) = &config.save_path {
        if let Err(e) = watcher.start(PathBuf::from(path), max_backups_per_game) {
            log::error!("Failed to restart watcher with new limit: {}", e);
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
    use tempfile::tempdir;

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
}
