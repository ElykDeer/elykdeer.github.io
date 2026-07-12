use crate::commands::{usage_error, Command, CommandResult};
use crate::terminal::{GameLaunch, OutputBlock, TerminalContext, WordHuntLaunchBoard};

pub struct WordHuntCommand;

const WORDHUNT_VERSION: &str = "1.2.0";

impl Command for WordHuntCommand {
    fn name(&self) -> &'static str {
        "wordhunt"
    }

    fn summary(&self) -> &'static str {
        "Launch the Word Hunt word-search game."
    }

    fn long_help(&self) -> &'static str {
        "Usage: wordhunt [hard|max|CxR] [reveal|clear|version]\n\nLaunches a full-screen word-search game. Find 5+ letter dictionary words in straight horizontal, vertical, or diagonal lines; valid standalone 4-letter selections count as bonus words. `wordhunt` uses a lower-density board. `wordhunt hard` preserves the fully loaded board and uses the largest square that fits the screen. `wordhunt max` is a legacy alias for that hard board. `wordhunt 16x20` starts a lower-density board with 16 columns and 20 rows when it fits. Board sizes must be at least 5x5. `wordhunt reveal` opens the last board with answers revealed. `wordhunt clear` resets the saved Word Hunt puzzle. `wordhunt version` prints the game version."
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
                _ => usage_error("Usage: wordhunt [hard|max|CxR] [reveal|clear|version]"),
            };
        }

        let mut reveal = false;
        let mut size = None::<WordHuntLaunchBoard>;
        for arg in args {
            if arg == "reveal" {
                if reveal {
                    return usage_error("Usage: wordhunt [hard|max|CxR] [reveal|clear|version]");
                }
                reveal = true;
            } else if arg == "hard" || arg == "max" {
                if size.is_some() {
                    return usage_error("Usage: wordhunt [hard|max|CxR] [reveal|clear|version]");
                }
                size = Some(WordHuntLaunchBoard::Max);
            } else if let Some(normalized) = parse_board_size(arg) {
                if size.is_some() {
                    return usage_error("Usage: wordhunt [hard|max|CxR] [reveal|clear|version]");
                }
                size = Some(normalized);
            } else {
                return usage_error("Usage: wordhunt [hard|max|CxR] [reveal|clear|version]");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_launches_the_existing_fully_loaded_max_board() {
        let mut context = TerminalContext::new();

        assert_eq!(
            WordHuntCommand.run(&mut context, &["hard".to_string()]),
            Ok(vec![OutputBlock::LaunchGame(GameLaunch::WordHunt {
                reveal: false,
                board: Some(WordHuntLaunchBoard::Max),
            })])
        );
    }

    #[test]
    fn hard_rejects_a_second_board_choice() {
        let mut context = TerminalContext::new();
        let result = WordHuntCommand.run(&mut context, &["hard".to_string(), "12x12".to_string()]);

        assert!(result.is_err());
    }
}
