use crate::commands::{Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct EchoCommand;

impl Command for EchoCommand {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn summary(&self) -> &'static str {
        "Print text."
    }

    fn long_help(&self) -> &'static str {
        "Usage: echo [text...]\n\nPrints text. Useful on its own or as input to a pipe."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        Ok(vec![OutputBlock::Text(args.join(" "))])
    }
}
