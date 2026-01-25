// ITD ODD Save Manager by andromarces

export const INIT_WATCHER_DEFERRED_MESSAGE = 'window not yet visible';

/**
 * Indicates whether init_watcher should be retried based on the error message.
 */
export function isInitWatcherDeferredError(message: string): boolean {
  return message.includes(INIT_WATCHER_DEFERRED_MESSAGE);
}
