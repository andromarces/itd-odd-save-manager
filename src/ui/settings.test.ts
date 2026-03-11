import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

/**
 * Builds the minimal DOM elements required by the settings feature.
 */
function createElements(): {
  launchGameButton: HTMLButtonElement;
  autoLaunchCheck: HTMLInputElement;
  autoCloseCheck: HTMLInputElement;
  maxBackupsInput: HTMLInputElement;
} {
  const launchGameButton = document.createElement("button");
  const autoLaunchCheck = document.createElement("input");
  autoLaunchCheck.type = "checkbox";
  const autoCloseCheck = document.createElement("input");
  autoCloseCheck.type = "checkbox";
  const maxBackupsInput = document.createElement("input");
  maxBackupsInput.type = "number";

  return { launchGameButton, autoLaunchCheck, autoCloseCheck, maxBackupsInput };
}

describe("setupSettingsFeature max-backups normalization", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it("passes a positive integer through unchanged", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const { setupSettingsFeature } = await import("./settings");
    const elements = createElements();
    elements.maxBackupsInput.value = "50";

    setupSettingsFeature(elements);
    elements.maxBackupsInput.dispatchEvent(new Event("change"));

    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    const args = vi.mocked(invoke).mock.calls[0][1] as Record<string, unknown>;
    expect(args["max_backups_per_game"]).toBe(50);
  });

  it("falls back to 100 when the input is empty", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const { setupSettingsFeature } = await import("./settings");
    const elements = createElements();
    elements.maxBackupsInput.value = "";

    setupSettingsFeature(elements);
    elements.maxBackupsInput.dispatchEvent(new Event("change"));

    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    const args = vi.mocked(invoke).mock.calls[0][1] as Record<string, unknown>;
    expect(args["max_backups_per_game"]).toBe(100);
  });

  it("clamps a negative value to 0", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const { setupSettingsFeature } = await import("./settings");
    const elements = createElements();
    elements.maxBackupsInput.value = "-5";

    setupSettingsFeature(elements);
    elements.maxBackupsInput.dispatchEvent(new Event("change"));

    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    const args = vi.mocked(invoke).mock.calls[0][1] as Record<string, unknown>;
    expect(args["max_backups_per_game"]).toBe(0);
  });

  it("normalizes a negative value back into the input element", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockResolvedValue(undefined);
    const { setupSettingsFeature } = await import("./settings");
    const elements = createElements();
    elements.maxBackupsInput.value = "-5";

    setupSettingsFeature(elements);
    elements.maxBackupsInput.dispatchEvent(new Event("change"));

    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(elements.maxBackupsInput.value).toBe("0");
  });

  it("truncates a decimal to its integer part", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    const { setupSettingsFeature } = await import("./settings");
    const elements = createElements();
    elements.maxBackupsInput.value = "3.7";

    setupSettingsFeature(elements);
    elements.maxBackupsInput.dispatchEvent(new Event("change"));

    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    const args = vi.mocked(invoke).mock.calls[0][1] as Record<string, unknown>;
    expect(args["max_backups_per_game"]).toBe(3);
  });
});
