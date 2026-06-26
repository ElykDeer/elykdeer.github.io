use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct MkdirCommand;

impl Command for MkdirCommand {
    fn name(&self) -> &'static str {
        "mkdir"
    }

    fn summary(&self) -> &'static str {
        "Create a directory."
    }

    fn long_help(&self) -> &'static str {
        "Usage: mkdir <path>\n\nCreates a directory."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let [path] = args else {
            return usage_error("Usage: mkdir <path>");
        };

        let cwd = ctx.cwd().to_string();
        ctx.vfs_mut()
            .mkdir(&cwd, path)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        ctx.sync_profile_from_runtime();

        Ok(Vec::new())
    }
}
