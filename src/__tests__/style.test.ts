import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * Reads the stylesheet from disk for static CSS assertions.
 */
function readStylesheet(): string {
  const stylesheetPath = resolve(__dirname, '..', 'style.css');
  return readFileSync(stylesheetPath, 'utf8');
}

describe('style.css', () => {
  it('keeps the backups table stretched to the available width', () => {
    const css = readStylesheet();
    const backupsTableRuleMatch = css.match(/#backups-table\s*\{[^}]*\}/);

    expect(
      backupsTableRuleMatch,
      'Expected a #backups-table rule block.',
    ).not.toBeNull();

    const backupsTableRule = backupsTableRuleMatch?.[0] ?? '';

    expect(backupsTableRule).toContain('width: 100%');
    expect(backupsTableRule).not.toContain('width: max-content');
  });

  it('enforces minimum column sizing for the backups table', () => {
    const css = readStylesheet();
    const backupsTableRuleMatch = css.match(/#backups-table\s*\{[^}]*\}/);

    expect(
      backupsTableRuleMatch,
      'Expected a #backups-table rule block.',
    ).not.toBeNull();

    const backupsTableRule = backupsTableRuleMatch?.[0] ?? '';

    expect(backupsTableRule).toContain('min-width: 680px');

    expect(css).toMatch(
      /#backups-table th:nth-child\(1\),\s*#backups-table td:nth-child\(1\)\s*\{[^}]*width:\s*120px/i,
    );
    expect(css).toMatch(
      /#backups-table th:nth-child\(2\),\s*#backups-table td:nth-child\(2\)\s*\{[^}]*width:\s*210px/i,
    );
    expect(css).toMatch(
      /#backups-table th:nth-child\(3\),\s*#backups-table td:nth-child\(3\)\s*\{[^}]*width:\s*350px/i,
    );
  });
});
