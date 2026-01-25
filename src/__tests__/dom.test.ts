import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

/**
 * Initializes the DOM container expected by the app module.
 */
function setupDom(): void {
  document.body.innerHTML = '<div id="app"></div>';
}

describe('tab navigation', () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.clearAllMocks();
    setupDom();

    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it('renders three tab buttons', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    expect(elements.tabButtons.length).toBe(3);
    expect(elements.tabButtons[0].textContent).toBe('Dashboard');
    expect(elements.tabButtons[1].textContent).toBe('Settings');
    expect(elements.tabButtons[2].textContent).toBe('Log');
  });

  it('renders three tab panels', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    expect(elements.tabPanels.length).toBe(3);
    expect(elements.tabPanels[0].dataset.tabPanel).toBe('dashboard');
    expect(elements.tabPanels[1].dataset.tabPanel).toBe('settings');
    expect(elements.tabPanels[2].dataset.tabPanel).toBe('log');
  });

  it('shows dashboard tab by default', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    expect(elements.tabButtons[0].classList.contains('active')).toBe(true);
    expect(elements.tabPanels[0].classList.contains('active')).toBe(true);
    expect(elements.tabPanels[0].hidden).toBe(false);
    expect(elements.tabPanels[1].hidden).toBe(true);
    expect(elements.tabPanels[2].hidden).toBe(true);
  });

  it('switches to settings tab when clicked', async () => {
    const { renderAppShell, setupTabNavigation } = await import('../ui/dom');
    const elements = renderAppShell();
    setupTabNavigation(elements);

    elements.tabButtons[1].click();

    expect(elements.tabButtons[0].classList.contains('active')).toBe(false);
    expect(elements.tabButtons[1].classList.contains('active')).toBe(true);
    expect(elements.tabPanels[0].hidden).toBe(true);
    expect(elements.tabPanels[1].hidden).toBe(false);
  });

  it('updates aria-selected on tab switch', async () => {
    const { renderAppShell, setupTabNavigation } = await import('../ui/dom');
    const elements = renderAppShell();
    setupTabNavigation(elements);

    expect(elements.tabButtons[0].getAttribute('aria-selected')).toBe('true');
    expect(elements.tabButtons[1].getAttribute('aria-selected')).toBe('false');

    elements.tabButtons[1].click();

    expect(elements.tabButtons[0].getAttribute('aria-selected')).toBe('false');
    expect(elements.tabButtons[1].getAttribute('aria-selected')).toBe('true');
  });

  it('has proper ARIA relationship between tabs and panels', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    // Each tab should have aria-controls pointing to its panel
    expect(elements.tabButtons[0].getAttribute('aria-controls')).toBe(
      'panel-dashboard',
    );
    expect(elements.tabButtons[1].getAttribute('aria-controls')).toBe(
      'panel-settings',
    );
    expect(elements.tabButtons[2].getAttribute('aria-controls')).toBe(
      'panel-log',
    );

    // Each panel should have aria-labelledby pointing to its tab
    expect(elements.tabPanels[0].getAttribute('aria-labelledby')).toBe(
      'tab-dashboard',
    );
    expect(elements.tabPanels[1].getAttribute('aria-labelledby')).toBe(
      'tab-settings',
    );
    expect(elements.tabPanels[2].getAttribute('aria-labelledby')).toBe(
      'tab-log',
    );
  });
});
