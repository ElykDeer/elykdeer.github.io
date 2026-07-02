use crate::commands::{usage_error, Command, CommandResult};

pub struct CurlCommand;

impl Command for CurlCommand {
    fn name(&self) -> &'static str {
        "curl"
    }

    fn summary(&self) -> &'static str {
        "Fetch a URL and print the response text."
    }

    fn long_help(&self) -> &'static str {
        "Usage: curl <url>\n\nFetches a URL with the browser's fetch API and prints the response text. Browser CORS rules apply."
    }

    fn run(&self, ctx: &mut crate::terminal::TerminalContext, args: &[String]) -> CommandResult {
        let Some(url) = args.first() else {
            return usage_error("Usage: curl <url>");
        };
        if args.len() != 1 {
            return usage_error("Usage: curl <url>");
        }

        ctx.request_fetch(url.to_string(), None);
        Ok(Vec::new())
    }
}

pub struct WgetCommand;

impl Command for WgetCommand {
    fn name(&self) -> &'static str {
        "wget"
    }

    fn summary(&self) -> &'static str {
        "Fetch a URL and save it to a file."
    }

    fn long_help(&self) -> &'static str {
        "Usage: wget [-O path] <url>\n\nFetches a URL with the browser's fetch API and saves the response text. Browser CORS rules apply."
    }

    fn run(&self, ctx: &mut crate::terminal::TerminalContext, args: &[String]) -> CommandResult {
        let (output_path, url) = match args {
            [url] => (default_output_path(url), url.as_str()),
            [flag, path, url] if flag == "-O" => (path.to_string(), url.as_str()),
            _ => return usage_error("Usage: wget [-O path] <url>"),
        };

        ctx.request_fetch(url.to_string(), Some(output_path));
        Ok(Vec::new())
    }
}

fn default_output_path(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let tail = trimmed
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or("index.html");
    tail.split(['?', '#']).next().unwrap_or(tail).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::TerminalContext;

    #[test]
    fn curl_requests_fetch_without_output_path() {
        let mut ctx = TerminalContext::new();

        let output = CurlCommand
            .run(&mut ctx, &["https://example.com".to_string()])
            .unwrap();

        assert!(output.is_empty());
        let request = ctx.take_fetch_request().unwrap();
        assert_eq!(request.url, "https://example.com");
        assert_eq!(request.output_path, None);
    }

    #[test]
    fn wget_defaults_output_path_from_url() {
        let mut ctx = TerminalContext::new();

        WgetCommand
            .run(
                &mut ctx,
                &["https://example.com/readme.txt?x=1".to_string()],
            )
            .unwrap();

        let request = ctx.take_fetch_request().unwrap();
        assert_eq!(request.url, "https://example.com/readme.txt?x=1");
        assert_eq!(request.output_path.as_deref(), Some("readme.txt"));
    }

    #[test]
    fn wget_supports_explicit_output_path() {
        let mut ctx = TerminalContext::new();

        WgetCommand
            .run(
                &mut ctx,
                &[
                    "-O".to_string(),
                    "saved.txt".to_string(),
                    "https://example.com".to_string(),
                ],
            )
            .unwrap();

        let request = ctx.take_fetch_request().unwrap();
        assert_eq!(request.output_path.as_deref(), Some("saved.txt"));
    }
}
