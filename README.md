# ITD ODD Save Manager

Portable auto‑save backup app for _Into the Dead: Our Darkest Days_.

## Features

- **Auto-Detection**: Automatically finds Steam save locations.
- **Manual Override**: Allows selecting any custom folder or file to watch.
- **Automated Backups**: Watches for changes and creates timestamped backups instantly.
- **Smart Restore**: browse and restore backups with a single click.
- **Steam Cloud Awareness**: Detects and warns about Steam Cloud conflicts.
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

1. **Save Path**: The app attempts to auto-detect your save path. If incorrect, enter the path manually in the "Active Configuration" section.
2. **Game Settings**:
   - Check **Auto-launch game** to start the game immediately when opening this manager.
   - Check **Close app when game exits** to shut down the manager automatically after you finish playing.

### Backups & Restore

- **Backups** are created automatically whenever the game saves.
- To **Restore**, click the "Restore" button next to a backup entry.
  - _Warning_: Restoring overwrites your current save.
  - If using Steam Cloud, you may need to disable it or go offline to prevent Steam from reverting your restore.

### System Tray

- Closing the window minimizes the app to the System Tray.
- Right-click the tray icon to access the menu.
- Use "Quit" from the tray menu to fully exit the application.

## Steam Cloud Support

The application automatically detects if your save files are located within the Steam Cloud synchronization folder (`userdata`).

- **Detection**: If a Steam Cloud path is detected, the app will flag it internally.
- **Restoring**: When attempting to restore a backup to a Steam Cloud location, you will receive a warning.
- **Conflict**: Steam Cloud may overwrite your restored file with its own cloud copy upon game launch. To prevent this, consider:
  - Disabling Steam Cloud for this game.
  - Launching Steam in Offline Mode.

## Development

### Prerequisites

- Rust (latest stable)
- Node.js & npm

### Setup

1. Clone the repository.
2. Run `npm install` to install dependencies and configure git hooks.
   - This project uses a `pre-commit` hook to enforce Rust formatting.

### Build

- Run `npm run dev` for development.
- Run `npm run build` to create the production executable.
- The compiled executable will be located in:
  - `src-tauri/target/release/ITD ODD Save Manager.exe` (Windows)
  - This file is portable and can be moved/run from anywhere.
