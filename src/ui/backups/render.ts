import { formatDate } from "../../ui_utils";
export { formatDate };
import type { BackupInfo } from "../types";

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
export function createBackupRow(backup: BackupInfo): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.backupId = backup.path;

  const fileCell = document.createElement("td");
  fileCell.textContent = getBackupDisplayName(backup);
  fileCell.title = backup.filename;
  if (backup.locked) {
    fileCell.textContent += " (Locked)";
  }

  const dateCell = document.createElement("td");
  dateCell.textContent = formatDate(backup.modified);

  const actionCell = document.createElement("td");
  actionCell.appendChild(createActionButton("Restore", backup.path, "restore"));
  actionCell.appendChild(
    createActionButton(backup.locked ? "Unlock" : "Lock", backup.path, "lock", "secondary"),
  );
  actionCell.appendChild(
    createActionButton(
      "Note",
      backup.path,
      "note",
      "secondary",
      backup.note ? "Edit Note" : "Add Note",
    ),
  );
  actionCell.appendChild(createActionButton("Delete", backup.path, "delete", "danger"));

  row.appendChild(fileCell);
  row.appendChild(dateCell);
  row.appendChild(actionCell);

  return row;
}

/**
 * Creates a visible note row for a backup.
 */
export function createNoteRow(note: string): HTMLTableRowElement {
  const noteRow = document.createElement("tr");
  noteRow.className = "note-row expanded";
  const noteCell = document.createElement("td");
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
  id: string,
  action: string,
  className?: string,
  title?: string,
): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.textContent = label;
  const baseClassName = `small ${className || ""}`.trim();
  btn.className = action === "lock" ? `${baseClassName} lock-toggle`.trim() : baseClassName;
  btn.dataset.backupId = id;
  btn.dataset.action = action;
  if (title) btn.title = title;
  return btn;
}
