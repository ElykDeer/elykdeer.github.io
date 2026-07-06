use crate::fs::EntryKind;

use super::output::OutputBlock;
use super::state::TerminalContext;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TabCompletionAction {
    pub replacement: Option<String>,
    pub output: Option<OutputBlock>,
}

pub(crate) fn complete_terminal_tab(
    ctx: &TerminalContext,
    input: &str,
) -> Option<TabCompletionAction> {
    let token = current_completion_token(input);
    let completing_command = input[..token.start].trim().is_empty();

    if completing_command {
        return complete_command_name(ctx, input, &token).map(|replacement| TabCompletionAction {
            replacement: Some(replacement),
            output: None,
        });
    }

    complete_path(ctx, input, &token)
}

#[cfg(test)]
fn complete_terminal_input(ctx: &TerminalContext, input: &str) -> Option<String> {
    complete_terminal_tab(ctx, input).and_then(|action| action.replacement)
}

fn complete_command_name(
    ctx: &TerminalContext,
    input: &str,
    token: &CompletionToken,
) -> Option<String> {
    if token.decoded.is_empty() {
        return None;
    }

    let mut candidates = ctx
        .command_metadata()
        .iter()
        .flat_map(|command| std::iter::once(command.name).chain(command.aliases.iter().copied()))
        .map(str::to_string)
        .chain(ctx.console_script_names())
        .filter(|name| name.starts_with(&token.decoded))
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates.dedup();

    complete_from_candidates(input, token, &candidates, true)
}

fn complete_path(
    ctx: &TerminalContext,
    input: &str,
    token: &CompletionToken,
) -> Option<TabCompletionAction> {
    let (dir_input, entry_prefix, replacement_prefix) = split_completion_path(&token.decoded);
    let entries = ctx.vfs().list_dir(ctx.cwd(), Some(dir_input)).ok()?;
    let mut candidates = entries
        .iter()
        .filter(|entry| entry.name.starts_with(entry_prefix))
        .map(|entry| {
            let token = if entry.kind == EntryKind::Directory {
                format!("{replacement_prefix}{}/", entry.name)
            } else {
                format!("{replacement_prefix}{}", entry.name)
            };
            let display = if entry.kind == EntryKind::Directory {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };
            (token, entry.kind == EntryKind::File, display)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.dedup_by(|left, right| left.0 == right.0);

    match candidates.as_slice() {
        [] if entry_prefix.is_empty() && token.decoded.ends_with('/') => {
            Some(TabCompletionAction {
                replacement: None,
                output: Some(OutputBlock::Text(format!("{}:\n(empty)", token.decoded))),
            })
        }
        [] => None,
        [(candidate, append_space, _)] => {
            let replacement = render_completion_token(candidate, token.style, *append_space);
            replace_input_token(input, token.start, &replacement).map(|replacement| {
                TabCompletionAction {
                    replacement: Some(replacement),
                    output: None,
                }
            })
        }
        _ => {
            let common = common_prefix(
                &candidates
                    .iter()
                    .map(|(candidate, _, _)| candidate.as_str())
                    .collect::<Vec<_>>(),
            )?;
            if common.len() > token.decoded.len() {
                let replacement = render_completion_token(&common, token.style, false);
                replace_input_token(input, token.start, &replacement).map(|replacement| {
                    TabCompletionAction {
                        replacement: Some(replacement),
                        output: None,
                    }
                })
            } else {
                let options = candidates
                    .iter()
                    .map(|(_, _, display)| display.as_str())
                    .collect::<Vec<_>>();
                completion_listing_for_token(&token.decoded, &options).map(|output| {
                    TabCompletionAction {
                        replacement: None,
                        output: Some(output),
                    }
                })
            }
        }
    }
}

fn completion_listing_for_token(token: &str, options: &[impl AsRef<str>]) -> Option<OutputBlock> {
    if options.is_empty() {
        return None;
    }

    let label = if token.is_empty() { "." } else { token };
    let rendered = options
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join("  ");

    Some(OutputBlock::Text(format!("{label}:\n{rendered}")))
}

fn split_completion_path(token: &str) -> (&str, &str, &str) {
    match token.rfind('/') {
        Some(0) => ("/", &token[1..], "/"),
        Some(index) => (&token[..index], &token[index + 1..], &token[..=index]),
        None => (".", token, ""),
    }
}

fn escape_completion_token(token: &str) -> String {
    let mut escaped = String::with_capacity(token.len());

    for ch in token.chars() {
        if ch.is_whitespace() || matches!(ch, '\\' | '\'' | '"') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}

fn escape_completion_token_for_quote(token: &str, quote: char) -> String {
    let mut escaped = String::with_capacity(token.len());

    for ch in token.chars() {
        if ch == '\\' || ch == quote {
            escaped.push('\\');
        }
        escaped.push(ch);
    }

    escaped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionStyle {
    Unquoted,
    Quoted(char),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CompletionToken {
    start: usize,
    decoded: String,
    style: CompletionStyle,
}

fn current_completion_token(input: &str) -> CompletionToken {
    let mut current = CompletionToken {
        start: input.len(),
        decoded: String::new(),
        style: CompletionStyle::Unquoted,
    };
    let mut active = false;
    let mut start = input.len();
    let mut decoded = String::new();
    let mut style = CompletionStyle::Unquoted;
    let mut quote: Option<char> = None;
    let mut escaping = false;

    for (index, ch) in input.char_indices() {
        if escaping {
            decoded.push(ch);
            escaping = false;
            continue;
        }

        if !active {
            if ch.is_whitespace() {
                continue;
            }
            active = true;
            start = index;
            decoded.clear();
            style = CompletionStyle::Unquoted;
        }

        if ch == '\\' {
            escaping = true;
            continue;
        }

        match quote {
            Some(active_quote) if ch == active_quote => {
                quote = None;
            }
            Some(_) => decoded.push(ch),
            None if ch == '\'' || ch == '"' => {
                if decoded.is_empty() && matches!(style, CompletionStyle::Unquoted) {
                    style = CompletionStyle::Quoted(ch);
                }
                quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                current = CompletionToken {
                    start,
                    decoded: decoded.clone(),
                    style,
                };
                active = false;
                start = input.len();
                decoded.clear();
                style = CompletionStyle::Unquoted;
            }
            None => decoded.push(ch),
        }
    }

    if active {
        CompletionToken {
            start,
            decoded,
            style,
        }
    } else {
        current
    }
}

fn render_completion_token(token: &str, style: CompletionStyle, append_space: bool) -> String {
    let rendered = match style {
        CompletionStyle::Unquoted => escape_completion_token(token),
        CompletionStyle::Quoted(quote) => format!(
            "{quote}{}{quote}",
            escape_completion_token_for_quote(token, quote)
        ),
    };

    if append_space {
        format!("{rendered} ")
    } else {
        rendered
    }
}

fn complete_from_candidates(
    input: &str,
    token: &CompletionToken,
    candidates: &[impl AsRef<str>],
    append_space_for_single: bool,
) -> Option<String> {
    match candidates {
        [] => None,
        [candidate] => {
            let candidate = candidate.as_ref();
            let replacement =
                render_completion_token(candidate, token.style, append_space_for_single);
            replace_input_token(input, token.start, &replacement)
        }
        _ => {
            let common = common_prefix(candidates)?;
            if common.len() > token.decoded.len() {
                let replacement = render_completion_token(&common, token.style, false);
                replace_input_token(input, token.start, &replacement)
            } else {
                None
            }
        }
    }
}

fn replace_input_token(input: &str, token_start: usize, replacement: &str) -> Option<String> {
    let completed = format!("{}{}", &input[..token_start], replacement);
    if completed == input {
        None
    } else {
        Some(completed)
    }
}

pub(crate) fn common_prefix(candidates: &[impl AsRef<str>]) -> Option<String> {
    let first = candidates.first()?.as_ref();
    let mut prefix_len = first.len();

    for candidate in candidates.iter().skip(1).map(AsRef::as_ref) {
        prefix_len = first
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(first.len()))
            .take_while(|index| {
                candidate
                    .get(..*index)
                    .is_some_and(|candidate_prefix| candidate_prefix == &first[..*index])
            })
            .last()
            .unwrap_or(0)
            .min(prefix_len);
    }

    Some(first[..prefix_len].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;

    #[test]
    fn escapes_completion_tokens_with_parser_sensitive_characters() {
        assert_eq!(
            escape_completion_token("project plan/quote\"file'.md"),
            "project\\ plan/quote\\\"file\\'.md"
        );
    }

    #[test]
    fn completion_escapes_single_path_match_for_parser() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.set_cwd("/scratch");
        ctx.vfs_mut()
            .write_file("/scratch", "project plan.md", "hello")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let completed = complete_terminal_input(&ctx, "cat pro").unwrap();

        assert_eq!(completed, "cat project\\ plan.md ");
        assert_eq!(
            crate::terminal::parse_command_line(&completed)
                .unwrap()
                .args,
            vec!["project plan.md"]
        );
    }

    #[test]
    fn completion_escapes_common_prefix_for_multiple_matches() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.set_cwd("/scratch");
        ctx.vfs_mut()
            .write_file("/scratch", "project plan.md", "hello")
            .unwrap();
        ctx.vfs_mut()
            .write_file("/scratch", "project log.md", "hello")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let completed = complete_terminal_input(&ctx, "cat pro").unwrap();

        assert_eq!(completed, "cat project\\ ");
        assert_eq!(
            crate::terminal::parse_command_line(&completed)
                .unwrap()
                .args,
            vec!["project "]
        );
    }

    #[test]
    fn completion_handles_quoted_path_tokens() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.set_cwd("/scratch");
        ctx.vfs_mut()
            .write_file("/scratch", "project plan.md", "hello")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let completed = complete_terminal_input(&ctx, "cat \"pro").unwrap();

        assert_eq!(completed, "cat \"project plan.md\" ");
        assert_eq!(
            crate::terminal::parse_command_line(&completed)
                .unwrap()
                .args,
            vec!["project plan.md"]
        );
    }

    #[test]
    fn completion_handles_escaped_spaces_in_path_tokens() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.set_cwd("/scratch");
        ctx.vfs_mut()
            .write_file("/scratch", "project plan.md", "hello")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let completed = complete_terminal_input(&ctx, "cat project\\ p").unwrap();

        assert_eq!(completed, "cat project\\ plan.md ");
        assert_eq!(
            crate::terminal::parse_command_line(&completed)
                .unwrap()
                .args,
            vec!["project plan.md"]
        );
    }

    #[test]
    fn command_completion_includes_installed_console_scripts() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().apply_external_mkdir("/python/site-packages");
        ctx.vfs_mut()
            .apply_external_mkdir("/python/site-packages/lolcat-1.0.dist-info");
        ctx.vfs_mut().apply_external_write(
            "/python/site-packages/lolcat-1.0.dist-info/entry_points.txt",
            "[console_scripts]\nlolcat = lolcat:main\n".to_string(),
        );
        ctx.sync_profile_from_runtime();

        let completed = complete_terminal_input(&ctx, "lol").unwrap();

        assert_eq!(completed, "lolcat ");
    }

    #[test]
    fn tab_on_directory_lists_entries_when_it_cannot_advance() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.vfs_mut().mkdir("/", "/scratch/docs").unwrap();
        ctx.set_cwd("/scratch");
        ctx.vfs_mut()
            .write_file("/scratch/docs", "alpha.md", "hello")
            .unwrap();
        ctx.vfs_mut()
            .write_file("/scratch/docs", "beta.md", "hello")
            .unwrap();
        ctx.sync_profile_from_runtime();

        let action = complete_terminal_tab(&ctx, "cat docs/").unwrap();

        assert_eq!(action.replacement, None);
        assert_eq!(
            action.output,
            Some(OutputBlock::Text("docs/:\nalpha.md  beta.md".to_string()))
        );
    }

    #[test]
    fn tab_on_empty_directory_lists_empty_directory() {
        let mut ctx = TerminalContext::new();
        ctx.set_command_metadata(commands::command_metadata());
        ctx.vfs_mut().mkdir("/", "/scratch").unwrap();
        ctx.vfs_mut().mkdir("/", "/scratch/docs").unwrap();
        ctx.set_cwd("/scratch");
        ctx.sync_profile_from_runtime();

        let action = complete_terminal_tab(&ctx, "cat docs/").unwrap();

        assert_eq!(action.replacement, None);
        assert_eq!(
            action.output,
            Some(OutputBlock::Text("docs/:\n(empty)".to_string()))
        );
    }
}
