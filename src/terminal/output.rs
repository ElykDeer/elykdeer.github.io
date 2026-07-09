use std::fmt::Write;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum OutputBlock {
    Text(String),
    Error(String),
    Panel {
        title: String,
        body: Vec<OutputBlock>,
    },
    FileView {
        path: String,
        content: String,
    },
    SaveManager,
    NanoEditor {
        path: String,
        content: String,
    },
    LaunchGame(GameLaunch),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum GameLaunch {
    Bubbles {
        hard: bool,
        cheat: bool,
    },
    #[cfg(debug_assertions)]
    Snek,
    Textropolis,
    WordHunt {
        reveal: bool,
        board: Option<WordHuntLaunchBoard>,
    },
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum WordHuntLaunchBoard {
    Max,
    Shape { cols: usize, rows: usize },
}

impl GameLaunch {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Bubbles { .. } => "bubbles",
            #[cfg(debug_assertions)]
            Self::Snek => "snek",
            Self::Textropolis => "textropolis",
            Self::WordHunt { .. } => "wordhunt",
        }
    }

    pub fn launch_label(&self) -> String {
        match self {
            Self::WordHunt {
                board: Some(board), ..
            } => format!("wordhunt:{}", board.label()),
            _ => self.id().to_string(),
        }
    }

    pub fn close_message(&self) -> String {
        format!("[{} closed]", self.launch_label())
    }
}

impl WordHuntLaunchBoard {
    pub fn label(self) -> String {
        match self {
            Self::Max => "max".to_string(),
            Self::Shape { cols, rows } => format!("{cols}x{rows}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnsiFragment {
    pub text: String,
    pub style: AnsiStyle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnsiStyle {
    pub foreground: Option<AnsiColor>,
    pub background: Option<AnsiColor>,
    pub bold: bool,
    pub faint: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnsiColor {
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl AnsiStyle {
    pub fn to_css(self) -> Option<String> {
        let mut css = String::new();
        if let Some(color) = self.foreground {
            let _ = write!(css, "color:{};", color.to_css());
        }
        if let Some(color) = self.background {
            let _ = write!(css, "background-color:{};", color.to_css());
        }
        if self.bold {
            css.push_str("font-weight:700;");
        }
        if self.faint {
            css.push_str("opacity:0.7;");
        }
        if self.italic {
            css.push_str("font-style:italic;");
        }
        let mut decorations = Vec::new();
        if self.underline {
            decorations.push("underline");
        }
        if self.strikethrough {
            decorations.push("line-through");
        }
        if !decorations.is_empty() {
            let _ = write!(css, "text-decoration:{};", decorations.join(" "));
        }
        if css.is_empty() {
            None
        } else {
            Some(css)
        }
    }
}

impl AnsiColor {
    fn to_css(self) -> String {
        let (r, g, b) = self.rgb();
        format!("rgb({r}, {g}, {b})")
    }

    fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Rgb(r, g, b) => (r, g, b),
            Self::Indexed(index) => xterm_color(index),
        }
    }
}

pub fn parse_ansi_fragments(input: &str) -> Vec<AnsiFragment> {
    let mut fragments = Vec::new();
    let mut style = AnsiStyle::default();
    let mut text = String::new();
    let bytes = input.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == 0x1b {
            if let Some((sequence_end, params)) = parse_csi_sgr(bytes, index) {
                push_fragment(&mut fragments, &mut text, style);
                apply_sgr(&mut style, &params);
                index = sequence_end;
                continue;
            }
        }

        if let Some(ch) = input[index..].chars().next() {
            text.push(ch);
            index += ch.len_utf8();
        } else {
            break;
        }
    }

    push_fragment(&mut fragments, &mut text, style);
    if fragments.is_empty() {
        fragments.push(AnsiFragment {
            text: String::new(),
            style: AnsiStyle::default(),
        });
    }
    fragments
}

fn push_fragment(fragments: &mut Vec<AnsiFragment>, text: &mut String, style: AnsiStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = fragments.last_mut() {
        if last.style == style {
            last.text.push_str(text);
            text.clear();
            return;
        }
    }
    fragments.push(AnsiFragment {
        text: std::mem::take(text),
        style,
    });
}

fn parse_csi_sgr(bytes: &[u8], start: usize) -> Option<(usize, Vec<u16>)> {
    if bytes.get(start + 1) != Some(&b'[') {
        return None;
    }

    let mut cursor = start + 2;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte.is_ascii_alphabetic() {
            if byte != b'm' {
                return Some((cursor + 1, Vec::new()));
            }
            let params = std::str::from_utf8(&bytes[start + 2..cursor]).ok()?;
            let values = if params.is_empty() {
                vec![0]
            } else {
                params
                    .split(';')
                    .map(|value| value.parse::<u16>().ok())
                    .collect::<Option<Vec<_>>>()?
            };
            return Some((cursor + 1, values));
        }
        cursor += 1;
    }

    None
}

fn apply_sgr(style: &mut AnsiStyle, params: &[u16]) {
    if params.is_empty() {
        return;
    }

    let mut index = 0;
    while index < params.len() {
        match params[index] {
            0 => *style = AnsiStyle::default(),
            1 => style.bold = true,
            2 => style.faint = true,
            3 => style.italic = true,
            4 => style.underline = true,
            9 => style.strikethrough = true,
            22 => {
                style.bold = false;
                style.faint = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            29 => style.strikethrough = false,
            30..=37 => style.foreground = Some(AnsiColor::Indexed((params[index] - 30) as u8)),
            39 => style.foreground = None,
            40..=47 => style.background = Some(AnsiColor::Indexed((params[index] - 40) as u8)),
            49 => style.background = None,
            90..=97 => style.foreground = Some(AnsiColor::Indexed((params[index] - 90 + 8) as u8)),
            100..=107 => {
                style.background = Some(AnsiColor::Indexed((params[index] - 100 + 8) as u8))
            }
            38 => {
                if let Some((color, consumed)) = parse_extended_color(&params[index + 1..]) {
                    style.foreground = Some(color);
                    index += consumed;
                }
            }
            48 => {
                if let Some((color, consumed)) = parse_extended_color(&params[index + 1..]) {
                    style.background = Some(color);
                    index += consumed;
                }
            }
            _ => {}
        }
        index += 1;
    }
}

fn parse_extended_color(params: &[u16]) -> Option<(AnsiColor, usize)> {
    match params {
        [5, index, ..] => Some((AnsiColor::Indexed((*index).try_into().ok()?), 2)),
        [2, r, g, b, ..] => Some((
            AnsiColor::Rgb(
                (*r).try_into().ok()?,
                (*g).try_into().ok()?,
                (*b).try_into().ok()?,
            ),
            4,
        )),
        _ => None,
    }
}

fn xterm_color(index: u8) -> (u8, u8, u8) {
    const ANSI_16: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    match index {
        0..=15 => ANSI_16[index as usize],
        16..=231 => {
            let value = index - 16;
            let r = value / 36;
            let g = (value % 36) / 6;
            let b = value % 6;
            (cube_color(r), cube_color(g), cube_color(b))
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

fn cube_color(value: u8) -> u8 {
    if value == 0 {
        0
    } else {
        55 + value * 40
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_ansi_fragments, AnsiColor, AnsiStyle};

    #[test]
    fn parses_indexed_foreground_and_reset() {
        let fragments = parse_ansi_fragments("plain \x1b[38;5;33mblue\x1b[0m done");

        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments[0].text, "plain ");
        assert_eq!(fragments[0].style, AnsiStyle::default());
        assert_eq!(fragments[1].text, "blue");
        assert_eq!(
            fragments[1].style,
            AnsiStyle {
                foreground: Some(AnsiColor::Indexed(33)),
                ..AnsiStyle::default()
            }
        );
        assert_eq!(fragments[2].text, " done");
        assert_eq!(fragments[2].style, AnsiStyle::default());
    }

    #[test]
    fn parses_rgb_background_and_text_attributes() {
        let fragments = parse_ansi_fragments("\x1b[1;4;48;2;12;34;56mhello\x1b[24m there\x1b[0m");

        assert_eq!(fragments.len(), 2);
        assert_eq!(fragments[0].text, "hello");
        assert_eq!(
            fragments[0].style,
            AnsiStyle {
                background: Some(AnsiColor::Rgb(12, 34, 56)),
                bold: true,
                underline: true,
                ..AnsiStyle::default()
            }
        );
        assert_eq!(fragments[1].text, " there");
        assert_eq!(
            fragments[1].style,
            AnsiStyle {
                background: Some(AnsiColor::Rgb(12, 34, 56)),
                bold: true,
                ..AnsiStyle::default()
            }
        );
    }

    #[test]
    fn leaves_incomplete_escape_sequences_as_text() {
        let fragments = parse_ansi_fragments("oops \x1b[38;5;33");

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].text, "oops \x1b[38;5;33");
        assert_eq!(fragments[0].style, AnsiStyle::default());
    }
}
