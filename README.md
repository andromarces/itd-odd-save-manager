# ITD ODD Save Manager

Portable auto‑save backup app for _Into the Dead: Our Darkest Days_.

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

1. **Save Path**: The app attempts to auto-detect your save path on Windows. If not found, enter the path manually in the "Active Configuration" section.
2. **Game Settings**:
   - Check **Auto-launch game** to start the game immediately when opening this manager.
   - Check **Close app when game exits** to shut down the manager automatically after you finish playing.

### Backups & Restore

- **Backups** are created automatically whenever the game saves.
- To **Restore**, click the "Restore" button next to a backup entry.
  - _Warning_: Restoring overwrites your current save.

### System Tray

- Closing the window minimizes the app to the System Tray.
- Right-click the tray icon to access the menu.
- Use "Quit" from the tray menu to fully exit the application.

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
- Run `npm run build` to build the frontend only.
- Run `npx tauri build` to create the production executable and installers.
- The compiled executable will be located at:
  - `src-tauri/target/release/app.exe` (Windows)
  - This file is portable and can be moved and run from anywhere.
- Installers will be located at:
  - `src-tauri/target/release/bundle/msi/ITD ODD Save Manager_0.1.0_x64_en-US.msi`
  - `src-tauri/target/release/bundle/nsis/ITD ODD Save Manager_0.1.0_x64-setup.exe`

## License & Attribution

This project is licensed under the MIT License with an additional attribution requirement.

**Attribution**:
"ITD ODD Save Manager by andromarces"
