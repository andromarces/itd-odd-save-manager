#[cfg(test)]
mod tests {
    use crate::backup::cleanup::delete_backups_batch;
    use crate::backup::common::BACKUP_DIR_NAME;
    use crate::backup::create::perform_backup_for_game;
    use crate::backup::data::{build_save_paths, BackupInfo};
    use crate::backup::index::BackupStore;
    use crate::backup::listing::{backup_info_from_folder, get_backups};
    use crate::backup::notes::{set_backup_lock, set_backup_note};
    use crate::backup::restore::restore_backup;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    /// Tests that save paths are constructed consistently for a game slot.
    #[test]
    fn test_build_save_paths() {
        let dir = tempdir().unwrap();
        let paths = build_save_paths(dir.path(), 2);

        assert_eq!(paths.main_filename, "gamesave_2.sav");
        assert_eq!(paths.bak_filename, "gamesave_2.sav.bak");
        assert_eq!(paths.main_path, dir.path().join("gamesave_2.sav"));
        assert_eq!(paths.bak_path, dir.path().join("gamesave_2.sav.bak"));
    }

    /// Tests that invalid backup folder names are ignored during listing.
    #[test]
    fn test_backup_info_from_folder_ignores_invalid_name() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let backup_root = save_dir.join(BACKUP_DIR_NAME);
        fs::create_dir_all(&backup_root).unwrap();

        let invalid_folder = backup_root.join("not-a-backup");
        fs::create_dir_all(&invalid_folder).unwrap();

        let result =
            backup_info_from_folder(&invalid_folder, "not-a-backup", save_dir, true).unwrap();
        assert!(result.is_none());
    }

    /// Tests the complete backup flow including folder creation, deduplication, and file copying.
    #[test]
    fn test_backup_flow() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1; // Internal 1 -> Display "Game 2"

        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "save data").unwrap();
        }

        // 1. Perform backup
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result.is_some());
        let backup_folder = result.unwrap();

        assert!(backup_folder.exists());
        // Verify naming: "Game 2 - ..."
        let folder_name = backup_folder.file_name().unwrap().to_string_lossy();
        assert!(folder_name.starts_with("Game 2 -"));

        assert!(backup_folder.join("gamesave_1.sav").exists());
        assert!(backup_folder.join(".hash").exists());

        // Verify index was created
        let index_path = save_dir.join(".backups").join("index.json");
        assert!(index_path.exists());

        // 2. Perform duplicate backup (should skip)
        let result_dup = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result_dup.is_none());

        // 3. Modify save and backup (should succeed)
        // Sleep to ensure timestamp changes (folder name resolution is seconds)
        std::thread::sleep(std::time::Duration::from_secs(2));
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "new data").unwrap();
        }
        let result_new = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result_new.is_some());
        assert_ne!(result_new.unwrap(), backup_folder); // Different timestamp folder

        // 4. List backups
        let backups = get_backups(save_dir, true).unwrap();
        assert_eq!(backups.len(), 2);
        assert_eq!(backups[0].game_number, 1);
        // Verify folder name logic in listing
        assert!(backups[0].filename.starts_with("Game 2 -"));
    }

    /// Tests that backup is skipped if only the .bak file exists.
    #[test]
    fn test_skip_if_only_bak() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 0;

        let bak_sav = save_dir.join("gamesave_0.sav.bak");
        File::create(&bak_sav).unwrap();

        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result.is_none());
    }

    /// Tests the restore functionality.
    #[test]
    fn test_restore() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 2;

        let main_sav = save_dir.join("gamesave_2.sav");
        let bak_sav = save_dir.join("gamesave_2.sav.bak");

        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "original").unwrap();
            let mut f2 = File::create(&bak_sav).unwrap();
            writeln!(f2, "original bak").unwrap();
        }

        // Backup
        let backup_folder = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();

        // Modify
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "corrupted").unwrap();
        }

        // Restore
        restore_backup(&backup_folder, save_dir).unwrap();

        let content = fs::read_to_string(&main_sav).unwrap();
        assert_eq!(content.trim(), "original");

        let content_bak = fs::read_to_string(&bak_sav).unwrap();
        assert_eq!(content_bak.trim(), "original bak");
    }

    /// Tests that the hash file contains only the hex digest.
    #[test]
    fn test_hash_content() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 5;
        let main_sav = save_dir.join("gamesave_5.sav");

        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "hash test").unwrap();
        }

        let backup_folder = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();
        let hash_file = backup_folder.join(".hash");

        let content = fs::read_to_string(hash_file).unwrap();
        assert!(content.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!content.contains("Sha256"));
    }

    /// Tests robustness against missing backup folders when index exists.
    #[test]
    fn test_missing_backup_folder_recovery() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1;

        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "important data").unwrap();
        }

        // 1. Create initial backup
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        let backup_folder = result.unwrap();

        // 2. Simulate user deleting the backup folder manually, but index remains
        fs::remove_dir_all(&backup_folder).unwrap();

        // 3. Perform backup again - should detect missing folder and recreate
        let result_retry = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(result_retry.is_some());
        let new_backup_folder = result_retry.unwrap();

        assert!(new_backup_folder.exists());
        let restored_sav = new_backup_folder.join("gamesave_1.sav");
        assert!(restored_sav.exists());
        assert!(new_backup_folder.join(".hash").exists());

        // Verify content matches original
        let content = fs::read_to_string(restored_sav).unwrap();
        assert_eq!(content.trim(), "important data");
    }

    /// Tests that the backup limit is enforced.
    #[test]
    fn test_backup_limit_enforcement() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 3;
        let main_sav = save_dir.join("gamesave_3.sav");

        // Limit to 2 backups
        let limit = 2;

        for i in 0..4 {
            {
                let mut f = File::create(&main_sav).unwrap();
                writeln!(f, "data {}", i).unwrap();
            }
            // Sleep to ensure distinct timestamps
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            perform_backup_for_game(save_dir, game_number, limit).unwrap();
        }

        let backups = get_backups(save_dir, true).unwrap();
        assert_eq!(backups.len(), 2);

        // Helper to check content of a backup
        let check_content = |backup: &BackupInfo, expected: &str| {
            let path = PathBuf::from(&backup.path).join("gamesave_3.sav");
            let content = fs::read_to_string(path).unwrap();
            assert_eq!(content.trim(), expected);
        };

        // backups[0] is newest
        check_content(&backups[0], "data 3");
        check_content(&backups[1], "data 2");
    }

    /// Tests that locked backups are excluded from the limit count and not deleted.
    #[test]
    fn test_backup_lock_enforcement() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 4;
        let main_sav = save_dir.join("gamesave_4.sav");
        let limit = 2;

        // 1. Create first backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data 1").unwrap();
        }
        let backup1_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 2. Lock the first backup
        set_backup_lock(&backup1_path, true).unwrap();

        // 3. Create second backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data 2").unwrap();
        }
        let _backup2_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 4. Create third backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data 3").unwrap();
        }
        let _backup3_path = perform_backup_for_game(save_dir, game_number, limit)
            .unwrap()
            .unwrap();

        // Check backups
        let backups = get_backups(save_dir, true).unwrap();
        assert_eq!(backups.len(), 3);

        // Verify content
        let check_content = |backup: &BackupInfo, expected: &str| {
            let path = PathBuf::from(&backup.path).join("gamesave_4.sav");
            let content = fs::read_to_string(path).unwrap();
            assert_eq!(content.trim(), expected);
        };

        // Sorted by modified desc: data 3, data 2, data 1
        check_content(&backups[0], "data 3");
        check_content(&backups[1], "data 2");
        check_content(&backups[2], "data 1");
        assert!(backups[2].locked);

        // 5. Create fourth backup -> limit 2 exceeded for unlocked?
        std::thread::sleep(std::time::Duration::from_secs(2));
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data 4").unwrap();
        }
        perform_backup_for_game(save_dir, game_number, limit).unwrap();

        let backups_final = get_backups(save_dir, true).unwrap();
        assert_eq!(backups_final.len(), 3);

        check_content(&backups_final[0], "data 4");
        check_content(&backups_final[1], "data 3");
        check_content(&backups_final[2], "data 1");
    }

    /// Tests that restoring an older backup does not result in a new duplicate backup.
    #[test]
    fn test_no_duplicate_after_restore() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 0;
        let main_sav = save_dir.join("gamesave_0.sav");

        // 1. Create first backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "version 1").unwrap();
        }
        let backup1_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();

        std::thread::sleep(std::time::Duration::from_secs(2));

        // 2. Modify and create second backup
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "version 2").unwrap();
        }
        let _backup2_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();

        // 3. Restore first backup
        restore_backup(&backup1_path, save_dir).unwrap();

        // 4. Try to backup again - it should be skipped because it matches backup 1
        let result = perform_backup_for_game(save_dir, game_number, 100).unwrap();
        assert!(
            result.is_none(),
            "Backup should have been skipped as it matches an existing backup (v1)"
        );

        // 5. Verify index was updated to point to backup 1
        let store = BackupStore::new(save_dir).unwrap();
        let entry = store.index.games.get(&game_number).unwrap();
        assert_eq!(
            entry.last_backup_path,
            backup1_path.file_name().unwrap().to_string_lossy()
        );
    }

    /// Tests note persistence and management.
    #[test]
    fn test_note_persistence() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1;

        // 1. Create a backup
        let main_sav = save_dir.join("gamesave_1.sav");
        {
            let mut f = File::create(&main_sav).unwrap();
            writeln!(f, "data").unwrap();
        }
        let backup_path = perform_backup_for_game(save_dir, game_number, 100)
            .unwrap()
            .unwrap();
        let folder_name = backup_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        // 2. Set a note
        set_backup_note(save_dir, &folder_name, Some("My Note".to_string())).unwrap();

        // 3. Verify in index
        let store = BackupStore::new(save_dir).unwrap();
        assert_eq!(store.index.notes.get(&folder_name).unwrap(), "My Note");

        // 4. Update note
        set_backup_note(save_dir, &folder_name, Some("Updated Note".to_string())).unwrap();
        let store2 = BackupStore::new(save_dir).unwrap();
        assert_eq!(
            store2.index.notes.get(&folder_name).unwrap(),
            "Updated Note"
        );

        // 5. Verify get_backups retrieves it
        let backups = get_backups(save_dir, true).unwrap();
        assert_eq!(backups[0].note.as_deref(), Some("Updated Note"));

        // 6. Remove note
        set_backup_note(save_dir, &folder_name, None).unwrap();
        let store3 = BackupStore::new(save_dir).unwrap();
        assert!(!store3.index.notes.contains_key(&folder_name));
    }

    /// Tests batch deletion logic.
    #[test]
    fn test_batch_delete() {
        let dir = tempdir().unwrap();
        let save_dir = dir.path();
        let game_number = 1;
        let main_sav = save_dir.join("gamesave_1.sav");

        // Helper to create a backup
        let create_backup = |content: &str, locked: bool| -> String {
            {
                let mut f = File::create(&main_sav).unwrap();
                writeln!(f, "{}", content).unwrap();
            }
            // Sleep to ensure unique timestamps
            std::thread::sleep(std::time::Duration::from_secs(2));
            let path = perform_backup_for_game(save_dir, game_number, 100)
                .unwrap()
                .unwrap();
            if locked {
                set_backup_lock(&path, true).unwrap();
            }
            path.file_name().unwrap().to_string_lossy().to_string()
        };

        create_backup("v1", false); // Oldest
        create_backup("v2", true); // Locked
        create_backup("v3", false);
        create_backup("v4", false); // Newest

        let backups = get_backups(save_dir, true).unwrap();
        assert_eq!(backups.len(), 4);

        // Scenario 1: Delete all but latest, EXCLUDE locked.
        let deleted = delete_backups_batch(save_dir, &[game_number], true, false).unwrap();
        assert_eq!(deleted, 2, "Should delete v1 and v3");

        let remaining = get_backups(save_dir, true).unwrap();
        assert_eq!(remaining.len(), 2);

        // Scenario 2: Delete ALL, INCLUDE locked.
        let deleted_2 = delete_backups_batch(save_dir, &[game_number], false, true).unwrap();
        assert_eq!(deleted_2, 2);

        let final_backups = get_backups(save_dir, true).unwrap();
        assert!(final_backups.is_empty());
    }
}
