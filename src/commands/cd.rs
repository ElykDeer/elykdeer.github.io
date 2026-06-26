use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct CdCommand;

impl Command for CdCommand {
    fn name(&self) -> &'static str {
        "cd"
    }

    fn summary(&self) -> &'static str {
        "Change the terminal working directory."
    }

    fn long_help(&self) -> &'static str {
        "Usage: cd [path]\n\nChanges the current directory. With no path, returns to /."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.len() > 1 {
            return usage_error("Usage: cd [path]");
        }

        let target = args.first().map(String::as_str).unwrap_or("/");
        let normalized = ctx
            .vfs()
            .resolve_path(ctx.cwd(), target)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        if !ctx.vfs().has_dir(&normalized) {
            return Err(crate::commands::CommandError::new(format!(
                "not a directory: {normalized}"
            )));
        }
        ctx.set_cwd(normalized);

        Ok(Vec::new())
    }
}
