use crate::commands::{usage_error, Command, CommandError, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct WcCommand;

impl Command for WcCommand {
    fn name(&self) -> &'static str {
        "wc"
    }

    fn summary(&self) -> &'static str {
        "Count lines, words, and bytes in files."
    }

    fn long_help(&self) -> &'static str {
        "Usage: wc <path> [path...]\n\nPrints line, word, and byte counts for one or more files."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.is_empty() {
            return usage_error("Usage: wc <path> [path...]");
        }

        let mut lines = Vec::new();
        let mut total_line_count = 0usize;
        let mut total_word_count = 0usize;
        let mut total_byte_count = 0usize;

        for path in args {
            let file = ctx.vfs().read_file(ctx.cwd(), path).map_err(fs_error)?;
            let line_count = file.content.bytes().filter(|byte| *byte == b'\n').count();
            let word_count = file.content.split_whitespace().count();
            let byte_count = file.content.len();

            total_line_count += line_count;
            total_word_count += word_count;
            total_byte_count += byte_count;
            lines.push(format!("{line_count} {word_count} {byte_count} {path}"));
        }

        if args.len() > 1 {
            lines.push(format!(
                "{total_line_count} {total_word_count} {total_byte_count} total"
            ));
        }

        Ok(vec![OutputBlock::Text(lines.join("\n"))])
    }
}

fn fs_error(err: impl ToString) -> CommandError {
    CommandError::new(err.to_string())
}
