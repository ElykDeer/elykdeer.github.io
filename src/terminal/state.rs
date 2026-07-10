use crate::commands::CommandMetadata;
use crate::content;
use crate::fs::{FsError, UserOverlay, VirtualFileSystem};
use crate::storage::{ProfileImportError, ProfileV1};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleScriptInvocation {
    pub name: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchInvocation {
    pub url: String,
    pub output_path: Option<String>,
    pub cwd: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PythonFileInvocation {
    pub path: String,
    pub args: Vec<String>,
}

/// The async producer of a two-stage pipe: a package console script
/// (`cowsay hi | ...`) or a Python file (`python3 g.py | ...`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeProducer {
    Script(ConsoleScriptInvocation),
    File(PythonFileInvocation),
}

/// A two-stage pipe whose producer runs async; its stdout becomes the
/// consumer's stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsolePipe {
    pub producer: PipeProducer,
    pub consumer: crate::terminal::ParsedCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandEffect {
    Clear,
    EnterPython,
    PipInstall(Vec<String>),
    ConsoleScript(ConsoleScriptInvocation),
    Fetch(FetchInvocation),
    PythonFile(PythonFileInvocation),
    ConsolePipe(ConsolePipe),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandExecution {
    pub output: Vec<crate::terminal::OutputBlock>,
    pub effect: Option<CommandEffect>,
}

impl CommandExecution {
    pub fn new(output: Vec<crate::terminal::OutputBlock>, effect: Option<CommandEffect>) -> Self {
        Self { output, effect }
    }

    pub fn from_output(output: Vec<crate::terminal::OutputBlock>) -> Self {
        Self::new(output, None)
    }
}

#[derive(Clone, Debug)]
pub struct TerminalContext {
    cwd: String,
    profile: ProfileV1,
    vfs: VirtualFileSystem,
    command_metadata: Vec<CommandMetadata>,
    command_effect: Option<CommandEffect>,
    stdin: Option<String>,
}

impl Default for TerminalContext {
    fn default() -> Self {
        let profile = ProfileV1::default();
        let vfs = VirtualFileSystem::with_empty_overlay(content::bundled_tree());
        Self {
            cwd: "/root".to_string(),
            profile,
            vfs,
            command_metadata: Vec::new(),
            command_effect: None,
            stdin: None,
        }
    }
}

impl TerminalContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_profile(profile: ProfileV1) -> Result<Self, TerminalContextError> {
        profile.validate()?;
        let vfs = VirtualFileSystem::new(content::bundled_tree(), profile.fs_overlay.clone())?;
        Ok(Self {
            cwd: "/root".to_string(),
            profile,
            vfs,
            command_metadata: Vec::new(),
            command_effect: None,
            stdin: None,
        })
    }

    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    pub fn set_cwd(&mut self, cwd: impl Into<String>) {
        self.cwd = cwd.into();
    }

    pub fn overlay_visible(&self) -> bool {
        self.profile.settings.overlay_visible
    }

    pub fn set_overlay_visible(&mut self, value: bool) {
        self.profile.settings.overlay_visible = value;
    }

    pub fn toggle_overlay(&mut self) -> bool {
        let visible = !self.profile.settings.overlay_visible;
        self.profile.settings.overlay_visible = visible;
        visible
    }

    pub fn profile(&self) -> &ProfileV1 {
        &self.profile
    }

    pub fn terminal_history(&self) -> Vec<String> {
        read_history(&self.vfs, SHELL_HISTORY_PATH)
    }

    pub fn record_history(&mut self, line: &str) {
        append_history(&mut self.vfs, SHELL_HISTORY_PATH, line);
    }

    /// The Python REPL keeps its own history file so its lines don't mingle with
    /// the shell's. Both live in `/root`, so they're normal files you can
    /// `cat`/`nano`/`rm`.
    pub fn python_history(&self) -> Vec<String> {
        read_history(&self.vfs, PYTHON_HISTORY_PATH)
    }

    pub fn record_python_history(&mut self, line: &str) {
        append_history(&mut self.vfs, PYTHON_HISTORY_PATH, line);
    }

    pub fn replace_profile(&mut self, profile: ProfileV1) -> Result<(), TerminalContextError> {
        profile.validate()?;
        self.vfs = VirtualFileSystem::new(content::bundled_tree(), profile.fs_overlay.clone())?;
        self.profile = profile;
        self.cwd = "/root".to_string();
        Ok(())
    }

    pub fn vfs(&self) -> &VirtualFileSystem {
        &self.vfs
    }

    pub fn vfs_mut(&mut self) -> &mut VirtualFileSystem {
        &mut self.vfs
    }

    pub fn sync_profile_from_runtime(&mut self) {
        self.profile.fs_overlay = self.vfs.overlay.clone();
    }

    pub fn command_metadata(&self) -> &[CommandMetadata] {
        &self.command_metadata
    }

    pub fn set_command_metadata(&mut self, metadata: Vec<CommandMetadata>) {
        self.command_metadata = metadata;
    }

    pub fn request_clear(&mut self) {
        self.request_effect(CommandEffect::Clear);
    }

    pub fn request_python(&mut self) {
        self.request_effect(CommandEffect::EnterPython);
    }

    pub fn request_pip(&mut self, packages: Vec<String>) {
        self.request_effect(CommandEffect::PipInstall(packages));
    }

    pub fn console_script_names(&self) -> Vec<String> {
        console_script_names(self.vfs())
    }

    pub fn has_console_script(&self, name: &str) -> bool {
        self.console_script_names()
            .binary_search_by(|candidate| candidate.as_str().cmp(name))
            .is_ok()
    }

    pub fn request_console_script(
        &mut self,
        name: String,
        args: Vec<String>,
        stdin: Option<String>,
    ) {
        self.request_effect(CommandEffect::ConsoleScript(ConsoleScriptInvocation {
            name,
            args,
            stdin,
        }));
    }

    pub fn request_fetch(&mut self, url: String, output_path: Option<String>) {
        let invocation = FetchInvocation {
            url,
            output_path,
            cwd: self.cwd.clone(),
        };
        self.request_effect(CommandEffect::Fetch(invocation));
    }

    pub fn set_stdin(&mut self, stdin: String) {
        self.stdin = Some(stdin);
    }

    pub fn take_stdin(&mut self) -> Option<String> {
        self.stdin.take()
    }

    pub fn request_python_file(&mut self, path: String, args: Vec<String>) {
        self.request_effect(CommandEffect::PythonFile(PythonFileInvocation {
            path,
            args,
        }));
    }

    pub fn request_console_pipe(
        &mut self,
        producer: PipeProducer,
        consumer: crate::terminal::ParsedCommand,
    ) {
        self.request_effect(CommandEffect::ConsolePipe(ConsolePipe {
            producer,
            consumer,
        }));
    }

    pub fn take_command_effect(&mut self) -> Option<CommandEffect> {
        self.command_effect.take()
    }

    fn request_effect(&mut self, effect: CommandEffect) {
        assert!(
            self.command_effect.is_none(),
            "a command cannot request more than one effect"
        );
        self.command_effect = Some(effect);
    }
}

const SHELL_HISTORY_PATH: &str = "/root/.history";
const PYTHON_HISTORY_PATH: &str = "/root/.python_history";
const MAX_HISTORY_ENTRIES: usize = 200;

// Entries are separated by a blank line so a single entry can span multiple
// lines (a Python block) and the file still reads/edits like normal text.
fn read_history(vfs: &VirtualFileSystem, path: &str) -> Vec<String> {
    vfs.read_file("/", path)
        .map(|file| history_entries_from_content(&file.content))
        .unwrap_or_default()
}

fn append_history(vfs: &mut VirtualFileSystem, path: &str, entry: &str) {
    let entry = entry.trim();
    if entry.is_empty() {
        return;
    }

    let mut entries = read_history(vfs, path);
    entries.push(entry.to_string());
    if entries.len() > MAX_HISTORY_ENTRIES {
        entries.drain(0..entries.len() - MAX_HISTORY_ENTRIES);
    }
    let _ = vfs.write_file("/", path, entries.join("\n\n"));
}

fn history_entries_from_content(content: &str) -> Vec<String> {
    content
        .split("\n\n")
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
        .collect()
}

fn console_script_names(vfs: &VirtualFileSystem) -> Vec<String> {
    let mut names = BTreeSet::new();
    let Ok(entries) = vfs.list_dir("/", Some("/python/site-packages")) else {
        return Vec::new();
    };

    for entry in entries {
        if entry.name.ends_with(".dist-info") {
            // Modern wheels declare console scripts in entry_points.txt.
            let path = format!("{}/entry_points.txt", entry.path);
            if let Ok(file) = vfs.read_file("/", &path) {
                names.extend(parse_console_script_names(&file.content));
            }
        } else if entry.name.ends_with(".data") {
            // Older wheels ship the executable as a plain script under data/scripts.
            let scripts_dir = format!("{}/scripts", entry.path);
            if let Ok(scripts) = vfs.list_dir("/", Some(&scripts_dir)) {
                for script in scripts {
                    if is_console_script_name(&script.name) {
                        names.insert(script.name);
                    }
                }
            }
        }
    }

    names.into_iter().collect()
}

fn parse_console_script_names(content: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_console_scripts = false;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_console_scripts = line[1..line.len() - 1]
                .trim()
                .eq_ignore_ascii_case("console_scripts");
            continue;
        }

        if !in_console_scripts {
            continue;
        }

        let Some((name, _target)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if is_console_script_name(name) {
            names.push(name.to_string());
        }
    }

    names
}

fn is_console_script_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}

#[derive(Debug)]
pub enum TerminalContextError {
    Profile(ProfileImportError),
    Filesystem(FsError),
}

impl fmt::Display for TerminalContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(err) => write!(f, "{err}"),
            Self::Filesystem(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TerminalContextError {}

impl From<ProfileImportError> for TerminalContextError {
    fn from(value: ProfileImportError) -> Self {
        Self::Profile(value)
    }
}

impl From<FsError> for TerminalContextError {
    fn from(value: FsError) -> Self {
        Self::Filesystem(value)
    }
}

impl From<UserOverlay> for TerminalContext {
    fn from(fs_overlay: UserOverlay) -> Self {
        let profile = ProfileV1::with_overlay(fs_overlay);
        Self::from_profile(profile).expect("valid overlay should build terminal context")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context_with_entry_points(content: &str) -> TerminalContext {
        let mut ctx = TerminalContext::new();
        ctx.vfs_mut().apply_external_mkdir("/python/site-packages");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/demo-1.0.dist-info");
        ctx.vfs_mut().apply_external_write(
            "/python/site-packages/demo-1.0.dist-info/entry_points.txt",
            content.to_string(),
        );
        ctx.sync_profile_from_runtime();
        ctx
    }

    #[test]
    fn discovers_data_scripts_from_wheel_data_dir() {
        let mut ctx = TerminalContext::new();
        ctx.vfs_mut().apply_external_mkdir("/python/site-packages");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/lolcat-0.43.1.data");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/lolcat-0.43.1.data/scripts");
        ctx.vfs_mut().apply_external_write(
            "/python/site-packages/lolcat-0.43.1.data/scripts/lolcat",
            "#!python\nprint('hi')\n".to_string(),
        );
        ctx.sync_profile_from_runtime();

        assert!(ctx.has_console_script("lolcat"));
    }

    #[test]
    fn discovers_console_scripts_from_dist_info_entry_points() {
        let ctx = context_with_entry_points(
            r#"
[gui_scripts]
demo-gui = demo:gui

[console_scripts]
lolcat = lolcat:main
demo-tool = demo.cli:main
bad script = demo:bad
"#,
        );

        assert_eq!(
            ctx.console_script_names(),
            vec!["demo-tool".to_string(), "lolcat".to_string()]
        );
        assert!(ctx.has_console_script("lolcat"));
        assert!(!ctx.has_console_script("demo-gui"));
    }

    #[test]
    fn records_one_typed_command_effect() {
        let mut ctx = TerminalContext::new();

        ctx.request_console_script(
            "lolcat".to_string(),
            vec!["--seed".to_string(), "7".to_string()],
            Some("hello\n".to_string()),
        );

        assert_eq!(
            ctx.take_command_effect(),
            Some(CommandEffect::ConsoleScript(ConsoleScriptInvocation {
                name: "lolcat".to_string(),
                args: vec!["--seed".to_string(), "7".to_string()],
                stdin: Some("hello\n".to_string()),
            }))
        );
        assert_eq!(ctx.take_command_effect(), None);
    }

    #[test]
    fn python_history_is_separate_from_shell_history() {
        let mut ctx = TerminalContext::new();
        ctx.record_history("ls /root");
        ctx.record_python_history("import os");

        assert_eq!(ctx.terminal_history(), ["ls /root".to_string()]);
        assert_eq!(ctx.python_history(), ["import os".to_string()]);
    }
}
