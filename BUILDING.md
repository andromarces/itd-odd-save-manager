# Building from Source

This document outlines how to build the ITD ODD Save Manager from source. The project strives for reproducible builds to ensure trust and security.

## Prerequisites

### General

- **Git**: For version control.
- **Node.js**: Version 20 (LTS).
- **Rust**: Version 1.93.0 (managed via `rust-toolchain.toml`).

### Windows

- **Microsoft Visual Studio C++ Build Tools**: Required for compiling Rust artifacts on Windows.

## Build Steps

1. **Clone the Repository**

   ```bash
   git clone https://github.com/andromarces/itd-odd-save-manager.git
   cd itd-odd-save-manager
   ```

2. **Install Frontend Dependencies**

   ```bash
   npm ci
   ```

   _Note: `npm ci` is recommended over `npm install` for reproducible environments as it respects `package-lock.json` strictly. Running `npm ci` also invokes the `prepare` lifecycle script, which installs the Husky git hooks automatically._

## Git Hooks

Git hooks are managed by [Husky](https://typicode.github.io/husky) and run automatically after `npm ci`.

### pre-commit

Runs [lint-staged](https://github.com/lint-staged/lint-staged) against staged files only:

- **JS/TS files** (`src/**/*.{ts,js}`): formatted with `oxfmt`.
- **Rust files** (`src-tauri/**/*.rs`): formatted with `rustfmt --edition 2021`.

### pre-push

Runs against files changed between the local branch and its upstream (`@{upstream}...HEAD`):

- **JS/TS files** (`src/**/*.{ts,js}`): linted with `oxlint`.
- **Rust files**: `cargo clippy -- -D warnings` runs in `src-tauri/` when at least one `.rs` file changed.

When no upstream branch is configured (e.g., on the first push of a new branch), the hook uses `git merge-base HEAD origin/main` (or `origin/master`) as the base, so only the branch's own commits are linted. If neither remote default branch is reachable, pre-push linting is skipped entirely.

3. **Build Frontend**

   ```bash
   npm run build
   ```

4. **Build Application**
   To build the release artifacts (installer and executable):

   ```bash
   npm run tauri build
   ```

   - The artifacts will be located in `src-tauri/target/release/bundle/`.
   - The raw executable is at `src-tauri/target/release/ITD ODD Save Manager.exe`.

## Verifying Reproducibility

To verify that the build matches the official release:

1. Check out the specific tag (e.g., `git checkout v1.0.0`).
2. Build using the steps above.
3. Compare the SHA256 hash of `src-tauri/target/release/ITD ODD Save Manager.exe` with the `SHA256SUMS.txt` provided in the GitHub Release.

_Note: Minor binary differences may occur due to timestamps or path variations, but are minimized via deterministic build flags in the CI._
