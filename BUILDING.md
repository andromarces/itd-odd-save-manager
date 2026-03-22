# Building from Source

This document outlines how to build the ITD ODD Save Manager from source. The project strives for reproducible builds to ensure trust and security.

## Prerequisites

### General

- **Git**: For version control.
- **Node.js**: Version 20 (LTS).
- **Rust**: Version 1.93.0 (managed via `rust-toolchain.toml`).

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
   npm ci
   ```

   _Note: `npm ci` is recommended over `npm install` for reproducible environments as it respects `package-lock.json` strictly. Running `npm ci` also invokes the `prepare` lifecycle script, which installs the Husky git hooks automatically._

## Git Hooks

Git hooks are managed by [Husky](https://typicode.github.io/husky) and run automatically after `npm ci`.

### pre-commit

Runs [lint-staged](https://github.com/lint-staged/lint-staged) against staged files only:

- **Tracked non-Rust files supported by `oxfmt`** (`*.{css,html,json,json5,jsonc,js,jsx,md,mdx,ts,tsx,toml,vue,yaml,yml}`): formatted with `oxfmt`.
- **Rust files** (`src-tauri/**/*.rs`): formatted with `rustfmt --edition 2021`.

### pre-push

Runs `scripts/pre-push-enforce.mjs` against files changed between the resolved base and HEAD.
The diff base is resolved as follows: `@{upstream}` when configured, otherwise the merge-base
against `origin/main` or `origin/master`, and skipped entirely when neither is reachable.

Checks execute in fail-fast order — the first failure stops all subsequent checks:

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
   npm run build
   ```

   To format the tracked non-Rust files that `oxfmt` supports, run:

   ```bash
   npm run format:oxfmt
   ```

4. **Build Application**
   To build the release artifacts (installer and executable):

   ```bash
   npm run tauri build
   ```

   - The artifacts will be located in `src-tauri/target/release/bundle/`.
   - The raw executable is at `src-tauri/target/release/ITD ODD Save Manager.exe`.

## Verifying Reproducibility

To verify that the build matches the official release:

1. Check out the specific tag (e.g., `git checkout v1.0.0`).
2. Build using the steps above.
3. Compare the SHA256 hash of `src-tauri/target/release/ITD ODD Save Manager.exe` with the `SHA256SUMS.txt` provided in the GitHub Release.

_Note: Minor binary differences may occur due to timestamps or path variations, but are minimized via deterministic build flags in the CI._
