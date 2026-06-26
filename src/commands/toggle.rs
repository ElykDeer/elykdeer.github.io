use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct ToggleCommand;

impl Command for ToggleCommand {
    fn name(&self) -> &'static str {
        "toggle"
    }

    fn summary(&self) -> &'static str {
        "Show or hide the profile card."
    }

    fn long_help(&self) -> &'static str {
        "Usage: toggle\n\nShows or hides the profile card layered over the terminal."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if !args.is_empty() {
            return usage_error("Usage: toggle");
        }

        ctx.toggle_overlay();

        Ok(Vec::new())
    }
}
