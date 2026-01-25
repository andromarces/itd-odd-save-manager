import type { BackupInfo } from '../types';

/**
 * Formats an ISO date string for display.
 */
export function formatDate(isoString: string): string {
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
  const gameLabel = getBackupDisplayName(backup);
  let message = `Are you sure you want to restore "${backup.original_filename}" (${gameLabel}) from ${formatDate(backup.modified)}?`;
  message += `\nThis will overwrite the current save files for ${gameLabel}.`;
  return message;
}

/**
 * Creates a table row for a backup.
 */
export function createBackupRow(backup: BackupInfo, index: number): HTMLTableRowElement {
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
  actionCell.appendChild(createActionButton('Restore', index, 'restore'));
  actionCell.appendChild(createActionButton('Lock', index, 'lock', backup.locked ? 'Unlock' : 'Lock', 'secondary'));
  actionCell.appendChild(createActionButton('Note', index, 'note', 'Note', 'secondary', backup.note ? 'Edit Note' : 'Add Note'));
  actionCell.appendChild(createActionButton('Delete', index, 'delete', 'Delete', 'danger'));

  row.appendChild(fileCell);
  row.appendChild(dateCell);
  row.appendChild(actionCell);

  return row;
}

/**
 * Creates a visible note row for a backup.
 */
export function createNoteRow(note: string): HTMLTableRowElement {
  const noteRow = document.createElement('tr');
  noteRow.className = 'note-row expanded';
  const noteCell = document.createElement('td');
  noteCell.colSpan = 3;
  noteCell.textContent = note;
  noteCell.title = note;
  noteRow.appendChild(noteCell);
  return noteRow;
}

/**
 * Helper to create a standard action button for the backup table.
 */
function createActionButton(
  label: string,
  index: number,
  action: string,
  displayLabel?: string,
  className?: string,
  title?: string
): HTMLButtonElement {
  const btn = document.createElement('button');
  btn.textContent = displayLabel || label;
  btn.className = `small ${className || ''}`.trim();
  btn.dataset.index = index.toString();
  btn.dataset.action = action;
  if (title) btn.title = title;
  if (action !== 'restore') btn.style.marginLeft = '8px';
  return btn;
}
