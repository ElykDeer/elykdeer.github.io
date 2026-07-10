use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext, WordHuntLaunchBoard};

pub struct WordHuntCommand;

const WORDHUNT_VERSION: &str = "1.x";

impl Command for WordHuntCommand {
    fn name(&self) -> &'static str {
        "wordhunt"
    }

    fn summary(&self) -> &'static str {
        "Launch the Word Hunt word-search game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: wordhunt [max|CxR] [reveal|clear|version]\n\nLaunches a full-screen word-search game. Find 4+ letter dictionary words in straight horizontal, vertical, or diagonal lines. `wordhunt` and `wordhunt max` use the largest square board that fits the screen. `wordhunt 16x20` starts a board with 16 columns and 20 rows when it fits. Board sizes must be at least 5x5. `wordhunt reveal` opens the last board with answers revealed. `wordhunt clear` resets the saved Word Hunt puzzle. `wordhunt version` prints the game version."
    }

    fn run(&self, _ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if matches!(args, [arg] if arg == "version") {
            return Ok(vec![OutputBlock::Text(format!(
                "Word Hunt {WORDHUNT_VERSION}"
            ))]);
        }
        if args.iter().any(|arg| arg == "clear") {
            return match args {
                [arg] if arg == "clear" => {
                    crate::games::clear_wordhunt_progress();
                    Ok(vec![OutputBlock::Text(
                        "Word Hunt progress cleared.".to_string(),
                    )])
                }
                _ => usage_error("Usage: wordhunt [max|CxR] [reveal|clear|version]"),
            };
        }

        let mut reveal = false;
        let mut size = None::<WordHuntLaunchBoard>;
        for arg in args {
            if arg == "reveal" {
                if reveal {
                    return usage_error("Usage: wordhunt [max|CxR] [reveal|clear|version]");
                }
                reveal = true;
            } else if arg == "max" {
                if size.is_some() {
                    return usage_error("Usage: wordhunt [max|CxR] [reveal|clear|version]");
                }
                size = Some(WordHuntLaunchBoard::Max);
            } else if let Some(normalized) = parse_board_size(arg) {
                if size.is_some() {
                    return usage_error("Usage: wordhunt [max|CxR] [reveal|clear|version]");
                }
                size = Some(normalized);
            } else {
                return usage_error("Usage: wordhunt [max|CxR] [reveal|clear|version]");
            }
        }

        Ok(vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
            reveal,
            board: size,
        })])
    }
}

fn parse_board_size(raw: &str) -> Option<WordHuntLaunchBoard> {
    let (cols, rows) = raw.split_once('x').or_else(|| raw.split_once('X'))?;
    let cols = cols.parse::<usize>().ok()?;
    let rows = rows.parse::<usize>().ok()?;
    if cols < 5 || rows < 5 {
        return None;
    }
    Some(WordHuntLaunchBoard::Shape { cols, rows })
}
