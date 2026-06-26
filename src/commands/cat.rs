use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct CatCommand;

impl Command for CatCommand {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn summary(&self) -> &'static str {
        "Show a file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: cat <path>\n\nPrints the contents of a file."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let Some(path) = args.first() else {
            return usage_error("Usage: cat <path>");
        };
        if args.len() != 1 {
            return usage_error("Usage: cat <path>");
        }

        let file = ctx
            .vfs()
            .read_file(ctx.cwd(), path)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;

        Ok(vec![OutputBlock::Text(file.content)])
    }
}
