use crate::commands::{Command, CommandError, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct TailCommand;

impl Command for TailCommand {
    fn name(&self) -> &'static str {
        "tail"
    }

    fn summary(&self) -> &'static str {
        "Print the last lines of a file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: tail [-n count] <path>\n\nPrints the last 10 lines of a file by default."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let (count, path) = crate::commands::head::parse_count_and_path(args, "tail")?;
        let file = ctx.vfs().read_file(ctx.cwd(), &path).map_err(fs_error)?;
        let lines = file.content.lines().collect::<Vec<_>>();
        let start = lines.len().saturating_sub(count);
        let text = lines[start..].join("\n");

        Ok(vec![OutputBlock::Text(text)])
    }
}

fn fs_error(err: impl ToString) -> CommandError {
    CommandError::new(err.to_string())
}
