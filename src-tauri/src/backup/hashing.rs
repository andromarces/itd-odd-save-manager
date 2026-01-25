use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::Path;

/// Calculates the SHA-256 hash of a file.
pub(crate) fn calculate_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}
