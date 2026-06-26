use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct RmCommand;

impl Command for RmCommand {
    fn name(&self) -> &'static str {
        "rm"
    }

    fn summary(&self) -> &'static str {
        "Remove a file or directory."
    }

    fn long_help(&self) -> &'static str {
        "Usage: rm [-r] <path>\n\nRemoves a file. Pass -r (or -rf) to remove a directory and everything inside it."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let mut recursive = false;
        let mut path = None;

        for arg in args {
            if let Some(flags) = arg.strip_prefix('-').filter(|_| arg != "-") {
                for flag in flags.chars() {
                    match flag {
                        'r' | 'R' => recursive = true,
                        'f' => {}
                        _ => return usage_error("Usage: rm [-r] <path>"),
                    }
                }
            } else if path.replace(arg).is_some() {
                return usage_error("Usage: rm [-r] <path>");
            }
        }

        let Some(path) = path else {
            return usage_error("Usage: rm [-r] <path>");
        };

        let cwd = ctx.cwd().to_string();
        ctx.vfs_mut()
            .remove_path(&cwd, path, recursive)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        ctx.sync_profile_from_runtime();

        Ok(Vec::new())
    }
}
