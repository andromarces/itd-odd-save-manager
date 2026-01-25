import { logActivity, safeInvoke } from '../ui_utils';
import type { AppElements } from './dom';
import type { BackupInfo } from './types';

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

      const deleteBtn = document.createElement('button');
      deleteBtn.textContent = 'Delete';
      deleteBtn.className = 'small danger';
      deleteBtn.dataset.index = index.toString();
      deleteBtn.dataset.action = 'delete';
      deleteBtn.style.marginLeft = '8px';

      actionCell.appendChild(restoreBtn);
      actionCell.appendChild(lockBtn);
      actionCell.appendChild(noteBtn);
      actionCell.appendChild(deleteBtn);

      row.appendChild(fileCell);
      row.appendChild(dateCell);
      row.appendChild(actionCell);
      fragment.appendChild(row);

      if (backup.note) {
        const noteRow = document.createElement('tr');
        noteRow.className = 'note-row';
        const noteCell = document.createElement('td');
        noteCell.colSpan = 3;
        noteCell.textContent = backup.note;
        noteCell.title = backup.note;
        noteRow.appendChild(noteCell);
        fragment.appendChild(noteRow);
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
   * Toggles the lock status of a backup.
   */
  async function toggleBackupLock(backup: BackupInfo): Promise<void> {
    const success = await safeInvoke(
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

    if (success !== undefined) {
      backup.locked = !backup.locked;
      renderBackups(currentBackups);
    }
  }

  /**
   * Edits the note for a backup.
   */
  async function editBackupNote(backup: BackupInfo): Promise<void> {
    const currentNote = backup.note || '';
    const newNote = window.prompt('Enter note for this backup:', currentNote);
    if (newNote === null) return; // Cancelled

    const success = await safeInvoke(
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

    if (success !== undefined) {
      backup.note = newNote.trim() || null;
      renderBackups(currentBackups);
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

    const success = await safeInvoke(
      'delete_backup_command',
      {
        backup_path: backup.path,
      },
      {
        actionName: 'delete backup',
        successLog: `Deleted backup: ${backup.filename}`,
        onError: () => logActivity(`Failed to delete ${backup.filename}`),
      },
    );

    if (success !== undefined) {
      const index = currentBackups.indexOf(backup);
      if (index > -1) {
        currentBackups.splice(index, 1);
        renderBackups(currentBackups);
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

  // --- Master Delete Logic ---

  function openMasterDeleteDialog() {
    if (currentBackups.length === 0) {
      alert('No backups available to delete.');
      return;
    }

    // Populate Game List
    const uniqueGames = new Set<number>();
    currentBackups.forEach((b) => uniqueGames.add(b.game_number));

    elements.masterDeleteGameList.innerHTML = '';
    const sortedGames = Array.from(uniqueGames).sort((a, b) => a - b);

    sortedGames.forEach((gameNum) => {
      const label = document.createElement('label');
      label.className = 'checkbox-label';
      const input = document.createElement('input');
      input.type = 'checkbox';
      input.value = gameNum.toString();
      input.checked = true; // Default to selected
      label.appendChild(input);
      label.appendChild(document.createTextNode(` Game ${gameNum + 1}`));
      elements.masterDeleteGameList.appendChild(label);
    });

    elements.masterDeleteDialog.showModal();
  }

  function closeMasterDeleteDialog() {
    elements.masterDeleteDialog.close();
  }

  async function handleMasterDeleteSubmit(event: Event) {
    event.preventDefault();

    // 1. Get Selected Games
    const selectedGames: number[] = [];
    const checkboxes =
      elements.masterDeleteGameList.querySelectorAll<HTMLInputElement>(
        'input[type="checkbox"]',
      );
    checkboxes.forEach((cb) => {
      if (cb.checked) {
        selectedGames.push(parseInt(cb.value, 10));
      }
    });

    if (selectedGames.length === 0) {
      alert('Please select at least one game.');
      return;
    }

    // 2. Get Mode
    let keepLatest = true;
    elements.masterDeleteModeRadios.forEach((radio) => {
      if (radio.checked && radio.value === 'all') {
        keepLatest = false;
      }
    });

    // 3. Get Locked Setting
    let deleteLocked = false;
    elements.masterDeleteLockedRadios.forEach((radio) => {
      if (radio.checked && radio.value === 'include') {
        deleteLocked = true;
      }
    });

    // 4. Confirm
    const modeText = keepLatest
      ? 'Delete all except latest'
      : 'Delete ALL backups';
    const lockedText = deleteLocked
      ? '(INCLUDING locked backups)'
      : '(excluding locked backups)';
    const confirmMsg = `Are you sure?\n\nAction: ${modeText}\nTarget: ${selectedGames.length} Game(s)\n${lockedText}\n\nThis cannot be undone.`;

    if (!confirm(confirmMsg)) return;

    elements.masterDeleteConfirmBtn.disabled = true;
    elements.masterDeleteConfirmBtn.textContent = 'Deleting...';

    const deletedCount = await safeInvoke<number>(
      'batch_delete_backups_command',
      {
        game_numbers: selectedGames,
        keep_latest: keepLatest,
        delete_locked: deleteLocked,
      },
      {
        actionName: 'batch delete',
        onError: () => logActivity('Failed to perform batch delete.'),
      },
    );

    if (deletedCount !== undefined) {
      logActivity(`Batch delete completed. Removed ${deletedCount} backups.`);
      closeMasterDeleteDialog();
      await loadBackups();
    }

    elements.masterDeleteConfirmBtn.disabled = false;
    elements.masterDeleteConfirmBtn.textContent = 'Delete';
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
        } else if (action === 'delete') {
          void deleteBackup(backup);
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
  elements.masterDeleteButton.addEventListener('click', openMasterDeleteDialog);
  elements.masterDeleteCancelBtn.addEventListener(
    'click',
    closeMasterDeleteDialog,
  );
  elements.masterDeleteForm.addEventListener(
    'submit',
    handleMasterDeleteSubmit,
  );

  return {
    loadBackups,
  };
}
