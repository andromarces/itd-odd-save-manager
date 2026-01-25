import { logActivity, safeInvoke } from '../ui_utils';
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
}

export interface ConfigDependencies {
  loadBackups: () => Promise<void>;
}

/**
 * Creates the configuration and discovery feature.
 */
export function createConfigFeature(
  elements: ConfigElements,
  deps: ConfigDependencies,
): ConfigFeature {
  /**
   * Updates the configuration status message.
   */
  function setConfigStatus(message: string, type: StatusType = 'info'): void {
    elements.configStatus.textContent = message;
    elements.configStatus.className = 'status-text';
    if (type !== 'info') {
      elements.configStatus.classList.add(type);
    }
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
      onError: () => setConfigStatus('Failed to load configuration.', 'error'),
    });

    if (!config) return;

    if (config.save_path) {
      elements.manualInput.value = config.save_path;
      setConfigStatus('Configuration loaded.', 'info');
      void deps.loadBackups();
    } else {
      elements.manualInput.value = '';
      setConfigStatus('No save path configured.', 'info');
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
      setConfigStatus('Please enter a path.', 'error');
      return;
    }

    setConfigStatus('Validating...', 'info');
    elements.saveButton.disabled = true;

    try {
      const isValid = await safeInvoke<boolean>(
        'validate_path',
        { path },
        {
          actionName: 'validate path',
          onError: () => setConfigStatus('Error validating path.', 'error'),
        },
      );

      if (isValid === undefined) return;

      if (!isValid) {
        setConfigStatus('Path does not exist or is invalid.', 'error');
        logActivity(`Invalid path entered: ${path}`);
        return;
      }

      const normalizedPath = await safeInvoke<string>(
        'set_save_path',
        { path },
        {
          actionName: 'save path',
          onError: () => {
            setConfigStatus('Error saving path.', 'error');
            void loadConfig();
          },
        },
      );

      if (!normalizedPath) return;

      elements.manualInput.value = normalizedPath;
      setConfigStatus('Save path updated successfully.', 'success');
      logActivity(`Save path updated: ${normalizedPath}`);
      void deps.loadBackups();
    } finally {
      elements.saveButton.disabled = false;
    }
  }

  /**
   * Calls the backend command to detect save paths and updates the UI.
   */
  async function detectSteamSavePaths(): Promise<void> {
    elements.detectButton.disabled = true;
    elements.detectButton.textContent = 'Scanning...';
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
          setConfigStatus('Path detected. Click "Set Path" to save.', 'info');
        }
      } else {
        logActivity('No paths detected.');
      }
    }

    elements.detectButton.disabled = false;
    elements.detectButton.textContent = 'Auto Detect Save Path';
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
      setConfigStatus(
        'Path selected from list. Click "Set Path" to save.',
        'info',
      );
    }
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

  /**
   * Handles the auto-detect button click.
   */
  function handleDetectClick(): void {
    void detectSteamSavePaths();
  }

  /**
   * Handles the save path button click.
   */
  function handleSavePathClick(): void {
    void savePath();
  }

  elements.detectButton.addEventListener('click', handleDetectClick);
  elements.saveButton.addEventListener('click', handleSavePathClick);
  elements.pathsList.addEventListener('click', handlePathSelectionClick);

  return {
    loadConfig,
    savePath,
    detectSteamSavePaths,
    applyAutoDetectionAvailability,
  };
}
