import { describe, expect, it } from 'vitest';
import type { BackupInfo } from '../types';
import { createBackupRow, createNoteRow, formatDate } from './render';

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
  it('formats a date as dd/MMM/yyyy hh:mm:ss AM/PM', () => {
    const formatted = formatDate('2026-01-09T03:04:08');
    expect(formatted).toBe('09/Jan/2026 03:04:08 AM');
  });
});

describe('createBackupRow', () => {
  it('renders the formatted date in the date column', () => {
    const backup = createBackup();
    const row = createBackupRow(backup, 0);
    const cells = row.querySelectorAll('td');
    expect(cells[1]?.textContent).toBe('09/Jan/2026 03:04:08 AM');
  });
});

describe('createNoteRow', () => {
  it('marks note rows as expanded by default', () => {
    const row = createNoteRow('Test note');
    expect(row.classList.contains('note-row')).toBe(true);
    expect(row.classList.contains('expanded')).toBe(true);
    const cell = row.querySelector('td');
    expect(cell?.colSpan).toBe(3);
    expect(cell?.textContent).toBe('Test note');
  });
});
