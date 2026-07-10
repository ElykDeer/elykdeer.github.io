use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext};

pub struct TextropolisCommand;

const TEXTROPOLIS_VERSION: &str = "1.x";

impl Command for TextropolisCommand {
    fn name(&self) -> &'static str {
        "textropolis"
    }

    fn summary(&self) -> &'static str {
        "Launch the Textropolis word game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: textropolis [version]\n\nLaunches a city-name word game. Build three-letter-or-longer words from the letters in each city name. `textropolis version` prints the game version."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        match args {
            [] => Ok(vec![OutputBlock::LaunchGame(GameLaunch::Textropolis)]),
            [arg] if arg == "version" => Ok(vec![OutputBlock::Text(format!(
                "Textropolis {TEXTROPOLIS_VERSION}"
            ))]),
            _ => usage_error("Usage: textropolis [version]"),
        }
    }
}
