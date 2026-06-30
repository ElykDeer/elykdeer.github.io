use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct TextropolisCommand;

impl Command for TextropolisCommand {
    fn name(&self) -> &'static str {
        "textropolis"
    }

    fn summary(&self) -> &'static str {
        "Launch the Textropolis word game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: textropolis\n\nLaunches a city-name word game. Build three-letter-or-longer words from the letters in each city name."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        match args {
            [] => Ok(vec![OutputBlock::LaunchGame {
                game_id: "textropolis".to_string(),
                hard: false,
            }]),
            _ => usage_error("Usage: textropolis"),
        }
    }
}
