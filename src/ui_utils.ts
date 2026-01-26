import { invoke } from '@tauri-apps/api/core';

export interface SafeInvokeOptions {
  actionName?: string;
  successLog?: string;
  successAlert?: string;
  alertOnError?: boolean;
  onError?: (error: unknown) => void;
  logContainer?: HTMLDivElement;
}

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
  options: SafeInvokeOptions = {},
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

/**
 * A wrapper around safeInvoke that focuses on performing an action with standard feedback.
 * Useful for button clicks and simple actions.
 */
export async function invokeAction<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  actionDescription: string,
  options: Omit<SafeInvokeOptions, 'actionName'> = {},
): Promise<T | undefined> {
  return safeInvoke<T>(command, args, {
    actionName: actionDescription,
    ...options,
  });
}

/**
 * Helper to manage button state during async operations.
 * Captures the current text, disables the button, sets busy text, runs the action,
 * and finally restores the original state. Returns the result of the action.
 */
export async function withBusyButton<T>(
  btn: HTMLButtonElement,
  busyText: string,
  action: () => Promise<T>,
): Promise<T> {
  const originalText = btn.textContent;
  const wasDisabled = btn.disabled;
  btn.disabled = true;
  btn.textContent = busyText;
  try {
    return await action();
  } finally {
    btn.disabled = wasDisabled;
    btn.textContent = originalText;
  }
}

/**
 * Standardizes status text updates across the UI.
 * Applies the 'status-text' class and optionally 'success' or 'error'.
 */
export function updateStatus(
  element: HTMLElement,
  message: string,
  type: 'info' | 'success' | 'error' = 'info',
): void {
  element.textContent = message;
  element.classList.add('status-text');
  element.classList.toggle('success', type === 'success');
  element.classList.toggle('error', type === 'error');
}

const MONTH_ABBREVIATIONS = [
  'Jan',
  'Feb',
  'Mar',
  'Apr',
  'May',
  'Jun',
  'Jul',
  'Aug',
  'Sep',
  'Oct',
  'Nov',
  'Dec',
];

/**
 * Pads a number to two digits.
 */
function padTwoDigits(value: number): string {
  return value.toString().padStart(2, '0');
}

/**
 * Formats a local hour value into 12-hour time.
 */
function formatHour12(hours24: number): string {
  const hours12 = hours24 % 12 || 12;
  return padTwoDigits(hours12);
}

/**
 * Returns the meridiem marker for a local hour value.
 */
function formatMeridiem(hours24: number): string {
  return hours24 >= 12 ? 'PM' : 'AM';
}

/**
 * Formats an ISO date string as dd/MMM/yyyy hh:mm:ss AM/PM.
 */
export function formatDate(isoString: string): string {
  const date = new Date(isoString);
  if (Number.isNaN(date.getTime())) {
    return isoString;
  }

  const day = padTwoDigits(date.getDate());
  const month = MONTH_ABBREVIATIONS[date.getMonth()] ?? '???';
  const year = date.getFullYear().toString();
  const hours = formatHour12(date.getHours());
  const minutes = padTwoDigits(date.getMinutes());
  const seconds = padTwoDigits(date.getSeconds());
  const meridiem = formatMeridiem(date.getHours());

  return `${day}/${month}/${year} ${hours}:${minutes}:${seconds} ${meridiem}`;
}
