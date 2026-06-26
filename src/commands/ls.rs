use crate::commands::{usage_error, Command, CommandResult};
use crate::fs::EntryKind;
use crate::terminal::{OutputBlock, TerminalContext};

pub struct LsCommand;

impl Command for LsCommand {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn summary(&self) -> &'static str {
        "List directory contents."
    }

    fn long_help(&self) -> &'static str {
        "Usage: ls [-a] [-l] [path]\n\nLists files and directories. Hides dotfiles unless -a; \
-l shows a long listing with sizes."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let mut all = false;
        let mut long = false;
        let mut path = None;

        for arg in args {
            if let Some(flags) = arg.strip_prefix('-').filter(|flags| !flags.is_empty()) {
                for flag in flags.chars() {
                    match flag {
                        'a' => all = true,
                        'l' => long = true,
                        _ => return usage_error("Usage: ls [-a] [-l] [path]"),
                    }
                }
            } else if path.replace(arg.as_str()).is_some() {
                return usage_error("Usage: ls [-a] [-l] [path]");
            }
        }

        list_dir(ctx, path, all, long)
    }
}

/// `ll` is the long, all-inclusive listing (`ls -la`).
pub struct LlCommand;

impl Command for LlCommand {
    fn name(&self) -> &'static str {
        "ll"
    }

    fn summary(&self) -> &'static str {
        "Long listing including dotfiles (ls -la)."
    }

    fn long_help(&self) -> &'static str {
        "Usage: ll [path]\n\nLong listing of a directory, including dotfiles."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        if args.len() > 1 {
            return usage_error("Usage: ll [path]");
        }
        list_dir(ctx, args.first().map(String::as_str), true, true)
    }
}

fn list_dir(ctx: &TerminalContext, path: Option<&str>, all: bool, long: bool) -> CommandResult {
    let entries = ctx
        .vfs()
        .list_dir(ctx.cwd(), path)
        .map_err(|err| crate::commands::CommandError::new(err.to_string()))?;

    let lines = entries
        .into_iter()
        .filter(|entry| all || !entry.name.starts_with('.'))
        .map(|entry| {
            if long {
                match entry.kind {
                    EntryKind::Directory => format!("-  {}/", entry.name),
                    EntryKind::File => {
                        let size = file_size(ctx, &entry.path);
                        format!("{size}  {}", entry.name)
                    }
                }
            } else {
                match entry.kind {
                    EntryKind::Directory => format!("{}/", entry.name),
                    EntryKind::File => entry.name,
                }
            }
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![OutputBlock::Text(lines.join("\n"))])
}

fn file_size(ctx: &TerminalContext, path: &str) -> usize {
    ctx.vfs()
        .read_file(ctx.cwd(), path)
        .map(|file| file.content.len())
        .unwrap_or(0)
}
