use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext};

pub struct SnekCommand;

const SNEK_VERSION: &str = "0.x";

impl Command for SnekCommand {
    fn name(&self) -> &'static str {
        "snek"
    }

    fn summary(&self) -> &'static str {
        "Launch the Snek game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: snek [cheat|clear|version]\n\nLaunches the in-terminal Snek game. Move with arrow keys, WASD, or swipe; pause with Space. `snek cheat` enables debug-only cheat buttons. `snek clear` resets your saved Snek game. `snek version` prints the game version."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        match args {
            [] => Ok(vec![OutputBlock::LaunchGame(GameLaunch::Snek {
                cheat: false,
            })]),
            [arg] if arg == "cheat" => Ok(vec![OutputBlock::LaunchGame(GameLaunch::Snek {
                cheat: true,
            })]),
            [arg] if arg == "clear" => {
                crate::games::clear_snek_save();
                Ok(vec![OutputBlock::Text("Snek save cleared.".to_string())])
            }
            [arg] if arg == "version" => {
                Ok(vec![OutputBlock::Text(format!("Snek {SNEK_VERSION}"))])
            }
            _ => usage_error("Usage: snek [cheat|clear|version]"),
        }
    }
}
