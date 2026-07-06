use crate::commands::{Command, CommandResult};
use crate::storage::import_profile_json;
use crate::terminal::{OutputBlock, TerminalContext};

pub struct ImportCommand;

impl Command for ImportCommand {
    fn name(&self) -> &'static str {
        "import"
    }

    fn summary(&self) -> &'static str {
        "Restore saved state from JSON."
    }

    fn long_help(&self) -> &'static str {
        "Usage: import <profile-json>\n\nReplaces the terminal profile from compact profile JSON. For full site backups and file upload, use save."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.is_empty() {
            return Err(crate::commands::CommandError::new(
                "Usage: import <profile-json>. For save files, run save.",
            ));
        }

        let json = args.join(" ");
        let profile = import_profile_json(&json)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        ctx.replace_profile(profile)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;

        Ok(vec![OutputBlock::Text("profile imported.".to_string())])
    }
}
