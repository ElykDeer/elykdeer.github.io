use crate::commands::{Command, CommandError, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct HeadCommand;

impl Command for HeadCommand {
    fn name(&self) -> &'static str {
        "head"
    }

    fn summary(&self) -> &'static str {
        "Print the first lines of a file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: head [-n count] <path>\n\nPrints the first 10 lines of a file by default."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let (count, path) = parse_count_and_path(args, "head")?;
        let file = ctx.vfs().read_file(ctx.cwd(), &path).map_err(fs_error)?;

        let text = file
            .content
            .lines()
            .take(count)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(vec![OutputBlock::Text(text)])
    }
}

pub(crate) fn parse_count_and_path(
    args: &[String],
    command: &str,
) -> Result<(usize, String), crate::commands::CommandError> {
    let usage = format!("Usage: {command} [-n count] <path>");
    match args {
        [path] => Ok((10, path.clone())),
        [flag, count, path] if flag == "-n" => {
            let count = count
                .parse::<usize>()
                .map_err(|_| CommandError::new(&usage))?;
            Ok((count, path.clone()))
        }
        _ => Err(CommandError::new(usage)),
    }
}

fn fs_error(err: impl ToString) -> CommandError {
    CommandError::new(err.to_string())
}
