use crate::commands::{Command, CommandResult};
use crate::storage::export_profile_json;
use crate::terminal::{OutputBlock, TerminalContext};

pub struct ExportCommand;

impl Command for ExportCommand {
    fn name(&self) -> &'static str {
        "export"
    }

    fn summary(&self) -> &'static str {
        "Print your saved state as JSON."
    }

    fn long_help(&self) -> &'static str {
        "Usage: export\n\nPrints your saved state (files and history) as JSON, plus a ready-to-paste import command to restore it elsewhere."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if !args.is_empty() {
            return Err(crate::commands::CommandError::new("Usage: export"));
        }

        let json = export_profile_json(ctx.profile())
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;
        let import_command = format!("import {}", quote_for_terminal(&json));

        Ok(vec![
            OutputBlock::FileView {
                path: "elyk.profile.v1.json".to_string(),
                content: json,
            },
            OutputBlock::FileView {
                path: "elyk.profile.v1.import-command.txt".to_string(),
                content: import_command,
            },
        ])
    }
}

fn quote_for_terminal(value: &str) -> String {
    let mut quoted = String::from("'");
    for ch in value.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '\'' => quoted.push_str("\\'"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::{parse_command_line, TerminalContext};

    #[test]
    fn export_includes_import_command_that_parses_back_to_json() {
        let ctx = TerminalContext::new();
        let json = export_profile_json(ctx.profile()).unwrap();
        let line = format!("import {}", quote_for_terminal(&json));
        let parsed = parse_command_line(&line).unwrap();

        assert_eq!(parsed.name, "import");
        assert_eq!(parsed.args, vec![json]);
    }
}
