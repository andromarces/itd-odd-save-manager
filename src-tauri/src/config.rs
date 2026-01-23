use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

/// Configuration structure for the application.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    /// The user-configured path to the game's save directory.
    pub save_path: Option<String>,
}

/// State wrapper for the application configuration.
pub struct ConfigState(pub Mutex<AppConfig>);

/// Resolves the path to the configuration file.
///
/// Attempts to locate `config.json` in the same directory as the executable.
/// Defaults to `config.json` in the current working directory if the executable path cannot be determined.
fn get_config_path() -> PathBuf {
    std::env::current_exe()
        .map(|p| p.parent().unwrap_or(Path::new(".")).join("config.json"))
        .unwrap_or_else(|_| PathBuf::from("config.json"))
}

/// Loads configuration from a specific file path.
///
/// Returns `AppConfig::default()` if the file does not exist or cannot be parsed.
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

/// Validates if the provided string is a valid directory path.
fn is_valid_dir(path: &str) -> bool {
    Path::new(path).is_dir()
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

/// Sets the save path in the configuration and persists it to disk.
///
/// Validates that the path exists and is a directory before saving.
#[tauri::command]
pub fn set_save_path(state: State<ConfigState>, path: String) -> Result<(), String> {
    log::info!("Attempting to set save path to: {}", path);

    if !is_valid_dir(&path) {
        log::warn!("Validation failed: Path does not exist or is not a directory");
        return Err("The provided path does not exist or is not a directory.".to_string());
    }

    let mut config = state.0.lock().map_err(|e| {
        log::error!("Failed to acquire lock on configuration state: {}", e);
        "Failed to acquire lock on configuration state".to_string()
    })?;

    config.save_path = Some(path);

    let config_path = get_config_path();
    let json = serde_json::to_string_pretty(&*config).map_err(|e| {
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

/// Checks if a path is valid (exists and is a directory).
#[tauri::command]
pub fn validate_path(path: String) -> bool {
    let is_valid = is_valid_dir(&path);
    log::info!("Validating path '{}': {}", path, is_valid);
    is_valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Tests that the AppConfig struct serializes to the expected JSON format.
    #[test]
    fn test_app_config_serialization() {
        let config = AppConfig {
            save_path: Some("C:\\Test".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(json, r#"{"save_path":"C:\\Test"}"#);
    }

    /// Tests that an invalid path string returns false.
    #[test]
    fn test_validate_path_invalid() {
        assert!(!is_valid_dir("::invalid::path::??"));
    }

    /// Tests that a valid directory path returns true.
    #[test]
    fn test_validate_path_valid() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let path_str = temp_dir.path().to_string_lossy().to_string();
        assert!(is_valid_dir(&path_str));
    }

    /// Tests loading configuration from an existing file.
    #[test]
    fn test_load_config_from_path_existing() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config = AppConfig {
            save_path: Some("TestPath".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        fs::write(&config_path, json).expect("failed to write config");

        let loaded = load_config_from_path(&config_path);
        assert_eq!(loaded.save_path, Some("TestPath".to_string()));
    }

    /// Tests loading configuration from a missing file returns default.
    #[test]
    fn test_load_config_from_path_missing() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("missing.json");

        let loaded = load_config_from_path(&config_path);
        assert!(loaded.save_path.is_none());
    }
}
