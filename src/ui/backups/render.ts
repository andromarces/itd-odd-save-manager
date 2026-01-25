import type { BackupInfo } from '../types';

const MONTH_ABBREVIATIONS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

/**
 * Pads a number to two digits.
 */
function padTwoDigits(value: number): string {
  return value.toString().padStart(2, '0');
}

/**
 * Formats a local hour value into 12-hour time.
 */
function formatHour12(hours24: number): string {
  const hours12 = hours24 % 12 || 12;
  return padTwoDigits(hours12);
}

/**
 * Returns the meridiem marker for a local hour value.
 */
function formatMeridiem(hours24: number): string {
  return hours24 >= 12 ? 'PM' : 'AM';
}

/**
 * Formats an ISO date string as dd/MMM/yyyy hh:mm:ss AM/PM.
 */
export function formatDate(isoString: string): string {
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) {
    return isoString;
  }

  const day = padTwoDigits(date.getDate());
  const month = MONTH_ABBREVIATIONS[date.getMonth()] ?? '???';
  const year = date.getFullYear().toString();
  const hours = formatHour12(date.getHours());
  const minutes = padTwoDigits(date.getMinutes());
  const seconds = padTwoDigits(date.getSeconds());
  const meridiem = formatMeridiem(date.getHours());

  return `${day}/${month}/${year} ${hours}:${minutes}:${seconds} ${meridiem}`;
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
export function createBackupRow(
  backup: BackupInfo,
  index: number,
): HTMLTableRowElement {
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
  actionCell.appendChild(
    createActionButton(
      'Lock',
      index,
      'lock',
      backup.locked ? 'Unlock' : 'Lock',
      'secondary',
    ),
  );
  actionCell.appendChild(
    createActionButton(
      'Note',
      index,
      'note',
      'Note',
      'secondary',
      backup.note ? 'Edit Note' : 'Add Note',
    ),
  );
  actionCell.appendChild(
    createActionButton('Delete', index, 'delete', 'Delete', 'danger'),
  );

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
  title?: string,
): HTMLButtonElement {
  const btn = document.createElement('button');
  btn.textContent = displayLabel || label;
  const baseClassName = `small ${className || ''}`.trim();
  btn.className =
    action === 'lock' ? `${baseClassName} lock-toggle`.trim() : baseClassName;
  btn.dataset.index = index.toString();
  btn.dataset.action = action;
  if (title) btn.title = title;
  if (action !== 'restore') btn.style.marginLeft = '8px';
  return btn;
}
