pub mod cleanup;
pub mod common;
pub mod create;
pub mod data;
pub mod hashing;
pub mod index;
pub mod listing;
pub mod notes;
pub mod restore;

#[cfg(test)]
mod tests;

// Re-export public API to maintain compatibility or ease of use
pub use cleanup::{delete_backup_folder, delete_backups_batch};
pub use data::BackupInfo;
pub use listing::get_backups;
pub use notes::{set_backup_lock, set_backup_note};
pub use restore::restore_backup;

// Internal exports needed for other modules
#[allow(unused_imports)]
pub(crate) use create::perform_backup_for_game;
pub(crate) use create::perform_backup_for_game_internal;
pub(crate) use index::{ensure_backup_root, load_index, save_index};
