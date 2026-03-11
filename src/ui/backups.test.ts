import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BackupInfo } from "./types";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ui_utils", () => ({
  invokeAction: vi.fn(),
  logActivity: vi.fn(),
  formatDate: vi.fn((d: string) => d),
}));

vi.mock("./backups/dialog", () => ({
  MasterDeleteController: vi.fn().mockImplementation(function () {
    return { open: vi.fn(), destroy: vi.fn() };
  }),
}));

/** Builds a minimal BackupInfo for testing. */
function makeBackup(overrides: Partial<BackupInfo> = {}): BackupInfo {
  return {
    path: "/backups/save_1.sav",
    filename: "save_1.sav",
    original_filename: "save_1.sav",
    original_path: "/saves/save_1.sav",
    size: 1024,
    modified: "2025-01-01T12:00:00Z",
    game_number: 0,
    locked: false,
    hash: "abc123",
    note: null,
    ...overrides,
  };
}

/** Builds the minimal DOM elements required by createBackupsFeature. */
function setupDom(): {
  manualInput: HTMLInputElement;
  refreshBackupsButton: HTMLButtonElement;
  backupsTable: HTMLTableElement;
  backupsList: HTMLTableSectionElement;
  masterDeleteButton: HTMLButtonElement;
  masterDeleteDialog: HTMLDialogElement;
  masterDeleteForm: HTMLFormElement;
  masterDeleteGameList: HTMLElement;
  masterDeleteModeRadios: NodeListOf<HTMLInputElement>;
  masterDeleteLockedRadios: NodeListOf<HTMLInputElement>;
  masterDeleteCancelBtn: HTMLButtonElement;
  masterDeleteConfirmBtn: HTMLButtonElement;
} {
  document.body.innerHTML = `
    <input id="manual-input" value="/saves/game.sav" />
    <button id="refresh-btn"></button>
    <table id="backups-table">
      <tbody id="backups-list"></tbody>
    </table>
    <button id="master-delete-btn"></button>
    <dialog id="master-delete-dialog">
      <form id="master-delete-form">
        <div id="master-delete-game-list"></div>
        <div id="master-delete-modes"></div>
        <div id="master-delete-locked"></div>
        <button id="master-delete-cancel"></button>
        <button id="master-delete-confirm"></button>
      </form>
    </dialog>
  `;

  return {
    manualInput: document.getElementById("manual-input") as HTMLInputElement,
    refreshBackupsButton: document.getElementById("refresh-btn") as HTMLButtonElement,
    backupsTable: document.getElementById("backups-table") as HTMLTableElement,
    backupsList: document.getElementById("backups-list") as HTMLTableSectionElement,
    masterDeleteButton: document.getElementById("master-delete-btn") as HTMLButtonElement,
    masterDeleteDialog: document.getElementById("master-delete-dialog") as HTMLDialogElement,
    masterDeleteForm: document.getElementById("master-delete-form") as HTMLFormElement,
    masterDeleteGameList: document.getElementById("master-delete-game-list") as HTMLElement,
    masterDeleteModeRadios: document.querySelectorAll(
      "#master-delete-modes input",
    ) as NodeListOf<HTMLInputElement>,
    masterDeleteLockedRadios: document.querySelectorAll(
      "#master-delete-locked input",
    ) as NodeListOf<HTMLInputElement>,
    masterDeleteCancelBtn: document.getElementById("master-delete-cancel") as HTMLButtonElement,
    masterDeleteConfirmBtn: document.getElementById("master-delete-confirm") as HTMLButtonElement,
  };
}

describe("createBackupsFeature - loadBackups serialization", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  /**
   * Verifies that a second overlapping loadBackups call does not immediately
   * issue a concurrent backend request — it joins the in-flight promise.
   */
  it("does not issue a concurrent second backend call when two loads overlap", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolve!: (v: BackupInfo[]) => void;
    const deferred = new Promise<BackupInfo[]>((res) => {
      resolve = res;
    });
    vi.mocked(invokeAction).mockReturnValue(deferred);

    const feature = createBackupsFeature(elements);

    feature.loadBackups();
    feature.loadBackups(); // Joins in-flight — must not start a second concurrent request

    // Before the deferred resolves: only one backend call should have been issued
    expect(vi.mocked(invokeAction)).toHaveBeenCalledTimes(1);

    resolve([makeBackup({ filename: "once.sav", path: "/b/once.sav" })]);
    await deferred; // Let the first load settle

    expect(elements.backupsList.innerHTML).toContain("once.sav");

    feature.destroy();
  });

  /**
   * Verifies that both overlapping callers receive the shared result.
   */
  it("returns the same promise to both overlapping callers", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolve!: (v: BackupInfo[]) => void;
    const deferred = new Promise<BackupInfo[]>((res) => {
      resolve = res;
    });
    vi.mocked(invokeAction).mockReturnValue(deferred);

    const feature = createBackupsFeature(elements);

    const call1 = feature.loadBackups();
    const call2 = feature.loadBackups();

    expect(call1).toBe(call2);

    resolve([]);
    await Promise.all([call1, call2]);

    feature.destroy();
  });

  /**
   * Verifies that a subsequent load after completion issues a new backend request.
   */
  it("issues a new backend call after the previous load completes", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const backup = makeBackup({ filename: "seq.sav", path: "/b/seq.sav" });
    vi.mocked(invokeAction).mockResolvedValue([backup]);

    const feature = createBackupsFeature(elements);

    await feature.loadBackups();
    await feature.loadBackups();

    expect(vi.mocked(invokeAction)).toHaveBeenCalledTimes(2);

    feature.destroy();
  });

  /**
   * Verifies that a single load call applies its result normally.
   */
  it("applies the result when only one call is in flight", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const backup = makeBackup({ filename: "only.sav", path: "/b/only.sav" });
    vi.mocked(invokeAction).mockResolvedValue([backup]);

    const feature = createBackupsFeature(elements);
    await feature.loadBackups();

    expect(elements.backupsList.innerHTML).toContain("only.sav");

    feature.destroy();
  });

  /**
   * Verifies that a force load while another is in flight starts a new backend
   * call immediately and discards the superseded in-flight result.
   */
  it("force load starts a new request and discards the superseded in-flight result", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const oldBackup = makeBackup({ filename: "old.sav", path: "/b/old.sav" });
    const newBackup = makeBackup({ filename: "new.sav", path: "/b/new.sav" });

    let resolveOld!: (v: BackupInfo[]) => void;
    let resolveNew!: (v: BackupInfo[]) => void;

    vi.mocked(invokeAction)
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveOld = res;
        }),
      )
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveNew = res;
        }),
      );

    const feature = createBackupsFeature(elements);

    const oldCall = feature.loadBackups(); // In-flight with old path
    const newCall = feature.loadBackups(true); // Force: path changed

    // Both backend calls should have been issued immediately
    expect(vi.mocked(invokeAction)).toHaveBeenCalledTimes(2);

    // Old result arrives — should be discarded (generation mismatch)
    resolveOld([oldBackup]);
    await oldCall;
    expect(elements.backupsList.innerHTML).not.toContain("old.sav");

    // New result arrives — should be applied
    resolveNew([newBackup]);
    await newCall;
    expect(elements.backupsList.innerHTML).toContain("new.sav");

    feature.destroy();
  });

  /**
   * Verifies that the abandoned in-flight promise's finally handler does not
   * clobber the new in-flight reference set by a force load.
   */
  it("abandoned in-flight finally does not clear the new in-flight reference", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolveOld!: (v: BackupInfo[]) => void;
    let resolveNew!: (v: BackupInfo[]) => void;

    vi.mocked(invokeAction)
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveOld = res;
        }),
      )
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveNew = res;
        }),
      );

    const feature = createBackupsFeature(elements);

    feature.loadBackups();
    const newCall = feature.loadBackups(true);

    // Old settles — its finally should not touch the new in-flight
    resolveOld([]);
    await Promise.resolve(); // Flush microtasks from old finally

    // The new call should still be resolvable (in-flight not clobbered)
    const backup = makeBackup({ filename: "intact.sav", path: "/b/intact.sav" });
    resolveNew([backup]);
    await newCall;

    expect(elements.backupsList.innerHTML).toContain("intact.sav");

    feature.destroy();
  });

  /**
   * Verifies that a refresh requested while a load is in flight triggers a
   * follow-up fetch once the current load completes.
   */
  it("schedules a follow-up fetch when a refresh arrives during an active load", async () => {
    const elements = setupDom();

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const followUpBackup = makeBackup({ filename: "follow.sav", path: "/b/follow.sav" });

    let resolveFirst!: (v: BackupInfo[]) => void;

    vi.mocked(invokeAction)
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveFirst = res;
        }),
      )
      .mockResolvedValue([followUpBackup]);

    const feature = createBackupsFeature(elements);

    const first = feature.loadBackups(); // In-flight
    feature.loadBackups(); // Event arrives — marks pendingRefresh

    // Only one backend call so far
    expect(vi.mocked(invokeAction)).toHaveBeenCalledTimes(1);

    resolveFirst([]); // First load completes; .finally() triggers the follow-up
    await first; // first resolves after .finally() completes

    // Follow-up backend call should have been issued by .finally()
    expect(vi.mocked(invokeAction)).toHaveBeenCalledTimes(2);

    feature.destroy();
  });
});

describe("createBackupsFeature - rowMap integrity after failed load", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = "";
  });

  /**
   * Regression: a failed load must clear rowMap so that a subsequent lock action
   * takes the safe renderBackups fallback instead of calling replaceChild on a
   * detached row, which would throw NotFoundError.
   */
  it("falls back to renderBackups on lock action after a failed load clears rowMap", async () => {
    const elements = setupDom();
    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const backup = makeBackup({ filename: "stale.sav", path: "/b/stale.sav" });

    vi.mocked(invokeAction)
      .mockResolvedValueOnce([backup]) // first load: success — rowMap populated
      .mockImplementationOnce(
        async (
          _action: unknown,
          _params: unknown,
          _label: unknown,
          options?: { onError?: () => void },
        ) => {
          options?.onError?.(); // second load: failure — clears rowMap and sets error innerHTML
          return undefined;
        },
      )
      .mockResolvedValueOnce(true); // toggle lock: success

    const feature = createBackupsFeature(elements);

    await feature.loadBackups();
    await feature.loadBackups();

    expect(elements.backupsList.innerHTML).toContain("error");

    // Inject a synthetic button and fire the lock action via table-level delegation
    const btn = document.createElement("button");
    btn.dataset.backupId = backup.path;
    btn.dataset.action = "lock";
    elements.backupsList.appendChild(btn);
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }));

    // Fallback renderBackups must re-render the backup without throwing
    await vi.waitUntil(() => elements.backupsList.innerHTML.includes("stale.sav"));
    expect(elements.backupsList.innerHTML).not.toContain("error");

    feature.destroy();
  });

  /**
   * Regression: a failed load must clear rowMap so that a subsequent note action
   * takes the safe renderBackups fallback instead of calling replaceChild on a
   * detached row, which would throw NotFoundError.
   */
  it("falls back to renderBackups on note action after a failed load clears rowMap", async () => {
    const elements = setupDom();
    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    const backup = makeBackup({ filename: "stale.sav", path: "/b/stale.sav" });
    vi.spyOn(window, "prompt").mockReturnValue("updated note");

    vi.mocked(invokeAction)
      .mockResolvedValueOnce([backup]) // first load: success — rowMap populated
      .mockImplementationOnce(
        async (
          _action: unknown,
          _params: unknown,
          _label: unknown,
          options?: { onError?: () => void },
        ) => {
          options?.onError?.(); // second load: failure — clears rowMap and sets error innerHTML
          return undefined;
        },
      )
      .mockResolvedValueOnce(true); // set note: success

    const feature = createBackupsFeature(elements);

    await feature.loadBackups();
    await feature.loadBackups();

    expect(elements.backupsList.innerHTML).toContain("error");

    // Inject a synthetic button and fire the note action via table-level delegation
    const btn = document.createElement("button");
    btn.dataset.backupId = backup.path;
    btn.dataset.action = "note";
    elements.backupsList.appendChild(btn);
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }));

    // Fallback renderBackups must re-render the backup without throwing
    await vi.waitUntil(() => elements.backupsList.innerHTML.includes("stale.sav"));
    expect(elements.backupsList.innerHTML).not.toContain("error");

    feature.destroy();
  });
});

describe("createBackupsFeature - refresh button state", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  /**
   * Verifies the button is disabled and shows the busy label while a load is active,
   * then restored to its original state after completion.
   */
  it("disables the button during a load and restores it after completion", async () => {
    const elements = setupDom();
    elements.refreshBackupsButton.textContent = "Refresh";
    elements.refreshBackupsButton.disabled = false;

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolve!: (v: BackupInfo[]) => void;
    vi.mocked(invokeAction).mockReturnValue(
      new Promise((res) => {
        resolve = res;
      }),
    );

    const feature = createBackupsFeature(elements);
    const load = feature.loadBackups();

    expect(elements.refreshBackupsButton.disabled).toBe(true);
    expect(elements.refreshBackupsButton.textContent).toBe("Refreshing...");

    resolve([]);
    await load;

    expect(elements.refreshBackupsButton.disabled).toBe(false);
    expect(elements.refreshBackupsButton.textContent).toBe("Refresh");

    feature.destroy();
  });

  /**
   * Verifies the button stays disabled throughout a forced load while an old
   * load is still in flight, and is only restored after the forced load finishes.
   */
  it("keeps the button busy for the full duration of a force load over an old in-flight", async () => {
    const elements = setupDom();
    elements.refreshBackupsButton.textContent = "Refresh";
    elements.refreshBackupsButton.disabled = false;

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolveOld!: (v: BackupInfo[]) => void;
    let resolveNew!: (v: BackupInfo[]) => void;

    vi.mocked(invokeAction)
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveOld = res;
        }),
      )
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveNew = res;
        }),
      );

    const feature = createBackupsFeature(elements);

    const oldCall = feature.loadBackups();
    const newCall = feature.loadBackups(true);

    // Old completes — button must stay busy because force load is still running
    resolveOld([]);
    await oldCall;

    expect(elements.refreshBackupsButton.disabled).toBe(true);
    expect(elements.refreshBackupsButton.textContent).toBe("Refreshing...");

    // Force load completes — button must now restore
    resolveNew([]);
    await newCall;

    expect(elements.refreshBackupsButton.disabled).toBe(false);
    expect(elements.refreshBackupsButton.textContent).toBe("Refresh");

    feature.destroy();
  });

  /**
   * Verifies the button stays busy during a pending follow-up load triggered by
   * a refresh event arriving mid-flight, and is only restored after the follow-up.
   */
  it("keeps the button busy through a pending follow-up load", async () => {
    const elements = setupDom();
    elements.refreshBackupsButton.textContent = "Refresh";
    elements.refreshBackupsButton.disabled = false;

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolveFirst!: (v: BackupInfo[]) => void;
    let resolveFollowUp!: (v: BackupInfo[]) => void;

    vi.mocked(invokeAction)
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveFirst = res;
        }),
      )
      .mockReturnValueOnce(
        new Promise((res) => {
          resolveFollowUp = res;
        }),
      );

    const feature = createBackupsFeature(elements);

    const first = feature.loadBackups();
    feature.loadBackups(); // Marks pendingRefresh

    // First load completes — follow-up starts synchronously in .finally()
    resolveFirst([]);
    await first;

    // Button must still be busy while the follow-up load runs
    expect(elements.refreshBackupsButton.disabled).toBe(true);
    expect(elements.refreshBackupsButton.textContent).toBe("Refreshing...");

    // Follow-up completes — button restores
    resolveFollowUp([]);
    // Wait for the follow-up promise (it is the new loadInFlight, not tracked directly)
    await Promise.resolve();
    await vi.waitUntil(() => !elements.refreshBackupsButton.disabled);

    expect(elements.refreshBackupsButton.textContent).toBe("Refresh");

    feature.destroy();
  });

  /**
   * Verifies that setRefreshAvailability called during a load is honored on
   * completion rather than being overwritten by a stale pre-load snapshot.
   */
  it("honors a setRefreshAvailability(false) call that occurs during a load", async () => {
    const elements = setupDom();
    elements.refreshBackupsButton.textContent = "Refresh";
    elements.refreshBackupsButton.disabled = false;

    const { invokeAction } = await import("../ui_utils");
    const { createBackupsFeature } = await import("./backups");

    let resolve!: (v: BackupInfo[]) => void;
    vi.mocked(invokeAction).mockReturnValue(
      new Promise((res) => {
        resolve = res;
      }),
    );

    const feature = createBackupsFeature(elements);
    feature.setRefreshAvailability(true);

    const load = feature.loadBackups();

    // Path becomes invalid during the load
    feature.setRefreshAvailability(false);

    resolve([]);
    await load;

    // Button must remain disabled — availability was revoked mid-load
    expect(elements.refreshBackupsButton.disabled).toBe(true);

    feature.destroy();
  });
});
