use std::ops::Range;

use gpui::{FontStyle, FontWeight, HighlightStyle, Hsla, UnderlineStyle, px, rgba};
use gpui_component::Theme;

use crate::theme::ResolvedMarkdownColors;

const DEFAULT_LINE_HEIGHT: f32 = 22.0;

/// All theme colors pre-resolved to Hsla. Created once per theme update,
/// then passed to highlight functions to avoid repeated string parsing.
#[derive(Debug, Clone)]
pub struct ResolvedColors {
    pub text: Hsla,
    pub bg: Hsla,
    pub border: Hsla,
    pub selection_bg: Hsla,
    pub text_muted: Hsla,
    pub heading_marker: Hsla,
    pub heading_fg: Hsla,
    pub list_marker: Hsla,
    pub quote_fg: Hsla,
    pub text_dim: Hsla,
    pub bold_fg: Hsla,
    pub italic_fg: Hsla,
    pub strikethrough_fg: Hsla,
    pub image_marker: Hsla,
    pub link_fg: Hsla,
    pub math_fg: Hsla,
    pub math_bg: Hsla,
    pub code_bg: Hsla,
    pub code_fg: Hsla,
    pub tag_fg: Hsla,
    code_syntax: [Hsla; 12],
}

impl ResolvedColors {
    pub fn from_theme(theme: &Theme, md: &ResolvedMarkdownColors) -> Self {
        Self {
            text: theme.foreground,
            bg: theme.background,
            border: theme.border,
            selection_bg: theme.selection,
            text_muted: theme.muted_foreground,
            heading_marker: md.heading_marker,
            heading_fg: md.heading,
            list_marker: md.list_marker,
            quote_fg: md.quote,
            text_dim: theme.muted_foreground,
            bold_fg: md.bold,
            italic_fg: md.italic,
            strikethrough_fg: md.strikethrough,
            image_marker: md.image_marker,
            link_fg: md.link,
            math_fg: md.math_fg,
            math_bg: md.math_bg,
            code_bg: md.code_bg,
            code_fg: md.code_fg,
            tag_fg: md.tag,
            code_syntax: extract_syntax_colors(theme),
        }
    }

    pub fn syntax_color(&self, index: usize) -> Option<Hsla> {
        self.code_syntax.get(index).copied()
    }
}

impl Default for ResolvedColors {
    fn default() -> Self {
        let white = rgba(0xFFFFFFFF).into();
        let gray = rgba(0xFF333333).into();
        Self {
            text: white,
            bg: gray,
            border: gray,
            selection_bg: gray,
            text_muted: white,
            heading_marker: white,
            heading_fg: white,
            list_marker: white,
            quote_fg: white,
            text_dim: white,
            bold_fg: white,
            italic_fg: white,
            strikethrough_fg: white,
            image_marker: white,
            link_fg: white,
            math_fg: white,
            math_bg: gray,
            code_bg: gray,
            code_fg: white,
            tag_fg: white,
            code_syntax: [white; 12],
        }
    }
}

/// Extract 12 syntax highlight colors from the theme's syntax colors.
/// Order matches HIGHLIGHT_NAMES in zelkova-highlight:
///   attribute, comment, constant, function, keyword, number,
///   operator, property, punctuation, string, tag, type
fn extract_syntax_colors(theme: &Theme) -> [Hsla; 12] {
    let syn = &theme.highlight_theme.style.syntax;
    let default_color = theme.foreground;
    let color_of = |name: &str| -> Hsla {
        syn.style(name)
            .and_then(|s| s.color)
            .unwrap_or(default_color)
    };
    [
        color_of("attribute"),
        color_of("comment"),
        color_of("constant"),
        color_of("function"),
        color_of("keyword"),
        color_of("number"),
        color_of("constructor"),
        color_of("property"),
        color_of("embedded"),
        color_of("string"),
        color_of("tag"),
        color_of("type"),
    ]
}

#[derive(Debug, Clone)]
pub enum BlockContext {
    Normal,
    Heading {
        level: u8,
    },
    ListItem {
        marker_len: usize,
    },
    BlockQuote,
    CodeBlock {
        #[allow(dead_code)]
        language: Option<String>,
    },
    TableSeparator,
    TableRow,
}

#[derive(Debug, Clone)]
pub struct HighlightedLine {
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
    pub image_urls: Vec<String>,
    pub line_height: f32,
    pub heading_level: Option<u8>,
    pub line_bg: Option<Hsla>,
}

/// Detect block context from a single line.
pub fn detect_line_context(line: &str, in_code_block: bool) -> BlockContext {
    if in_code_block {
        return BlockContext::CodeBlock { language: None };
    }

    if let Some(rest) = line.strip_prefix('#') {
        let mut level = 1u8;
        let mut s = rest;
        while s.starts_with('#') && level < 6 {
            level += 1;
            s = &s[1..];
        }
        if s.starts_with(' ') || s.is_empty() {
            return BlockContext::Heading { level };
        }
    }

    if line.starts_with('|') {
        let is_sep = line
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ' || c == '\t');
        if is_sep && line.contains('-') {
            return BlockContext::TableSeparator;
        }
        if line.contains('|') {
            return BlockContext::TableRow;
        }
    }

    if line.starts_with("- ") || line.starts_with("* ") || line.starts_with("+ ") {
        return BlockContext::ListItem { marker_len: 2 };
    }

    if let Some(dot_pos) = line.find(". ")
        && dot_pos > 0
        && line[..dot_pos].chars().all(|c| c.is_ascii_digit())
    {
        return BlockContext::ListItem {
            marker_len: dot_pos + 2,
        };
    }

    if line.starts_with('>') {
        return BlockContext::BlockQuote;
    }

    BlockContext::Normal
}

fn heading_font_size(level: u8) -> f32 {
    match level {
        1 => 34.0,
        2 => 30.0,
        3 => 26.0,
        4 => 24.0,
        5 => 22.0,
        _ => 22.0,
    }
}

/// Highlight a fenced code block boundary line (```lang```).
pub fn highlight_fence_line(line: &str, colors: &ResolvedColors) -> HighlightedLine {
    let backtick_count = line.bytes().take_while(|&b| b == b'`').count();
    let rest = &line[backtick_count..];
    let rest_trimmed = rest.trim();
    let mut highlights = Vec::new();

    if backtick_count > 0 {
        highlights.push((
            0..backtick_count,
            HighlightStyle {
                color: Some(colors.code_fg),
                background_color: Some(colors.code_bg),
                fade_out: Some(0.3),
                ..Default::default()
            },
        ));
    }

    if !rest_trimmed.is_empty() {
        let label_start = backtick_count + (rest.len() - rest.trim_start().len());
        let label_end = label_start + rest_trimmed.len();
        highlights.push((
            label_start..label_end,
            HighlightStyle {
                color: Some(colors.syntax_color(4).unwrap_or(colors.text)),
                background_color: Some(colors.code_bg),
                ..Default::default()
            },
        ));
        if label_end < line.len() {
            highlights.push((
                label_end..line.len(),
                HighlightStyle {
                    background_color: Some(colors.code_bg),
                    ..Default::default()
                },
            ));
        }
    } else if backtick_count < line.len() {
        highlights.push((
            backtick_count..line.len(),
            HighlightStyle {
                background_color: Some(colors.code_bg),
                ..Default::default()
            },
        ));
    }

    HighlightedLine {
        highlights,
        image_urls: Vec::new(),
        line_height: DEFAULT_LINE_HEIGHT,
        heading_level: None,
        line_bg: Some(colors.code_bg),
    }
}

/// Highlight a single line, returning byte-ranged styles and optional image URL.
pub fn highlight_line(
    line: &str,
    context: &BlockContext,
    colors: &ResolvedColors,
) -> HighlightedLine {
    let mut highlights = Vec::new();
    let mut image_urls = Vec::new();
    let mut line_height = DEFAULT_LINE_HEIGHT;

    match context {
        BlockContext::Heading { level } => {
            line_height = heading_font_size(*level);
            let hash_end = *level as usize;
            if line.len() >= hash_end && line[..hash_end].chars().all(|c| c == '#') {
                highlights.push((
                    0..hash_end,
                    HighlightStyle {
                        color: Some(colors.heading_marker),
                        ..Default::default()
                    },
                ));
                let rest_start = if line.len() > hash_end && line.as_bytes()[hash_end] == b' ' {
                    hash_end + 1
                } else {
                    hash_end
                };
                if rest_start < line.len() {
                    highlights.push((
                        rest_start..line.len(),
                        HighlightStyle {
                            color: Some(colors.heading_fg),
                            font_weight: Some(FontWeight::BOLD),
                            ..Default::default()
                        },
                    ));
                    let (ihl, imgs) = scan_inline(&line[rest_start..], rest_start, colors);
                    highlights.extend(ihl);
                    image_urls = imgs;
                }
                return HighlightedLine {
                    highlights,
                    image_urls,
                    line_height,
                    heading_level: Some(*level),
                    line_bg: None,
                };
            }
        }
        BlockContext::ListItem { marker_len } => {
            let ml = (*marker_len).min(line.len());
            if ml > 0 {
                highlights.push((0..ml, HighlightStyle::color(colors.list_marker)));
            }
        }
        BlockContext::BlockQuote => {
            if line.starts_with('>') {
                let end = if line.len() > 1 && line.as_bytes()[1] == b' ' {
                    2
                } else {
                    1
                };
                highlights.push((0..end, HighlightStyle::color(colors.quote_fg)));
                if end < line.len() {
                    highlights.push((
                        end..line.len(),
                        HighlightStyle {
                            color: Some(colors.quote_fg),
                            font_style: Some(FontStyle::Italic),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        BlockContext::CodeBlock { .. } => {
            return HighlightedLine {
                highlights: vec![(
                    0..line.len().max(1),
                    HighlightStyle {
                        color: Some(colors.code_fg),
                        ..Default::default()
                    },
                )],
                image_urls: Vec::new(),
                line_height: DEFAULT_LINE_HEIGHT,
                heading_level: None,
                line_bg: Some(colors.code_bg),
            };
        }
        BlockContext::TableSeparator => {
            return HighlightedLine {
                highlights: vec![(
                    0..line.len().max(1),
                    HighlightStyle {
                        fade_out: Some(0.5),
                        ..Default::default()
                    },
                )],
                image_urls: Vec::new(),
                line_height: DEFAULT_LINE_HEIGHT,
                heading_level: None,
                line_bg: None,
            };
        }
        BlockContext::TableRow => {
            let dim_pipe = HighlightStyle {
                color: Some(colors.quote_fg),
                ..Default::default()
            };
            let mut i = 0;
            let bytes = line.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'|' {
                    highlights.push((i..i + 1, dim_pipe));
                    i += 1;
                } else {
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'|' {
                        i += 1;
                    }
                    if start < i {
                        let cell = &line[start..i];
                        let trimmed = cell.trim();
                        if !trimmed.is_empty() {
                            let trim_start = start + cell.len() - cell.trim_start().len();
                            let trim_end = i - cell.trim_end().len();
                            highlights.push((
                                trim_start..trim_end,
                                HighlightStyle {
                                    color: Some(colors.text_dim),
                                    ..Default::default()
                                },
                            ));
                        }
                    }
                }
            }
            let (ihl, imgs) = scan_inline(line, 0, colors);
            highlights.extend(ihl);
            image_urls = imgs;
            return HighlightedLine {
                highlights,
                image_urls,
                line_height: DEFAULT_LINE_HEIGHT,
                heading_level: None,
                line_bg: None,
            };
        }
        BlockContext::Normal => {}
    }

    let skip = match context {
        BlockContext::ListItem { marker_len } => (*marker_len).min(line.len()),
        BlockContext::BlockQuote => {
            if line.starts_with("> ") {
                2
            } else if line.starts_with('>') {
                1
            } else {
                0
            }
        }
        _ => 0,
    };

    if skip < line.len() {
        let (ihl, imgs) = scan_inline(&line[skip..], skip, colors);
        highlights.extend(ihl);
        image_urls = imgs;
    }

    HighlightedLine {
        highlights,
        image_urls,
        line_height,
        heading_level: None,
        line_bg: None,
    }
}

/// Create a marker style: content color with fade_out.
fn marker_style(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        fade_out: Some(0.4),
        ..Default::default()
    }
}

fn scan_inline(
    text: &str,
    offset: usize,
    colors: &ResolvedColors,
) -> (Vec<(Range<usize>, HighlightStyle)>, Vec<String>) {
    let mut highlights = Vec::new();
    let mut image_urls = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Bold **text** or __text__
        if (bytes[i] == b'*' || bytes[i] == b'_') && i + 1 < bytes.len() && bytes[i + 1] == bytes[i]
        {
            let marker = bytes[i];
            if let Some(end) = find_closing_double(bytes, i + 2, marker) {
                let ms = marker_style(colors.bold_fg);
                highlights.push((offset + i..offset + i + 2, ms));
                highlights.push((
                    offset + i + 2..offset + end,
                    HighlightStyle {
                        color: Some(colors.bold_fg),
                        ..Default::default()
                    },
                ));
                highlights.push((offset + end..offset + end + 2, ms));
                i = end + 2;
                continue;
            }
        }

        // Strikethrough ~~text~~
        if bytes[i] == b'~'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'~'
            && let Some(end) = find_closing_double(bytes, i + 2, b'~')
        {
            let ms = marker_style(colors.strikethrough_fg);
            highlights.push((offset + i..offset + i + 2, ms));
            highlights.push((
                offset + i + 2..offset + end,
                HighlightStyle {
                    color: Some(colors.strikethrough_fg),
                    strikethrough: Some(gpui::StrikethroughStyle {
                        thickness: px(1.0),
                        color: Some(colors.strikethrough_fg),
                    }),
                    ..Default::default()
                },
            ));
            highlights.push((offset + end..offset + end + 2, ms));
            i = end + 2;
            continue;
        }

        // Italic *text* or _text_
        if bytes[i] == b'*' || bytes[i] == b'_' {
            let marker = bytes[i];
            if i + 1 < bytes.len() && bytes[i + 1] == marker {
                i += 1;
                continue;
            }
            if let Some(end) = find_closing_single(bytes, i + 1, marker) {
                let ms = marker_style(colors.italic_fg);
                highlights.push((offset + i..offset + i + 1, ms));
                highlights.push((
                    offset + i + 1..offset + end,
                    HighlightStyle {
                        color: Some(colors.italic_fg),
                        ..Default::default()
                    },
                ));
                highlights.push((offset + end..offset + end + 1, ms));
                i = end + 1;
                continue;
            }
        }

        // Code span `code`
        if bytes[i] == b'`' {
            let count = count_backticks(bytes, i);
            if let Some(end) = find_closing_backticks(bytes, i + count, count) {
                let ms = HighlightStyle {
                    color: Some(colors.code_fg),
                    background_color: Some(colors.code_bg),
                    fade_out: Some(0.4),
                    ..Default::default()
                };
                let code_style = HighlightStyle {
                    color: Some(colors.code_fg),
                    background_color: Some(colors.code_bg),
                    ..Default::default()
                };
                highlights.push((offset + i..offset + i + count, ms));
                highlights.push((offset + i + count..offset + end, code_style));
                highlights.push((offset + end..offset + end + count, ms));
                i = end + count;
                continue;
            }
        }

        // Image ![alt](url)
        if bytes[i] == b'!'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'['
            && let Some((url, end)) = parse_image(bytes, i + 2)
        {
            highlights.push((
                offset + i..offset + end,
                HighlightStyle {
                    color: Some(colors.image_marker),
                    ..Default::default()
                },
            ));
            image_urls.push(url);
            i = end;
            continue;
        }

        // Link [text](url)
        if bytes[i] == b'['
            && let Some(end) = parse_link(bytes, i + 1)
        {
            highlights.push((
                offset + i..offset + end,
                HighlightStyle {
                    color: Some(colors.link_fg),
                    underline: Some(UnderlineStyle {
                        thickness: gpui::px(1.0),
                        color: Some(colors.link_fg),
                        wavy: false,
                    }),
                    ..Default::default()
                },
            ));
            i = end;
            continue;
        }

        // Math $...$
        if bytes[i] == b'$'
            && let Some(end) = find_closing_single(bytes, i + 1, b'$')
        {
            let ms = HighlightStyle {
                color: Some(colors.math_fg),
                background_color: Some(colors.math_bg),
                fade_out: Some(0.4),
                ..Default::default()
            };
            highlights.push((offset + i..offset + i + 1, ms));
            highlights.push((
                offset + i + 1..offset + end,
                HighlightStyle {
                    color: Some(colors.math_fg),
                    background_color: Some(colors.math_bg),
                    ..Default::default()
                },
            ));
            highlights.push((offset + end..offset + end + 1, ms));
            i = end + 1;
            continue;
        }

        i += 1;
    }

    (highlights, image_urls)
}

fn find_closing_double(bytes: &[u8], start: usize, marker: u8) -> Option<usize> {
    let mut i = start;
    while i + 1 < bytes.len() {
        if bytes[i] == marker && bytes[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_single(bytes: &[u8], start: usize, marker: u8) -> Option<usize> {
    for i in start..bytes.len() {
        if bytes[i] == marker {
            return Some(i);
        }
    }
    None
}

fn count_backticks(bytes: &[u8], start: usize) -> usize {
    bytes[start..].iter().take_while(|&&b| b == b'`').count()
}

fn find_closing_backticks(bytes: &[u8], start: usize, count: usize) -> Option<usize> {
    let mut i = start;
    while i + count <= bytes.len() {
        if bytes[i..i + count].iter().all(|&b| b == b'`') {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_image(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    while i < bytes.len() && bytes[i] != b']' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // skip ]
    if i >= bytes.len() || bytes[i] != b'(' {
        return None;
    }
    i += 1; // skip (
    let url_start = i;
    while i < bytes.len() && bytes[i] != b')' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let url = String::from_utf8_lossy(&bytes[url_start..i]).to_string();
    Some((url, i + 1))
}

fn parse_link(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i < bytes.len() && bytes[i] != b']' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    i += 1; // skip ]
    if i >= bytes.len() || bytes[i] != b'(' {
        return None;
    }
    i += 1; // skip (
    while i < bytes.len() && bytes[i] != b')' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    Some(i + 1)
}

/// Parse "#RRGGBB" hex color to Hsla.
pub fn parse_hex(hex: &str) -> Hsla {
    crate::theme::try_parse_hex(hex).unwrap_or(rgba(0xFFFFFFFF).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> ResolvedColors {
        ResolvedColors::default()
    }

    #[test]
    fn detect_heading() {
        assert!(matches!(
            detect_line_context("# Hello", false),
            BlockContext::Heading { level: 1 }
        ));
        assert!(matches!(
            detect_line_context("### World", false),
            BlockContext::Heading { level: 3 }
        ));
    }

    #[test]
    fn detect_list_item() {
        assert!(matches!(
            detect_line_context("- item", false),
            BlockContext::ListItem { marker_len: 2 }
        ));
        assert!(matches!(
            detect_line_context("1. first", false),
            BlockContext::ListItem { marker_len: 3 }
        ));
    }

    #[test]
    fn detect_blockquote() {
        assert!(matches!(
            detect_line_context("> quote", false),
            BlockContext::BlockQuote
        ));
    }

    #[test]
    fn detect_code_block() {
        assert!(matches!(
            detect_line_context("code here", true),
            BlockContext::CodeBlock { .. }
        ));
    }

    #[test]
    fn detect_normal() {
        assert!(matches!(
            detect_line_context("just text", false),
            BlockContext::Normal
        ));
    }

    #[test]
    fn highlight_heading_line() {
        let hl = highlight_line("# Hello", &BlockContext::Heading { level: 1 }, &colors());
        assert!(!hl.highlights.is_empty());
        assert!(hl.highlights.len() >= 2);
    }

    #[test]
    fn highlight_list_item() {
        let hl = highlight_line(
            "- task item",
            &BlockContext::ListItem { marker_len: 2 },
            &colors(),
        );
        assert!(!hl.highlights.is_empty());
        assert_eq!(hl.highlights[0].0.start, 0);
        assert_eq!(hl.highlights[0].0.end, 2);
    }

    #[test]
    fn highlight_inline_bold() {
        let hl = highlight_line("hello **world** end", &BlockContext::Normal, &colors());
        assert!(hl.highlights.iter().any(|(_, s)| s.color.is_some()));
    }

    #[test]
    fn highlight_inline_code() {
        let hl = highlight_line("use `code` here", &BlockContext::Normal, &colors());
        assert!(
            hl.highlights
                .iter()
                .any(|(_, s)| s.background_color.is_some())
        );
    }

    #[test]
    fn highlight_inline_link() {
        let hl = highlight_line(
            "click [here](http://example.com)",
            &BlockContext::Normal,
            &colors(),
        );
        assert!(hl.highlights.iter().any(|(_, s)| s.underline.is_some()));
    }

    #[test]
    fn highlight_inline_image() {
        let hl = highlight_line("![alt](image.png)", &BlockContext::Normal, &colors());
        assert_eq!(hl.image_urls, vec!["image.png".to_string()]);
    }

    #[test]
    fn highlight_code_block_no_inline() {
        let hl = highlight_line(
            "**bold**",
            &BlockContext::CodeBlock { language: None },
            &colors(),
        );
        assert!(
            hl.highlights
                .iter()
                .all(|(_, s)| s.font_weight.is_none() && s.font_style.is_none())
        );
    }

    #[test]
    fn parse_hex_color() {
        let c = super::parse_hex("#89b4fa");
        assert!(c.a > 0.0);
    }
}
