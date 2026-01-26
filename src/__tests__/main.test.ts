import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  transformCallback: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
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
    const emptyMessage = document.querySelector('#paths li.empty');
    expect(emptyMessage).not.toBeNull();
    expect(emptyMessage?.textContent).toBeTruthy();
  });
});
