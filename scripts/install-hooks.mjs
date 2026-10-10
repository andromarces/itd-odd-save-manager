import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const GIT_COMMAND = process.platform === "win32" ? "git.exe" : "git";
const VP_COMMAND = "vp";
const SHELL_ON_WIN = { shell: process.platform === "win32" };
const HUSKY_HOOKS_PATH = ".husky/_";

/**
 * Installs the Vite+ git hook dispatcher.
 *
 * `vp config` skips the install while `core.hooksPath` holds another value, and a husky
 * clone keeps `.husky/_`. This clears a local `core.hooksPath` of exactly `.husky/_` first.
 * Any other local value, or no value, is left unchanged.
 *
 * @param {{
 *   run?: (command: string, args: string[], options?: object) => { status: number | null, stdout?: string },
 * }} [dependencies] Injectable dependencies for tests.
 * @returns {number} Exit code of `vp config`, or 1 when it cannot run.
 */
export function installHooks({ run = spawnSync } = {}) {
  const current = run(GIT_COMMAND, ["config", "--local", "--get", "core.hooksPath"], {
    encoding: "utf8",
  });

  if (current.status === 0 && (current.stdout ?? "").trim() === HUSKY_HOOKS_PATH) {
    run(GIT_COMMAND, ["config", "--local", "--unset", "core.hooksPath"], { stdio: "inherit" });
  }

  const result = run(VP_COMMAND, ["config", "--no-agent"], { stdio: "inherit", ...SHELL_ON_WIN });
  return typeof result.status === "number" ? result.status : 1;
}

if (
  process.argv[1] &&
  import.meta.url.startsWith("file:") &&
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1])
) {
  process.exit(installHooks());
}
