use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext};

pub struct BubblesCommand;

const BUBBLES_VERSION: &str = "2.x";

impl Command for BubblesCommand {
    fn name(&self) -> &'static str {
        "bubbles"
    }

    fn summary(&self) -> &'static str {
        "Launch the bubbles game."
    }

    fn long_help(&self) -> &'static str {
        if cfg!(debug_assertions) {
            "Usage: bubbles [hard] [cheat] | bubbles clear | bubbles version\n\nLaunches the in-terminal bubbles game. Aim with mouse, touch, or arrow keys; \
shoot with click, tap, Space, or Enter. `bubbles hard` starts hard mode. `bubbles cheat` enables debug-only cheat buttons. `bubbles clear` resets your saved high score and saved games. `bubbles version` prints the game version."
        } else {
            "Usage: bubbles [hard] | bubbles clear | bubbles version\n\nLaunches the in-terminal bubbles game. Aim with mouse, touch, or arrow keys; \
shoot with click, tap, Space, or Enter. `bubbles hard` starts hard mode. `bubbles clear` resets your saved high score and saved games. `bubbles version` prints the game version."
        }
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if matches!(args, [arg] if arg == "version") {
            return Ok(vec![OutputBlock::Text(format!(
                "Bubbles {BUBBLES_VERSION}"
            ))]);
        }
        if matches!(args, [arg] if arg == "clear") {
            crate::games::clear_high_score();
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
        "Usage: bubbles [hard] [cheat] | bubbles clear | bubbles version"
    } else {
        "Usage: bubbles [hard] | bubbles clear | bubbles version"
    }
}
