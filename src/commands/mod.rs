use crate::terminal::{parse_command_line, OutputBlock, ParseError, TerminalContext};

pub mod bubbles;
pub mod cat;
pub mod cd;
pub mod clear;
pub mod echo;
pub mod export;
pub mod find;
pub mod grep;
pub mod head;
pub mod help;
pub mod import;
pub mod ls;
pub mod mkdir;
pub mod nano;
pub mod net;
pub mod pip;
pub mod pwd;
pub mod python;
pub mod rm;
pub mod sh;
pub mod tail;
pub mod toggle;
pub mod wc;

pub type CommandResult = Result<Vec<OutputBlock>, CommandError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandError {
    pub message: String,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandMetadata {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub long_help: &'static str,
}

pub trait Command {
    fn name(&self) -> &'static str;

    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    fn summary(&self) -> &'static str;

    fn long_help(&self) -> &'static str;

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult;

    fn metadata(&self) -> CommandMetadata {
        CommandMetadata {
            name: self.name(),
            aliases: self.aliases(),
            summary: self.summary(),
            long_help: self.long_help(),
        }
    }
}

pub fn registry() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(help::HelpCommand),
        Box::new(ls::LsCommand),
        Box::new(ls::LlCommand),
        Box::new(cd::CdCommand),
        Box::new(pwd::PwdCommand),
        Box::new(cat::CatCommand),
        Box::new(grep::GrepCommand),
        Box::new(head::HeadCommand),
        Box::new(tail::TailCommand),
        Box::new(wc::WcCommand),
        Box::new(find::FindCommand),
        Box::new(echo::EchoCommand),
        Box::new(net::CurlCommand),
        Box::new(net::WgetCommand),
        Box::new(nano::NanoCommand),
        Box::new(mkdir::MkdirCommand),
        Box::new(rm::RmCommand),
        Box::new(sh::ShCommand),
        Box::new(python::PythonCommand),
        Box::new(pip::PipCommand),
        Box::new(export::ExportCommand),
        Box::new(import::ImportCommand),
        Box::new(clear::ClearCommand),
        Box::new(toggle::ToggleCommand),
        Box::new(bubbles::BubblesCommand),
    ]
}

pub fn command_metadata() -> Vec<CommandMetadata> {
    registry()
        .iter()
        .map(|command| command.metadata())
        .collect()
}

pub fn find_command<'a>(commands: &'a [Box<dyn Command>], name: &str) -> Option<&'a dyn Command> {
    commands
        .iter()
        .find(|command| command.name() == name || command.aliases().contains(&name))
        .map(|command| command.as_ref())
}

pub fn run_line(ctx: &mut TerminalContext, line: &str) -> Vec<OutputBlock> {
    match parse_command_line(line) {
        Ok(parsed) => run_parsed(ctx, parsed),
        Err(ParseError::Empty) => Vec::new(),
        Err(ParseError::TrailingEscape) => {
            vec![OutputBlock::Error(
                "Command ended with an unfinished escape.".to_string(),
            )]
        }
        Err(ParseError::UnterminatedQuote(quote)) => vec![OutputBlock::Error(format!(
            "Unterminated {} quote.",
            quote_name(quote)
        ))],
        Err(ParseError::EmptyPipelineStage) => {
            vec![OutputBlock::Error("Empty pipeline stage.".to_string())]
        }
        Err(ParseError::EmptyRedirectTarget) => vec![OutputBlock::Error(
            "Output redirection needs exactly one target path.".to_string(),
        )],
        Err(ParseError::UnsupportedRedirect) => vec![OutputBlock::Error(
            "Only simple '>' output redirection to one file is supported.".to_string(),
        )],
    }
}

pub fn run_parsed(
    ctx: &mut TerminalContext,
    parsed: crate::terminal::ParsedCommand,
) -> Vec<OutputBlock> {
    let commands = registry();
    ctx.set_command_metadata(commands.iter().map(|command| command.metadata()).collect());

    match find_command(&commands, &parsed.name) {
        Some(command) => match command.run(ctx, &parsed.args) {
            Ok(output) => output,
            Err(err) => vec![OutputBlock::Error(err.message)],
        },
        None if ctx.has_console_script(&parsed.name) => {
            ctx.request_console_script(parsed.name, parsed.args, None);
            Vec::new()
        }
        None => vec![OutputBlock::Error(format!(
            "Unknown command '{}'. Type 'help' to see available commands.",
            parsed.name
        ))],
    }
}

/// Run a file of shell commands, one per line (blank lines and `#` comments
/// skipped). Async commands (python3/pip and console scripts) aren't supported
/// inside a script — their pending requests are dropped so they don't leak.
pub fn run_script(ctx: &mut TerminalContext, source: &str) -> Vec<OutputBlock> {
    let mut output = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        output.extend(run_line(ctx, line));
        ctx.take_python_requested();
        ctx.take_pip_request();
        ctx.take_console_script_request();
        ctx.take_fetch_request();
        ctx.take_python_file_request();
    }
    output
}

fn quote_name(quote: char) -> &'static str {
    match quote {
        '\'' => "single",
        '"' => "double",
        _ => "quoted",
    }
}

pub(crate) fn usage_error(message: impl Into<String>) -> CommandResult {
    Err(CommandError::new(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_command_has_metadata() {
        let commands = registry();

        assert_eq!(commands.len(), 25);
        for command in commands {
            assert!(!command.name().trim().is_empty());
            assert!(!command.summary().trim().is_empty(), "{}", command.name());
            assert!(!command.long_help().trim().is_empty(), "{}", command.name());
        }
    }

    #[test]
    fn aliases_resolve_to_command() {
        let commands = registry();

        assert_eq!(find_command(&commands, "?").unwrap().name(), "help");
    }

    #[test]
    fn python_repl_command_has_no_short_aliases() {
        let commands = registry();

        assert!(find_command(&commands, "python3").is_some());
        for disallowed in ["python", "py", "micropip"] {
            assert!(
                find_command(&commands, disallowed).is_none(),
                "{disallowed}"
            );
        }
    }

    #[test]
    fn registry_does_not_include_out_of_scope_commands() {
        let commands = registry();
        let names = commands
            .iter()
            .map(|command| command.name())
            .collect::<Vec<_>>();

        for disallowed in [
            "open",
            "write",
            "edit",
            "play",
            "achievements",
            "about",
            "contact",
            "projects",
            "repair",
        ] {
            assert!(!names.contains(&disallowed), "{disallowed}");
        }
    }

    #[test]
    fn ls_hides_dotfiles_unless_all() {
        let mut ctx = TerminalContext::new();
        ctx.set_cwd("/root");
        ctx.vfs_mut().write_file("/root", ".hidden", "x").unwrap();
        ctx.vfs_mut()
            .write_file("/root", "visible.txt", "y")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert!(matches!(
            run_line(&mut ctx, "ls").as_slice(),
            [OutputBlock::Text(text)] if text.contains("visible.txt") && !text.contains(".hidden")
        ));
        assert!(matches!(
            run_line(&mut ctx, "ls -a").as_slice(),
            [OutputBlock::Text(text)] if text.contains(".hidden")
        ));
    }

    #[test]
    fn ll_uses_compact_long_listing_width() {
        let mut ctx = TerminalContext::new();
        ctx.set_cwd("/root");
        ctx.vfs_mut().write_file("/root", "a.txt", "1").unwrap();
        ctx.vfs_mut()
            .write_file("/root", "long.txt", "1234567890")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert_eq!(
            run_line(&mut ctx, "ll"),
            vec![OutputBlock::Text("1  a.txt\n10  long.txt".to_string())]
        );
    }

    #[test]
    fn echo_prints_arguments_as_text() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "echo hello world"),
            vec![OutputBlock::Text("hello world".to_string())]
        );
    }

    #[test]
    fn unknown_command_returns_error_block() {
        let mut ctx = TerminalContext::new();
        let output = run_line(&mut ctx, "wat");

        assert_eq!(
            output,
            vec![OutputBlock::Error(
                "Unknown command 'wat'. Type 'help' to see available commands.".to_string()
            )]
        );
    }

    #[test]
    fn installed_console_script_dispatches_to_python_bridge() {
        let mut ctx = TerminalContext::new();
        ctx.vfs_mut().apply_external_mkdir("/python/site-packages");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/lolcat-1.0.dist-info");
        ctx.vfs_mut().apply_external_write(
            "/python/site-packages/lolcat-1.0.dist-info/entry_points.txt",
            "[console_scripts]\nlolcat = lolcat:main\n".to_string(),
        );
        ctx.sync_profile_from_runtime();

        let output = run_line(&mut ctx, "lolcat --seed 3 hello");

        assert!(output.is_empty());
        assert_eq!(
            ctx.take_console_script_request(),
            Some(crate::terminal::ConsoleScriptInvocation {
                name: "lolcat".to_string(),
                args: vec!["--seed".to_string(), "3".to_string(), "hello".to_string()],
                stdin: None,
            })
        );
    }

    #[test]
    fn executes_filesystem_commands_over_user_files() {
        let mut ctx = TerminalContext::new();

        assert!(run_line(&mut ctx, "mkdir /scratch").is_empty());
        assert!(run_line(&mut ctx, "cd /scratch").is_empty());
        assert_eq!(ctx.cwd(), "/scratch");

        ctx.vfs_mut()
            .write_file("/scratch", "note.md", "hello from overlay")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert!(matches!(
            run_line(&mut ctx, "cat note.md").as_slice(),
            [OutputBlock::Text(content)] if content == "hello from overlay"
        ));
    }

    #[test]
    fn grep_filters_matching_lines_from_vfs_files() {
        let mut ctx = TerminalContext::new();
        ctx.set_cwd("/root");
        ctx.vfs_mut()
            .write_file("/root", "notes.txt", "alpha\nbeta\nalphabet\n")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert_eq!(
            run_line(&mut ctx, "grep alpha notes.txt"),
            vec![OutputBlock::Text("alpha\nalphabet".to_string())]
        );
        assert_eq!(
            run_line(&mut ctx, "grep -n alpha notes.txt"),
            vec![OutputBlock::Text("1:alpha\n3:alphabet".to_string())]
        );
    }

    #[test]
    fn head_tail_and_wc_report_expected_text() {
        let mut ctx = TerminalContext::new();
        ctx.set_cwd("/root");
        ctx.vfs_mut()
            .write_file("/root", "sample.txt", "one\ntwo\nthree\nfour\n")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert_eq!(
            run_line(&mut ctx, "head -n 2 sample.txt"),
            vec![OutputBlock::Text("one\ntwo".to_string())]
        );
        assert_eq!(
            run_line(&mut ctx, "tail -n 2 sample.txt"),
            vec![OutputBlock::Text("three\nfour".to_string())]
        );
        assert_eq!(
            run_line(&mut ctx, "wc sample.txt"),
            vec![OutputBlock::Text("4 4 19 sample.txt".to_string())]
        );
    }

    #[test]
    fn find_recurses_and_supports_name_filter() {
        let mut ctx = TerminalContext::new();
        ctx.set_cwd("/root");
        ctx.vfs_mut().mkdir("/root", "scratch").unwrap();
        ctx.vfs_mut().mkdir("/root", "scratch/docs").unwrap();
        ctx.vfs_mut()
            .write_file("/root/scratch/docs", "alpha.txt", "a")
            .unwrap();
        ctx.vfs_mut()
            .write_file("/root/scratch", "beta.md", "b")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert_eq!(
            run_line(&mut ctx, "find scratch"),
            vec![OutputBlock::Text(
                "/root/scratch\n/root/scratch/beta.md\n/root/scratch/docs\n/root/scratch/docs/alpha.txt"
                    .to_string()
            )]
        );
        assert_eq!(
            run_line(&mut ctx, "find scratch -name '*.txt'"),
            vec![OutputBlock::Text(
                "/root/scratch/docs/alpha.txt".to_string()
            )]
        );
    }

    #[test]
    fn import_rejects_invalid_profile_without_corrupting_context() {
        let mut ctx = TerminalContext::new();
        ctx.vfs_mut()
            .write_file("/", "/local.md", "keep me")
            .unwrap();
        ctx.sync_profile_from_runtime();

        assert!(matches!(
            run_line(&mut ctx, r#"import '{"version":2}'"#).as_slice(),
            [OutputBlock::Error(message)] if message.contains("unsupported profile version")
        ));
        assert_eq!(
            ctx.vfs().read_file("/", "/local.md").unwrap().content,
            "keep me"
        );
    }

    #[test]
    fn bubbles_hard_launches_hard_mode() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "bubbles hard"),
            vec![OutputBlock::LaunchGame {
                game_id: "bubbles".to_string(),
                hard: true,
            }]
        );
    }

    #[test]
    fn zero_arg_commands_reject_extra_args() {
        for line in [
            "clear now",
            "pwd /",
            "toggle overlay",
            "help pwd extra",
        ] {
            let mut ctx = TerminalContext::new();
            let output = run_line(&mut ctx, line);
            assert!(
                matches!(output.as_slice(), [OutputBlock::Error(message)] if message.starts_with("Usage:")),
                "{line}: {output:?}"
            );
        }
    }
}
