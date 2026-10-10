# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Pre-push enforcement script with diff-scoped, fail-fast checks (forbidden paths, gitignore sanity, secret heuristic, format, lint, doc comments, clippy).
- Pre-push check that rejects file-wide lint suppressions.
- `noscan` marker to suppress false-positive secret detection in the pre-push script.
- Husky and lint-staged git hooks, replacing manual git hooks.
- `oxfmt` formatting for non-Rust files.
- `ci-success` gate job as a stable required CI check.
- Dependabot configuration with package groups and Cargo support.
- Handling of stale game-running state when the auto-close setting is disabled.
- Clamping of the maximum backups setting to 0 or greater, with the normalized value shown in the input.
- SHA-256 hex validation test for backup file hashing.
- `.gitattributes` file.

### Changed

- Save path validation is consolidated into the backend `set_save_path` command, and the `validate_path` command is removed.
- Backup index persistence prunes entries for deleted backups.
- Batch backup logic in the watcher fetches backups per game number.
- Backup list loading deduplicates requests and limits concurrency.
- Backup row lookup uses a row map.
- Watcher reliability and state consistency are improved.
- Backup file hashing is improved.
- `main.test.ts` is refactored to import domain-specific UI modules.
- Linux support is removed. The application targets Windows only.
- CI skips tests and build for Dependabot pull requests.
- Dependencies are updated through Dependabot.

### Fixed

- Row integrity regression after a failed backup list load.
- False-positive suppression detection for regex literals and prose comments in the pre-push script.
- `oxfmt` wrapper script tolerates paths that cannot be formatted.
- `rand` is updated to 0.9.3 and 0.8.6 in `Cargo.lock`.

### Removed

- Unused `save_config` function and the Vite icon reference.

## [1.0.0] - 2026-01-31

### Added

- Initial project structure.
- Basic save file monitoring and backup functionality.
- UI for listing and restoring backups.
- CI/CD pipeline for automated testing and releases.
- Documentation for building and security.
