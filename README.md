# ITD ODD Save Manager

Portable auto‑save backup app for _Into the Dead: Our Darkest Days_. The initial focus is high‑level implementation options based on how the game saves, with a preference for free tooling and low‑effort maintenance.

## Scope

- Identify how the game saves and where files live.
- Propose high‑level implementation options (no deep details yet).
- Keep the solution portable and easy to build.

## Functional Features

- Auto‑detect save location(s) for the game.
- Manual override to specify save location(s).
- File‑watcher‑based backups aligned to day/night cycle changes.
- “Cycle detection” or event coalescing to avoid excessive backups.
- Backups stored in subfolders next to the save files.
- Restore picker to revert to a chosen backup snapshot.
- Steam Cloud‑aware mode (warnings/guardrails).
- Optional auto‑launch and auto‑close tied to the game.

## Non‑Functional / UX

- Tray app with a normal window when not minimized.
- Low resource usage; must not slow PC or game.
- Portable; no installer.
- Prefer a single‑EXE build if possible.
- Free tooling preferred and easy to implement/maintain.

## Platform / Store

- Primary: Steam on Windows.
- Ideally support other stores/installs via auto‑detect + manual path.
- Open to macOS/Linux alternatives; can drop Windows‑only features if needed.

## Key Design Decisions

- Favor event‑driven file watching over polling for low resource use.
- Keep backups adjacent to save files for portability and easy discovery.
- Treat cross‑OS support as a possible pivot that may require different tech.

## License

See `LICENSE.md`.
