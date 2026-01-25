import { getElement } from '../ui_utils';

export interface AppElements {
  detectButton: HTMLButtonElement;
  pathsList: HTMLUListElement;
  manualInput: HTMLInputElement;
  saveButton: HTMLButtonElement;
  configStatus: HTMLParagraphElement;
  refreshBackupsButton: HTMLButtonElement;
  masterDeleteButton: HTMLButtonElement;
  backupsTable: HTMLTableElement;
  backupsList: HTMLTableSectionElement;
  launchGameButton: HTMLButtonElement;
  autoLaunchCheck: HTMLInputElement;
  autoCloseCheck: HTMLInputElement;
  maxBackupsInput: HTMLInputElement;
  tabButtons: NodeListOf<HTMLButtonElement>;
  tabPanels: NodeListOf<HTMLElement>;
  // Dialog Elements
  masterDeleteDialog: HTMLDialogElement;
  masterDeleteForm: HTMLFormElement;
  masterDeleteGameList: HTMLDivElement;
  masterDeleteModeRadios: NodeListOf<HTMLInputElement>;
  masterDeleteLockedRadios: NodeListOf<HTMLInputElement>;
  masterDeleteCancelBtn: HTMLButtonElement;
  masterDeleteConfirmBtn: HTMLButtonElement;
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

    <nav class="tab-nav" role="tablist">
      <button id="tab-dashboard" class="tab-button active" data-tab="dashboard" role="tab" aria-selected="true" aria-controls="panel-dashboard">Dashboard</button>
      <button id="tab-settings" class="tab-button" data-tab="settings" role="tab" aria-selected="false" aria-controls="panel-settings">Settings</button>
      <button id="tab-log" class="tab-button" data-tab="log" role="tab" aria-selected="false" aria-controls="panel-log">Log</button>
    </nav>

    <div id="panel-dashboard" class="tab-panel active" data-tab-panel="dashboard" role="tabpanel" aria-labelledby="tab-dashboard">
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
        <h2>Backups</h2>
        <div class="actions">
          <button id="refresh-backups" type="button" disabled>Refresh Backups</button>
          <button id="master-delete-btn" type="button" class="danger">Delete Backups...</button>
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
    </div>

    <div id="panel-settings" class="tab-panel" data-tab-panel="settings" role="tabpanel" aria-labelledby="tab-settings" hidden>
      <section class="panel">
        <h2>Active Configuration</h2>
        <div class="input-group">
          <input type="text" id="manual-path" placeholder="C:\\Path\\To\\Save\\Folder" spellcheck="false" />
          <button id="save-config" type="button">Set Path</button>
        </div>
        <p id="config-status" class="status-text"></p>
      </section>

      <section class="panel">
        <h2>Backup Settings</h2>
        <div class="input-group">
          <label for="max-backups-input">Max Backups Per Game:</label>
          <input type="number" id="max-backups-input" min="0" value="100" />
        </div>
        <p class="hint">The limit applies to each game slot individually. Set to 0 for unlimited.</p>
      </section>

      <section class="panel">
        <h2>Discovery</h2>
        <div class="actions">
          <button id="detect" type="button">Auto Detect Save Path</button>
        </div>
        <ul id="paths" class="paths" aria-live="polite"></ul>
      </section>
    </div>

    <div id="panel-log" class="tab-panel" data-tab-panel="log" role="tabpanel" aria-labelledby="tab-log" hidden>
      <section class="panel">
        <h2>Activity Log</h2>
        <div id="activity-log" class="log-box"></div>
      </section>
    </div>
  </main>

  <dialog id="master-delete-dialog" class="modal">
    <form id="master-delete-form" method="dialog">
      <h2>Delete Backups</h2>
      
      <fieldset>
        <legend>Select Games</legend>
        <div id="master-delete-game-list" class="checkbox-list">
          <!-- Populated dynamically -->
        </div>
      </fieldset>

      <fieldset>
        <legend>Action</legend>
        <div class="radio-group">
          <label class="radio-label">
            <input type="radio" name="delete-mode" value="all" />
            Delete all backups
          </label>
          <label class="radio-label">
            <input type="radio" name="delete-mode" value="all-but-latest" checked />
            Delete all except latest
          </label>
        </div>
      </fieldset>

      <fieldset>
        <legend>Locked Backups</legend>
        <div class="radio-group">
          <label class="radio-label">
            <input type="radio" name="delete-locked" value="exclude" checked />
            Exclude locked backups (Keep them)
          </label>
          <label class="radio-label">
            <input type="radio" name="delete-locked" value="include" />
            Include locked backups (Delete them)
          </label>
        </div>
      </fieldset>

      <div class="modal-actions">
        <button type="button" id="master-delete-cancel">Cancel</button>
        <button type="submit" id="master-delete-confirm" class="danger">Delete</button>
      </div>
    </form>
  </dialog>
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
    masterDeleteButton: getElement<HTMLButtonElement>('#master-delete-btn'),
    backupsTable: getElement<HTMLTableElement>('#backups-table'),
    backupsList: getElement<HTMLTableSectionElement>('#backups-list'),
    launchGameButton: getElement<HTMLButtonElement>('#launch-game'),
    autoLaunchCheck: getElement<HTMLInputElement>('#auto-launch-check'),
    autoCloseCheck: getElement<HTMLInputElement>('#auto-close-check'),
    maxBackupsInput: getElement<HTMLInputElement>('#max-backups-input'),
    tabButtons: document.querySelectorAll<HTMLButtonElement>('.tab-button'),
    tabPanels: document.querySelectorAll<HTMLElement>('.tab-panel'),
    // Dialog
    masterDeleteDialog: getElement<HTMLDialogElement>('#master-delete-dialog'),
    masterDeleteForm: getElement<HTMLFormElement>('#master-delete-form'),
    masterDeleteGameList: getElement<HTMLDivElement>(
      '#master-delete-game-list',
    ),
    masterDeleteModeRadios: document.querySelectorAll<HTMLInputElement>(
      'input[name="delete-mode"]',
    ),
    masterDeleteLockedRadios: document.querySelectorAll<HTMLInputElement>(
      'input[name="delete-locked"]',
    ),
    masterDeleteCancelBtn: getElement<HTMLButtonElement>(
      '#master-delete-cancel',
    ),
    masterDeleteConfirmBtn: getElement<HTMLButtonElement>(
      '#master-delete-confirm',
    ),
  };
}

/**
 * Attaches click listeners to tab buttons for panel switching.
 */
export function setupTabNavigation(elements: AppElements): void {
  elements.tabButtons.forEach((button) => {
    button.addEventListener('click', () => {
      const targetTab = button.dataset.tab;
      if (!targetTab) return;

      // Update button states
      elements.tabButtons.forEach((btn) => {
        const isActive = btn.dataset.tab === targetTab;
        btn.classList.toggle('active', isActive);
        btn.setAttribute('aria-selected', String(isActive));
      });

      // Update panel visibility
      elements.tabPanels.forEach((panel) => {
        const isActive = panel.dataset.tabPanel === targetTab;
        panel.classList.toggle('active', isActive);
        panel.hidden = !isActive;
      });
    });
  });
}
