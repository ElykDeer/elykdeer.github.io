use crate::commands::{usage_error, Command, CommandResult};
use crate::fs::{path, FsError};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct NanoCommand;

impl Command for NanoCommand {
    fn name(&self) -> &'static str {
        "nano"
    }

    fn summary(&self) -> &'static str {
        "Open an in-app editor for a file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: nano <path>\n\nOpens a simple editor for a file. Save writes it; Cancel discards changes."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let Some(path) = args.first() else {
            return usage_error("Usage: nano <path>");
        };
        if args.len() != 1 {
            return usage_error("Usage: nano <path>");
        }

        let (path, content) = match ctx.vfs().read_file(ctx.cwd(), path) {
            Ok(file) => (file.path, file.content),
            Err(FsError::NotFound(_)) => {
                let resolved_path = ctx
                    .vfs()
                    .resolve_path(ctx.cwd(), path)
                    .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
                let parent = path::parent_path(&resolved_path).ok_or_else(|| {
                    crate::commands::CommandError::new(format!(
                        "cannot edit directory: {resolved_path}"
                    ))
                })?;
                if !ctx.vfs().has_dir(&parent) {
                    return Err(crate::commands::CommandError::new(format!(
                        "parent directory does not exist: {parent}"
                    )));
                }
                (resolved_path, String::new())
            }
            Err(err) => return Err(crate::commands::CommandError::new(err.to_string())),
        };

        Ok(vec![OutputBlock::NanoEditor { path, content }])
    }
}
