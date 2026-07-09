use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext};

pub struct BubblesCommand;

impl Command for BubblesCommand {
    fn name(&self) -> &'static str {
        "bubbles"
    }

    fn summary(&self) -> &'static str {
        "Launch the bubbles game."
    }

    fn long_help(&self) -> &'static str {
        if cfg!(debug_assertions) {
            "Usage: bubbles [hard] [cheat] | bubbles clear\n\nLaunches the in-terminal bubbles game. Aim with mouse, touch, or arrow keys; \
shoot with click, tap, Space, or Enter. `bubbles hard` starts hard mode. `bubbles cheat` enables debug-only cheat buttons. `bubbles clear` resets your saved high score and saved games."
        } else {
            "Usage: bubbles [hard|clear]\n\nLaunches the in-terminal bubbles game. Aim with mouse, touch, or arrow keys; \
shoot with click, tap, Space, or Enter. `bubbles hard` starts hard mode. `bubbles clear` resets your saved high score and saved games."
        }
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if matches!(args, [arg] if arg == "clear") {
            crate::game::clear_high_score();
            return Ok(vec![OutputBlock::Text(
                "High score and saved games cleared.".to_string(),
            )]);
        }

        let mut hard = false;
        #[cfg(debug_assertions)]
        let mut cheat = false;
        #[cfg(not(debug_assertions))]
        let cheat = false;
        for arg in args {
            match arg.as_str() {
                "hard" if !hard => hard = true,
                #[cfg(debug_assertions)]
                "cheat" if !cheat => cheat = true,
                _ => return usage_error(bubbles_usage()),
            }
        }

        Ok(vec![OutputBlock::LaunchGame(GameLaunch::Bubbles {
            hard,
            cheat,
        })])
    }
}

fn bubbles_usage() -> &'static str {
    if cfg!(debug_assertions) {
        "Usage: bubbles [hard] [cheat] | bubbles clear"
    } else {
        "Usage: bubbles [hard|clear]"
    }
}
