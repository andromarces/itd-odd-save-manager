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
  tabButtons: NodeListOf<HTMLButtonElement>;
  tabPanels: NodeListOf<HTMLElement>;
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
    tabButtons: document.querySelectorAll<HTMLButtonElement>('.tab-button'),
    tabPanels: document.querySelectorAll<HTMLElement>('.tab-panel'),
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

