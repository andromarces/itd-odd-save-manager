import { getElement } from '../ui_utils';

export interface AppElements {
  detectButton: HTMLButtonElement;
  pathsList: HTMLUListElement;
  manualInput: HTMLInputElement;
  saveButton: HTMLButtonElement;
  configStatus: HTMLParagraphElement;
  refreshBackupsButton: HTMLButtonElement;
  backupsList: HTMLTableSectionElement;
  launchGameButton: HTMLButtonElement;
  autoLaunchCheck: HTMLInputElement;
  autoCloseCheck: HTMLInputElement;
}

const APP_TEMPLATE = `
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
        <button id="detect" type="button">Auto Detect Save Path</button>
      </div>
      <ul id="paths" class="paths" aria-live="polite"></ul>
    </section>
  </main>
`;

/**
 * Renders the application shell and returns the typed element references.
 */
export function renderAppShell(): AppElements {
  const app = getElement<HTMLDivElement>('#app');
  app.innerHTML = APP_TEMPLATE;

  return {
    detectButton: getElement<HTMLButtonElement>('#detect'),
    pathsList: getElement<HTMLUListElement>('#paths'),
    manualInput: getElement<HTMLInputElement>('#manual-path'),
    saveButton: getElement<HTMLButtonElement>('#save-config'),
    configStatus: getElement<HTMLParagraphElement>('#config-status'),
    refreshBackupsButton: getElement<HTMLButtonElement>('#refresh-backups'),
    backupsList: getElement<HTMLTableSectionElement>('#backups-list'),
    launchGameButton: getElement<HTMLButtonElement>('#launch-game'),
    autoLaunchCheck: getElement<HTMLInputElement>('#auto-launch-check'),
    autoCloseCheck: getElement<HTMLInputElement>('#auto-close-check'),
  };
}

