use crate::commands::{Command, CommandResult};
use crate::terminal::{OutputBlock, TerminalContext};

pub struct PythonCommand;

impl Command for PythonCommand {
    fn name(&self) -> &'static str {
        "python3"
    }

    fn summary(&self) -> &'static str {
        "Start an interactive Python console."
    }

    fn long_help(&self) -> &'static str {
        "Usage: python3 [file] [args...]\n\nWith no arguments, starts an interactive Python console \
(Pyodide). With a file, runs it like `python3 file.py`. Use exit() or Ctrl-D to leave the console."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let [file, rest @ ..] = args else {
            ctx.request_python();
            return Ok(vec![OutputBlock::Text("Starting Python…".to_string())]);
        };

        // Resolve and verify the file up front so a typo errors immediately.
        let path = ctx
            .vfs()
            .read_file(ctx.cwd(), file)
            .map_err(|err| crate::commands::CommandError::new(err.to_string()))?
            .path;
        ctx.request_python_file(path, rest.to_vec());
        Ok(Vec::new())
    }
}
