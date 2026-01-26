import { describe, expect, it } from 'vitest';
import type { BackupInfo } from '../types';
import { createBackupRow, createNoteRow } from './render';
import { formatDate } from '../../ui_utils';

/**
 * Creates a backup info object for render tests.
 */
function createBackup(overrides: Partial<BackupInfo> = {}): BackupInfo {
  return {
    path: 'C:\\Backups\\gamesave_1.sav',
    filename: 'gamesave_1.sav',
    original_filename: 'gamesave_1.sav',
    original_path: 'C:\\Saves\\gamesave_1.sav',
    size: 1024,
    modified: '2026-01-09T03:04:08',
    game_number: 0,
    locked: false,
    hash: 'abc123',
    note: null,
    ...overrides,
  };
}

describe('formatDate', () => {
  it('formats a date with all expected components', () => {
    const formatted = formatDate('2026-01-09T03:04:08');
    expect(formatted).toMatch(
      /^\d{2}\/\w{3}\/\d{4} \d{2}:\d{2}:\d{2} (AM|PM)$/,
    );
    expect(formatted).toContain('2026');
    expect(formatted).toContain('Jan');
  });
});

describe('createBackupRow', () => {
  it('renders a formatted date in one of the cells', () => {
    const backup = createBackup();
    const row = createBackupRow(backup);
    expect(row.dataset.backupId).toBe(backup.path);
    const cells = Array.from(row.querySelectorAll('td'));
    const formattedDate = formatDate(backup.modified);
    const cellWithDate = cells.find(
      (cell) => cell.textContent === formattedDate,
    );
    expect(cellWithDate).toBeDefined();
  });
});

describe('createNoteRow', () => {
  it('displays the note text', () => {
    const row = createNoteRow('Test note');
    const cell = row.querySelector('td');
    expect(cell).toBeDefined();
    expect(cell?.textContent).toBe('Test note');
  });
});
