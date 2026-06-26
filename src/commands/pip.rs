use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct PipCommand;

impl Command for PipCommand {
    fn name(&self) -> &'static str {
        "pip"
    }

    fn summary(&self) -> &'static str {
        "Install Python packages from PyPI."
    }

    fn long_help(&self) -> &'static str {
        "Usage: pip install <package>...\n\nInstalls pure-Python packages from PyPI \
into /python/site-packages, where they appear as normal files. Installs run in your browser and persist \
with the rest of your filesystem. Package console scripts become terminal commands by name."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let [subcommand, packages @ ..] = args else {
            return usage_error("Usage: pip install <package>...");
        };
        if subcommand != "install" {
            return usage_error(format!(
                "Unknown pip command '{subcommand}'. Try: pip install <package>..."
            ));
        }
        if packages.is_empty() {
            return usage_error("Usage: pip install <package>...");
        }

        ctx.request_pip(packages.to_vec());
        Ok(vec![OutputBlock::Text(format!(
            "Installing {}…",
            packages.join(" ")
        ))])
    }
}
