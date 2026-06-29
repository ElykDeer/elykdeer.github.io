use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct BubblesCommand;

impl Command for BubblesCommand {
    fn name(&self) -> &'static str {
        "bubbles"
    }

    fn summary(&self) -> &'static str {
        "Launch the bubbles game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: bubbles [hard|clear]\n\nLaunches the in-terminal bubbles game. Aim with mouse, touch, or arrow keys; \
shoot with click, tap, Space, or Enter. `bubbles hard` starts hard mode. `bubbles clear` resets your saved high score."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        match args {
            [] => Ok(vec![OutputBlock::LaunchGame {
                game_id: "bubbles".to_string(),
                hard: false,
            }]),
            [arg] if arg == "hard" => Ok(vec![OutputBlock::LaunchGame {
                game_id: "bubbles".to_string(),
                hard: true,
            }]),
            [arg] if arg == "clear" => {
                crate::game::clear_high_score();
                Ok(vec![OutputBlock::Text("High score cleared.".to_string())])
            }
            _ => usage_error("Usage: bubbles [hard|clear]"),
        }
    }
}
