use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext};

pub struct SnekCommand;

impl Command for SnekCommand {
    fn name(&self) -> &'static str {
        "snek"
    }

    fn summary(&self) -> &'static str {
        "Launch the Snek game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: snek [clear]\n\nLaunches the in-terminal Snek game. Move with arrow keys, WASD, or swipe; pause with Space. `snek clear` resets your saved Snek game."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        match args {
            [] => Ok(vec![OutputBlock::LaunchGame(GameLaunch::Snek)]),
            [arg] if arg == "clear" => {
                crate::game::clear_snek_save();
                Ok(vec![OutputBlock::Text("Snek save cleared.".to_string())])
            }
            _ => usage_error("Usage: snek [clear]"),
        }
    }
}
