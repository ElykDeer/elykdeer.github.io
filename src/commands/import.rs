use crate::commands::{usage_error, Command, CommandResult};
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
        "Usage: import <profile-json>\n\nReplaces your saved state with the given JSON (as produced by export). Invalid JSON is rejected and leaves your state untouched."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.is_empty() {
            return usage_error("Usage: import <profile-json>");
        }

        let json = args.join(" ");
        let profile = import_profile_json(&json)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        ctx.replace_profile(profile)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;

        Ok(vec![OutputBlock::Text("profile imported.".to_string())])
    }
}
