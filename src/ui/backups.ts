import { logActivity, safeInvoke } from '../ui_utils';
import type { AppElements } from './dom';
import type { BackupInfo } from './types';

type BackupsElements = Pick<AppElements, 'manualInput' | 'refreshBackupsButton' | 'backupsList'>;

export interface BackupsFeature {
  loadBackups: () => Promise<void>;
}

/**
 * Formats an ISO date string for display.
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
 * Returns the display label for a backup entry.
 */
export function getBackupDisplayName(backup: BackupInfo): string {
  return `Game ${backup.game_number + 1}`;
}

/**
 * Builds the restore confirmation message for a backup entry.
 */
export function buildRestoreConfirmationMessage(backup: BackupInfo): string {
  const gameLabel = `Game ${backup.game_number + 1}`;
  let message = `Are you sure you want to restore "${backup.original_filename}" (${gameLabel}) from ${formatDate(backup.modified)}?`;
  message += `\nThis will overwrite the current save files for ${gameLabel}.`;
  return message;
}

/**
 * Creates the backups feature, wiring the UI and returning feature actions.
 */
export function createBackupsFeature(elements: BackupsElements): BackupsFeature {
  let currentBackups: BackupInfo[] = [];

  /**
   * Renders the list of backups into the table body.
   */
  function renderBackups(backups: BackupInfo[]): void {
    elements.backupsList.innerHTML = '';

    if (backups.length === 0) {
      elements.backupsList.innerHTML =
        '<tr><td colspan="3" class="empty">No backups found.</td></tr>';
      return;
    }

    const fragment = document.createDocumentFragment();
    backups.forEach((backup, index) => {
      const row = document.createElement('tr');

      const fileCell = document.createElement('td');
      fileCell.textContent = getBackupDisplayName(backup);
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

      fragment.appendChild(row);
    });
    elements.backupsList.appendChild(fragment);
  }

  /**
   * Loads backups from the backend and updates the table.
   */
  async function loadBackups(): Promise<void> {
    if (!elements.manualInput.value) return;

    elements.refreshBackupsButton.textContent = 'Refreshing...';
    elements.refreshBackupsButton.disabled = true;

    const backups = await safeInvoke<BackupInfo[]>(
      'get_backups_command',
      undefined,
      {
        actionName: 'load backups',
        onError: () => {
          elements.backupsList.innerHTML =
            '<tr><td colspan="3" class="error">Failed to load backups</td></tr>';
        },
      },
    );

    if (backups) {
      currentBackups = backups;
      renderBackups(backups);
      logActivity(`Loaded ${backups.length} backups.`);
    }

    elements.refreshBackupsButton.textContent = 'Refresh Backups';
    elements.refreshBackupsButton.disabled = false;
  }

  /**
   * Restores a selected backup after user confirmation.
   */
  async function restoreBackup(backup: BackupInfo): Promise<void> {
    const message = buildRestoreConfirmationMessage(backup);
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
   * Handles delegated restore clicks on the backups table.
   */
  function handleBackupsListClick(event: Event): void {
    const target = event.target as HTMLElement;
    const button = target.closest('button');
    if (!button || !button.dataset.index) return;

    const index = parseInt(button.dataset.index, 10);
    const backup = currentBackups[index];
    if (backup) {
      void restoreBackup(backup);
    }
  }

  elements.backupsList.addEventListener('click', handleBackupsListClick);

  return {
    loadBackups,
  };
}
