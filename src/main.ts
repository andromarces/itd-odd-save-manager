// ITD ODD Save Manager by andromarces

import { invoke } from '@tauri-apps/api/core';
import './style.css';

interface AppConfig {
  save_path: string | null;
  auto_launch_game: boolean;
  auto_close: boolean;
}

interface BackupInfo {
  path: string;
  filename: string;
  original_filename: string;
  original_path: string;
  size: number;
  modified: string;
  game_number: number;
}

/**
 * Helper to safely query DOM elements. Throws if not found.
 */
function getElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Element not found: ${selector}`);
  }
  return element;
}

const app = getElement<HTMLDivElement>('#app');

app.innerHTML = `
  <main class="layout">
    <header class="header">
      <p class="kicker">Steam on Windows</p>
      <h1>ITD ODD Save Manager</h1>
      <p class="subhead">
        Manage your save files for Into the Dead: Our Darkest Days.
      </p>
    </header>

    <section class="panel">
      <h2>Game Launcher</h2>
      <div class="actions">
        <button id="launch-game" type="button" class="primary">Launch Game</button>
      </div>
      <div class="checkbox-group">
        <label class="checkbox-label">
          <input type="checkbox" id="auto-launch-check" />
          Auto-launch game when app starts
        </label>
        <label class="checkbox-label">
          <input type="checkbox" id="auto-close-check" />
          Close app when game exits
        </label>
      </div>
    </section>

    <section class="panel">
      <h2>Active Configuration</h2>
      <div class="input-group">
        <input type="text" id="manual-path" placeholder="C:\\Path\\To\\Save\\Folder" spellcheck="false" />
        <button id="save-config" type="button">Set Path</button>
      </div>
      <p id="config-status" class="status-text"></p>
    </section>

    <section class="panel">
      <h2>Backups</h2>
      <div class="actions">
        <button id="refresh-backups" type="button" disabled>Refresh Backups</button>
      </div>
      <div class="table-container">
        <table id="backups-table">
          <thead>
            <tr>
              <th>File</th>
              <th>Date</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody id="backups-list">
            <tr><td colspan="3" class="empty">No backups found.</td></tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="panel">
      <h2>Activity Log</h2>
      <div id="activity-log" class="log-box"></div>
    </section>

    <section class="panel">
      <h2>Discovery</h2>
      <div class="actions">
        <button id="detect" type="button">Auto Detect Steam Paths</button>
      </div>
      <ul id="paths" class="paths" aria-live="polite"></ul>
    </section>
  </main>
`;

// Elements
const detectButton = getElement<HTMLButtonElement>('#detect');
const pathsList = getElement<HTMLUListElement>('#paths');
const manualInput = getElement<HTMLInputElement>('#manual-path');
const saveButton = getElement<HTMLButtonElement>('#save-config');
const configStatus = getElement<HTMLParagraphElement>('#config-status');
const refreshBackupsButton = getElement<HTMLButtonElement>('#refresh-backups');
const backupsList = getElement<HTMLTableSectionElement>('#backups-list');
const launchGameButton = getElement<HTMLButtonElement>('#launch-game');
const autoLaunchCheck = getElement<HTMLInputElement>('#auto-launch-check');
const autoCloseCheck = getElement<HTMLInputElement>('#auto-close-check');
const logBox = getElement<HTMLDivElement>('#activity-log');

const MAX_LOG_ENTRIES = 100;

// State
let currentBackups: BackupInfo[] = [];

/**
 * Appends a message to the activity log with a timestamp.
 */
function logActivity(message: string): void {
  const entry = document.createElement('div');
  entry.className = 'log-entry';

  const time = document.createElement('span');
  time.className = 'time';
  time.textContent = new Date().toLocaleTimeString();

  entry.appendChild(time);
  entry.appendChild(document.createTextNode(message));

  logBox.appendChild(entry);

  // Cap the log size
  while (logBox.childElementCount > MAX_LOG_ENTRIES) {
    logBox.firstElementChild?.remove();
  }

  logBox.scrollTop = logBox.scrollHeight;
}

/**
 * Updates the configuration status message.
 */
function setConfigStatus(
  message: string,
  type: 'info' | 'success' | 'error' = 'info',
): void {
  configStatus.textContent = message;
  configStatus.className = 'status-text';
  if (type !== 'info') {
    configStatus.classList.add(type);
  }
}

/**
 * Renders the detected save paths into the list element.
 */
function renderPaths(paths: string[]): void {
  pathsList.innerHTML = '';

  if (paths.length === 0) {
    const item = document.createElement('li');
    item.textContent = 'No Steam save paths detected.';
    item.classList.add('empty');
    pathsList.appendChild(item);
    return;
  }

  for (const path of paths) {
    const item = document.createElement('li');
    item.textContent = path;
    item.title = 'Click to use this path';
    pathsList.appendChild(item);
  }
}

/**
 * Formats a date string for display.
 */
function formatDate(isoString: string): string {
  try {
    const date = new Date(isoString);
    return date.toLocaleString();
  } catch {
    return isoString;
  }
}

/**
 * Renders the list of backups.
 */
function renderBackups(backups: BackupInfo[]): void {
  backupsList.innerHTML = '';

  if (backups.length === 0) {
    backupsList.innerHTML =
      '<tr><td colspan="3" class="empty">No backups found.</td></tr>';
    return;
  }

  backups.forEach((backup, index) => {
    const row = document.createElement('tr');

    const fileCell = document.createElement('td');
    fileCell.textContent = backup.original_filename;
    fileCell.title = backup.filename;

    const dateCell = document.createElement('td');
    dateCell.textContent = formatDate(backup.modified);

    const actionCell = document.createElement('td');
    const restoreBtn = document.createElement('button');
    restoreBtn.textContent = 'Restore';
    restoreBtn.className = 'small';
    restoreBtn.dataset.index = index.toString();

    actionCell.appendChild(restoreBtn);

    row.appendChild(fileCell);
    row.appendChild(dateCell);
    row.appendChild(actionCell);

    backupsList.appendChild(row);
  });
}

/**
 * Helper to safely invoke Tauri commands with standardized logging and error handling.
 */
async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options: {
    actionName?: string;
    successLog?: string;
    successAlert?: string;
    alertOnError?: boolean;
    onError?: (error: unknown) => void;
  } = {},
): Promise<T | undefined> {
  const action = options.actionName || command;
  try {
    const data = await invoke<T>(command, args);

    if (options.successLog) {
      logActivity(options.successLog);
    }

    if (options.successAlert) {
      alert(options.successAlert);
    }

    return data;
  } catch (error) {
    const msg = `Failed to ${action}`;
    let errorStr = String(error);
    if (error instanceof Error) {
      errorStr = error.message;
    } else if (typeof error === 'object' && error !== null) {
      try {
        errorStr = JSON.stringify(error);
      } catch {
        // Fallback to default String conversion if stringify fails
      }
    }

    console.error(`${msg}:`, error);
    logActivity(`${msg}: ${errorStr}`);

    if (options.alertOnError) {
      alert(`${msg}: ${errorStr}`);
    }

    if (options.onError) {
      options.onError(error);
    }

    return undefined;
  }
}

/**
 * Loads backups from the backend.
 */
async function loadBackups(): Promise<void> {
  if (!manualInput.value) return;

  refreshBackupsButton.textContent = 'Refreshing...';
  refreshBackupsButton.disabled = true;

  const backups = await safeInvoke<BackupInfo[]>(
    'get_backups_command',
    undefined,
    {
      actionName: 'load backups',
      onError: () => {
        backupsList.innerHTML =
          '<tr><td colspan="3" class="error">Failed to load backups</td></tr>';
      },
    },
  );

  if (backups) {
    currentBackups = backups;
    renderBackups(backups);
    logActivity(`Loaded ${backups.length} backups.`);
  }

  refreshBackupsButton.textContent = 'Refresh Backups';
  refreshBackupsButton.disabled = false;
}

/**
 * Restores a backup.
 */
async function restoreBackup(backup: BackupInfo): Promise<void> {
  let message = `Are you sure you want to restore "${backup.original_filename}" (Game ${backup.game_number + 1}) from ${formatDate(backup.modified)}?`;
  message += `\nThis will overwrite the current save files for Game ${backup.game_number + 1}.`;

  const isCloud = await safeInvoke<boolean>(
    'check_steam_cloud_path',
    { path: backup.original_path },
    {
      actionName: 'check Steam Cloud status',
    },
  );

  if (isCloud) {
    message += `\n\nWARNING: Steam Cloud folder detected.\nSteam may overwrite this restore with its cloud copy unless you launch in Offline Mode or disable Steam Cloud.`;
  }

  const confirmed = window.confirm(message);
  if (!confirmed) return;

  await safeInvoke(
    'restore_backup_command',
    {
      backup_path: backup.path,
      target_path: backup.original_path,
    },
    {
      actionName: 'restore backup',
      successLog: `Restored backup: ${backup.filename}`,
      successAlert: 'Restore successful!',
      alertOnError: true,
    },
  );
}

/**
 * Saves game settings (auto launch/close).
 */
async function saveGameSettings(): Promise<void> {
  const autoLaunch = autoLaunchCheck.checked;
  const autoClose = autoCloseCheck.checked;

  await safeInvoke(
    'set_game_settings',
    {
      auto_launch_game: autoLaunch,
      auto_close: autoClose,
    },
    {
      actionName: 'save game settings',
      successLog: `Updated game settings: Auto-Launch=${autoLaunch}, Auto-Close=${autoClose}`,
    },
  );
}

/**
 * Launches the game.
 */
async function launchGame(): Promise<void> {
  logActivity('Launching game...');
  await safeInvoke('launch_game', undefined, {
    actionName: 'launch game',
    successLog: 'Game launch command sent.',
    alertOnError: true,
  });
}

/**
 * Loads the current configuration from the backend.
 */
async function loadConfig(): Promise<void> {
  const config = await safeInvoke<AppConfig>('get_config', undefined, {
    actionName: 'load config',
    onError: () => setConfigStatus('Failed to load configuration.', 'error'),
  });

  if (config) {
    if (config.save_path) {
      manualInput.value = config.save_path;
      setConfigStatus('Configuration loaded.', 'info');
      void loadBackups();
    } else {
      manualInput.value = '';
      setConfigStatus('No save path configured.', 'info');
    }

    // Set checkboxes
    autoLaunchCheck.checked = config.auto_launch_game;
    autoCloseCheck.checked = config.auto_close;

    logActivity('Configuration loaded.');
  }
}

/**
 * Validates and saves the user-provided path.
 */
async function savePath(): Promise<void> {
  const path = manualInput.value.trim();

  if (!path) {
    setConfigStatus('Please enter a path.', 'error');
    return;
  }

  setConfigStatus('Validating...', 'info');
  saveButton.disabled = true;

  try {
    const isValid = await safeInvoke<boolean>(
      'validate_path',
      { path },
      {
        actionName: 'validate path',
        onError: () => setConfigStatus('Error validating path.', 'error'),
      },
    );

    if (isValid === undefined) return; // Error occurred

    if (!isValid) {
      setConfigStatus('Path does not exist or is invalid.', 'error');
      logActivity(`Invalid path entered: ${path}`);
      return;
    }

    const normalizedPath = await safeInvoke<string>(
      'set_save_path',
      { path },
      {
        actionName: 'save path',
        onError: () => {
          setConfigStatus('Error saving path.', 'error');
          // Reload config to reflect potential clearing of save_path by backend (auto-backup disabled)
          void loadConfig();
        },
      },
    );

    if (normalizedPath) {
      manualInput.value = normalizedPath;
      setConfigStatus('Save path updated successfully.', 'success');
      logActivity(`Save path updated: ${normalizedPath}`);
      void loadBackups();
    }
  } finally {
    saveButton.disabled = false;
  }
}

/**
 * Calls the backend command to detect Steam save paths.
 */
async function detectSteamSavePaths(): Promise<void> {
  detectButton.disabled = true;
  detectButton.textContent = 'Scanning...';
  logActivity('Scanning for save paths...');

  const paths = await safeInvoke<string[]>(
    'detect_steam_save_paths',
    undefined,
    {
      actionName: 'detect steam save paths',
      onError: () => {
        pathsList.innerHTML = '<li class="error">Detection failed</li>';
      },
    },
  );

  if (paths) {
    renderPaths(paths);
    if (paths.length > 0) {
      logActivity(`Detected ${paths.length} potential paths.`);
      if (!manualInput.value) {
        manualInput.value = paths[0];
        setConfigStatus('Path detected. Click "Set Path" to save.', 'info');
      }
    } else {
      logActivity('No paths detected.');
    }
  }

  detectButton.disabled = false;
  detectButton.textContent = 'Auto Detect Steam Paths';
}

// Event Listeners

/**
 * Handler for the "Auto Detect Steam Paths" button click.
 * Triggers the backend detection logic and updates the UI.
 */
const onDetectClick = (): void => {
  void detectSteamSavePaths();
};
detectButton.addEventListener('click', onDetectClick);

/**
 * Handler for the "Set Path" button click.
 * Validates and saves the user-provided save path.
 */
const onSavePathClick = (): void => {
  void savePath();
};
saveButton.addEventListener('click', onSavePathClick);

/**
 * Handler for the "Refresh Backups" button click.
 * Reloads the list of backups from the backend.
 */
const onRefreshBackupsClick = (): void => {
  void loadBackups();
};
refreshBackupsButton.addEventListener('click', onRefreshBackupsClick);

/**
 * Handler for the "Launch Game" button click.
 * Sends the launch command to the backend.
 */
const onLaunchGameClick = (): void => {
  void launchGame();
};
launchGameButton.addEventListener('click', onLaunchGameClick);

/**
 * Handler for changes to the "Auto-launch game" checkbox.
 * Saves the updated game settings.
 */
const onAutoLaunchChange = (): void => {
  void saveGameSettings();
};
autoLaunchCheck.addEventListener('change', onAutoLaunchChange);

/**
 * Handler for changes to the "Auto-close app" checkbox.
 * Saves the updated game settings.
 */
const onAutoCloseChange = (): void => {
  void saveGameSettings();
};
autoCloseCheck.addEventListener('change', onAutoCloseChange);

/**
 * Delegated handler for clicking on detected paths.
 */
pathsList.addEventListener('click', (event) => {
  const target = event.target as HTMLElement;
  const li = target.closest('li');

  // Ignore clicks if not on an LI or if it's the empty/error message
  if (!li || li.classList.contains('empty') || li.classList.contains('error'))
    return;

  const path = li.textContent;
  if (path) {
    manualInput.value = path;
    setConfigStatus(
      'Path selected from list. Click "Set Path" to save.',
      'info',
    );
  }
});

/**
 * Delegated handler for clicking on backup restore buttons.
 */
backupsList.addEventListener('click', (event) => {
  const target = event.target as HTMLElement;
  const button = target.closest('button');

  if (!button || !button.dataset.index) return;

  const index = parseInt(button.dataset.index, 10);
  const backup = currentBackups[index];

  if (backup) {
    void restoreBackup(backup);
  }
});

// Initial load
void loadConfig();