use crate::commands::{run_script, usage_error, Command, CommandResult};
use crate::terminal::TerminalContext;

pub struct ShCommand;

impl Command for ShCommand {
    fn name(&self) -> &'static str {
        "sh"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["source"]
    }

    fn summary(&self) -> &'static str {
        "Run a shell script file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: sh <file>\n\nRuns each line of a file as a shell command. Blank lines and # comments \
are skipped. Interactive/async commands (python3, pip, package scripts) are not supported in scripts."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let [file] = args else {
            return usage_error("Usage: sh <file>");
        };
        let source = ctx
            .vfs()
            .read_file(ctx.cwd(), file)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?
            .content;
        Ok(run_script(ctx, &source))
    }
}
