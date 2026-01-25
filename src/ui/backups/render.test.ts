import { describe, expect, it } from 'vitest';
import { createNoteRow } from './render';

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
