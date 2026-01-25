import { describe, expect, it } from 'vitest';

import {
  getInvokeErrorMessage,
  INIT_WATCHER_DEFERRED_MESSAGE,
  isInitWatcherDeferredError,
} from './watcher_init';

describe('getInvokeErrorMessage', () => {
  it('returns the message for Error instances', () => {
    const message = getInvokeErrorMessage(new Error('boom'));
    expect(message).toBe('boom');
  });

  it('stringifies plain objects', () => {
    const message = getInvokeErrorMessage({ code: 'E_INIT' });
    expect(message).toBe('{"code":"E_INIT"}');
  });

  it('returns strings as-is', () => {
    const message = getInvokeErrorMessage('simple error');
    expect(message).toBe('simple error');
  });
});

describe('isInitWatcherDeferredError', () => {
  it('returns true when the deferred message is present', () => {
    const message = `Watcher initialization deferred: ${INIT_WATCHER_DEFERRED_MESSAGE}`;
    expect(isInitWatcherDeferredError(message)).toBe(true);
  });

  it('returns false for unrelated errors', () => {
    expect(isInitWatcherDeferredError('permission denied')).toBe(false);
  });
});
