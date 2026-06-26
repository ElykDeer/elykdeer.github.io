# elyk.dev

This is the Rust/WebAssembly version of `elyk.dev`.

It is still a personal site first: a quiet overlay with my name, role, and contact links sits over a terminal. The terminal is the deeper interface. Type `help` to explore files, Python, the editor, and the built-in bubbles game.

## Run It

Install the Rust WASM target and Trunk:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
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

```sh
cargo fmt --check
cargo test
cargo check
cargo check --target wasm32-unknown-unknown
cargo clippy --all-targets --all-features -- -D warnings
```

## Project Layout

```text
src/
  app.rs          terminal UI and profile overlay
  commands/      terminal commands
  content/       read-only filesystem roots
  fs/            virtual filesystem and overlay logic
  game/          bubbles rules and canvas renderer
  python/        Pyodide bridge
  storage/       local profile import/export
  terminal/      parser, output blocks, context
```

Runtime state lives in IndexedDB. `/root` is the home directory, and `pip install` stores importable packages under `/python/site-packages`. Packages that expose `console_scripts` can be run as terminal commands by name; text output can be piped into them, for example `echo "hi" | lolcat`.
