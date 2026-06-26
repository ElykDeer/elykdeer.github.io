use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    EmptyInput,
    AboveRoot,
    InvalidSegment(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "path cannot be empty"),
            Self::AboveRoot => write!(f, "path cannot navigate above /"),
            Self::InvalidSegment(segment) => write!(f, "invalid path segment: {segment}"),
        }
    }
}

impl std::error::Error for PathError {}

pub fn normalize_path(cwd: &str, input: &str) -> Result<String, PathError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(PathError::EmptyInput);
    }

    if input.starts_with('/') {
        normalize_segments(input.split('/'))
    } else {
        let base = normalize_absolute(cwd)?;
        let joined = if base == "/" {
            format!("/{input}")
        } else {
            format!("{base}/{input}")
        };
        normalize_segments(joined.split('/'))
    }
}

pub fn normalize_absolute(path: &str) -> Result<String, PathError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(PathError::EmptyInput);
    }
    if !path.starts_with('/') {
        return Err(PathError::InvalidSegment(path.to_string()));
    }
    normalize_segments(path.split('/'))
}

fn normalize_segments<'a>(
    segments: impl IntoIterator<Item = &'a str>,
) -> Result<String, PathError> {
    let mut normalized = Vec::new();

    for segment in segments {
        match segment {
            "" | "." => {}
            ".." => {
                if normalized.pop().is_none() {
                    return Err(PathError::AboveRoot);
                }
            }
            _ => {
                validate_segment(segment)?;
                normalized.push(segment);
            }
        }
    }

    if normalized.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", normalized.join("/")))
    }
}

fn validate_segment(segment: &str) -> Result<(), PathError> {
    if segment.is_empty() {
        return Err(PathError::InvalidSegment(segment.to_string()));
    }
    if segment.contains('/') || segment.contains('\0') {
        return Err(PathError::InvalidSegment(segment.to_string()));
    }
    Ok(())
}

pub fn parent_path(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }

    let trimmed = path.trim_end_matches('/');
    let index = trimmed.rfind('/')?;
    if index == 0 {
        Some("/".to_string())
    } else {
        Some(trimmed[..index].to_string())
    }
}

fn file_name(path: &str) -> Option<&str> {
    if path == "/" {
        None
    } else {
        path.rsplit('/').next().filter(|name| !name.is_empty())
    }
}

pub fn is_under(path: &str, ancestor: &str) -> bool {
    if ancestor == "/" {
        return path != "/";
    }
    path.strip_prefix(ancestor)
        .is_some_and(|rest| rest.starts_with('/'))
}

pub fn direct_child_name(path: &str, parent: &str) -> Option<String> {
    if path == parent {
        return None;
    }

    if parent_path(path).as_deref() == Some(parent) {
        file_name(path).map(ToString::to_string)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_repeated_slashes_and_dot_segments() {
        assert_eq!(
            normalize_path("/docs", "./drafts//today.md").unwrap(),
            "/docs/drafts/today.md"
        );
        assert_eq!(normalize_path("/", "///manual.md").unwrap(), "/manual.md");
    }

    #[test]
    fn prevents_navigation_above_root() {
        assert_eq!(
            normalize_path("/", "../secret").unwrap_err(),
            PathError::AboveRoot
        );
    }
}
