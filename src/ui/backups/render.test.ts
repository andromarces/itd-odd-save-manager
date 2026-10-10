import { describe, expect, it } from "vite-plus/test";
import type { BackupInfo } from "../types";
import { createBackupRow, createNoteRow } from "./render";
import { formatDate } from "../../ui_utils";

/**
 * Creates a backup info object for render tests.
 */
function createBackup(overrides: Partial<BackupInfo> = {}): BackupInfo {
  return {
    path: "C:\\Backups\\gamesave_1.sav",
    filename: "gamesave_1.sav",
    original_filename: "gamesave_1.sav",
    original_path: "C:\\Saves\\gamesave_1.sav",
    size: 1024,
    modified: "2026-01-09T03:04:08",
    game_number: 0,
    locked: false,
    hash: "abc123",
    note: null,
    ...overrides,
  };
}

describe("formatDate", () => {
  it("formats a date with all expected components", () => {
    const formatted = formatDate("2026-01-09T03:04:08");
    expect(formatted).toMatch(/^\d{2}\/\w{3}\/\d{4} \d{2}:\d{2}:\d{2} (AM|PM)$/);
    expect(formatted).toContain("2026");
    expect(formatted).toContain("Jan");
  });
});

describe("createBackupRow", () => {
  it("renders a formatted date in one of the cells", () => {
    const backup = createBackup();
    const row = createBackupRow(backup);
    expect(row.dataset.backupId).toBe(backup.path);
    const cells = Array.from(row.querySelectorAll("td"));
    const formattedDate = formatDate(backup.modified);
    const cellWithDate = cells.find((cell) => cell.textContent === formattedDate);
    expect(cellWithDate).toBeDefined();
  });

  /**
   * Verifies the lock button label follows the lock state while the other action labels stay fixed.
   * Not redundant: the createActionButton refactor collapsed two label parameters into one, and no
   * other test covers the Lock/Unlock switch.
   */
  it("labels the lock button by lock state and keeps the other action labels", () => {
    const labels = (backup: BackupInfo) =>
      Array.from(createBackupRow(backup).querySelectorAll("button")).map((b) => b.textContent);

    expect(labels(createBackup({ locked: false }))).toEqual(["Restore", "Lock", "Note", "Delete"]);
    expect(labels(createBackup({ locked: true }))).toEqual(["Restore", "Unlock", "Note", "Delete"]);
  });
});

describe("createNoteRow", () => {
  it("displays the note text", () => {
    const row = createNoteRow("Test note");
    const cell = row.querySelector("td");
    expect(cell).toBeDefined();
    expect(cell?.textContent).toBe("Test note");
  });
});
