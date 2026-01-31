# ITD ODD Save Manager

Portable auto-save backup app for _Into the Dead: Our Darkest Days_.

## Features

- **Auto-Detection**: Automatically finds the local save folder on Windows at `%USERPROFILE%\AppData\LocalLow\PikPok\IntoTheDeadOurDarkestDays`.
- **Manual Override**: Allows selecting any custom folder or file to watch.
- **Automated Backups**: Watches for changes and creates timestamped backups instantly.
- **Smart Restore**: browse and restore backups with a single click.
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

### Backups & Restore

- **Backups** are created automatically whenever the game saves.
- **Restoration** is performed by selecting the "Restore" button next to a backup entry.
  - _Warning_: Restoring overwrites the current save.

### System Tray

- Closing the window minimizes the app to the System Tray.
- The tray icon menu provides access to actions.
- The "Quit" action fully exits the application.

## Development

### Prerequisites

- Rust 1.93.0
- Node.js 20 (LTS) & npm

### Setup

1. Clone the repository.
2. Run `npm install` to install dependencies and configure git hooks.
   - This project uses a `pre-commit` hook to enforce Rust formatting.

### Build

For detailed build instructions and reproducibility steps, please refer to [BUILDING.md](BUILDING.md).

- `npm run dev` starts the development server.
- `npm run tauri build` creates the production executable and installers.

## Verification

Integrity verification steps:
1.  The `SHA256SUMS.txt` file is available on the [GitHub Release](https://github.com/andromarces/itd-odd-save-manager/releases).
2.  The SHA256 hash of the downloaded installer or zip file is calculated.
3.  The calculated hash is compared with the one in the text file.

## Privacy & Security

*   **Local-First**: All save data and backups are stored locally on the local machine.
*   **No Telemetry**: This application does not collect or transmit user data.
*   **Network**: The application is offline-capable.

## License & Attribution

This project is licensed under the MIT License with an additional attribution requirement.

**Attribution**:
"ITD ODD Save Manager by andromarces"
