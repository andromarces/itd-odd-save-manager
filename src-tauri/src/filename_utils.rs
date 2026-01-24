use std::path::Path;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
