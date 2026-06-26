use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["?"]
    }

    fn summary(&self) -> &'static str {
        "List commands or show detailed help for one command."
    }

    fn long_help(&self) -> &'static str {
        "Usage: help [command]\n\nWithout arguments, lists visible commands from command metadata. With a command name, shows that command's detailed help."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.len() > 1 {
            return usage_error("Usage: help [command]");
        }

        if let Some(name) = args.first() {
            let Some(command) = ctx
                .command_metadata()
                .iter()
                .find(|command| command.name == name || command.aliases.contains(&name.as_str()))
            else {
                return Ok(vec![OutputBlock::Error(format!(
                    "No help is registered for '{}'.",
                    name
                ))]);
            };

            return Ok(vec![OutputBlock::Panel {
                title: format!("help {}", command.name),
                body: vec![OutputBlock::Text(command.long_help.to_string())],
            }]);
        }

        let mut lines = Vec::new();
        lines.push("Available commands:".to_string());
        lines.push(String::new());
        lines.extend(ctx.command_metadata().iter().map(|command| {
            let aliases = if command.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", command.aliases.join(", "))
            };

            format!("  {}{} - {}", command.name, aliases, command.summary)
        }));
        lines.push(String::new());
        lines.push("Run `help <command>` for details.".to_string());

        Ok(vec![OutputBlock::Text(lines.join("\n"))])
    }
}
