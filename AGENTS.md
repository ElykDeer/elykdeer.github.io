# Agent Guide

## Product

`elyk.dev` is a client-side Rust/Leptos WASM personal site whose primary UI is
an in-browser terminal. The profile overlay, terminal, editor, save manager, and
games are the product; do not replace the first screen with a marketing page.

`index.html` contains a real static version of the initial terminal/profile UI.
It paints before WASM, then `src/main.rs` mounts Leptos and removes the boot
elements. Keep that first paint independent of IndexedDB, dictionaries,
Pyodide, and other large assets. The terminal waits briefly for profile
hydration before accepting commands so stored state cannot be overwritten.

## Runtime Map

- `index.html`: first-paint markup/styles and browser APIs for IndexedDB, site
  backups, compressed word assets, drag input, and lazy Pyodide loading.
- `src/app.rs`: Leptos terminal UI and the top-level executor for command
  effects, persistence, async output, game panels, and Python jobs.
- `src/commands/`: command implementations, metadata, help, and the registry.
- `src/terminal/`: parser/completion, `TerminalContext`, `CommandEffect`, and
  typed `OutputBlock`/`GameLaunch` values.
- `src/content/`: immutable files bundled into the terminal filesystem.
- `src/fs/`: normalized paths and the writable `UserOverlay` over bundled files.
- `src/python/`: Rust/Pyodide bridge, filesystem mirror, and FIFO job executor.
- `src/games/`: game namespaces. State owns rules and transactions; views own
  rendering/input; storage modules own browser save keys.
- `src/storage/`: versioned profile and site-backup schemas plus IndexedDB I/O.
- `src/styles.css`: application and game styling.
- `.github/workflows/deploy.yml`: complete build gate and GitHub Pages deploy.

## How To Work

Use subagents as the normal way to parallelize substantial work and keep the
main context focused:

- Delegate bounded, independent tasks early. Give workers explicit file or
  responsibility ownership and remind them to preserve concurrent changes.
- Use focused explorers for specific codebase questions, implementation workers
  for disjoint edits, and independent reviewers after integration.
- Send only the task-specific architecture, constraints, and file paths an agent
  needs. Do not copy the entire conversation into every assignment.
- Keep the critical path and cross-cutting decisions in the main agent. Review
  every returned patch and run the appropriate repository checks after integration.
- Close completed agents promptly. Do not duplicate work already delegated.
- Keep this file and related docs current whenever stable architecture, workflow,
  persistence, asset, or release conventions change.

Agents must never push. Create commits only when the user explicitly requests
one; leave remote publication to the user. Never rewrite history, force-update a
ref, or run destructive Git commands without an explicit request and a clean,
verified worktree.

## Terminal Flow

Commands implement `commands::Command` and are registered in
`commands::registry`. `commands::execute_line` returns one `CommandExecution`:
synchronous `OutputBlock`s plus at most one optional `CommandEffect`.
`TerminalContext` temporarily owns that effect; `run_parsed` must consume it
before another command runs. `app.rs` is the only interactive effect executor.

Use `commands::run_line` only when effects are intentionally unsupported, such
as focused native tests. Redirects, pipelines, and `sh` must explicitly consume
or reject effects so an async request cannot leak into the next command.

To add a command:

1. Add `src/commands/<name>.rs` and implement all command metadata/help.
2. Export and register it in `src/commands/mod.rs`.
3. Return normal output directly. For async/browser work, add a typed effect and
   top-level executor branch rather than ambient flags or spawned work in a
   command.
4. Update registry-count, dispatch, help, redirect/pipe, and error-path tests as
   applicable.

## Filesystem And Persistence

The virtual filesystem combines the read-only bundled tree with a user overlay.
Writes must go through `VirtualFileSystem`; imports validate that an overlay
cannot redefine bundled paths. Shell and Python history are ordinary files under
`/root`. Python packages live under `/python/site-packages`.

`ProfileV1` is stored in IndexedDB under `elyk.profile.v1`. Profile and
`SiteBackupV1` versions are strict: invalid versions/shapes are rejected, not
silently repaired or cleared. Game saves live in localStorage. Site backups
combine the profile with only the keys listed in `BACKUP_STORAGE_KEYS`.

When adding/changing persistence:

- Validate untrusted save fields before constructing runtime state.
- Add new game keys to `BACKUP_STORAGE_KEYS` and backup round-trip tests.
- Preserve current Textropolis `v1` progress. Other unreleased/pre-1.0 game
  saves may reject incompatible versions without migrations; do not auto-clear.
- Bubbles resumes only saves matching its version, mode, board, queue, and
  resumability invariants.

## Python Boundary

Pyodide is loaded only when Python functionality is requested. Every operation
that touches its shared runtime, including completion and prewarming, must go
through `python::spawn_job`. A FIFO job owns Pyodide until filesystem changes
have been synchronized back and the updated profile has been persisted. Capture
terminal filesystem snapshots when a queued job starts, not when it is queued.

Known limits: serialization is page-local, so separate tabs can still race on
IndexedDB. Binary Pyodide files are not represented by the text-only user
overlay and are session-only. The HTML page backup embeds save data but is not a
self-contained offline copy of JS/WASM/assets.

## Games

Commands launch games with `OutputBlock::LaunchGame(GameLaunch::...)`.
`games::GameLaunchView` translates that value into a component; commands do not
mount components directly. Keep rules, save validation, and atomic purchases in
state modules rather than mutating game internals from views.

To add a game:

1. Create its namespace under `src/games/`, normally with state/view/storage
   modules and native state tests.
2. Add a command, `GameLaunch` variant, `GameLaunchView` branch, and exports.
3. Add save keys and backup tests if it persists.
4. Provide native fallbacks or target gates where browser-only Leptos/canvas
   code would otherwise break native tests and Clippy.
5. Extend an existing shared module only for a concrete shared contract. For
   example, word games share JSON DTO parsing, not game rules or UI state.

Snek is experimental and entirely gated by `debug_assertions`. Release builds
must not contain its command, launch/output variants, storage key, state, view,
or strings. Bubbles cheat parsing and controls must also compile out in release.

## Word Assets

Tracked release inputs are:

- `wordlist.json.gz`: lightweight generation/validation words.
- `dictionary.json.gz`: combined definitions for both word games.
- `word-assets.json`: URLs and a version derived from both gzip SHA-256 hashes.

Both games demand-load these after initial mount. Asset URLs include the manifest
version and use the browser HTTP cache; do not add unconditional startup loads or
a second custom persistent cache.

`scripts/build_textropolis_dictionary.py` rebuilds the data from WordNet,
ESDB/SCOWL, optional Webster, and optional default-ESDB inputs. See its `--help`
and source metadata for current input contracts. A typical invocation supplies
`--wordnet-zip`, `--esdb-large-wordlist`, and `--scowl`; gzip output is
deterministic and the script updates `word-assets.json`.

The combined dictionary is canonical compact JSON inside deterministic gzip.
This keeps decoded size and browser parse overhead down without introducing a
second runtime format or cache layer.

Generated uncompressed dictionaries, scoped dictionaries, and audit reports are
ignored intermediates. Do not commit them. After regeneration, run
`python3 scripts/check_word_assets.py` and verify the release build contains all
three tracked files.

## Build Modes And UI

The project uses Rust 1.88+ and CI pins Trunk 0.21.14. `debug_assertions` changes
the command registry and compiled game code. The canonical gate runs the normal
test/check suite once, then verifies release compilation and the built artifact
rather than duplicating every check in both modes. `Trunk.toml` hashes built
JS/WASM/CSS; word assets are copied with their own manifest version. `dist/` is
generated and ignored.

The root `--app-height` is intentionally captured before the mobile keyboard
opens so terminal games do not shrink. Preserve that behavior when changing
viewport logic. Keep controls touch-usable and check both desktop and mobile
layouts for overlap and text clipping.

## Required Validation

Run focused tests while developing. Before a ship-ready commit, run the relevant
checks with the native host toolchain, outside Docker, so each target/profile is
compiled only when the change warrants it. The usual Rust gate is:

```sh
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Add `cargo check --target wasm32-unknown-unknown` for browser-bound Rust changes,
`python3 scripts/check_word_assets.py` for word-asset changes, and a pinned Trunk
release build plus release-artifact checks for release-bound UI or debug-gating
changes. Use `act -j build` only when `.github/workflows/deploy.yml`, deployment
packaging, or the equivalence of the complete CI gate itself changed and local
workflow emulation is specifically useful. Do not substitute Dockerized checks
for available host tools or rerun equivalent checks merely for ceremony.

The workflow publishes `dist/` to GitHub Pages and restores `CNAME`. Agents may
inspect existing remote runs with `gh run list` / `gh run view`, but must not
push or dispatch workflows.

On this macOS workspace, avoid `rg`; use `git grep`, scoped `grep`, and `find`.
Preserve unrelated dirty-worktree changes and do not commit generated build
output.
