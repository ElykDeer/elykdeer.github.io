#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedPipeline {
    pub stages: Vec<ParsedCommand>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedLine {
    pub pipeline: ParsedPipeline,
    pub stdout_redirect: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    TrailingEscape,
    UnterminatedQuote(char),
    EmptyPipelineStage,
    EmptyRedirectTarget,
    UnsupportedRedirect,
}

pub fn parse_command_line(input: &str) -> Result<ParsedCommand, ParseError> {
    let tokens = tokenize(input)?;

    let Some((name, args)) = tokens.split_first() else {
        return Err(ParseError::Empty);
    };

    Ok(ParsedCommand {
        name: name.clone(),
        args: args.to_vec(),
    })
}

pub fn parse_pipeline(input: &str) -> Result<ParsedPipeline, ParseError> {
    let stages = split_pipeline(input)?
        .into_iter()
        .map(parse_command_line)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedPipeline { stages })
}

pub fn parse_line(input: &str) -> Result<ParsedLine, ParseError> {
    let (command, stdout_redirect) = split_redirection(input)?;
    let pipeline = parse_pipeline(command)?;

    let stdout_redirect = match stdout_redirect {
        Some(target) => {
            if target.trim_start().starts_with('>') {
                return Err(ParseError::UnsupportedRedirect);
            }

            let tokens = tokenize(target)?;
            match tokens.as_slice() {
                [] => return Err(ParseError::EmptyRedirectTarget),
                [path] => Some(path.clone()),
                _ => return Err(ParseError::UnsupportedRedirect),
            }
        }
        None => None,
    };

    Ok(ParsedLine {
        pipeline,
        stdout_redirect,
    })
}

fn split_pipeline(input: &str) -> Result<Vec<&str>, ParseError> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut escaping = false;

    for (index, ch) in input.char_indices() {
        if escaping {
            escaping = false;
            continue;
        }

        if ch == '\\' {
            escaping = true;
            continue;
        }

        match quote {
            Some(active_quote) if ch == active_quote => quote = None,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '|' => {
                let part = input[start..index].trim();
                if part.is_empty() {
                    return Err(ParseError::EmptyPipelineStage);
                }
                parts.push(part);
                start = index + ch.len_utf8();
            }
            None => {}
        }
    }

    if escaping {
        return Err(ParseError::TrailingEscape);
    }

    if let Some(active_quote) = quote {
        return Err(ParseError::UnterminatedQuote(active_quote));
    }

    let part = input[start..].trim();
    if part.is_empty() {
        if parts.is_empty() {
            return Err(ParseError::Empty);
        }
        return Err(ParseError::EmptyPipelineStage);
    }
    parts.push(part);
    Ok(parts)
}

fn split_redirection(input: &str) -> Result<(&str, Option<&str>), ParseError> {
    let mut quote: Option<char> = None;
    let mut escaping = false;
    let mut redirect_start = None;

    for (index, ch) in input.char_indices() {
        if escaping {
            escaping = false;
            continue;
        }

        if ch == '\\' {
            escaping = true;
            continue;
        }

        match quote {
            Some(active_quote) if ch == active_quote => quote = None,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '>' => {
                redirect_start = Some(index);
                break;
            }
            None => {}
        }
    }

    if escaping {
        return Err(ParseError::TrailingEscape);
    }

    if let Some(active_quote) = quote {
        return Err(ParseError::UnterminatedQuote(active_quote));
    }

    let Some(index) = redirect_start else {
        return Ok((input, None));
    };

    let command = input[..index].trim_end();
    if command.is_empty() {
        return Err(ParseError::Empty);
    }

    let target = input[index + 1..].trim();
    if target.is_empty() {
        return Err(ParseError::EmptyRedirectTarget);
    }

    Ok((command, Some(target)))
}

pub fn tokenize(input: &str) -> Result<Vec<String>, ParseError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaping = false;
    let mut token_started = false;

    for ch in input.chars() {
        if escaping {
            current.push(ch);
            token_started = true;
            escaping = false;
            continue;
        }

        if ch == '\\' {
            escaping = true;
            token_started = true;
            continue;
        }

        match quote {
            Some(active_quote) if ch == active_quote => {
                quote = None;
                token_started = true;
            }
            Some(_) => {
                current.push(ch);
                token_started = true;
            }
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                token_started = true;
            }
            None if ch.is_whitespace() => {
                if token_started {
                    tokens.push(current);
                    current = String::new();
                    token_started = false;
                }
            }
            None => {
                current.push(ch);
                token_started = true;
            }
        }
    }

    if escaping {
        return Err(ParseError::TrailingEscape);
    }

    if let Some(active_quote) = quote {
        return Err(ParseError::UnterminatedQuote(active_quote));
    }

    if token_started {
        tokens.push(current);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_command() {
        let parsed = parse_command_line("cat /manual.md").unwrap();

        assert_eq!(parsed.name, "cat");
        assert_eq!(parsed.args, vec!["/manual.md"]);
    }

    #[test]
    fn preserves_quoted_spaces() {
        let parsed = parse_command_line("mkdir 'project plan' \"final drafts\"").unwrap();

        assert_eq!(parsed.name, "mkdir");
        assert_eq!(parsed.args, vec!["project plan", "final drafts"]);
    }

    #[test]
    fn supports_escaping_inside_quotes() {
        let parsed = parse_command_line("cat \"docs/quote\\\"file.md\"").unwrap();

        assert_eq!(parsed.args, vec!["docs/quote\"file.md"]);
    }

    #[test]
    fn reports_unterminated_quote() {
        let err = parse_command_line("cat 'about").unwrap_err();

        assert_eq!(err, ParseError::UnterminatedQuote('\''));
    }

    #[test]
    fn parses_pipeline_outside_quotes() {
        let parsed = parse_pipeline("echo 'hi | there' | lolcat --seed 7").unwrap();

        assert_eq!(parsed.stages.len(), 2);
        assert_eq!(parsed.stages[0].name, "echo");
        assert_eq!(parsed.stages[0].args, vec!["hi | there"]);
        assert_eq!(parsed.stages[1].name, "lolcat");
        assert_eq!(parsed.stages[1].args, vec!["--seed", "7"]);
    }

    #[test]
    fn parses_output_redirection_outside_quotes() {
        let parsed = parse_line(r#"echo "hello > there" > "test file.txt""#).unwrap();

        assert_eq!(parsed.pipeline.stages.len(), 1);
        assert_eq!(parsed.pipeline.stages[0].name, "echo");
        assert_eq!(parsed.pipeline.stages[0].args, vec!["hello > there"]);
        assert_eq!(parsed.stdout_redirect, Some("test file.txt".to_string()));
    }

    #[test]
    fn rejects_missing_redirect_target() {
        assert_eq!(
            parse_line("echo hello >").unwrap_err(),
            ParseError::EmptyRedirectTarget
        );
    }

    #[test]
    fn rejects_unsupported_redirect_shapes() {
        assert_eq!(
            parse_line("echo hello > out.txt extra").unwrap_err(),
            ParseError::UnsupportedRedirect
        );
        assert_eq!(
            parse_line("echo hello >> out.txt").unwrap_err(),
            ParseError::UnsupportedRedirect
        );
    }

    #[test]
    fn rejects_empty_pipeline_stages() {
        assert_eq!(
            parse_pipeline("echo hi | ").unwrap_err(),
            ParseError::EmptyPipelineStage
        );
        assert_eq!(
            parse_pipeline("| lolcat").unwrap_err(),
            ParseError::EmptyPipelineStage
        );
    }
}
