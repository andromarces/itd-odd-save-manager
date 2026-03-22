import { describe, expect, it, vi } from "vitest";

import {
  checkForbiddenPaths,
  checkGitignoreSanity,
  checkRustSuppressions,
  checkSuppressionComments,
  classifyFiles,
  findMissingDocComments,
  getAddedLines,
  getChangedFiles,
  hasSecretPattern,
  isForbiddenPath,
  isSuppressed,
  resolveBase,
  runEnforce,
} from "./pre-push-enforce.mjs";

describe("resolveBase", () => {
  it("returns '@{upstream}' when upstream is configured", () => {
    // Req: configured upstream is used as the diff base to limit checks to new commits.
    const run = vi.fn(() => ({ status: 0 }));
    expect(resolveBase(run)).toBe("@{upstream}");
  });

  it("returns merge-base SHA against origin/main when upstream is absent", () => {
    // Req: falls back to merge-base against origin/main to scope the diff to branch-only commits.
    const SHA = "abc1234def5678";
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 1 }) // @{upstream} not found
      .mockReturnValueOnce({ status: 0 }) // origin/main exists
      .mockReturnValueOnce({ status: 0, stdout: `${SHA}\n` }); // merge-base result
    expect(resolveBase(run)).toBe(SHA);
  });

  it("falls back to origin/master when origin/main is not reachable", () => {
    // Req: origin/master is tried when origin/main is absent to maximise branch coverage.
    const SHA = "deadbeef123456";
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 1 }) // @{upstream} not found
      .mockReturnValueOnce({ status: 1 }) // origin/main not found
      .mockReturnValueOnce({ status: 0 }) // origin/master exists
      .mockReturnValueOnce({ status: 0, stdout: `${SHA}\n` }); // merge-base result
    expect(resolveBase(run)).toBe(SHA);
  });

  it("returns null when no upstream or remote default branch is reachable", () => {
    // Req: null signals the orchestrator to skip checks rather than fail the push.
    const run = vi.fn(() => ({ status: 1 }));
    expect(resolveBase(run)).toBeNull();
  });
});

describe("getChangedFiles", () => {
  it("returns the list of files changed between base and HEAD", () => {
    // Req: only files within the push diff are passed to subsequent checks.
    const run = vi.fn(() => ({ status: 0, stdout: "src/a.ts\nsrc/b.ts\n" }));
    expect(getChangedFiles("@{upstream}", run)).toEqual(["src/a.ts", "src/b.ts"]);
  });

  it("returns an empty array when no files changed", () => {
    // Req: empty diff produces no files and skips all file-level checks.
    const run = vi.fn(() => ({ status: 0, stdout: "" }));
    expect(getChangedFiles("@{upstream}", run)).toEqual([]);
  });

  it("excludes deleted files by passing --diff-filter=d to git", () => {
    // Req: deleted files must not reach format or lint tools that require readable paths.
    const run = vi.fn(() => ({ status: 0, stdout: "src/a.ts\n" }));
    getChangedFiles("@{upstream}", run);
    expect(run).toHaveBeenCalledWith(
      expect.any(String),
      expect.arrayContaining(["--diff-filter=d"]),
      expect.any(Object),
    );
  });
});

describe("getAddedLines", () => {
  it("extracts lines starting with '+' while excluding diff headers", () => {
    // Req: only genuinely added content is passed to secret-pattern detection.
    const diffOutput = "diff --git a/f b/f\n+++ b/f\n+secret=abc\n+normal line\n"; // noscan
    const run = vi.fn(() => ({ status: 0, stdout: diffOutput }));
    expect(getAddedLines("@{upstream}", run)).toEqual(["+secret=abc", "+normal line"]); // noscan
  });
});

describe("classifyFiles", () => {
  it("separates JS/TS/framework files, Rust files, and oxfmt-supported files", () => {
    // Req: correct classification routes each changed file to the appropriate check.
    const files = ["src/a.ts", "src/lib.rs", "README.md", "styles.css", "app.vue"];
    const result = classifyFiles(files);
    expect(result.jsTs).toEqual(["src/a.ts", "app.vue"]);
    expect(result.rust).toEqual(["src/lib.rs"]);
    expect(result.oxfmt).toContain("src/a.ts");
    expect(result.oxfmt).toContain("README.md");
    expect(result.oxfmt).toContain("styles.css");
    expect(result.oxfmt).toContain("app.vue");
    expect(result.oxfmt).not.toContain("src/lib.rs");
  });
});

describe("isForbiddenPath", () => {
  it("flags .env and .env.* files as forbidden, ignoring unrelated files", () => {
    // Req: secrets-bearing dotenv files must never be committed.
    expect(isForbiddenPath(".env")).toBe(true);
    expect(isForbiddenPath(".env.local")).toBe(true);
    expect(isForbiddenPath("config/.env.production")).toBe(true);
    expect(isForbiddenPath("README.md")).toBe(false);
    expect(isForbiddenPath("src/env.ts")).toBe(false);
  });
});

describe("checkForbiddenPaths", () => {
  it("returns only the forbidden files from the given list", () => {
    // Req: non-forbidden files are not included in the violation report.
    expect(checkForbiddenPaths([".env", "src/main.ts", ".env.local"])).toEqual([
      ".env",
      ".env.local",
    ]);
  });
});

describe("checkGitignoreSanity", () => {
  it("passes when .gitignore contains both .env and .env.* entries", () => {
    // Req: both exact and glob entries must be present to protect all dotenv variants.
    expect(checkGitignoreSanity(() => ".env\n.env.*\nnode_modules\n")).toBe(true);
  });

  it("fails when the .env entry is absent", () => {
    // Req: missing .env entry leaves the exact file unprotected.
    expect(checkGitignoreSanity(() => ".env.*\nnode_modules\n")).toBe(false);
  });

  it("fails when .gitignore cannot be read", () => {
    // Req: unreadable or absent .gitignore is treated as a sanity failure.
    expect(
      checkGitignoreSanity(() => {
        throw new Error("ENOENT");
      }),
    ).toBe(false);
  });
});

describe("hasSecretPattern", () => {
  it("detects credential assignments in added lines", () => {
    // Req: common secret assignment patterns in added code are flagged before push.
    expect(hasSecretPattern("+  password=supersecret")).toBe(true); // noscan
    expect(hasSecretPattern("+  api_key=xyz123abc")).toBe(true); // noscan
    expect(hasSecretPattern("+  TOKEN=Bearer abc123")).toBe(true); // noscan
    expect(hasSecretPattern("+  secret=my-secret-value")).toBe(true); // noscan
  });

  it("does not flag ordinary code or log lines", () => {
    // Req: benign added lines must not produce false-positive secret rejections.
    expect(hasSecretPattern("+  const x = 42;")).toBe(false);
    expect(hasSecretPattern("+  console.log('done');")).toBe(false);
    expect(hasSecretPattern("+  // password is validated elsewhere")).toBe(false);
  });

  it("does not flag token or secret variable assignments whose value is a function call or property access", () => {
    // Req: common benign patterns like token = getToken() or secret = response.secret must not block a push.
    expect(hasSecretPattern("+  const token = getToken()")).toBe(false);
    expect(hasSecretPattern("+  const secret = computeSecret(x)")).toBe(false);
    expect(hasSecretPattern("+  const token = response.token")).toBe(false);
  });
});

describe("isSuppressed", () => {
  it("returns true when the line contains the noscan marker as a whole word", () => {
    // Req: lines carrying the noscan marker are excluded from secret-pattern scanning.
    expect(isSuppressed("+  password=supersecret // noscan")).toBe(true);
    expect(isSuppressed("+  api_key=xyz # noscan")).toBe(true);
    expect(isSuppressed("+  secret=val // NOSCAN")).toBe(true);
  });

  it("returns false for lines without the noscan marker", () => {
    // Req: ordinary lines without the marker remain subject to secret scanning.
    expect(isSuppressed("+  password=supersecret")).toBe(false); // noscan
    expect(isSuppressed("+  const x = 42;")).toBe(false);
  });

  it("does not treat partial word matches as suppression markers", () => {
    // Req: the marker must appear as a whole word to avoid accidental suppression on unrelated identifiers.
    expect(isSuppressed("+  const noscanResult = true;")).toBe(false);
  });
});

describe("findMissingDocComments", () => {
  it("reports no violation for a named JS function preceded by a JSDoc block", () => {
    // Req: named functions with JSDoc above them satisfy the documentation requirement.
    const content =
      "/**\n * Computes something.\n * @param {number} x Input.\n * @returns {number} Result.\n */\nfunction compute(x) {\n  return x * 2;\n}\n";
    expect(findMissingDocComments(["src/compute.js"], () => content)).toEqual([]);
  });

  it("reports a violation for a named JS function with no documentation comment", () => {
    // Req: named functions without any preceding comment are flagged.
    const content = "function compute(x) {\n  return x * 2;\n}\n";
    const violations = findMissingDocComments(["src/compute.js"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/compute.js", name: "compute" });
  });

  it("reports no violation for an exported const arrow function with JSDoc", () => {
    // Req: exported arrow functions documented with JSDoc pass the check.
    const content = "/**\n * Formats a string.\n */\nexport const format = (s) => s.trim();\n";
    expect(findMissingDocComments(["src/format.ts"], () => content)).toEqual([]);
  });

  it("reports a violation for an exported const arrow function without JSDoc", () => {
    // Req: exported arrow functions missing JSDoc are flagged.
    const content = "export const format = (s) => s.trim();\n";
    const violations = findMissingDocComments(["src/format.ts"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/format.ts", name: "format" });
  });

  it("reports no violation for a Rust pub fn preceded by a doc comment", () => {
    // Req: Rust public functions with /// doc comments satisfy the documentation requirement.
    const content = "/// Processes an item.\npub fn process(x: i32) -> i32 {\n    x\n}\n";
    expect(findMissingDocComments(["src/lib.rs"], () => content)).toEqual([]);
  });

  it("reports a violation for a Rust pub fn without a preceding doc comment", () => {
    // Req: Rust public functions without /// comments are flagged.
    const content = "pub fn process(x: i32) -> i32 {\n    x\n}\n";
    const violations = findMissingDocComments(["src/lib.rs"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/lib.rs", name: "process" });
  });

  it("does not flag anonymous callbacks as doc-comment violations", () => {
    // Req: anonymous functions are out of scope to avoid false positives on callbacks.
    const content = "const result = arr.map(function(x) { return x; });\n";
    expect(findMissingDocComments(["src/utils.js"], () => content)).toEqual([]);
  });
});

describe("checkSuppressionComments", () => {
  it("accepts a line-scoped eslint-disable-next-line with an explicit rule", () => {
    // Req: single-line suppressions naming a specific rule satisfy the narrow-suppression policy.
    const content = "// eslint-disable-next-line no-console\nconsole.log('x');\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("accepts a line-scoped eslint-disable-line with an explicit rule", () => {
    // Req: inline suppressions naming a specific rule satisfy the narrow-suppression policy.
    const content = "console.log('x'); // eslint-disable-line no-console\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("accepts an oxlint-disable-next-line with an explicit rule", () => {
    // Req: oxlint-disable-next-line with an explicit rule satisfies the narrow-suppression policy.
    const content = "// oxlint-disable-next-line no-console\nconsole.log('x');\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("rejects a file-wide eslint-disable with no rule name", () => {
    // Req: blanket disable comments with no rule name are broader than policy permits.
    const content = "/* eslint-disable */\nconsole.log('x');\n";
    const violations = checkSuppressionComments(["src/a.ts"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/a.ts", line: 1 });
  });

  it("rejects a block-scoped eslint-disable even when a rule name is present", () => {
    // Req: block-scope disable is broader than line-scope and is prohibited even with an explicit rule.
    const content = "/* eslint-disable no-console */\nconsole.log('x');\n";
    const violations = checkSuppressionComments(["src/a.ts"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/a.ts", line: 1 });
  });

  it("rejects an eslint-disable-next-line with no rule name", () => {
    // Req: line-scoped suppressions without an explicit rule name are not sufficiently narrow.
    const content = "// eslint-disable-next-line\nconsole.log('x');\n";
    const violations = checkSuppressionComments(["src/a.ts"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/a.ts", line: 1 });
  });

  it("rejects a block-scoped oxlint-disable even when a rule name is present", () => {
    // Req: oxlint-disable block directives are subject to the same scope policy as eslint-disable.
    const content = "/* oxlint-disable no-console */\nconsole.log('x');\n";
    const violations = checkSuppressionComments(["src/a.ts"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/a.ts", line: 1 });
  });

  it("does not flag directive keywords that appear inside string literals", () => {
    // Req: string content is not a suppression comment; flagging it would cause false positives
    //      when test fixtures or documentation strings contain directive-shaped text.
    const content = 'const msg = "/* eslint-disable no-console */";\n';
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("does not flag directive keywords inside a multiline template literal", () => {
    // Req: template literal content spanning multiple lines is not a suppression comment;
    //      flagging it would cause false positives when multiline test fixtures contain
    //      directive-shaped text on intermediate lines.
    const content = "const s = `first line\neslint-disable-next-line no-console\nthird`;\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("skips non-JS/TS files", () => {
    // Req: suppression scan applies only to JS/TS/framework files, not prose or config files.
    const content = "/* eslint-disable no-console */\n";
    expect(checkSuppressionComments(["README.md"], () => content)).toEqual([]);
  });

  it("does not flag directive keywords inside a regex literal", () => {
    // Req: regex literals are not suppression comments; flagging them causes false positives
    //      when the scanner's own pattern definitions contain directive-shaped text.
    const content = "const re = /(eslint-disable|oxlint-disable)/;\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });

  it("does not flag directive keywords mentioned mid-sentence in a prose comment", () => {
    // Req: prose comments that reference directive names for documentation purposes are not
    //      suppression directives and must not be flagged.
    const content =
      "// Note: oxlint-disable-next-line with an explicit rule is the allowed form.\n";
    expect(checkSuppressionComments(["src/a.ts"], () => content)).toEqual([]);
  });
});

describe("checkRustSuppressions", () => {
  it("rejects an inner allow attribute applied at crate or module scope", () => {
    // Req: #![allow(...)] applies to the entire module or crate and violates the narrow-suppression policy.
    const content = "#![allow(dead_code)]\npub fn foo() {}\n";
    const violations = checkRustSuppressions(["src/lib.rs"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/lib.rs", line: 1 });
  });

  it("rejects #[allow(clippy::all)] as an overly broad category", () => {
    // Req: blanket Clippy category suppressions mask too many warnings to be acceptable.
    const content = "#[allow(clippy::all)]\npub fn foo() {}\n";
    const violations = checkRustSuppressions(["src/lib.rs"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/lib.rs", line: 1 });
  });

  it("rejects #[allow(clippy::restriction)] as an overly broad category", () => {
    // Req: the restriction lint group covers many rules and is too coarse for policy compliance.
    const content = "#[allow(clippy::restriction)]\npub fn foo() {}\n";
    const violations = checkRustSuppressions(["src/lib.rs"], () => content);
    expect(violations).toHaveLength(1);
    expect(violations[0]).toMatchObject({ file: "src/lib.rs", line: 1 });
  });

  it("does not reject a narrow item-level #[expect] with a reason", () => {
    // Req: #[expect(rule, reason = "...")] is the preferred suppression form and must not be flagged.
    const content = '#[expect(dead_code, reason = "used only in tests")]\npub fn foo() {}\n';
    expect(checkRustSuppressions(["src/lib.rs"], () => content)).toEqual([]);
  });

  it("skips non-Rust files", () => {
    // Req: Rust suppression scan applies only to .rs files.
    const content = "#![allow(dead_code)]\n";
    expect(checkRustSuppressions(["src/a.ts"], () => content)).toEqual([]);
  });
});

describe("runEnforce", () => {
  it("exits with 0 and logs a skip message when no upstream base can be resolved", () => {
    // Req: pre-push is skipped rather than failing when git base resolution yields no result.
    const run = vi.fn(() => ({ status: 1 }));
    const log = vi.fn();
    expect(runEnforce({ run, log, error: vi.fn(), readFile: vi.fn() })).toBe(0);
    expect(log).toHaveBeenCalledWith(expect.stringContaining("Skipping"));
  });

  it("returns 1 immediately on forbidden-path detection without running further checks", () => {
    // Req: fail-fast stops at the first violation so no sensitive-data overhead accumulates.
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 0 }) // resolveBase: upstream found
      .mockReturnValueOnce({ status: 0, stdout: ".env\n" }); // getChangedFiles
    const error = vi.fn();
    const readFile = vi.fn();

    const exitCode = runEnforce({ run, log: vi.fn(), error, readFile });

    expect(exitCode).toBe(1);
    expect(run).toHaveBeenCalledTimes(2); // resolveBase + getChangedFiles only
    expect(readFile).not.toHaveBeenCalled(); // gitignore check was not reached
    expect(error).toHaveBeenCalled();
  });

  it("returns 0 when there are no changed files and the gitignore is sound", () => {
    // Req: a clean push with a valid gitignore should never be blocked.
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 0 }) // resolveBase
      .mockReturnValueOnce({ status: 0, stdout: "" }) // getChangedFiles: no files
      .mockReturnValueOnce({ status: 0, stdout: "" }); // getAddedLines: no added lines
    const readFile = vi.fn(() => ".env\n.env.*\n");

    expect(runEnforce({ run, log: vi.fn(), error: vi.fn(), readFile })).toBe(0);
  });

  it("returns 1 on a rejected JS/TS suppression comment before format and lint tools run", () => {
    // Req: a broad suppression directive in a changed JS/TS file is caught in step 4 and blocks
    //      the push before the slower oxfmt and oxlint tools are invoked.
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 0 }) // resolveBase
      .mockReturnValueOnce({ status: 0, stdout: "src/a.ts\n" }) // getChangedFiles
      .mockReturnValueOnce({ status: 0, stdout: "" }); // getAddedLines: no secrets
    const readFile = vi
      .fn()
      .mockReturnValueOnce(".env\n.env.*\n") // checkGitignoreSanity
      .mockReturnValueOnce("/* eslint-disable no-console */\n"); // checkSuppressionComments: src/a.ts
    const error = vi.fn();

    const exitCode = runEnforce({ run, log: vi.fn(), error, readFile });

    expect(exitCode).toBe(1);
    expect(run).toHaveBeenCalledTimes(3); // no oxfmt or oxlint invocations
    expect(error).toHaveBeenCalledWith(expect.stringContaining("suppression"));
  });

  it("returns 1 on a crate-wide Rust allow attribute before cargo clippy runs", () => {
    // Req: an inner #![allow(...)] in a changed Rust file is caught in step 4, preventing
    //      the slower cargo clippy run from executing.
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 0 }) // resolveBase
      .mockReturnValueOnce({ status: 0, stdout: "src-tauri/src/lib.rs\n" }) // getChangedFiles
      .mockReturnValueOnce({ status: 0, stdout: "" }); // getAddedLines: no secrets
    const readFile = vi
      .fn()
      .mockReturnValueOnce(".env\n.env.*\n") // checkGitignoreSanity
      .mockReturnValueOnce("#![allow(dead_code)]\npub fn foo() {}\n"); // checkRustSuppressions: lib.rs
    const error = vi.fn();

    const exitCode = runEnforce({ run, log: vi.fn(), error, readFile });

    expect(exitCode).toBe(1);
    expect(run).toHaveBeenCalledTimes(3); // no rustfmt or clippy invocations
    expect(error).toHaveBeenCalledWith(expect.stringContaining("suppression"));
  });

  it("does not block a push when a secret-pattern line carries the noscan marker", () => {
    // Req: intentionally suppressed lines must not cause false-positive push rejections.
    const run = vi
      .fn()
      .mockReturnValueOnce({ status: 0 }) // resolveBase
      .mockReturnValueOnce({ status: 0, stdout: "" }) // getChangedFiles: no files
      .mockReturnValueOnce({ status: 0, stdout: "+password=supersecret // noscan\n" }); // getAddedLines
    const readFile = vi.fn(() => ".env\n.env.*\n");

    expect(runEnforce({ run, log: vi.fn(), error: vi.fn(), readFile })).toBe(0);
  });
});
