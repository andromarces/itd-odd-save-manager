# ITD ODD Save Manager

Auto-save backup app for _Into the Dead: Our Darkest Days_.

## Features

- **Auto-Detection**: Automatically finds the local save folder on Windows at `%USERPROFILE%\AppData\LocalLow\PikPok\IntoTheDeadOurDarkestDays`.
- **Manual Folder Selection**: Allows selecting a custom folder to watch.
- **Automated Backups**: Watches for changes and creates timestamped backups automatically.
- **Smart Restore**: browse and restore backups with easy confirmation.
- **Backup Retention**: Limits the number of backups kept per game slot. The default is 100, and 0 means unlimited. The limit is set in the Settings tab. When the limit is reached, the oldest unlocked backups of that slot are deleted automatically.
- **Locked Backups**: A locked backup is exempt from automatic limit enforcement and is skipped by batch deletion unless locked backups are explicitly included.
- **Backup Notes**: A free-text note can be attached to each backup.
- **Master Delete**: Batch deletion of backups by game slot. Options delete all backups or all except the latest, and include or exclude locked backups.
- **Game Integration**:
  - Launch the game directly from the app.
  - **Auto-Launch**: Optional setting to launch the game when the app starts.
  - **Auto-Close**: Optional setting to close the app automatically when the game exits.
- **System Tray**:
  - Minimizes to system tray when closed.
  - Quick actions (Open, Launch Game, Quit) from the tray icon.
  - Monitors game status in the background.

## Usage

### Configuration

1. **Save Path**: The application attempts to auto-detect the save path on Windows. If not detected, the path is set manually in the "Active Configuration" section.
2. **Game Settings**:
   - The **Auto-launch game** option starts the game immediately upon application launch.
   - The **Close app when game exits** option shuts down the manager automatically after game termination.

### Steam Launch Options

1. In Steam, open the game's Properties.
2. In the Launch Options field, set the command below so the save manager starts when the game starts:
   - `"C:\Path\To\ITD ODD Save Manager\ITD ODD Save Manager.exe" %command%`
   - Replace the path with the actual location of the installed executable.

### Steam Cloud Saves

If no existing save is detected in the local folder, disable Steam Cloud for this game and copy the Steam Cloud save files to the local folder before launching the game.

1. Close the game and the save manager.
2. Check the local save folder at `C:\Users\<YourUserName>\AppData\LocalLow\PikPok\IntoTheDeadOurDarkestDays` (example pattern: `%USERPROFILE%\AppData\LocalLow\PikPok\IntoTheDeadOurDarkestDays`). If the folder has no save files, turn off Steam Cloud for this game in Steam.
3. Copy the Steam Cloud save files from `C:\Program Files (x86)\Steam\userdata\<SteamID>\2239710\remote` to `C:\Users\<YourUserName>\AppData\LocalLow\PikPok\IntoTheDeadOurDarkestDays`.
   - Expected file names to copy: `gamesave_0.sav`, `gamesave_0.sav.bak`, `gamesave_1.sav`, `gamesave_1.sav.bak`, `gamesave_2.sav`, `gamesave_2.sav.bak`.
4. Launch the game once to confirm the save loads locally.

### Backups & Restore

- **Backups** are created automatically whenever the game saves.
- **Restoration** is performed by selecting the "Restore" button next to a backup entry.
  - _Warning_: Restoring overwrites the current save.

### System Tray

- Closing the window minimizes the app to the System Tray.
- The tray icon menu provides access to actions.
- The "Quit" action fully exits the application.

## Supported Platforms

Windows only.

## Development

### Prerequisites

- Rust: minimum supported version 1.95.0. The build toolchain is pinned to 1.99.0 in `rust-toolchain.toml`, and `rustup` installs it automatically.
- Node.js 26 (CI and release builds; the version is set in `.nvmrc`). Local development also works on Node.js 24.15.0 or a later 24.x release. Node.js 25 is not supported.
  - `nvm-windows` users (checked on 2.0.1-hotfix.1): plain `nvm use` does not read `.nvmrc`. Run `nvm install 26` and `nvm use 26`. The [nvm-windows documentation](https://docs.nvm-windows.com/features/modes) states that shim mode (`nvm use shim`) selects the version from `.nvmrc` automatically. That behavior is not tested, and `nvm use shim` changes the global operating mode of the machine.
- pnpm 12.10.1, the exact version pinned in the `packageManager` field of `package.json`. Install it with the [standalone installer](https://pnpm.io/installation). Corepack is not used.

### Setup

1. Clone the repository.
2. Run `pnpm install --frozen-lockfile` to install dependencies from `pnpm-lock.yaml` and configure git hooks. The `virtualStoreType: global` setting in `pnpm-workspace.yaml` keeps one shared copy of each dependency in the pnpm store.
   - This project uses a `pre-commit` hook (format-only) and a `pre-push` hook (diff-scoped, fail-fast policy enforcement). See [BUILDING.md](BUILDING.md) for details, including the steps to migrate an existing clone from the previous Husky hooks.

### Build

For detailed build instructions and reproducibility steps, please refer to [BUILDING.md](BUILDING.md).

- `pnpm run dev` starts the development server.
- `pnpm run test:run` runs the frontend and `scripts/` tests.
- `pnpm run lint` lints the repository with `oxlint`.
- `pnpm run format` formats tracked files with `oxfmt` and `rustfmt`.
- `pnpm run tauri build` creates the production executable and installers.

## Verification

Integrity verification steps:

1. The `SHA256SUMS.txt` file is available on the [GitHub Release](https://github.com/andromarces/itd-odd-save-manager/releases).
2. The SHA256 hash of the downloaded installer or zip file is calculated.
3. The calculated hash is compared with the one in the text file.

## Privacy & Security

- **Local-First**: All save data and backups are stored locally on the local machine.
- **No Telemetry**: This application does not collect or transmit user data.
- **Network**: The application is offline-capable.

## License & Attribution

This project is licensed under the MIT License with an additional attribution requirement.

**Attribution**:
"ITD ODD Save Manager by andromarces"
