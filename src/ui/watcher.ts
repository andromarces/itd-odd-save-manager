import { invoke } from '@tauri-apps/api/core';
import { isInitWatcherDeferredError } from '../watcher_init';
import { getInvokeErrorMessage, logActivity } from '../ui_utils';

/**
 * Initializes the watcher after the UI is painted, retrying until visible.
 */
export function initWatcherAfterPaint(): void {
  /**
   * Attempts watcher initialization and retries if the UI is not visible yet.
   */
  const retryInitWatcher = (attempt: number): void => {
    void invoke('init_watcher')
      .then(() => {
        logActivity('Watcher initialized.');
      })
      .catch((error) => {
        const message = getInvokeErrorMessage(error);
        if (isInitWatcherDeferredError(message)) {
          const delayMs = Math.min(1000, 50 + attempt * 50);
          setTimeout(() => {
            retryInitWatcher(attempt + 1);
          }, delayMs);
          return;
        }

        logActivity(`Failed to initialize watcher: ${message}`);
      });
  };

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      retryInitWatcher(0);
    });
  });
}
