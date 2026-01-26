import { invokeAction, logActivity } from '../ui_utils';
import { listen } from '@tauri-apps/api/event';
import type { AppElements } from './dom';
import type { BackupInfo } from './types';
import {
  createBackupRow,
  createNoteRow,
  getBackupDisplayName,
  formatDate,
  buildRestoreConfirmationMessage,
} from './backups/render';
import { MasterDeleteController } from './backups/dialog';

export { getBackupDisplayName, buildRestoreConfirmationMessage };

type BackupsElements = Pick<
  AppElements,
  | 'manualInput'
  | 'refreshBackupsButton'
  | 'backupsTable'
  | 'backupsList'
  | 'masterDeleteButton'
  | 'masterDeleteDialog'
  | 'masterDeleteForm'
  | 'masterDeleteGameList'
  | 'masterDeleteModeRadios'
  | 'masterDeleteLockedRadios'
  | 'masterDeleteCancelBtn'
  | 'masterDeleteConfirmBtn'
>;

export interface BackupsFeature {
  loadBackups: () => Promise<void>;
  destroy: () => void;
}

/**
 * Creates the backups feature, wiring the UI and returning feature actions.
 */
export function createBackupsFeature(
  elements: BackupsElements,
): BackupsFeature {
  let currentBackups: BackupInfo[] = [];
  let currentBackupsMap = new Map<string, BackupInfo>();

  const masterDelete = new MasterDeleteController(elements, () =>
    loadBackups(),
  );

  const unlistenPromise = listen('backups-updated', () => {
    void loadBackups();
  });

  /**
   * Helper to find a row by backup ID.
   */
  function findRowByBackupId(id: string): HTMLTableRowElement | undefined {
    return Array.from(elements.backupsList.children).find(
      (el) => (el as HTMLElement).dataset.backupId === id,
    ) as HTMLTableRowElement | undefined;
  }

  /**
   * Renders the list of backups into the table body.
   */
  function renderBackups(backups: BackupInfo[]): void {
    if (backups.length === 0) {
      elements.backupsList.innerHTML =
        '<tr><td colspan="3" class="empty">No backups found.</td></tr>';
      return;
    }

    const fragment = document.createDocumentFragment();
    backups.forEach((backup) => {
      fragment.appendChild(createBackupRow(backup));
      if (backup.note) {
        fragment.appendChild(createNoteRow(backup.note));
      }
    });

    elements.backupsList.replaceChildren(fragment);
  }

  /**
   * Loads backups from the backend and updates the table.
   */
  async function loadBackups(): Promise<void> {
    if (!elements.manualInput.value) return;

    elements.refreshBackupsButton.textContent = 'Refreshing...';
    elements.refreshBackupsButton.disabled = true;

    const backups = await invokeAction<BackupInfo[]>(
      'get_backups_command',
      undefined,
      'load backups',
      {
        onError: () => {
          elements.backupsList.innerHTML =
            '<tr><td colspan="3" class="error">Failed to load backups</td></tr>';
        },
      },
    );

    if (backups) {
      currentBackups = backups;
      currentBackupsMap = new Map(backups.map((b) => [b.path, b]));
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
    const success = await invokeAction(
      'toggle_backup_lock_command',
      {
        backup_path: backup.path,
        locked: !backup.locked,
      },
      'toggle backup lock',
      {
        onError: () =>
          logActivity(`Failed to toggle lock for ${backup.filename}`),
      },
    );

    if (success !== undefined) {
      // Update local state
      backup.locked = !backup.locked;

      const row = findRowByBackupId(backup.path);

      if (row) {
        const newRow = createBackupRow(backup);
        elements.backupsList.replaceChild(newRow, row);
      } else {
        // Fallback if row not found (shouldn't happen usually)
        renderBackups(currentBackups);
      }
    }
  }

  /**
   * Edits the note for a backup.
   */
  async function editBackupNote(backup: BackupInfo): Promise<void> {
    const currentNote = backup.note || '';
    const newNote = window.prompt('Enter note for this backup:', currentNote);
    if (newNote === null) return;

    const success = await invokeAction(
      'set_backup_note_command',
      {
        backup_filename: backup.filename,
        note: newNote.trim() || null,
      },
      'set backup note',
      {
        onError: () => logActivity(`Failed to set note for ${backup.filename}`),
      },
    );

    if (success !== undefined) {
      backup.note = newNote.trim() || null;
      const row = findRowByBackupId(backup.path);

      if (row) {
        const newRow = createBackupRow(backup);
        elements.backupsList.replaceChild(newRow, row);

        const nextSibling = newRow.nextElementSibling;
        const hasNoteRow = nextSibling?.classList.contains('note-row');

        if (backup.note) {
          const newNoteRow = createNoteRow(backup.note);
          if (hasNoteRow && nextSibling) {
            elements.backupsList.replaceChild(newNoteRow, nextSibling);
          } else {
            newRow.insertAdjacentElement('afterend', newNoteRow);
          }
        } else if (hasNoteRow && nextSibling) {
          elements.backupsList.removeChild(nextSibling);
        }
      } else {
        renderBackups(currentBackups);
      }
    }
  }

  /**
   * Deletes a specific backup after user confirmation.
   */
  async function deleteBackup(backup: BackupInfo): Promise<void> {
    const confirmed = window.confirm(
      `Are you sure you want to delete the backup for ${getBackupDisplayName(
        backup,
      )} from ${formatDate(backup.modified)}?\nThis action cannot be undone.`,
    );
    if (!confirmed) return;

    const success = await invokeAction(
      'delete_backup_command',
      {
        backup_path: backup.path,
      },
      'delete backup',
      {
        successLog: `Deleted backup: ${backup.filename}`,
        onError: () => logActivity(`Failed to delete ${backup.filename}`),
      },
    );

    if (success !== undefined) {
      // Find index by path, not object identity, to handle potential list refreshes
      // Assumption: backup.path is unique and stable (canonicalized by backend)
      const index = currentBackups.findIndex((b) => b.path === backup.path);
      if (index > -1) {
        currentBackups.splice(index, 1);
        currentBackupsMap.delete(backup.path);
        renderBackups(currentBackups);
      } else {
        // Fallback: if we can't find it locally (maybe list refreshed)
        if (elements.manualInput.value) {
          void loadBackups();
        } else {
          // If input is empty, we can't reload, but we can try to clean up the stale DOM row
          const row = findRowByBackupId(backup.path);
          if (row) {
            row.remove();
            // Also remove associated note row if present
            const nextSibling = row.nextElementSibling;
            if (nextSibling?.classList.contains('note-row')) {
              nextSibling.remove();
            }
          }
        }
      }
    }
  }

  /**
   * Restores a selected backup after user confirmation.
   */
  async function restoreBackup(backup: BackupInfo): Promise<void> {
    const message = buildRestoreConfirmationMessage(backup);
    const confirmed = window.confirm(message);
    if (!confirmed) return;

    await invokeAction(
      'restore_backup_command',
      {
        backup_path: backup.path,
        target_path: backup.original_path,
      },
      'restore backup',
      {
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

    if (button && button.dataset.backupId) {
      const id = button.dataset.backupId;
      const backup = currentBackupsMap.get(id);

      if (backup) {
        const action = button.dataset.action;
        if (action === 'restore') {
          void restoreBackup(backup);
        } else if (action === 'lock') {
          void toggleBackupLock(backup);
        } else if (action === 'note') {
          void editBackupNote(backup);
        } else if (action === 'delete') {
          void deleteBackup(backup);
        }
      }
      return;
    }

    const row = target.closest('tr');
    if (row && row.classList.contains('note-row')) {
      return;
    }
  }

  elements.backupsTable.addEventListener('click', handleBackupsTableClick);
  elements.masterDeleteButton.addEventListener('click', () =>
    masterDelete.open(currentBackups),
  );
  elements.refreshBackupsButton.addEventListener('click', () => loadBackups());

  return {
    loadBackups,
    destroy: () => {
      void unlistenPromise.then((unlisten) => unlisten());
    },
  };
}
