// ITD ODD Save Manager by andromarces

export const INIT_WATCHER_DEFERRED_MESSAGE = 'window not yet visible';

/**
 * Normalizes an invoke error into a readable string.
 */
export function getInvokeErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'object' && error !== null) {
    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }
  return String(error);
}

/**
 * Indicates whether init_watcher should be retried based on the error message.
 */
export function isInitWatcherDeferredError(message: string): boolean {
  return message.includes(INIT_WATCHER_DEFERRED_MESSAGE);
}
