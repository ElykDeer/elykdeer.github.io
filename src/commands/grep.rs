use crate::commands::{usage_error, Command, CommandError, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct GrepCommand;

impl Command for GrepCommand {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn summary(&self) -> &'static str {
        "Print lines matching a pattern."
    }

    fn long_help(&self) -> &'static str {
        "Usage: grep [-n] <pattern> <path> [path...]\n\nSearches one or more files and prints matching lines."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let mut show_numbers = false;
        let mut positionals = Vec::new();

        for arg in args {
            match arg.as_str() {
                "-n" => show_numbers = true,
                _ if arg.starts_with('-') => {
                    return usage_error("Usage: grep [-n] <pattern> <path> [path...]");
                }
                _ => positionals.push(arg.as_str()),
            }
        }

        if positionals.len() < 2 {
            return usage_error("Usage: grep [-n] <pattern> <path> [path...]");
        }

        let pattern = positionals[0];
        let paths = &positionals[1..];
        let multiple_files = paths.len() > 1;
        let mut matches = Vec::new();

        for path in paths {
            let file = ctx.vfs().read_file(ctx.cwd(), path).map_err(fs_error)?;

            for (index, line) in file.content.lines().enumerate() {
                if line.contains(pattern) {
                    let rendered = if multiple_files && show_numbers {
                        format!("{path}:{}:{line}", index + 1)
                    } else if multiple_files {
                        format!("{path}:{line}")
                    } else if show_numbers {
                        format!("{}:{line}", index + 1)
                    } else {
                        line.to_string()
                    };
                    matches.push(rendered);
                }
            }
        }

        if matches.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![OutputBlock::Text(matches.join("\n"))])
        }
    }
}

fn fs_error(err: impl ToString) -> CommandError {
    CommandError::new(err.to_string())
}
