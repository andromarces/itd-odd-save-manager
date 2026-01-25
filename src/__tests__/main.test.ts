import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

/**
 * Initializes the DOM container expected by the app module.
 */
function setupDom(): void {
  document.body.innerHTML = '<div id="app"></div>';
}

describe('restore confirmation message', () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
    setupDom();

    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === 'get_config') {
        return Promise.resolve({
          save_path: null,
          auto_launch_game: false,
          auto_close: false,
          max_backups_per_game: 100,
        });
      }
      if (command === 'is_auto_detection_supported') {
        return Promise.resolve(true);
      }
      return Promise.resolve(undefined);
    });
  });

  it('omits additional warnings', async () => {
    const { buildRestoreConfirmationMessage } = await import('../main');

    const message = buildRestoreConfirmationMessage({
      path: 'C:\\Backups\\gamesave_0.sav',
      filename: 'gamesave_0.sav',
      original_filename: 'gamesave_0.sav',
      original_path: 'C:\\Saves\\gamesave_0.sav',
      size: 1024,
      modified: '2025-01-01T12:00:00Z',
      game_number: 0,
      locked: false,
      hash: 'mock-hash',
    });

    const lines = message.split('\n');

    expect(message).toContain('Are you sure you want to restore');
    expect(message).toContain('Game 1');
    expect(lines).toHaveLength(2);
  });

  /**
   * Verifies the display label for backup rows.
   */
  it('uses a game label for backups', async () => {
    const { getBackupDisplayName } = await import('../main');

    const label = getBackupDisplayName({
      path: 'C:\\Backups\\gamesave_1.sav',
      filename: 'gamesave_1.sav',
      original_filename: 'gamesave_1.sav',
      original_path: 'C:\\Saves\\gamesave_1.sav',
      size: 1024,
      modified: '2025-01-01T12:00:00Z',
      game_number: 1,
      locked: false,
      hash: 'mock-hash',
    });

    expect(label).toBe('Game 2');
  });

  /**
   * Verifies that auto-detection is hidden when unsupported.
   */
  it('hides auto-detection when unsupported', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === 'get_config') {
        return Promise.resolve({
          save_path: null,
          auto_launch_game: false,
          auto_close: false,
          max_backups_per_game: 100,
        });
      }
      if (command === 'is_auto_detection_supported') {
        return Promise.resolve(false);
      }
      return Promise.resolve(undefined);
    });

    const { applyAutoDetectionAvailability } = await import('../main');
    await applyAutoDetectionAvailability();

    expect(document.querySelector('#detect')).toBeNull();
    expect(document.querySelector('#paths li.empty')?.textContent).toContain(
      'Auto-detection is only available on Windows.',
    );
  });
});
