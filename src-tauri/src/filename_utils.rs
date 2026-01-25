use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use std::path::{Path, PathBuf};

/// Represents parsed information from a game save filename.
#[derive(Debug, PartialEq, Eq)]
pub struct SaveFileInfo {
    /// The zero-based game number extracted from the filename.
    pub game_number: u32,
    /// True if the file is a backup (.bak), false if it is the main save (.sav).
    pub is_bak: bool,
}

/// Parses a filename string to extract game number and file type.
///
/// Expects filenames in the format `gamesave_{N}.sav` or `gamesave_{N}.sav.bak`.
///
/// # Arguments
///
/// * `filename` - The filename to parse.
///
/// # Returns
///
/// * `Option<SaveFileInfo>` - The parsed info if valid, or None.
pub fn parse_filename(filename: &str) -> Option<SaveFileInfo> {
    if !filename.starts_with("gamesave_") {
        return None;
    }

    // Expected format: gamesave_{N}.sav or gamesave_{N}.sav.bak

    let rest = &filename["gamesave_".len()..];

    // Find the first dot, which should end the number part
    let dot_index = rest.find('.')?;

    let number_str = &rest[..dot_index];
    let game_number = number_str.parse::<u32>().ok()?;

    let suffix = &rest[dot_index..];

    if suffix == ".sav" {
        Some(SaveFileInfo {
            game_number,
            is_bak: false,
        })
    } else if suffix == ".sav.bak" {
        Some(SaveFileInfo {
            game_number,
            is_bak: true,
        })
    } else {
        None
    }
}

/// Parses a file path to extract game save information.
///
/// # Arguments
///
/// * `path` - The path to the file.
///
/// # Returns
///
/// * `Option<SaveFileInfo>` - The parsed info if valid, or None.
pub fn parse_path(path: &Path) -> Option<SaveFileInfo> {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(parse_filename)
}

/// Normalizes a path to a directory.
///
/// Logic:
/// 1. If path points to an existing file, return its parent.
/// 2. If path does not exist but has an extension (looks like a file), return its parent.
/// 3. Otherwise, return the path as is (assuming it is a directory).
///
/// # Arguments
///
/// * `path` - The path to normalize.
///
/// # Returns
///
/// * `Result<PathBuf, String>` - The normalized directory path.
pub fn normalize_to_directory(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Invalid file path: has no parent".to_string())
    } else if !path.exists() && path.extension().is_some() {
        path.parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| "Invalid file path: has no parent".to_string())
    } else {
        Ok(path.to_path_buf())
    }
}

const BACKUP_FOLDER_PREFIX: &str = "Game ";
const BACKUP_FOLDER_SEPARATOR: &str = " - ";
const BACKUP_TIMESTAMP_FORMAT: &str = "%d-%b-%Y %I-%M-%S %p";

/// Formats a backup folder name for a specific game and timestamp.
///
/// Format: "Game {N} - {Timestamp}"
/// where Timestamp is "dd-MMM-yyyy hh-mm-ss AM"
pub fn format_backup_folder_name(game_number: u32, timestamp: DateTime<Local>) -> String {
    let display_number = game_number + 1;
    let timestamp_str = timestamp.format(BACKUP_TIMESTAMP_FORMAT).to_string();
    format!(
        "{}{}{}{}",
        BACKUP_FOLDER_PREFIX, display_number, BACKUP_FOLDER_SEPARATOR, timestamp_str
    )
}

/// Parsed result from a backup folder name.
#[derive(Debug, PartialEq, Eq)]
pub struct BackupFolderInfo {
    pub game_number: u32,
    pub timestamp: DateTime<Local>,
}

/// Parses a backup folder name to extract game number and timestamp.
///
/// Tries to parse the timestamp from the folder name. If parsing fails,
/// returns None.
pub fn parse_backup_folder_name(folder_name: &str) -> Option<BackupFolderInfo> {
    let (prefix, date_part) = folder_name.split_once(BACKUP_FOLDER_SEPARATOR)?;

    let stripped_prefix = prefix.strip_prefix(BACKUP_FOLDER_PREFIX)?;
    let display_number = stripped_prefix.parse::<u32>().ok()?;
    // Internal game number is 0-based
    let game_number = display_number.saturating_sub(1);

    if let Ok(naive_dt) = NaiveDateTime::parse_from_str(date_part, BACKUP_TIMESTAMP_FORMAT) {
        match Local.from_local_datetime(&naive_dt) {
            chrono::LocalResult::Single(dt) => Some(BackupFolderInfo {
                game_number,
                timestamp: dt,
            }),
            chrono::LocalResult::Ambiguous(dt1, _) => Some(BackupFolderInfo {
                game_number,
                timestamp: dt1,
            }),
            chrono::LocalResult::None => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    /// Tests parsing of valid .sav filenames.
    #[test]
    fn test_parse_filename_valid_sav() {
        assert_eq!(
            parse_filename("gamesave_0.sav"),
            Some(SaveFileInfo {
                game_number: 0,
                is_bak: false
            })
        );
        assert_eq!(
            parse_filename("gamesave_123.sav"),
            Some(SaveFileInfo {
                game_number: 123,
                is_bak: false
            })
        );
    }

    /// Tests parsing of valid .sav.bak filenames.
    #[test]
    fn test_parse_filename_valid_bak() {
        assert_eq!(
            parse_filename("gamesave_0.sav.bak"),
            Some(SaveFileInfo {
                game_number: 0,
                is_bak: true
            })
        );
        assert_eq!(
            parse_filename("gamesave_5.sav.bak"),
            Some(SaveFileInfo {
                game_number: 5,
                is_bak: true
            })
        );
    }

    /// Tests that invalid filenames return None.
    #[test]
    fn test_parse_filename_invalid() {
        assert_eq!(parse_filename("gamesave_abc.sav"), None);
        assert_eq!(parse_filename("other_0.sav"), None);
        assert_eq!(parse_filename("gamesave_0.txt"), None);
        assert_eq!(parse_filename("gamesave_0"), None);
        assert_eq!(parse_filename("gamesave_.sav"), None); // No number
        assert_eq!(parse_filename("gamesave_0.sav.other"), None);
    }

    /// Tests parsing from a full path.
    #[test]
    fn test_parse_path() {
        let p = PathBuf::from("base").join("subdir").join("gamesave_1.sav");
        assert_eq!(
            parse_path(&p),
            Some(SaveFileInfo {
                game_number: 1,
                is_bak: false
            })
        );
    }

    /// Tests path normalization logic.
    #[test]
    fn test_normalize_to_directory() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let dir_path = temp_dir.path();

        // 1. Existing Directory
        assert_eq!(
            normalize_to_directory(dir_path).unwrap(),
            dir_path.to_path_buf()
        );

        // 2. Existing File
        let file_path = dir_path.join("test.sav");
        File::create(&file_path).unwrap();
        assert_eq!(
            normalize_to_directory(&file_path).unwrap(),
            dir_path.to_path_buf()
        );

        // 3. Non-existent file (has extension)
        let future_file = dir_path.join("future.sav");
        assert_eq!(
            normalize_to_directory(&future_file).unwrap(),
            dir_path.to_path_buf()
        );

        // 4. Non-existent directory (no extension)
        let future_dir = dir_path.join("future_dir");
        assert_eq!(normalize_to_directory(&future_dir).unwrap(), future_dir);
    }

    #[test]
    fn test_backup_folder_formatting_and_parsing() {
        // Use a fixed time for testing
        let dt = Local.timestamp_opt(1706173200, 0).unwrap(); // Jan 25 2024 12:00:00 PM roughly
        let game_number = 1; // Game 2

        let folder_name = format_backup_folder_name(game_number, dt);
        // Check format roughly (depends on locale, but chrono format is explicit)
        // "%d-%b-%Y %I-%M-%S %p"
        assert!(folder_name.starts_with("Game 2 - "));

        let parsed = parse_backup_folder_name(&folder_name).expect("Failed to parse");
        assert_eq!(parsed.game_number, game_number);
        // Allow for some second precision loss if any, but string format is second precise
        assert_eq!(parsed.timestamp.timestamp(), dt.timestamp());
    }
}
