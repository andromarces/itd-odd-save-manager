import { describe, expect, it, vi } from "vite-plus/test";

import { installHooks } from "./install-hooks.mjs";

/**
 * Builds a spawn stub that answers the `git config --local --get core.hooksPath` read
 * with the given outcome and succeeds for every other command.
 */
function createRun(hooksPathRead, vpStatus = 0) {
  return vi.fn((command, args) => {
    if (args.includes("--get")) return hooksPathRead;
    if (args.includes("config") && args.includes("--no-agent")) return { status: vpStatus };
    return { status: 0 };
  });
}

const unsetCalls = (run) => run.mock.calls.filter(([, args]) => args.includes("--unset"));
const vpCalls = (run) => run.mock.calls.filter(([, args]) => args.includes("--no-agent"));

describe("installHooks", () => {
  it("clears a local husky core.hooksPath before installing the Vite+ hooks", () => {
    // Req: an existing husky clone must not keep `vp config` from installing the dispatcher.
    const run = createRun({ status: 0, stdout: ".husky/_\n" });

    expect(installHooks({ run })).toBe(0);

    expect(unsetCalls(run)).toHaveLength(1);
    expect(unsetCalls(run)[0][1]).toEqual(["config", "--local", "--unset", "core.hooksPath"]);
    expect(vpCalls(run)).toHaveLength(1);
    expect(run.mock.calls.indexOf(unsetCalls(run)[0])).toBeLessThan(
      run.mock.calls.indexOf(vpCalls(run)[0]),
    );
  });

  it("leaves a custom local core.hooksPath unchanged", () => {
    // Req: a clone with its own hooks path keeps that value; only the Vite+ install runs.
    const run = createRun({ status: 0, stdout: "tools/hooks\n" });

    expect(installHooks({ run })).toBe(0);

    expect(unsetCalls(run)).toHaveLength(0);
    expect(vpCalls(run)).toHaveLength(1);
  });

  it("installs the Vite+ hooks when no local core.hooksPath is set", () => {
    // Req: a fresh clone (git exits 1 for an unset key) installs without any unset call.
    const run = createRun({ status: 1, stdout: "" });

    expect(installHooks({ run })).toBe(0);

    expect(unsetCalls(run)).toHaveLength(0);
    expect(vpCalls(run)).toHaveLength(1);
  });

  it("treats a failed core.hooksPath read as no value", () => {
    // Req: a git read error must not abort the install or unset anything.
    const run = createRun({ status: null, error: new Error("spawn failed") });

    expect(installHooks({ run })).toBe(0);

    expect(unsetCalls(run)).toHaveLength(0);
    expect(vpCalls(run)).toHaveLength(1);
  });

  it("exits with the exit code of `vp config`", () => {
    // Req: install failures must fail `pnpm install` through the prepare script.
    const run = createRun({ status: 1, stdout: "" }, 3);

    expect(installHooks({ run })).toBe(3);
  });

  it("returns 1 when `vp config` cannot be spawned", () => {
    // Req: a missing exit status (spawn failure) must not read as success.
    const run = vi.fn((command, args) =>
      args.includes("--get") ? { status: 1, stdout: "" } : { status: null },
    );

    expect(installHooks({ run })).toBe(1);
  });
});
