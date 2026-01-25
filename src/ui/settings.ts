import { logActivity, safeInvoke } from '../ui_utils';
import type { AppElements } from './dom';

type SettingsElements = Pick<
  AppElements,
  'launchGameButton' | 'autoLaunchCheck' | 'autoCloseCheck' | 'maxBackupsInput'
>;

/**
 * Sets up the game launcher and settings feature.
 */
export function setupSettingsFeature(elements: SettingsElements): void {
  /**
   * Saves game settings (auto launch and auto close).
   */
  async function saveGameSettings(): Promise<void> {
    const autoLaunch = elements.autoLaunchCheck.checked;
    const autoClose = elements.autoCloseCheck.checked;
    const rawMax = parseInt(elements.maxBackupsInput.value, 10);
    const maxBackups = isNaN(rawMax) ? 100 : rawMax;

    await safeInvoke(
      'set_game_settings',
      {
        auto_launch_game: autoLaunch,
        auto_close: autoClose,
        max_backups_per_game: maxBackups,
      },
      {
        actionName: 'save game settings',
        successLog: `Updated game settings: Auto-Launch=${autoLaunch}, Auto-Close=${autoClose}, Max-Backups=${maxBackups}`,
      },
    );
  }

  /**
   * Launches the game via the backend command.
   */
  async function launchGame(): Promise<void> {
    logActivity('Launching game...');
    await safeInvoke('launch_game', undefined, {
      actionName: 'launch game',
      successLog: 'Game launch command sent.',
      alertOnError: true,
    });
  }

  /**
   * Handles the launch game button click.
   */
  function handleLaunchGameClick(): void {
    void launchGame();
  }

  /**
   * Handles the auto launch setting change.
   */
  function handleAutoLaunchChange(): void {
    void saveGameSettings();
  }

  /**
   * Handles the auto close setting change.
   */
  function handleAutoCloseChange(): void {
    void saveGameSettings();
  }

  elements.launchGameButton.addEventListener('click', handleLaunchGameClick);
  elements.autoLaunchCheck.addEventListener('change', handleAutoLaunchChange);
  elements.autoCloseCheck.addEventListener('change', handleAutoCloseChange);
  elements.maxBackupsInput.addEventListener(
    'change',
    () => void saveGameSettings(),
  );
}
