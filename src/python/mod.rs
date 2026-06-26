//! Async bridge to the Pyodide runtime loaded by `index.html`. Rust starts the
//! load and feeds the REPL; eval state lives in Pyodide's `__main__` and
//! persists for the lifetime of the page.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = elykPyodideReady)]
    fn pyodide_ready() -> bool;

    #[wasm_bindgen(js_name = elykEnsurePyodide)]
    fn ensure_pyodide() -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykRunPython)]
    fn run_python_js(code: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykFsApply)]
    fn fs_apply_js(ops_json: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykFsSnapshot)]
    fn fs_snapshot_js() -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykPipInstall)]
    fn pip_install_js(pkgs_json: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykRunConsoleScript)]
    fn run_console_script_js(name: &str, args_json: &str, stdin: Option<&str>) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykRunPythonFile)]
    fn run_python_file_js(path: &str, args_json: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_name = elykPythonComplete)]
    fn python_complete_js(line: &str) -> js_sys::Promise;
}

/// What we last wrote to Pyodide's filesystem. Diffing against it lets the sync
/// skip unchanged files and propagate terminal-side deletes without disturbing
/// files Python created itself.
#[derive(Default)]
struct FsMirror {
    files: BTreeMap<String, String>,
    dirs: BTreeSet<String>,
}

thread_local! {
    static MIRROR: RefCell<FsMirror> = RefCell::new(FsMirror::default());
}

/// Whether Pyodide has finished loading.
pub fn is_ready() -> bool {
    pyodide_ready()
}

/// Start loading Pyodide; safe to call repeatedly.
pub async fn ensure_loaded() -> Result<(), String> {
    JsFuture::from(ensure_pyodide())
        .await
        .map(|_| ())
        .map_err(describe_js_error)
}

pub enum Eval {
    /// Complete statement; holds captured stdout/stderr (Python errors included).
    Complete(String),
    /// Partial block; the caller should keep buffering under a `...` prompt.
    Incomplete,
}

#[derive(Deserialize)]
struct EvalResult {
    #[serde(default)]
    incomplete: bool,
    #[serde(default)]
    output: String,
}

/// Evaluate buffered REPL source. `Err` is reserved for bridge failures, not
/// Python errors (those come back inside `Eval::Complete`).
pub async fn run_source(source: &str) -> Result<Eval, String> {
    let result: EvalResult = call_json(run_python_js(source)).await?;
    if result.incomplete {
        Ok(Eval::Incomplete)
    } else {
        Ok(Eval::Complete(result.output))
    }
}

#[derive(Deserialize)]
struct PipResult {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    output: String,
}

/// Install packages into `/python/site-packages`. Returns the pip log,
/// in the `Err` arm on failure.
pub async fn pip_install(packages: &[String]) -> Result<String, String> {
    let pkgs_json = serde_json::to_string(packages).map_err(|err| err.to_string())?;
    let result: PipResult = call_json(pip_install_js(&pkgs_json)).await?;
    let output = result.output.trim_end_matches('\n').to_string();
    if result.ok {
        Ok(output)
    } else {
        Err(output)
    }
}

#[derive(Deserialize)]
struct ConsoleScriptResult {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    output: String,
}

/// Run an installed package `console_scripts` entry point by name. `args`
/// becomes `sys.argv[1:]` for the duration of the call. `stdin`, when present,
/// is exposed as `sys.stdin`.
pub async fn run_console_script(
    name: &str,
    args: &[String],
    stdin: Option<&str>,
) -> Result<String, String> {
    let args_json = serde_json::to_string(args).map_err(|err| err.to_string())?;
    let result: ConsoleScriptResult =
        call_json(run_console_script_js(name, &args_json, stdin)).await?;
    let output = result.output.trim_end_matches('\n').to_string();
    if result.ok {
        Ok(output)
    } else {
        Err(output)
    }
}

/// Run a Python file (`python3 file.py [args]`) and return captured output.
pub async fn run_python_file(path: &str, args: &[String]) -> Result<String, String> {
    let args_json = serde_json::to_string(args).map_err(|err| err.to_string())?;
    let result: ConsoleScriptResult = call_json(run_python_file_js(path, &args_json)).await?;
    let output = result.output.trim_end_matches('\n').to_string();
    if result.ok {
        Ok(output)
    } else {
        Err(output)
    }
}

/// A REPL tab-completion result: the fragment being completed and its matches.
pub struct Completion {
    pub fragment: String,
    pub matches: Vec<String>,
}

#[derive(Deserialize)]
struct CompletionResult {
    text: String,
    matches: Vec<String>,
}

/// Complete the trailing identifier in `line` against the live REPL namespace.
pub async fn complete(line: &str) -> Result<Completion, String> {
    let result: CompletionResult = call_json(python_complete_js(line)).await?;
    Ok(Completion {
        fragment: result.text,
        matches: result.matches,
    })
}

/// Push the terminal's filesystem into Pyodide and chdir into `cwd`, writing
/// only what changed since the last sync and unlinking terminal-side deletes.
pub async fn sync_to_pyodide(
    files: &[(String, String)],
    dirs: &[String],
    cwd: &str,
) -> Result<(), String> {
    let desired_files: BTreeMap<&String, &String> = files
        .iter()
        .map(|(path, content)| (path, content))
        .collect();
    let desired_dirs: BTreeSet<&String> = dirs.iter().collect();

    let mut mkdirs: Vec<String> = Vec::new();
    let mut writes: Vec<(String, String)> = Vec::new();
    let mut unlinks: Vec<String> = Vec::new();
    let mut rmdirs: Vec<String> = Vec::new();

    MIRROR.with(|mirror| {
        let mirror = mirror.borrow();
        for dir in dirs {
            if !mirror.dirs.contains(dir) {
                mkdirs.push(dir.clone());
            }
        }
        for (path, content) in files {
            if mirror.files.get(path) != Some(content) {
                writes.push((path.clone(), content.clone()));
            }
        }
        for path in mirror.files.keys() {
            if !desired_files.contains_key(path) {
                unlinks.push(path.clone());
            }
        }
        for dir in &mirror.dirs {
            if !desired_dirs.contains(dir) {
                rmdirs.push(dir.clone());
            }
        }
    });

    // Deepest first, so directories are empty before rmdir.
    rmdirs.sort_by_key(|path| std::cmp::Reverse(path.matches('/').count()));

    let ops = serde_json::json!({
        "cwd": cwd,
        "mkdirs": mkdirs,
        "writes": writes,
        "unlinks": unlinks,
        "rmdirs": rmdirs,
    });
    JsFuture::from(fs_apply_js(&ops.to_string()))
        .await
        .map_err(describe_js_error)?;

    MIRROR.with(|mirror| {
        let mut mirror = mirror.borrow_mut();
        mirror.files = files.iter().cloned().collect();
        mirror.dirs = dirs.iter().cloned().collect();
    });
    Ok(())
}

pub enum FsChange {
    Mkdir(String),
    Write(String, String),
    Remove(String),
}

#[derive(Deserialize)]
struct SnapshotEntry {
    path: String,
    #[serde(default)]
    dir: bool,
    #[serde(default)]
    binary: bool,
    #[serde(default)]
    content: Option<String>,
}

/// Read Pyodide's filesystem back and return what Python changed since the last
/// sync. Binary files are skipped — the overlay stores text only.
pub async fn sync_from_pyodide() -> Result<Vec<FsChange>, String> {
    let entries: Vec<SnapshotEntry> = call_json(fs_snapshot_js()).await?;

    let mut seen_files: BTreeMap<String, String> = BTreeMap::new();
    let mut seen_dirs: BTreeSet<String> = BTreeSet::new();
    let mut binary_paths: BTreeSet<String> = BTreeSet::new();
    for entry in entries {
        if entry.dir {
            seen_dirs.insert(entry.path);
        } else if entry.binary {
            binary_paths.insert(entry.path);
        } else if let Some(content) = entry.content {
            seen_files.insert(entry.path, content);
        }
    }

    let mut changes = Vec::new();
    MIRROR.with(|mirror| {
        let mut mirror = mirror.borrow_mut();

        for dir in &seen_dirs {
            if !mirror.dirs.contains(dir) {
                changes.push(FsChange::Mkdir(dir.clone()));
            }
        }
        for (path, content) in &seen_files {
            if mirror.files.get(path) != Some(content) {
                changes.push(FsChange::Write(path.clone(), content.clone()));
            }
        }
        // A path the mirror had but the snapshot doesn't was deleted in Python.
        // Binary files never enter the mirror, so they can't be falsely removed.
        for path in mirror.files.keys() {
            if !seen_files.contains_key(path) && !binary_paths.contains(path) {
                changes.push(FsChange::Remove(path.clone()));
            }
        }
        for dir in &mirror.dirs {
            if !seen_dirs.contains(dir) {
                changes.push(FsChange::Remove(dir.clone()));
            }
        }

        mirror.files = seen_files;
        mirror.dirs = seen_dirs;
    });

    Ok(changes)
}

/// Await a bridge call that resolves to a JSON string and deserialize it.
async fn call_json<T: for<'de> Deserialize<'de>>(promise: js_sys::Promise) -> Result<T, String> {
    let value = JsFuture::from(promise).await.map_err(describe_js_error)?;
    let json = value
        .as_string()
        .ok_or_else(|| "Python bridge returned no result".to_string())?;
    serde_json::from_str(&json).map_err(|err| err.to_string())
}

fn describe_js_error(err: JsValue) -> String {
    err.as_string()
        .or_else(|| {
            js_sys::Reflect::get(&err, &JsValue::from_str("message"))
                .ok()
                .and_then(|message| message.as_string())
        })
        .unwrap_or_else(|| "Python runtime error".to_string())
}
