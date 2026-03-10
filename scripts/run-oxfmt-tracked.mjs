import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const GIT_COMMAND = process.platform === "win32" ? "git.exe" : "git";
const RUST_EXTENSION = ".rs";
const OXFMT_ENTRYPOINT = path.resolve("node_modules", "oxfmt", "bin", "oxfmt");

/**
 * Splits NUL-delimited git output into individual file paths.
 *
 * @param {string} output Raw `git ls-files -z` output.
 * @returns {string[]} Parsed file paths.
 */
export function splitTrackedFiles(output) {
  return output.split("\0").filter(Boolean);
}

/**
 * Returns whether the file should remain in rustfmt's scope.
 *
 * @param {string} filePath Candidate file path.
 * @returns {boolean} True when the file is a Rust source file.
 */
export function isRustFile(filePath) {
  return path.extname(filePath).toLowerCase() === RUST_EXTENSION;
}

/**
 * Filters candidate files down to the set that oxfmt should receive.
 *
 * @param {string[]} filePaths Candidate file paths.
 * @returns {string[]} Non-Rust file paths.
 */
export function selectOxfmtFiles(filePaths) {
  return filePaths.filter((filePath) => !isRustFile(filePath));
}

/**
 * Separates oxfmt flags from optional file paths passed by lint-staged.
 *
 * @param {string[]} argv Raw CLI arguments.
 * @returns {{ files: string[], oxfmtArgs: string[] }} Parsed arguments.
 */
export function parseArguments(argv) {
  const files = [];
  const oxfmtArgs = [];

  for (const arg of argv) {
    if (arg.startsWith("-")) {
      oxfmtArgs.push(arg);
      continue;
    }

    files.push(arg);
  }

  return { files, oxfmtArgs };
}

/**
 * Reads the repository's tracked files when no explicit file list is provided.
 *
 * @param {(command: string, args: string[], options?: object) => { status: number | null, stdout?: string, stderr?: string }} run Spawn implementation.
 * @returns {string[]} Tracked file paths from git.
 */
export function readTrackedFiles(run = spawnSync) {
  const result = run(GIT_COMMAND, ["ls-files", "-z"], { encoding: "utf8" });

  if (result.error) {
    throw result.error;
  }

  if (result.status !== 0) {
    throw new Error(result.stderr || "Failed to read tracked files from git.");
  }

  return splitTrackedFiles(result.stdout ?? "");
}

/**
 * Runs oxfmt against either the provided file list or every tracked non-Rust file.
 *
 * @param {string[]} argv Raw wrapper CLI arguments.
 * @param {{
 *   run?: (command: string, args: string[], options?: object) => { status: number | null, stdout?: string, stderr?: string },
 *   log?: (message: string) => void,
 *   error?: (message: string) => void,
 * }} [dependencies] Injectable dependencies for tests.
 * @returns {number} Process exit code.
 */
export function runOxfmt(argv, dependencies = {}) {
  const run = dependencies.run ?? spawnSync;
  const log = dependencies.log ?? console.log;
  const error = dependencies.error ?? console.error;
  const { files, oxfmtArgs } = parseArguments(argv);

  try {
    const candidateFiles = files.length > 0 ? files : readTrackedFiles(run);
    const targetFiles = selectOxfmtFiles(candidateFiles);

    if (targetFiles.length === 0) {
      log("No non-Rust tracked files matched for oxfmt.");
      return 0;
    }

    const result = run(process.execPath, [OXFMT_ENTRYPOINT, ...oxfmtArgs, ...targetFiles], {
      stdio: "inherit",
    });

    if (result.error) {
      throw result.error;
    }

    return typeof result.status === "number" ? result.status : 1;
  } catch (caughtError) {
    const message =
      caughtError instanceof Error ? caughtError.message : "run-oxfmt-tracked failed unexpectedly.";
    error(message);
    return 1;
  }
}

if (
  process.argv[1] &&
  import.meta.url.startsWith("file:") &&
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1])
) {
  process.exit(runOxfmt(process.argv.slice(2)));
}
