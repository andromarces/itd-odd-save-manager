import {
  logActivity,
  safeInvoke,
  updateStatus,
  withBusyButton,
} from '../ui_utils';
import type { AppElements } from './dom';
import type { AppConfig, StatusType } from './types';

type ConfigElements = Pick<
  AppElements,
  | 'manualInput'
  | 'saveButton'
  | 'configStatus'
  | 'detectButton'
  | 'pathsList'
  | 'autoLaunchCheck'
  | 'autoCloseCheck'
  | 'maxBackupsInput'
>;

export interface ConfigFeature {
  loadConfig: () => Promise<void>;
  savePath: () => Promise<void>;
  detectSteamSavePaths: () => Promise<void>;
  applyAutoDetectionAvailability: () => Promise<void>;
  destroy: () => void;
}

export interface ConfigDependencies {
  loadBackups: () => Promise<void>;
  setRefreshAvailability: (isEnabled: boolean) => void;
}

/**
 * Creates the configuration and discovery feature.
 */
export function createConfigFeature(
  elements: ConfigElements,
  deps: ConfigDependencies,
): ConfigFeature {
  let currentValidPath: string | null = null;

  /**
   * Updates the configuration status message.
   */
  function setStatus(message: string, type: StatusType = 'info'): void {
    updateStatus(elements.configStatus, message, type);
  }

  /**
   * Updates the refresh availability based on the current input and valid path.
   */
  function updateRefreshAvailability(): void {
    const inputValue = elements.manualInput.value.trim();
    const hasValidPath =
      currentValidPath !== null && inputValue === currentValidPath;
    deps.setRefreshAvailability(hasValidPath);
  }

  /**
   * Sets the current valid path and refresh availability state.
   */
  function setValidPath(path: string | null): void {
    currentValidPath = path;
    updateRefreshAvailability();
  }

  /**
   * Renders detected save paths into the list element.
   */
  function renderPaths(paths: string[]): void {
    elements.pathsList.innerHTML = '';

    if (paths.length === 0) {
      const item = document.createElement('li');
      item.textContent = 'No save paths detected.';
      item.classList.add('empty');
      elements.pathsList.appendChild(item);
      return;
    }

    const fragment = document.createDocumentFragment();
    for (const path of paths) {
      const item = document.createElement('li');
      item.textContent = path;
      item.title = 'Click to use this path';
      fragment.appendChild(item);
    }
    elements.pathsList.appendChild(fragment);
  }

  /**
   * Loads the current configuration from the backend.
   */
  async function loadConfig(): Promise<void> {
    const config = await safeInvoke<AppConfig>('get_config', undefined, {
      actionName: 'load config',
      onError: () => setStatus('Failed to load configuration.', 'error'),
    });

    if (!config) return;

    if (config.save_path) {
      elements.manualInput.value = config.save_path;
      setStatus('Configuration loaded.', 'info');
      setValidPath(config.save_path);
      void deps.loadBackups();
    } else {
      elements.manualInput.value = '';
      setStatus('No save path configured.', 'info');
      setValidPath(null);
    }

    elements.autoLaunchCheck.checked = config.auto_launch_game;
    elements.autoCloseCheck.checked = config.auto_close;
    elements.maxBackupsInput.value = config.max_backups_per_game.toString();

    logActivity('Configuration loaded.');
  }

  /**
   * Validates and saves the user-provided path.
   */
  async function savePath(): Promise<void> {
    const path = elements.manualInput.value.trim();
    if (!path) {
      setStatus('Please enter a path.', 'error');
      return;
    }

    await withBusyButton(elements.saveButton, 'Validating...', async () => {
      setStatus('Validating...', 'info');

      const isValid = await safeInvoke<boolean>(
        'validate_path',
        { path },
        {
          actionName: 'validate path',
          onError: () => setStatus('Error validating path.', 'error'),
        },
      );

      if (isValid === undefined) return;

      if (!isValid) {
        setStatus('Path does not exist or is invalid.', 'error');
        logActivity(`Invalid path entered: ${path}`);
        return;
      }

      const normalizedPath = await safeInvoke<string>(
        'set_save_path',
        { path },
        {
          actionName: 'save path',
          onError: () => {
            setStatus('Error saving path.', 'error');
            void loadConfig();
          },
        },
      );

      if (!normalizedPath) return;

      elements.manualInput.value = normalizedPath;
      setValidPath(normalizedPath);
      setStatus('Save path updated successfully.', 'success');
      logActivity(`Save path updated: ${normalizedPath}`);
      void deps.loadBackups();
    });
  }

  /**
   * Calls the backend command to detect save paths and updates the UI.
   */
  async function detectSteamSavePaths(): Promise<void> {
    await withBusyButton(elements.detectButton, 'Scanning...', async () => {
      logActivity('Scanning for save paths...');

      const paths = await safeInvoke<string[]>(
        'detect_steam_save_paths',
        undefined,
        {
          actionName: 'detect save paths',
          onError: () => {
            elements.pathsList.innerHTML =
              '<li class="error">Detection failed</li>';
          },
        },
      );

      if (paths) {
        renderPaths(paths);
        if (paths.length > 0) {
          logActivity(`Detected ${paths.length} potential paths.`);
          if (!elements.manualInput.value) {
            elements.manualInput.value = paths[0];
            setStatus('Path detected. Click "Set Path" to save.', 'info');
            updateRefreshAvailability();
          }
        } else {
          logActivity('No paths detected.');
        }
      }
    });
  }

  /**
   * Handles delegated clicks on the detected paths list.
   */
  function handlePathSelectionClick(event: Event): void {
    const target = event.target as HTMLElement;
    const li = target.closest('li');
    if (
      !li ||
      li.classList.contains('empty') ||
      li.classList.contains('error')
    ) {
      return;
    }

    const path = li.textContent;
    if (path) {
      elements.manualInput.value = path;
      setStatus('Path selected from list. Click "Set Path" to save.', 'info');
      updateRefreshAvailability();
    }
  }

  /**
   * Handles manual path input changes.
   */
  function handleManualPathInput(): void {
    updateRefreshAvailability();
  }

  /**
   * Applies platform-specific auto-detection availability to the UI.
   */
  async function applyAutoDetectionAvailability(): Promise<void> {
    const supported = await safeInvoke<boolean>(
      'is_auto_detection_supported',
      undefined,
      {
        actionName: 'check auto-detection support',
      },
    );

    if (supported !== false) return;

    elements.detectButton.remove();
    elements.pathsList.innerHTML = '';
    const item = document.createElement('li');
    item.textContent =
      'Auto-detection is only available on Windows. Enter a path manually.';
    item.classList.add('empty');
    elements.pathsList.appendChild(item);
  }

  const onDetectClick = () => void detectSteamSavePaths();
  const onSaveClick = () => void savePath();

  elements.detectButton.addEventListener('click', onDetectClick);
  elements.saveButton.addEventListener('click', onSaveClick);
  elements.pathsList.addEventListener('click', handlePathSelectionClick);
  elements.manualInput.addEventListener('input', handleManualPathInput);

  updateRefreshAvailability();

  return {
    loadConfig,
    savePath,
    detectSteamSavePaths,
    applyAutoDetectionAvailability,
    destroy: () => {
      elements.detectButton.removeEventListener('click', onDetectClick);
      elements.saveButton.removeEventListener('click', onSaveClick);
      elements.pathsList.removeEventListener('click', handlePathSelectionClick);
      elements.manualInput.removeEventListener('input', handleManualPathInput);
    },
  };
}
