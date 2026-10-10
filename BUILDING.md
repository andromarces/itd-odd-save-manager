# Building from Source

This document outlines how to build the ITD ODD Save Manager from source. The project strives for reproducible builds to ensure trust and security.

## Prerequisites

### General

- **Git**: For version control.
- **Node.js**: Version 26 for CI and release builds (the version is set in `.nvmrc`). Local development also works on Node.js 24.15.0 or a later 24.x release. Node.js 25 is not supported.
- **pnpm**: The exact version in the `packageManager` field of `package.json` (12.10.1). Install it with the [standalone installer](https://pnpm.io/installation). Corepack is not used.
- **Rust**: Minimum supported version 1.95.0. The build toolchain is pinned to 1.99.0 in `rust-toolchain.toml`, and `rustup` installs it automatically on the first build.

### Windows

- **Microsoft Visual Studio C++ Build Tools**: Required for compiling Rust artifacts on Windows.

## Build Steps

1. **Clone the Repository**

   ```bash
   git clone https://github.com/andromarces/itd-odd-save-manager.git
   cd itd-odd-save-manager
   ```

2. **Install Frontend Dependencies**

   ```bash
   pnpm install --frozen-lockfile
   ```

   _Note: `--frozen-lockfile` gives reproducible environments because it fails if `pnpm-lock.yaml` does not match `package.json`. The install also runs the `prepare` lifecycle script, which installs the Vite+ git hooks automatically. `pnpm-workspace.yaml` sets `virtualStoreType: global`, so `node_modules` entries link into the shared pnpm store (run `pnpm store path` to locate it)._

## Git Hooks

Git hooks are managed by the [Vite+](https://viteplus.dev/guide/commit-hooks) hook dispatcher and run automatically after `pnpm install`. The `prepare` script runs `scripts/install-hooks.mjs`, which clears a local `core.hooksPath` of exactly `.husky/_` and then runs `vp config --no-agent`. Project-owned hook scripts live in `.vite-hooks/`.

### pre-commit

Runs `vp staged` (the `staged` block in `vite.config.ts`) against staged files only:

- **Tracked non-Rust files supported by `oxfmt`** (`*.{css,html,json,json5,jsonc,js,jsx,md,mdx,ts,tsx,toml,vue,yaml,yml}`): formatted with `oxfmt`.
- **Rust files** (`src-tauri/**/*.rs`): formatted with `rustfmt --edition 2021`.

### pre-push

Runs `scripts/pre-push-enforce.mjs` against files changed between the resolved base and HEAD.
The diff base is resolved as follows: `@{upstream}` when configured, otherwise the merge-base
against `origin/main` or `origin/master`, and skipped entirely when neither is reachable.

Checks execute in fail-fast order. The first failure stops all subsequent checks:

1. **Forbidden paths**: rejects `.env` and `.env.*` files.
2. **Gitignore sanity**: verifies `.gitignore` contains both `.env` and `.env.*` entries.
3. **Secret heuristic**: scans added diff lines for credential assignment patterns
   (`password=`, `api_key=`, `private_key=`, `token=` / `secret=` when the value is not a <!-- noscan -->
   function call or property access, PEM private key headers). Anonymous callbacks, property
   reads, and function return values are excluded to reduce false positives.
4. **oxfmt format check**: checks changed files supported by oxfmt
   (`*.{css,html,json,json5,jsonc,js,jsx,md,mdx,ts,tsx,toml,vue,yaml,yml}`).
5. **Rust format check**: checks changed `.rs` files with `rustfmt --check --edition 2021`.
6. **oxlint**: lints changed JS/TS/framework files using `.oxlintrc.json`
   (`no-debugger` enforced; jsdoc plugin loaded for future rule expansion).
7. **Doc comment heuristic**: flags named JS/TS function declarations, exported const
   arrow functions, and Rust `pub fn` declarations that lack a preceding documentation comment.
   Class methods and private Rust functions are out of scope (high false-positive risk without
   an AST parser) and remain manual-review only.
8. **Cargo clippy**: runs `cargo clippy --all-targets -- -D warnings` in `src-tauri/` when
   at least one `.rs` file changed.

**Automated enforcement coverage:**

| Check                                                                | Automated                                                   |
| -------------------------------------------------------------------- | ----------------------------------------------------------- |
| Formatting (JS/TS/markup/config/Rust)                                | Yes                                                         |
| Lint correctness (JS/TS/framework)                                   | Yes                                                         |
| Rust lint (when Rust changed)                                        | Yes                                                         |
| .env committed                                                       | Yes                                                         |
| .gitignore protection                                                | Yes                                                         |
| Obvious secret leakage                                               | Yes (heuristic; excludes function calls and property reads) |
| Doc comment presence (named functions, exported arrows, Rust pub fn) | Yes (heuristic)                                             |
| Doc comment presence (class methods, private Rust fn)                | Manual review only                                          |
| KISS / YAGNI / DRY / SOLID                                           | Manual review only                                          |
| TDD discipline / test organization                                   | Manual review only                                          |
| Logging completeness                                                 | Manual review only                                          |

3. **Build Frontend**

   ```bash
   pnpm run build
   ```

   To format the tracked non-Rust files that `oxfmt` supports, run:

   ```bash
   pnpm run format:oxfmt
   ```

4. **Build Application**
   To build the release artifacts (installer and executable):

   ```bash
   pnpm run tauri build
   ```

   - The artifacts will be located in `src-tauri/target/release/bundle/`.
   - The raw executable is at `src-tauri/target/release/ITD ODD Save Manager.exe`.

## Frontend Scripts

The frontend toolchain runs through [Vite+](https://viteplus.dev/guide/) (`vp`), installed as the `vite-plus` devDependency. No global `vp` install is needed.

| Script                   | Command                                                             |
| ------------------------ | ------------------------------------------------------------------- |
| `pnpm run dev`           | `vp dev`, serves `http://localhost:5173`                            |
| `pnpm run build`         | `tsc && vp build`, type-checks `src` before the bundle              |
| `pnpm run preview`       | `vp preview`, serves `dist/`                                        |
| `pnpm run test`          | `vp test watch`                                                     |
| `pnpm run test:run`      | `vp test`, a single run of all frontend and `scripts/` tests        |
| `pnpm run test:coverage` | `vp test run --coverage`, writes `coverage/`                        |
| `pnpm run lint`          | `oxlint --config .oxlintrc.json --report-unused-disable-directives` |
| `pnpm run format:oxfmt`  | `node ./scripts/run-oxfmt-tracked.mjs`                              |

## Migrating an Existing Clone

Run these steps in a clone that used the previous Husky hooks:

1. Pull the change: `git pull`.
2. Remove the old installed tooling and the old Husky dispatcher.
   - Windows (PowerShell): `Remove-Item -Recurse -Force node_modules, .husky`
   - macOS: `rm -rf node_modules .husky`
3. Reinstall: `pnpm install --frozen-lockfile`. The `prepare` script clears a local `core.hooksPath` of exactly `.husky/_`, then runs `vp config --no-agent`.
4. Check the hooks: `git config core.hooksPath` must print `.vite-hooks/_`. A clone with another custom `core.hooksPath` keeps that value, and the Vite+ hooks stay inactive. To use them, remove the custom value, then run `pnpm exec vp hooks enable`.
5. Verify the tools: `pnpm exec vp --version`, `pnpm run test:run`, `pnpm run lint`, `pnpm run build`.

## Verifying Reproducibility

To verify that the build matches the official release:

1. Check out the specific tag (e.g., `git checkout v1.0.0`).
2. Build using the steps above.
3. Compare the SHA256 hash of `src-tauri/target/release/ITD ODD Save Manager.exe` with the `SHA256SUMS.txt` provided in the GitHub Release.

_Note: Minor binary differences may occur due to timestamps or path variations, but are minimized via deterministic build flags in the CI._
