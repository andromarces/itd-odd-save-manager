import { logActivity, safeInvoke } from '../ui_utils';
import type { AppElements } from './dom';
import type { BackupInfo } from './types';

type BackupsElements = Pick<
  AppElements,
  'manualInput' | 'refreshBackupsButton' | 'backupsTable'
>;

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
export function createBackupsFeature(
  elements: BackupsElements,
): BackupsFeature {
  let currentBackups: BackupInfo[] = [];

  /**
   * Renders the list of backups into the table body.
   */
  function renderBackups(backups: BackupInfo[]): void {
    // Clear existing bodies
    while (elements.backupsTable.tBodies.length > 0) {
      elements.backupsTable.removeChild(elements.backupsTable.tBodies[0]);
    }

    if (backups.length === 0) {
      const tbody = document.createElement('tbody');
      tbody.innerHTML =
        '<tr><td colspan="3" class="empty">No backups found.</td></tr>';
      elements.backupsTable.appendChild(tbody);
      return;
    }

    const fragment = document.createDocumentFragment();
    backups.forEach((backup, index) => {
      const tbody = document.createElement('tbody');
      const row = document.createElement('tr');

      const fileCell = document.createElement('td');
      fileCell.textContent = getBackupDisplayName(backup);
      fileCell.title = backup.filename;
      if (backup.locked) {
        fileCell.textContent += ' (Locked)';
      }

      const dateCell = document.createElement('td');
      dateCell.textContent = formatDate(backup.modified);

      const actionCell = document.createElement('td');

      const restoreBtn = document.createElement('button');
      restoreBtn.textContent = 'Restore';
      restoreBtn.className = 'small';
      restoreBtn.dataset.index = index.toString();
      restoreBtn.dataset.action = 'restore';

      const lockBtn = document.createElement('button');
      lockBtn.textContent = backup.locked ? 'Unlock' : 'Lock';
      lockBtn.className = 'small secondary';
      lockBtn.dataset.index = index.toString();
      lockBtn.dataset.action = 'lock';
      lockBtn.style.marginLeft = '8px';

      const noteBtn = document.createElement('button');
      noteBtn.textContent = 'Note';
      noteBtn.className = 'small secondary';
      noteBtn.dataset.index = index.toString();
      noteBtn.dataset.action = 'note';
      noteBtn.style.marginLeft = '8px';
      noteBtn.title = backup.note ? 'Edit Note' : 'Add Note';

      actionCell.appendChild(restoreBtn);
      actionCell.appendChild(lockBtn);
      actionCell.appendChild(noteBtn);

      row.appendChild(fileCell);
      row.appendChild(dateCell);
      row.appendChild(actionCell);
      tbody.appendChild(row);

      if (backup.note) {
        const noteRow = document.createElement('tr');
        noteRow.className = 'note-row';
        const noteCell = document.createElement('td');
        noteCell.colSpan = 3;
        noteCell.textContent = backup.note;
        noteCell.title = backup.note;
        noteRow.appendChild(noteCell);
        tbody.appendChild(noteRow);
      }

      fragment.appendChild(tbody);
    });
    elements.backupsTable.appendChild(fragment);
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
          // Fallback for error state
          while (elements.backupsTable.tBodies.length > 0) {
            elements.backupsTable.removeChild(elements.backupsTable.tBodies[0]);
          }
          const tbody = document.createElement('tbody');
          tbody.innerHTML =
            '<tr><td colspan="3" class="error">Failed to load backups</td></tr>';
          elements.backupsTable.appendChild(tbody);
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
   * Toggles the lock status of a backup.
   */
  async function toggleBackupLock(backup: BackupInfo): Promise<void> {
    await safeInvoke(
      'toggle_backup_lock_command',
      {
        backup_path: backup.path,
        locked: !backup.locked,
      },
      {
        actionName: 'toggle backup lock',
        onError: () =>
          logActivity(`Failed to toggle lock for ${backup.filename}`),
      },
    );
    await loadBackups();
  }

  /**
   * Edits the note for a backup.
   */
  async function editBackupNote(backup: BackupInfo): Promise<void> {
    const currentNote = backup.note || '';
    const newNote = window.prompt('Enter note for this backup:', currentNote);
    if (newNote === null) return; // Cancelled

    await safeInvoke(
      'set_backup_note_command',
      {
        backup_filename: backup.filename,
        note: newNote.trim() || null,
      },
      {
        actionName: 'set backup note',
        onError: () => logActivity(`Failed to set note for ${backup.filename}`),
      },
    );
    await loadBackups();
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
   * Handles delegated clicks on the backups table.
   */
  function handleBackupsTableClick(event: Event): void {
    const target = event.target as HTMLElement;
    const button = target.closest('button');

    if (button && button.dataset.index) {
      const index = parseInt(button.dataset.index, 10);
      const backup = currentBackups[index];
      if (backup) {
        const action = button.dataset.action;
        if (action === 'restore') {
          void restoreBackup(backup);
        } else if (action === 'lock') {
          void toggleBackupLock(backup);
        } else if (action === 'note') {
          void editBackupNote(backup);
        }
      }
      return;
    }

    // Handle note row expansion
    const row = target.closest('tr');
    if (row && row.classList.contains('note-row')) {
      row.classList.toggle('expanded');
    }
  }

  elements.backupsTable.addEventListener('click', handleBackupsTableClick);

  return {
    loadBackups,
  };
}
