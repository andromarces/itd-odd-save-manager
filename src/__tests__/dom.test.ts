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

  it('shows dashboard tab by default', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    const activeButtons = Array.from(elements.tabButtons).filter((btn) =>
      btn.classList.contains('active'),
    );
    const activePanels = Array.from(elements.tabPanels).filter((panel) =>
      panel.classList.contains('active'),
    );
    const visiblePanels = Array.from(elements.tabPanels).filter(
      (panel) => !panel.hidden,
    );

    expect(activeButtons.length).toBe(1);
    expect(activePanels.length).toBe(1);
    expect(visiblePanels.length).toBe(1);
    expect(activePanels[0]).toBe(visiblePanels[0]);
  });

  it('switches to settings tab when clicked', async () => {
    const { renderAppShell, setupTabNavigation } = await import('../ui/dom');
    const elements = renderAppShell();
    setupTabNavigation(elements);

    const inactiveButton = Array.from(elements.tabButtons).find(
      (btn) => !btn.classList.contains('active'),
    );
    expect(inactiveButton).toBeDefined();

    inactiveButton?.click();

    const activeButtons = Array.from(elements.tabButtons).filter((btn) =>
      btn.classList.contains('active'),
    );
    const visiblePanels = Array.from(elements.tabPanels).filter(
      (panel) => !panel.hidden,
    );

    expect(activeButtons.length).toBe(1);
    expect(activeButtons[0]).toBe(inactiveButton);
    expect(visiblePanels.length).toBe(1);
  });

  it('updates aria-selected on tab switch', async () => {
    const { renderAppShell, setupTabNavigation } = await import('../ui/dom');
    const elements = renderAppShell();
    setupTabNavigation(elements);

    const selectedButtons = Array.from(elements.tabButtons).filter(
      (btn) => btn.getAttribute('aria-selected') === 'true',
    );
    expect(selectedButtons.length).toBe(1);

    const unselectedButton = Array.from(elements.tabButtons).find(
      (btn) => btn.getAttribute('aria-selected') !== 'true',
    );
    expect(unselectedButton).toBeDefined();

    unselectedButton?.click();

    const selectedAfterClick = Array.from(elements.tabButtons).filter(
      (btn) => btn.getAttribute('aria-selected') === 'true',
    );
    expect(selectedAfterClick.length).toBe(1);
    expect(selectedAfterClick[0]).toBe(unselectedButton);
  });

  it('has proper ARIA relationship between tabs and panels', async () => {
    const { renderAppShell } = await import('../ui/dom');
    const elements = renderAppShell();

    for (const button of Array.from(elements.tabButtons)) {
      const controlsId = button.getAttribute('aria-controls');
      expect(controlsId).toBeTruthy();

      const controlledPanel = Array.from(elements.tabPanels).find(
        (panel) => panel.id === controlsId,
      );
      expect(controlledPanel).toBeDefined();

      const buttonId = button.id;
      expect(buttonId).toBeTruthy();
      expect(controlledPanel?.getAttribute('aria-labelledby')).toBe(buttonId);
    }
  });
});
