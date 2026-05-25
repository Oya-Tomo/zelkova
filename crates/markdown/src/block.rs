use crate::ast::*;

/// Detects block boundaries in a Markdown document.
/// Returns a list of "block slices" — (start_line, end_line, BlockKind).
pub fn detect_blocks(lines: &[&str]) -> Vec<BlockSlice> {
    let mut slices = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // Blank line
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // ATX heading
        if let Some(level) = parse_atx_heading(line) {
            slices.push(BlockSlice {
                start: i,
                end: i,
                kind: BlockKind::Heading { level },
            });
            i += 1;
            continue;
        }

        // Code fence
        if let Some(fence) = parse_code_fence(line) {
            let lang = fence.language;
            let start = i;
            i += 1;
            while i < lines.len() && !is_closing_fence(lines[i], &fence.marker) {
                i += 1;
            }
            let end = if i < lines.len() { i } else { lines.len() - 1 };
            slices.push(BlockSlice {
                start,
                end,
                kind: BlockKind::CodeBlock { language: lang },
            });
            i += 1;
            continue;
        }

        // Math block ($$)
        if line.trim() == "$$" {
            let start = i;
            i += 1;
            while i < lines.len() && lines[i].trim() != "$$" {
                i += 1;
            }
            let end = if i < lines.len() { i } else { lines.len() - 1 };
            slices.push(BlockSlice {
                start,
                end,
                kind: BlockKind::MathBlock,
            });
            i += 1;
            continue;
        }

        // Horizontal rule
        if is_thematic_break(line) {
            slices.push(BlockSlice {
                start: i,
                end: i,
                kind: BlockKind::ThematicBreak,
            });
            i += 1;
            continue;
        }

        // Table (check for separator line on next line)
        if i + 1 < lines.len() && is_table_separator(lines[i + 1]) && is_table_row(line) {
            let start = i;
            i += 2; // skip header + separator
            while i < lines.len() && is_table_row(lines[i]) {
                i += 1;
            }
            slices.push(BlockSlice {
                start,
                end: i - 1,
                kind: BlockKind::Table,
            });
            continue;
        }

        // Block quote
        if line.starts_with('>') || line.starts_with("> ") {
            let start = i;
            while i < lines.len() && (lines[i].starts_with('>') || lines[i].starts_with("> ")) {
                i += 1;
            }
            slices.push(BlockSlice {
                start,
                end: i - 1,
                kind: BlockKind::BlockQuote,
            });
            continue;
        }

        // List
        if let Some(marker) = parse_list_marker(line) {
            let start = i;
            while i < lines.len() {
                if lines[i].trim().is_empty() {
                    break;
                }
                if parse_list_marker(lines[i]).is_some() {
                    i += 1;
                } else if lines[i].starts_with(' ') || lines[i].starts_with('\t') {
                    i += 1;
                } else {
                    break;
                }
            }
            slices.push(BlockSlice {
                start,
                end: i - 1,
                kind: BlockKind::List {
                    first_marker: marker,
                },
            });
            continue;
        }

        // Footnote definition
        if let Some(label) = parse_footnote_def(line) {
            let start = i;
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            slices.push(BlockSlice {
                start,
                end: i - 1,
                kind: BlockKind::FootnoteDef { label },
            });
            continue;
        }

        // HTML block
        if line.trim_start().starts_with('<') {
            let start = i;
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            slices.push(BlockSlice {
                start,
                end: i - 1,
                kind: BlockKind::HtmlBlock,
            });
            continue;
        }

        // Paragraph (fallback)
        let start = i;
        while i < lines.len() && !lines[i].trim().is_empty() {
            i += 1;
            // stop if next line is another block type
            if i < lines.len() {
                let next = lines[i];
                if parse_atx_heading(next).is_some()
                    || parse_code_fence(next).is_some()
                    || is_thematic_break(next)
                    || next.starts_with('>')
                    || parse_list_marker(next).is_some()
                {
                    break;
                }
            }
        }
        slices.push(BlockSlice {
            start,
            end: i - 1,
            kind: BlockKind::Paragraph,
        });
    }

    slices
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockSlice {
    pub start: usize,
    pub end: usize,
    pub kind: BlockKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    Heading { level: u8 },
    Paragraph,
    CodeBlock { language: Option<String> },
    List { first_marker: ListMarker },
    BlockQuote,
    Table,
    ThematicBreak,
    MathBlock,
    HtmlBlock,
    FootnoteDef { label: String },
}

pub struct FenceInfo {
    pub marker: String,
    pub language: Option<String>,
}

fn parse_atx_heading(line: &str) -> Option<u8> {
    let trimmed = line.trim_start();
    let level = trimmed.bytes().take_while(|&b| b == b'#').count();
    if level >= 1 && level <= 6 {
        let rest = &trimmed[level..];
        if rest.is_empty() || rest.starts_with(' ') {
            return Some(level as u8);
        }
    }
    None
}

fn parse_code_fence(line: &str) -> Option<FenceInfo> {
    let trimmed = line.trim();
    let marker_char = if trimmed.starts_with("```") {
        '`'
    } else if trimmed.starts_with("~~~") {
        '~'
    } else {
        return None;
    };
    let marker_len = trimmed
        .bytes()
        .take_while(|&b| b == marker_char as u8)
        .count();
    if marker_len < 3 {
        return None;
    }
    let lang = trimmed[marker_len..].trim();
    let language = if lang.is_empty() {
        None
    } else {
        Some(lang.to_string())
    };
    Some(FenceInfo {
        marker: marker_char.to_string().repeat(marker_len),
        language,
    })
}

fn is_closing_fence(line: &str, marker: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with(&marker.chars().next().unwrap().to_string()) {
        return false;
    }
    let fence_char = marker.chars().next().unwrap();
    let count = trimmed.chars().take_while(|&c| c == fence_char).count();
    count >= 3
}

fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.chars().next().unwrap();
    if first != '-' && first != '*' && first != '_' {
        return false;
    }
    let count = trimmed.chars().filter(|&c| c == first).count();
    count >= 3 && trimmed.chars().all(|c| c == first || c == ' ' || c == '\t')
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') || trimmed.ends_with('|')
}

fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.contains('-') {
        return false;
    }
    trimmed.split('|').all(|cell| {
        let cell = cell.trim();
        if cell.is_empty() {
            return true;
        }
        cell == ":"
            || cell == "-:"
            || cell == ":-:"
            || cell == ":-"
            || cell.chars().all(|c| c == '-')
    })
}

pub fn parse_list_marker(line: &str) -> Option<ListMarker> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("- ") {
        Some(ListMarker::Dash)
    } else if trimmed.starts_with("* ") {
        Some(ListMarker::Star)
    } else if trimmed.starts_with("+ ") {
        Some(ListMarker::Plus)
    } else {
        // numbered: "1. "
        let dot_pos = trimmed.find(". ")?;
        let num_str = &trimmed[..dot_pos];
        let num: u32 = num_str.parse().ok()?;
        Some(ListMarker::Number(num))
    }
}

fn parse_footnote_def(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("[^") {
        return None;
    }
    let end = trimmed.find("]:")?;
    let label = trimmed[2..end].to_string();
    Some(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_heading() {
        let lines = vec!["# Hello", "## World"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 2);
        assert!(matches!(slices[0].kind, BlockKind::Heading { level: 1 }));
        assert!(matches!(slices[1].kind, BlockKind::Heading { level: 2 }));
    }

    #[test]
    fn detect_code_block() {
        let lines = vec!["```rust", "fn main() {}", "```"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 1);
        assert!(matches!(slices[0].kind, BlockKind::CodeBlock { .. }));
        assert_eq!(slices[0].start, 0);
        assert_eq!(slices[0].end, 2);
    }

    #[test]
    fn detect_thematic_break() {
        let lines = vec!["---", "***", "___"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 3);
        assert!(matches!(slices[0].kind, BlockKind::ThematicBreak));
    }

    #[test]
    fn detect_list() {
        let lines = vec!["- item 1", "- item 2", "  continuation"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 1);
        assert!(matches!(slices[0].kind, BlockKind::List { .. }));
    }

    #[test]
    fn detect_block_quote() {
        let lines = vec!["> quoted", "> still quoted"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 1);
        assert!(matches!(slices[0].kind, BlockKind::BlockQuote));
    }

    #[test]
    fn detect_table() {
        let lines = vec!["| a | b |", "| --- | --- |", "| 1 | 2 |"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 1);
        assert!(matches!(slices[0].kind, BlockKind::Table));
    }

    #[test]
    fn detect_paragraph() {
        let lines = vec!["Hello world", "", "Second paragraph"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 2);
        assert!(matches!(slices[0].kind, BlockKind::Paragraph));
        assert!(matches!(slices[1].kind, BlockKind::Paragraph));
    }

    #[test]
    fn detect_math_block() {
        let lines = vec!["$$", "E = mc^2", "$$"];
        let slices = detect_blocks(&lines);
        assert_eq!(slices.len(), 1);
        assert!(matches!(slices[0].kind, BlockKind::MathBlock));
    }

    #[test]
    fn parse_list_marker_variants() {
        assert!(matches!(
            parse_list_marker("- item"),
            Some(ListMarker::Dash)
        ));
        assert!(matches!(
            parse_list_marker("* item"),
            Some(ListMarker::Star)
        ));
        assert!(matches!(
            parse_list_marker("+ item"),
            Some(ListMarker::Plus)
        ));
        assert!(matches!(
            parse_list_marker("1. item"),
            Some(ListMarker::Number(1))
        ));
        assert!(parse_list_marker("plain text").is_none());
    }
}
