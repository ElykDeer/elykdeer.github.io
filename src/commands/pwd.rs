use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct PwdCommand;

impl Command for PwdCommand {
    fn name(&self) -> &'static str {
        "pwd"
    }

    fn summary(&self) -> &'static str {
        "Print the current working directory."
    }

    fn long_help(&self) -> &'static str {
        "Usage: pwd\n\nPrints the current directory."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if !args.is_empty() {
            return usage_error("Usage: pwd");
        }

        Ok(vec![OutputBlock::Text(ctx.cwd().to_string())])
    }
}
