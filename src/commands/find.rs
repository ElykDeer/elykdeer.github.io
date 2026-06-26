use crate::commands::{Command, CommandError, CommandResult};
use crate::fs::EntryKind;
use crate::terminal::{OutputBlock, TerminalContext};

pub struct FindCommand;

impl Command for FindCommand {
    fn name(&self) -> &'static str {
        "find"
    }

    fn summary(&self) -> &'static str {
        "Recursively list paths under a directory."
    }

    fn long_help(&self) -> &'static str {
        "Usage: find [path] [-name pattern]\n\nRecursively prints paths from the virtual filesystem."
    }

    fn run(&self, ctx: &mut TerminalContext, args: &[String]) -> CommandResult {
        let (start, name_pattern) = parse_args(args)?;
        let start = ctx
            .vfs()
            .resolve_path(ctx.cwd(), &start)
            .map_err(fs_error)?;

        if !ctx.vfs().has_path(&start) {
            return Err(CommandError::new(format!("not found: {start}")));
        }

        let mut paths = Vec::new();
        collect_paths(ctx, &start, &mut paths)?;

        let filtered = paths
            .into_iter()
            .filter(|path| {
                name_pattern
                    .as_ref()
                    .is_none_or(|pattern| matches_name(path, pattern))
            })
            .collect::<Vec<_>>();

        if filtered.is_empty() {
            Ok(Vec::new())
        } else {
            Ok(vec![OutputBlock::Text(filtered.join("\n"))])
        }
    }
}

fn parse_args(args: &[String]) -> Result<(String, Option<String>), CommandError> {
    let mut start = None;
    let mut name_pattern = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "-name" => {
                let Some(pattern) = args.get(index + 1) else {
                    return Err(CommandError::new("Usage: find [path] [-name pattern]"));
                };
                if name_pattern.replace(pattern.clone()).is_some() {
                    return Err(CommandError::new("Usage: find [path] [-name pattern]"));
                }
                index += 2;
            }
            value if value.starts_with('-') => {
                return Err(CommandError::new("Usage: find [path] [-name pattern]"));
            }
            value => {
                if start.replace(value.to_string()).is_some() {
                    return Err(CommandError::new("Usage: find [path] [-name pattern]"));
                }
                index += 1;
            }
        }
    }

    Ok((start.unwrap_or_else(|| ".".to_string()), name_pattern))
}

fn collect_paths(
    ctx: &TerminalContext,
    path: &str,
    output: &mut Vec<String>,
) -> Result<(), CommandError> {
    output.push(path.to_string());
    if !ctx.vfs().has_dir(path) {
        return Ok(());
    }

    for entry in ctx
        .vfs()
        .list_dir(ctx.cwd(), Some(path))
        .map_err(fs_error)?
    {
        output.push(entry.path.clone());
        if entry.kind == EntryKind::Directory {
            collect_children(ctx, &entry.path, output)?;
        }
    }

    Ok(())
}

fn collect_children(
    ctx: &TerminalContext,
    dir: &str,
    output: &mut Vec<String>,
) -> Result<(), CommandError> {
    for entry in ctx.vfs().list_dir(ctx.cwd(), Some(dir)).map_err(fs_error)? {
        output.push(entry.path.clone());
        if entry.kind == EntryKind::Directory {
            collect_children(ctx, &entry.path, output)?;
        }
    }

    Ok(())
}

fn matches_name(path: &str, pattern: &str) -> bool {
    let name = if path == "/" {
        "/"
    } else {
        path.rsplit('/').next().unwrap_or(path)
    };
    matches_glob(name.as_bytes(), pattern.as_bytes())
}

fn matches_glob(text: &[u8], pattern: &[u8]) -> bool {
    let mut text_index = 0usize;
    let mut pattern_index = 0usize;
    let mut star_index = None;
    let mut match_index = 0usize;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == text[text_index])
        {
            text_index += 1;
            pattern_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            match_index = text_index;
            pattern_index += 1;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            match_index += 1;
            text_index = match_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

fn fs_error(err: impl ToString) -> CommandError {
    CommandError::new(err.to_string())
}
