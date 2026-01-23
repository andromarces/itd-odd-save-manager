// ITD ODD Save Manager by andromarces

use crate::watcher::FileWatcher;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

/// Configuration structure for the application.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    /// The user-configured path to the game's save directory or file.
    pub save_path: Option<String>,
    /// Whether to automatically launch the game when the app starts.
    #[serde(default)]
    pub auto_launch_game: bool,
    /// Whether to automatically close the app when the game exits.
    #[serde(default)]
    pub auto_close: bool,
}

/// State wrapper for the application configuration.
pub struct ConfigState(pub Mutex<AppConfig>);

/// Resolves the path to the configuration file.
fn get_config_path() -> PathBuf {
    std::env::current_exe()
        .map(|p| p.parent().unwrap_or(Path::new(".")).join("config.json"))
        .unwrap_or_else(|_| PathBuf::from("config.json"))
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

/// Loads the initial configuration from the default location.
pub fn load_initial_config() -> AppConfig {
    load_config_from_path(&get_config_path())
}

/// Validates if the provided string is a valid path (file or directory).
fn is_valid_path(path: &str) -> bool {
    Path::new(path).exists()
}

/// Retrieves the current application configuration.
#[tauri::command]
pub fn get_config(state: State<ConfigState>) -> Result<AppConfig, String> {
    log::info!("Retrieving configuration");
    state.0.lock().map(|config| config.clone()).map_err(|e| {
        log::error!("Failed to access configuration state: {}", e);
        "Failed to access configuration state".to_string()
    })
}

/// Helper to safely update configuration: locks, clones, mutates, saves to disk, then updates memory.
fn update_config<F>(config_state: &State<ConfigState>, mutator: F) -> Result<(), String>
where
    F: FnOnce(&mut AppConfig),
{
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
#[tauri::command]
pub fn set_save_path(
    config_state: State<ConfigState>,
    watcher: State<FileWatcher>,
    path: String,
) -> Result<(), String> {
    log::info!("Attempting to set save path to: {}", path);

    if !is_valid_path(&path) {
        log::warn!("Validation failed: Path does not exist");
        return Err("The provided path does not exist.".to_string());
    }

    update_config(&config_state, |config| {
        config.save_path = Some(path.clone());
    })?;

    // Update Watcher
    // This is done after saving config
    let path_buf = PathBuf::from(path);
    if let Err(e) = watcher.start(path_buf) {
        log::error!("Failed to start watcher: {}", e);
        return Err(format!(
            "Configuration saved, but failed to start watcher: {}",
            e
        ));
    }

    Ok(())
}

/// Sets the game launch and auto-close settings.
#[tauri::command]
pub fn set_game_settings(
    config_state: State<ConfigState>,
    auto_launch_game: bool,
    auto_close: bool,
) -> Result<(), String> {
    log::info!(
        "Setting game settings: auto_launch={}, auto_close={}",
        auto_launch_game,
        auto_close
    );

    update_config(&config_state, |config| {
        config.auto_launch_game = auto_launch_game;
        config.auto_close = auto_close;
    })
}

/// Serializes and writes the configuration to disk.
fn save_config(config: &AppConfig) -> Result<(), String> {
    let config_path = get_config_path();
    let json = serde_json::to_string_pretty(config).map_err(|e| {
        log::error!("Failed to serialize config: {}", e);
        format!("Failed to serialize config: {}", e)
    })?;

    fs::write(&config_path, json).map_err(|e| {
        log::error!("Failed to write config file: {}", e);
        format!("Failed to write config file: {}", e)
    })?;

    log::info!("Configuration saved successfully to {:?}", config_path);
    Ok(())
}

/// Checks if a path is valid.
#[tauri::command]
pub fn validate_path(path: String) -> bool {
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
        };
        let json = serde_json::to_string(&config).unwrap();
        // Field order depends on struct definition or serde implementation.
        // Check if it contains the fields.
        assert!(json.contains(r#""save_path":"C:\\Test""#));
        assert!(json.contains(r#""auto_launch_game":true"#));
        assert!(json.contains(r#""auto_close":true"#));
    }

    /// Tests that the default configuration has expected values.
    #[test]
    fn test_app_config_default() {
        let config = AppConfig::default();
        assert!(config.save_path.is_none());
        assert!(!config.auto_launch_game);
        assert!(!config.auto_close);
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
        };
        let json = serde_json::to_string(&config).unwrap();
        fs::write(&config_path, json).expect("failed to write config");

        let loaded = load_config_from_path(&config_path);
        assert_eq!(loaded.save_path, Some("TestPath".to_string()));
        assert!(loaded.auto_launch_game);
        assert!(!loaded.auto_close);
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
    }
}
