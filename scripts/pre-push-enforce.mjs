import { readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

import { runOxfmt } from "./run-oxfmt-tracked.mjs";

const GIT_COMMAND = process.platform === "win32" ? "git.exe" : "git";
const RUST_EXT = ".rs";
const SHELL_ON_WIN = { shell: process.platform === "win32" };

const JSTS_EXTENSIONS = new Set([
  ".js",
  ".mjs",
  ".cjs",
  ".ts",
  ".mts",
  ".cts",
  ".jsx",
  ".tsx",
  ".vue",
  ".svelte",
  ".astro",
]);

const OXFMT_EXTENSIONS = new Set([
  ".css",
  ".html",
  ".json",
  ".json5",
  ".jsonc",
  ".js",
  ".jsx",
  ".md",
  ".mdx",
  ".ts",
  ".tsx",
  ".toml",
  ".vue",
  ".yaml",
  ".yml",
]);

/** Named JS/TS function declaration — captures name in group 4. */
const NAMED_FUNCTION_RE = /^\s*(export\s+)?(default\s+)?(async\s+)?function\s+(\w+)\s*\(/;

/** Exported const arrow function — captures name in group 1. */
const EXPORTED_ARROW_RE = /^\s*export\s+const\s+(\w+)\s*=\s*(async\s+)?\(/;

/** Rust public function — captures name in group 2. */
const RUST_PUB_FN_RE = /^\s*pub\s+(async\s+)?fn\s+(\w+)\s*[<(]/;

const SECRET_PATTERNS = [
  /\b(password|passwd|pwd)\s*[=:]\s*\S+/i,
  /\b(api[_-]?key|apikey)\s*[=:]\s*\S+/i,
  /\b(secret|token)\s*[=:]\s*(?!\w+\s*\()(?!\w+\.\w)\S+/i,
  /\b(private[_-]?key)\s*[=:]\s*\S+/i,
  /-----BEGIN\s+(RSA\s+)?PRIVATE\s+KEY-----/,
];

const NOSCAN_RE = /\bnoscan\b/i;

/** Detects any eslint or oxlint disable directive in a JS/TS source line. */
const JSTS_SUPPRESS_ANY_RE = /(eslint-disable|oxlint-disable)/;

/**
 * Matches the only allowed suppression form: a line comment scoped to the next or current
 * line, followed by at least one explicit rule name.
 */
const JSTS_SUPPRESS_ALLOWED_RE = /\/\/\s*(eslint|oxlint)-disable-(next-line|line)\s+\S/;

/**
 * Matches JS/TS string literals so their content can be stripped before suppression
 * scanning. Prevents false positives when directive keywords appear inside strings.
 * Applied per-line; handles single-line double-quoted, single-quoted, and template literals.
 */
const STRING_LITERAL_RE = /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`/g;

/**
 * Matches template literals across multiple lines for full-file preprocessing before
 * per-line suppression scanning. Newlines inside the literal are preserved so that
 * line numbers remain stable after blanking; all other interior characters are replaced
 * with spaces.
 */
const MULTILINE_TEMPLATE_RE = /`(?:[^`\\]|\\.)*`/g;

/** Detects Rust inner allow attributes that apply at crate or module scope. */
const RUST_INNER_ALLOW_RE = /#!\[allow\(/;

/** Detects Rust allow attributes for broad lint categories that policy prohibits. */
const RUST_BROAD_ALLOW_RE = /#\[allow\(clippy::(all|restriction)\b/;

const FORBIDDEN_NAMES = new Set([".env"]);
const FORBIDDEN_PREFIX = ".env.";

/**
 * Reads a file's content as UTF-8 text.
 *
 * @param {string} filePath - Path to the file.
 * @returns {string} File content.
 */
function defaultReadFile(filePath) {
  return readFileSync(filePath, "utf8");
}

/**
 * Checks whether a preceding non-empty line is a documentation comment.
 *
 * @param {string[]} lines - All lines of the file.
 * @param {number} index - Line index of the function declaration.
 * @param {boolean} isRust - Whether to apply Rust doc-comment rules.
 * @returns {boolean} True when a doc comment is found immediately before the function.
 */
function hasPrecedingDocComment(lines, index, isRust) {
  for (let i = index - 1; i >= 0; i--) {
    const trimmed = lines[i].trim();
    if (!trimmed) continue;

    if (isRust) {
      if (trimmed.startsWith("///") || trimmed.startsWith("//!")) return true;
      if (trimmed.startsWith("#[") || trimmed.startsWith("#![")) continue;
      break;
    } else {
      if (trimmed === "*/" || trimmed.endsWith("*/")) return true;
      if (trimmed.startsWith("//")) return true;
      if (trimmed.startsWith("@")) continue;
      break;
    }
  }
  return false;
}

/**
 * Resolves the upstream diff base for the current branch.
 *
 * Returns "@{upstream}" when configured, a merge-base SHA when a remote default
 * branch is reachable, or null when no base can be determined.
 *
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null, stdout?: string }} [run] - Spawn implementation.
 * @returns {string | null} Base ref or null.
 */
export function resolveBase(run = spawnSync) {
  const upstreamResult = run(
    GIT_COMMAND,
    ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"],
    { encoding: "utf8" },
  );

  if (upstreamResult.status === 0) {
    return "@{upstream}";
  }

  for (const candidate of ["origin/main", "origin/master"]) {
    const verifyResult = run(GIT_COMMAND, ["rev-parse", "--verify", candidate], {
      encoding: "utf8",
    });
    if (verifyResult.status !== 0) continue;

    const mergeBaseResult = run(GIT_COMMAND, ["merge-base", "HEAD", candidate], {
      encoding: "utf8",
    });
    if (mergeBaseResult.status === 0) {
      return (mergeBaseResult.stdout ?? "").trim();
    }
  }

  return null;
}

/**
 * Returns the list of files changed between base and HEAD.
 *
 * @param {string} base - Diff base ref or SHA.
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null, stdout?: string }} [run] - Spawn implementation.
 * @returns {string[]} Changed file paths.
 */
export function getChangedFiles(base, run = spawnSync) {
  const result = run(GIT_COMMAND, ["diff", "--name-only", "--diff-filter=d", `${base}...HEAD`], {
    encoding: "utf8",
  });

  if (result.status !== 0) {
    throw new Error((result.stderr ?? "") || "Failed to retrieve changed files from git.");
  }

  return (result.stdout ?? "").trim().split("\n").filter(Boolean);
}

/**
 * Returns lines added in the diff between base and HEAD, excluding diff headers.
 *
 * @param {string} base - Diff base ref or SHA.
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null, stdout?: string }} [run] - Spawn implementation.
 * @returns {string[]} Added lines (prefixed with "+").
 */
export function getAddedLines(base, run = spawnSync) {
  const result = run(GIT_COMMAND, ["diff", `${base}...HEAD`, "--unified=0"], {
    encoding: "utf8",
  });

  if (result.status !== 0) return [];

  return (result.stdout ?? "")
    .split("\n")
    .filter((line) => line.startsWith("+") && !line.startsWith("+++"));
}

/**
 * Classifies changed files by tool scope.
 *
 * @param {string[]} files - Changed file paths.
 * @returns {{ jsTs: string[], rust: string[], oxfmt: string[] }} Classified file lists.
 */
export function classifyFiles(files) {
  const jsTs = [];
  const rust = [];
  const oxfmt = [];

  for (const file of files) {
    const ext = path.extname(file).toLowerCase();

    if (ext === RUST_EXT) {
      rust.push(file);
    } else {
      if (JSTS_EXTENSIONS.has(ext)) jsTs.push(file);
      if (OXFMT_EXTENSIONS.has(ext)) oxfmt.push(file);
    }
  }

  return { jsTs, rust, oxfmt };
}

/**
 * Returns whether a file path is forbidden from being committed.
 *
 * @param {string} filePath - File path to evaluate.
 * @returns {boolean} True when the file is a dotenv file.
 */
export function isForbiddenPath(filePath) {
  const basename = path.basename(filePath);
  return FORBIDDEN_NAMES.has(basename) || basename.startsWith(FORBIDDEN_PREFIX);
}

/**
 * Filters a file list down to forbidden paths.
 *
 * @param {string[]} files - File paths to inspect.
 * @returns {string[]} Forbidden paths found.
 */
export function checkForbiddenPaths(files) {
  return files.filter(isForbiddenPath);
}

/**
 * Verifies that .gitignore contains both .env and .env.* entries.
 *
 * @param {(path: string) => string} [readFile] - Injectable file reader.
 * @returns {boolean} True when both required entries are present.
 */
export function checkGitignoreSanity(readFile = defaultReadFile) {
  try {
    const content = readFile(".gitignore");
    const lines = content.split("\n").map((l) => l.trim());
    const hasEnv = lines.some((l) => l === ".env");
    const hasEnvGlob = lines.some((l) => l === ".env.*" || l === ".env*");
    return hasEnv && hasEnvGlob;
  } catch {
    return false;
  }
}

/**
 * Returns whether an added diff line contains a recognizable secret pattern.
 *
 * @param {string} line - A single added line from a unified diff.
 * @returns {boolean} True when the line matches a known credential pattern.
 */
export function hasSecretPattern(line) {
  return SECRET_PATTERNS.some((pattern) => pattern.test(line));
}

/**
 * Returns whether an added diff line carries a noscan suppression marker.
 *
 * Lines marked with the word "noscan" (case-insensitive, as a whole word) are
 * excluded from secret-pattern scanning. Use this for test fixtures or other
 * lines that intentionally contain credential-shaped strings.
 *
 * @param {string} line - A single added line from a unified diff.
 * @returns {boolean} True when the line should be excluded from secret scanning.
 */
export function isSuppressed(line) {
  return NOSCAN_RE.test(line);
}

/**
 * Scans changed JS/TS files for file-wide or block-wide lint suppression comments.
 *
 * Only line-scoped suppressions with an explicit rule name are accepted:
 * `// eslint-disable-next-line rule` and `// eslint-disable-line rule`.
 * Block-scope directives such as `/* eslint-disable rule *\/` are rejected
 * even when a rule name is present.
 *
 * @param {string[]} files - File paths to inspect.
 * @param {(path: string) => string} [readFile] - Injectable file reader.
 * @returns {{ file: string, line: number, text: string }[]} Violations found.
 */
export function checkSuppressionComments(files, readFile = defaultReadFile) {
  const violations = [];

  for (const file of files) {
    if (!JSTS_EXTENSIONS.has(path.extname(file).toLowerCase())) continue;

    let content;
    try {
      content = readFile(file);
    } catch {
      continue;
    }

    const originalLines = content.split("\n");
    // Blank multiline template literal bodies in the full content first so that
    // directive-shaped text on interior lines does not trigger false positives.
    // Non-newline characters are replaced with spaces to preserve line numbers.
    const preprocessed = content.replace(
      MULTILINE_TEMPLATE_RE,
      (match) => "`" + match.slice(1, -1).replace(/[^\n]/g, " ") + "`",
    );
    const processedLines = preprocessed.split("\n");

    for (let i = 0; i < processedLines.length; i++) {
      const stripped = processedLines[i].replace(STRING_LITERAL_RE, '""');
      if (JSTS_SUPPRESS_ANY_RE.test(stripped) && !JSTS_SUPPRESS_ALLOWED_RE.test(stripped)) {
        violations.push({ file, line: i + 1, text: originalLines[i].trim() });
      }
    }
  }

  return violations;
}

/**
 * Scans changed Rust files for crate/module-wide or broad lint suppression attributes.
 *
 * Rejects inner attributes (`#![allow(...)]`) and overly broad categories such as
 * `clippy::all` and `clippy::restriction`. The preferred suppression form is
 * `#[expect(rule, reason = "...")]` on the specific item.
 *
 * @param {string[]} files - Rust file paths to inspect.
 * @param {(path: string) => string} [readFile] - Injectable file reader.
 * @returns {{ file: string, line: number, text: string }[]} Violations found.
 */
export function checkRustSuppressions(files, readFile = defaultReadFile) {
  const violations = [];

  for (const file of files) {
    if (path.extname(file).toLowerCase() !== RUST_EXT) continue;

    let content;
    try {
      content = readFile(file);
    } catch {
      continue;
    }

    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      if (RUST_INNER_ALLOW_RE.test(line) || RUST_BROAD_ALLOW_RE.test(line)) {
        violations.push({ file, line: i + 1, text: line.trim() });
      }
    }
  }

  return violations;
}

/**
 * Scans changed JS/TS and Rust files for named functions or public Rust functions
 * that lack a preceding documentation comment.
 *
 * Intentionally narrow scope: named function declarations, exported const arrow
 * functions, and Rust pub fn declarations. Anonymous callbacks are excluded.
 *
 * @param {string[]} files - File paths to inspect.
 * @param {(path: string) => string} [readFile] - Injectable file reader.
 * @returns {{ file: string, line: number, name: string }[]} Violations found.
 */
export function findMissingDocComments(files, readFile = defaultReadFile) {
  const violations = [];

  for (const file of files) {
    let content;
    try {
      content = readFile(file);
    } catch {
      continue;
    }

    const ext = path.extname(file).toLowerCase();
    const isRust = ext === RUST_EXT;

    if (!isRust && !JSTS_EXTENSIONS.has(ext)) continue;

    const lines = content.split("\n");

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];

      if (isRust) {
        const match = line.match(RUST_PUB_FN_RE);
        if (match && !hasPrecedingDocComment(lines, i, true)) {
          violations.push({ file, line: i + 1, name: match[2] });
        }
      } else {
        const namedMatch = line.match(NAMED_FUNCTION_RE);
        if (namedMatch) {
          if (!hasPrecedingDocComment(lines, i, false)) {
            violations.push({ file, line: i + 1, name: namedMatch[4] });
          }
          continue;
        }

        const arrowMatch = line.match(EXPORTED_ARROW_RE);
        if (arrowMatch && !hasPrecedingDocComment(lines, i, false)) {
          violations.push({ file, line: i + 1, name: arrowMatch[1] });
        }
      }
    }
  }

  return violations;
}

/**
 * Runs oxfmt in check mode against the provided file list.
 *
 * @param {string[]} files - Non-Rust files to check.
 * @param {{ run?: Function, log?: Function, error?: Function }} [deps] - Injectable dependencies.
 * @returns {number} Process exit code.
 */
export function checkOxfmt(files, deps = {}) {
  return runOxfmt(["--check", ...files], deps);
}

/**
 * Checks Rust file formatting using rustfmt in check mode.
 *
 * @param {string[]} files - Rust files to check.
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null }} [run] - Spawn implementation.
 * @returns {number} Process exit code.
 */
export function checkRustfmt(files, run = spawnSync) {
  const result = run("rustfmt", ["--check", "--edition", "2021", ...files], {
    stdio: "inherit",
    ...SHELL_ON_WIN,
  });
  return typeof result.status === "number" ? result.status : 1;
}

/**
 * Runs oxlint against the provided JS/TS/framework files.
 *
 * @param {string[]} files - Files to lint.
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null }} [run] - Spawn implementation.
 * @returns {number} Process exit code.
 */
export function checkOxlint(files, run = spawnSync) {
  const result = run(
    "oxlint",
    ["--config", ".oxlintrc.json", "--report-unused-disable-directives", ...files],
    {
      stdio: "inherit",
      ...SHELL_ON_WIN,
    },
  );
  return typeof result.status === "number" ? result.status : 1;
}

/**
 * Runs cargo clippy in src-tauri/ with all-targets and warnings-as-errors.
 *
 * @param {(cmd: string, args: string[], opts?: object) => { status: number | null }} [run] - Spawn implementation.
 * @param {(msg: string) => void} [log] - Log function.
 * @returns {number} Process exit code.
 */
export function checkCargoClippy(run = spawnSync, log = console.log) {
  log("Rust files changed. Running cargo clippy in src-tauri/...");
  const result = run(
    "cargo",
    [
      "clippy",
      "--all-targets",
      "--",
      "-D",
      "warnings",
      "-W",
      "clippy::allow_attributes",
      "-W",
      "clippy::allow_attributes_without_reason",
    ],
    {
      stdio: "inherit",
      cwd: "src-tauri",
      ...SHELL_ON_WIN,
    },
  );
  return typeof result.status === "number" ? result.status : 1;
}

/**
 * Orchestrates all pre-push enforcement checks in fail-fast order.
 *
 * Checks run against files changed between the resolved base and HEAD only.
 * Returns 0 when all checks pass or when no base can be determined.
 *
 * @param {{
 *   run?: (cmd: string, args: string[], opts?: object) => { status: number | null, stdout?: string },
 *   log?: (msg: string) => void,
 *   error?: (msg: string) => void,
 *   readFile?: (path: string) => string,
 * }} [options] - Injectable dependencies.
 * @returns {number} Exit code: 0 for pass, 1 for any violation.
 */
export function runEnforce({
  run = spawnSync,
  log = console.log,
  error = console.error,
  readFile = defaultReadFile,
} = {}) {
  const base = resolveBase(run);

  if (base === null) {
    log("No upstream and no remote default branch found. Skipping pre-push checks.");
    return 0;
  }

  const files = getChangedFiles(base, run);
  const { jsTs, rust, oxfmt } = classifyFiles(files);

  // Step 1: Forbidden paths
  const forbidden = checkForbiddenPaths(files);
  if (forbidden.length > 0) {
    error(`Forbidden paths staged for commit: ${forbidden.join(", ")}`);
    return 1;
  }

  // Step 2: Gitignore sanity
  if (!checkGitignoreSanity(readFile)) {
    error(".gitignore must contain both .env and .env.* entries.");
    return 1;
  }

  // Step 3: Secrets heuristic on added lines
  const secretLines = getAddedLines(base, run).filter(
    (line) => hasSecretPattern(line) && !isSuppressed(line),
  );
  if (secretLines.length > 0) {
    error("Potential secrets detected in added lines:");
    for (const line of secretLines) error(`  ${line}`);
    return 1;
  }

  // Step 4: Suppression scan on changed JS/TS and Rust files
  const suppressionViolations = [
    ...checkSuppressionComments(jsTs, readFile),
    ...checkRustSuppressions(rust, readFile),
  ];
  if (suppressionViolations.length > 0) {
    error(
      "Broad lint suppression comments are prohibited (use line-scoped forms with explicit rule names):",
    );
    for (const v of suppressionViolations) error(`  ${v.file}:${v.line}: ${v.text}`);
    return 1;
  }

  // Step 5: oxfmt format check on changed supported files
  if (oxfmt.length > 0) {
    const code = checkOxfmt(oxfmt, { run, log, error });
    if (code !== 0) return code;
  }

  // Step 6: Rust format check on changed .rs files
  if (rust.length > 0) {
    const code = checkRustfmt(rust, run);
    if (code !== 0) return code;
  }

  // Step 7: oxlint on changed JS/TS/framework files
  if (jsTs.length > 0) {
    const code = checkOxlint(jsTs, run);
    if (code !== 0) return code;
  }

  // Step 8: Doc comment heuristic on changed JS/TS and Rust files
  const docViolations = findMissingDocComments([...jsTs, ...rust], readFile);
  if (docViolations.length > 0) {
    error("Missing documentation comments:");
    for (const v of docViolations) error(`  ${v.file}:${v.line} - ${v.name}`);
    return 1;
  }

  // Step 9: Cargo clippy (only when Rust files changed)
  if (rust.length > 0) {
    const code = checkCargoClippy(run, log);
    if (code !== 0) return code;
  }

  return 0;
}

if (
  process.argv[1] &&
  import.meta.url.startsWith("file:") &&
  path.resolve(fileURLToPath(import.meta.url)) === path.resolve(process.argv[1])
) {
  process.exit(runEnforce());
}
