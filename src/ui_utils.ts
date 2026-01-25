import { invoke } from '@tauri-apps/api/core';

/**
 * Helper to safely query DOM elements. Throws if not found.
 */
export function getElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) {
    throw new Error(`Element not found: ${selector}`);
  }
  return element;
}

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

let cachedLogContainer: HTMLDivElement | null = null;

/**
 * Appends a message to the activity log with a timestamp.
 */
export function logActivity(
  message: string,
  logContainer?: HTMLDivElement,
): void {
  // Invalidated stale cache if element is removed from DOM
  if (cachedLogContainer && !document.body.contains(cachedLogContainer)) {
    cachedLogContainer = null;
  }

  const container =
    logContainer ||
    cachedLogContainer ||
    (cachedLogContainer =
      document.querySelector<HTMLDivElement>('#activity-log'));
  if (!container) return;

  const entry = document.createElement('div');
  entry.className = 'log-entry';

  const time = document.createElement('span');
  time.className = 'time';
  time.textContent = new Date().toLocaleTimeString();

  entry.appendChild(time);
  entry.appendChild(document.createTextNode(message));

  container.appendChild(entry);

  // Cap the log size
  const MAX_LOG_ENTRIES = 100;
  while (container.childElementCount > MAX_LOG_ENTRIES) {
    container.firstElementChild?.remove();
  }

  container.scrollTop = container.scrollHeight;
}

/**
 * Helper to safely invoke Tauri commands with standardized logging and error handling.
 */
export async function safeInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  options: {
    actionName?: string;
    successLog?: string;
    successAlert?: string;
    alertOnError?: boolean;
    onError?: (error: unknown) => void;
    logContainer?: HTMLDivElement;
  } = {},
): Promise<T | undefined> {
  const action = options.actionName || command;
  try {
    const data = await invoke<T>(command, args);

    if (options.successLog) {
      logActivity(options.successLog, options.logContainer);
    }

    if (options.successAlert) {
      alert(options.successAlert);
    }

    return data;
  } catch (error) {
    const msg = `Failed to ${action}`;
    const errorStr = getInvokeErrorMessage(error);

    console.error(`${msg}:`, error);
    logActivity(`${msg}: ${errorStr}`, options.logContainer);

    if (options.alertOnError) {
      alert(`${msg}: ${errorStr}`);
    }

    if (options.onError) {
      options.onError(error);
    }

    return undefined;
  }
}
