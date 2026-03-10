import { describe, expect, it, vi } from "vitest";

import {
  isRustFile,
  parseArguments,
  runOxfmt,
  selectOxfmtFiles,
  splitTrackedFiles,
} from "./run-oxfmt-tracked.mjs";

describe("splitTrackedFiles", () => {
  it("parses NUL-delimited file lists", () => {
    expect(splitTrackedFiles("README.md\0package.json\0")).toEqual(["README.md", "package.json"]);
  });
});

describe("isRustFile", () => {
  it("matches Rust source files case-insensitively", () => {
    expect(isRustFile("src-tauri/src/main.rs")).toBe(true);
    expect(isRustFile("src-tauri/src/MAIN.RS")).toBe(true);
    expect(isRustFile("package.json")).toBe(false);
  });
});

describe("selectOxfmtFiles", () => {
  it("excludes files reserved for rustfmt", () => {
    expect(selectOxfmtFiles(["README.md", "src-tauri/src/main.rs", "src/style.css"])).toEqual([
      "README.md",
      "src/style.css",
    ]);
  });
});

describe("parseArguments", () => {
  it("separates oxfmt flags from file paths", () => {
    expect(parseArguments(["--check", "README.md", "package.json"])).toEqual({
      files: ["README.md", "package.json"],
      oxfmtArgs: ["--check"],
    });
  });
});

describe("runOxfmt", () => {
  it("formats explicit non-Rust file lists without calling git", () => {
    const run = vi.fn(() => ({ status: 0 }));

    const exitCode = runOxfmt(["--check", "README.md", "src-tauri/src/main.rs"], {
      run,
      log: vi.fn(),
      error: vi.fn(),
    });

    expect(exitCode).toBe(0);
    expect(run).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledWith(
      process.execPath,
      [expect.stringContaining("node_modules"), "--check", "README.md"],
      { stdio: "inherit" },
    );
  });

  it("falls back to tracked files when no explicit files are provided", () => {
    const run = vi
      .fn()
      .mockReturnValueOnce({
        status: 0,
        stdout: "README.md\0src-tauri/src/main.rs\0package.json\0",
      })
      .mockReturnValueOnce({ status: 0 });

    const exitCode = runOxfmt(["--list-different"], {
      run,
      log: vi.fn(),
      error: vi.fn(),
    });

    expect(exitCode).toBe(0);
    expect(run).toHaveBeenNthCalledWith(
      1,
      expect.stringMatching(/git(\.exe)?$/),
      ["ls-files", "-z"],
      { encoding: "utf8" },
    );
    expect(run).toHaveBeenNthCalledWith(
      2,
      process.execPath,
      [expect.stringContaining("node_modules"), "--list-different", "README.md", "package.json"],
      { stdio: "inherit" },
    );
  });

  it("returns success when only Rust files are present", () => {
    const run = vi.fn(() => ({
      status: 0,
      stdout: "src-tauri/src/main.rs\0src-tauri/src/lib.rs\0",
    }));
    const log = vi.fn();

    const exitCode = runOxfmt([], {
      run,
      log,
      error: vi.fn(),
    });

    expect(exitCode).toBe(0);
    expect(run).toHaveBeenCalledTimes(1);
    expect(log).toHaveBeenCalledWith("No non-Rust tracked files matched for oxfmt.");
  });
});
