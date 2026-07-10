# elyk.dev

This is the Rust/WebAssembly version of `elyk.dev`.

It is still a personal site first: a quiet overlay with my name, role, and contact links sits over a terminal. The terminal is the deeper interface. Type `help` to explore files, Python, the editor, and the built-in games.

## Run It

Install the Rust WASM target and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
```

Start a local server:

```sh
trunk serve --port 8080
```

Build the static site:

```sh
trunk build --release
```

## Test It

Run the same build job used by GitHub Pages:

```sh
act -j build
```

Use targeted `cargo test <filter>` and `cargo clippy --all-targets --all-features -- -D warnings`
while developing; there is no need to repeat the full workflow command list
before running `act`.

## Project Layout

```text
src/
  app.rs          terminal UI and profile overlay
  commands/      terminal commands
  content/       read-only filesystem roots
  fs/            virtual filesystem and overlay logic
  games/         Bubbles, Textropolis, WordHunt, and debug-only Snek
  python/        Pyodide bridge
  storage/       local profile import/export
  terminal/      parser, output blocks, context
```

Runtime profile state lives in IndexedDB; current game saves use local storage and are included in site backups. `/root` is the home directory, and `pip install` stores importable packages under `/python/site-packages`. Packages that expose `console_scripts` can be run as terminal commands by name; text output can be piped into them, for example `echo "hi" | lolcat`.

WordHunt and Textropolis share `wordlist.json.gz` and `dictionary.json.gz`. The tiny `word-assets.json` manifest contains their content-hash version, so browsers reuse unchanged assets. Generated uncompressed dictionaries and audit reports are intentionally ignored.

Snek is experimental and compiled only in debug builds. Release builds also omit all cheat controls.
