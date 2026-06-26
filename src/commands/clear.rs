use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &'static str {
        "clear"
    }

    fn summary(&self) -> &'static str {
        "Clear terminal output."
    }

    fn long_help(&self) -> &'static str {
        "Usage: clear\n\nClears the terminal output. Ctrl-L does the same."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if !args.is_empty() {
            return usage_error("Usage: clear");
        }

        ctx.request_clear();
        Ok(Vec::new())
    }
}
