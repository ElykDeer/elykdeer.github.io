use crate::commands::{Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct SaveCommand;

impl Command for SaveCommand {
    fn name(&self) -> &'static str {
        "save"
    }

    fn summary(&self) -> &'static str {
        "Open save, restore, and page-backup tools."
    }

    fn long_help(&self) -> &'static str {
        "Usage: save\n\nOpens the save box for downloading save data, restoring from a save file, or downloading an HTML page backup with the save data embedded."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if !args.is_empty() {
            return Err(crate::commands::CommandError::new("Usage: save"));
        }

        Ok(vec![OutputBlock::SaveManager])
    }
}
