import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

/**
 * Builds the DOM elements required by the config feature.
 */
function createElements(): {
  manualInput: HTMLInputElement;
  saveButton: HTMLButtonElement;
  configStatus: HTMLParagraphElement;
  detectButton: HTMLButtonElement;
  pathsList: HTMLUListElement;
  autoLaunchCheck: HTMLInputElement;
  autoCloseCheck: HTMLInputElement;
  maxBackupsInput: HTMLInputElement;
} {
  document.body.innerHTML = '';

  const manualInput = document.createElement('input');
  const saveButton = document.createElement('button');
  saveButton.textContent = 'Set Path';
  const configStatus = document.createElement('p');
  const detectButton = document.createElement('button');
  const pathsList = document.createElement('ul');
  const autoLaunchCheck = document.createElement('input');
  const autoCloseCheck = document.createElement('input');
  const maxBackupsInput = document.createElement('input');

  return {
    manualInput,
    saveButton,
    configStatus,
    detectButton,
    pathsList,
    autoLaunchCheck,
    autoCloseCheck,
    maxBackupsInput,
  };
}

describe('config refresh availability', () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
    document.body.innerHTML = '';
  });

  it('enables refresh when config provides a valid save path', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue({
      save_path: 'C:\\Saves',
      auto_launch_game: false,
      auto_close: false,
      max_backups_per_game: 100,
    });

    const { createConfigFeature } = await import('./config');
    const elements = createElements();
    const setRefreshAvailability = vi.fn();

    const feature = createConfigFeature(elements, {
      loadBackups: vi.fn().mockResolvedValue(undefined),
      setRefreshAvailability,
    });

    await feature.loadConfig();

    expect(setRefreshAvailability).toHaveBeenLastCalledWith(true);
  });

  it('disables refresh when no save path is configured', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue({
      save_path: null,
      auto_launch_game: false,
      auto_close: false,
      max_backups_per_game: 100,
    });

    const { createConfigFeature } = await import('./config');
    const elements = createElements();
    const setRefreshAvailability = vi.fn();

    const feature = createConfigFeature(elements, {
      loadBackups: vi.fn().mockResolvedValue(undefined),
      setRefreshAvailability,
    });

    await feature.loadConfig();

    expect(setRefreshAvailability).toHaveBeenLastCalledWith(false);
  });

  it('disables refresh when the input diverges from the valid path', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue({
      save_path: 'C:\\Saves',
      auto_launch_game: false,
      auto_close: false,
      max_backups_per_game: 100,
    });

    const { createConfigFeature } = await import('./config');
    const elements = createElements();
    const setRefreshAvailability = vi.fn();

    const feature = createConfigFeature(elements, {
      loadBackups: vi.fn().mockResolvedValue(undefined),
      setRefreshAvailability,
    });

    await feature.loadConfig();

    elements.manualInput.value = 'C:\\Other';
    elements.manualInput.dispatchEvent(new Event('input', { bubbles: true }));

    expect(setRefreshAvailability).toHaveBeenLastCalledWith(false);
  });

  it('enables refresh after a successful save path validation', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === 'validate_path') {
        return Promise.resolve(true);
      }
      if (command === 'set_save_path') {
        return Promise.resolve('C:\\Saves');
      }
      return Promise.resolve(undefined);
    });

    const { createConfigFeature } = await import('./config');
    const elements = createElements();
    elements.manualInput.value = 'C:\\Saves';
    const setRefreshAvailability = vi.fn();

    const feature = createConfigFeature(elements, {
      loadBackups: vi.fn().mockResolvedValue(undefined),
      setRefreshAvailability,
    });

    await feature.savePath();

    expect(setRefreshAvailability).toHaveBeenLastCalledWith(true);
    expect(elements.manualInput.value).toBe('C:\\Saves');
  });
});
