use crate::terminal::state::{CommandEffect, CommandExecution};
use crate::terminal::{parse_command_line, OutputBlock, ParseError, TerminalContext};

pub mod bubbles;
pub mod cat;
pub mod cd;
pub mod clear;
pub mod echo;
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
pub mod save;
pub mod sh;
#[cfg(debug_assertions)]
pub mod snek;
pub mod tail;
pub mod textropolis;
pub mod toggle;
pub mod wc;
pub mod wordhunt;

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
    with_debug_commands(vec![
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
        Box::new(import::ImportCommand),
        Box::new(save::SaveCommand),
        Box::new(clear::ClearCommand),
        Box::new(toggle::ToggleCommand),
        Box::new(bubbles::BubblesCommand),
        Box::new(textropolis::TextropolisCommand),
        Box::new(wordhunt::WordHuntCommand),
    ])
}

#[cfg(debug_assertions)]
fn with_debug_commands(mut commands: Vec<Box<dyn Command>>) -> Vec<Box<dyn Command>> {
    commands.push(Box::new(snek::SnekCommand));
    commands
}

#[cfg(not(debug_assertions))]
fn with_debug_commands(commands: Vec<Box<dyn Command>>) -> Vec<Box<dyn Command>> {
    commands
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
    let mut execution = execute_line(ctx, line);
    if execution.effect.is_some() {
        execution.output.push(OutputBlock::Error(
            "That command requires the top-level terminal executor.".to_string(),
        ));
    }
    execution.output
}

pub fn execute_line(ctx: &mut TerminalContext, line: &str) -> CommandExecution {
    match parse_command_line(line) {
        Ok(parsed) => run_parsed(ctx, parsed),
        Err(ParseError::Empty) => CommandExecution::default(),
        Err(ParseError::TrailingEscape) => CommandExecution::from_output(vec![OutputBlock::Error(
            "Command ended with an unfinished escape.".to_string(),
        )]),
        Err(ParseError::UnterminatedQuote(quote)) => {
            CommandExecution::from_output(vec![OutputBlock::Error(format!(
                "Unterminated {} quote.",
                quote_name(quote)
            ))])
        }
        Err(ParseError::EmptyPipelineStage) => {
            CommandExecution::from_output(vec![OutputBlock::Error(
                "Empty pipeline stage.".to_string(),
            )])
        }
        Err(ParseError::EmptyRedirectTarget) => {
            CommandExecution::from_output(vec![OutputBlock::Error(
                "Output redirection needs exactly one target path.".to_string(),
            )])
        }
        Err(ParseError::UnsupportedRedirect) => {
            CommandExecution::from_output(vec![OutputBlock::Error(
                "Only simple '>' output redirection to one file is supported.".to_string(),
            )])
        }
    }
}

pub fn run_parsed(
    ctx: &mut TerminalContext,
    parsed: crate::terminal::ParsedCommand,
) -> CommandExecution {
    assert!(
        ctx.take_command_effect().is_none(),
        "the previous command effect was not consumed"
    );
    let commands = registry();
    ctx.set_command_metadata(commands.iter().map(|command| command.metadata()).collect());

    let output = match find_command(&commands, &parsed.name) {
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
    };

    CommandExecution::new(output, ctx.take_command_effect())
}

/// Run a file of shell commands, one per line (blank lines and `#` comments
/// skipped). Effects requiring the interactive executor are reported and
/// rejected at the line where they occur.
pub fn run_script(ctx: &mut TerminalContext, source: &str) -> Vec<OutputBlock> {
    let mut output = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let execution = execute_line(ctx, line);
        output.extend(execution.output);
        if let Some(effect) = execution.effect {
            output.push(OutputBlock::Error(format!(
                "sh: {} is not supported in shell scripts.",
                effect_description(&effect)
            )));
        }
    }
    output
}

fn effect_description(effect: &CommandEffect) -> &'static str {
    match effect {
        CommandEffect::Clear => "clear",
        CommandEffect::EnterPython => "interactive Python",
        CommandEffect::PipInstall(_) => "a pip installation",
        CommandEffect::ConsoleScript(_) => "a package console script",
        CommandEffect::Fetch(_) => "a network request",
        CommandEffect::PythonFile(_) => "Python file execution",
        CommandEffect::ConsolePipe(_) => "an async pipeline",
    }
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
    use crate::terminal::{GameLaunch, WordHuntLaunchBoard};

    #[test]
    fn every_registered_command_has_metadata() {
        let commands = registry();

        let expected_len = if cfg!(debug_assertions) { 28 } else { 27 };
        assert_eq!(commands.len(), expected_len);
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

        let execution = execute_line(&mut ctx, "lolcat --seed 3 hello");

        assert!(execution.output.is_empty());
        assert_eq!(
            execution.effect,
            Some(CommandEffect::ConsoleScript(
                crate::terminal::ConsoleScriptInvocation {
                    name: "lolcat".to_string(),
                    args: vec!["--seed".to_string(), "3".to_string(), "hello".to_string()],
                    stdin: None,
                }
            ))
        );
    }

    #[test]
    fn shell_script_reports_and_consumes_async_effects() {
        let mut ctx = TerminalContext::new();

        let output = run_script(&mut ctx, "curl https://example.com\necho still-running");

        assert_eq!(
            output,
            vec![
                OutputBlock::Error(
                    "sh: a network request is not supported in shell scripts.".to_string()
                ),
                OutputBlock::Text("still-running".to_string()),
            ]
        );
        assert_eq!(ctx.take_command_effect(), None);
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
    fn import_rejects_full_site_backup_without_corrupting_context() {
        let mut ctx = TerminalContext::new();
        ctx.vfs_mut()
            .write_file("/", "/local.md", "keep me")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let backup = crate::storage::export_site_backup_json(
            TerminalContext::new().profile(),
            Default::default(),
        )
        .unwrap();

        assert!(matches!(
            run_line(&mut ctx, &format!("import '{}'", backup)).as_slice(),
            [OutputBlock::Error(message)] if message.contains("save box")
        ));
        assert_eq!(
            ctx.vfs().read_file("/", "/local.md").unwrap().content,
            "keep me"
        );
    }

    #[test]
    fn save_box_commands_return_save_manager() {
        let mut ctx = TerminalContext::new();

        assert_eq!(run_line(&mut ctx, "save"), vec![OutputBlock::SaveManager]);
        assert!(matches!(
            run_line(&mut ctx, "import").as_slice(),
            [OutputBlock::Error(message)] if message.contains("Usage: import")
        ));
        assert!(matches!(
            run_line(&mut ctx, "export").as_slice(),
            [OutputBlock::Error(message)] if message.contains("Unknown command")
        ));
        assert!(matches!(
            run_line(&mut ctx, "download").as_slice(),
            [OutputBlock::Error(message)] if message.contains("Unknown command")
        ));
    }

    #[test]
    fn bubbles_hard_launches_hard_mode() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "bubbles hard"),
            vec![OutputBlock::LaunchGame(GameLaunch::Bubbles {
                hard: true,
                cheat: false
            })]
        );
    }

    #[test]
    fn bubbles_version_reports_game_version() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "bubbles version"),
            vec![OutputBlock::Text("Bubbles 2.1.0".to_string())]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn bubbles_cheat_launches_debug_cheat_mode() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "bubbles cheat"),
            vec![OutputBlock::LaunchGame(GameLaunch::Bubbles {
                hard: false,
                cheat: true
            })]
        );
        assert_eq!(
            run_line(&mut ctx, "bubbles hard cheat"),
            vec![OutputBlock::LaunchGame(GameLaunch::Bubbles {
                hard: true,
                cheat: true
            })]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn snek_launches_game() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "snek"),
            vec![OutputBlock::LaunchGame(GameLaunch::Snek { cheat: false })]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn snek_cheat_launches_debug_cheat_mode() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "snek cheat"),
            vec![OutputBlock::LaunchGame(GameLaunch::Snek { cheat: true })]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn snek_clear_reports_confirmation() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "snek clear"),
            vec![OutputBlock::Text("Snek save cleared.".to_string())]
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn snek_version_reports_game_version() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "snek version"),
            vec![OutputBlock::Text("Snek 0.14.0".to_string())]
        );
    }

    #[test]
    fn textropolis_launches_game() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "textropolis"),
            vec![OutputBlock::LaunchGame(GameLaunch::Textropolis)]
        );
    }

    #[test]
    fn textropolis_version_reports_game_version() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "textropolis version"),
            vec![OutputBlock::Text("Textropolis 1.0.0".to_string())]
        );
    }

    #[test]
    fn wordhunt_launches_game() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "wordhunt"),
            vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
                reveal: false,
                board: None,
            })]
        );
    }

    #[test]
    fn wordhunt_version_reports_game_version() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "wordhunt version"),
            vec![OutputBlock::Text("Word Hunt 1.2.0".to_string())]
        );
    }

    #[test]
    fn wordhunt_reveal_launches_revealed_game() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "wordhunt reveal"),
            vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
                reveal: true,
                board: None,
            })]
        );
    }

    #[test]
    fn wordhunt_accepts_board_size_and_reveal() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "wordhunt 21x20 reveal"),
            vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
                reveal: true,
                board: Some(WordHuntLaunchBoard::Shape { cols: 21, rows: 20 }),
            })]
        );
    }

    #[test]
    fn wordhunt_accepts_max_size() {
        let mut ctx = TerminalContext::new();

        assert_eq!(
            run_line(&mut ctx, "wordhunt max"),
            vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
                reveal: false,
                board: Some(WordHuntLaunchBoard::Max),
            })]
        );
    }

    #[test]
    fn wordhunt_rejects_too_small_board_size() {
        let mut ctx = TerminalContext::new();
        let output = run_line(&mut ctx, "wordhunt 4x5");

        assert!(
            matches!(output.as_slice(), [OutputBlock::Error(message)] if message.starts_with("Usage:")),
            "{output:?}"
        );
    }

    #[test]
    fn zero_arg_commands_reject_extra_args() {
        for line in [
            "clear now",
            "pwd /",
            "toggle overlay",
            "help pwd extra",
            "textropolis now",
            "save now",
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
