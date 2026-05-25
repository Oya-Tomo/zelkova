use crate::ast::*;
use crate::block::{self, BlockKind};
use crate::inline;

/// Parse a Markdown string into a MarkdownDoc.
pub fn parse(input: &str) -> MarkdownDoc {
    let (frontmatter, body) = split_frontmatter(input);
    let lines: Vec<&str> = body.lines().collect();
    let slices = block::detect_blocks(&lines);

    let blocks = slices.into_iter().map(|slice| {
        build_block(&lines, &slice)
    }).collect();

    MarkdownDoc { frontmatter, blocks }
}

fn split_frontmatter(input: &str) -> (Option<String>, &str) {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") { return (None, input); }
    let rest = &trimmed[3..];
    let Some(end_idx) = rest.find("---") else { return (None, input) };
    let yaml = rest[..end_idx].trim().to_string();
    let body = rest[end_idx + 3..].trim_start();
    (Some(yaml), body)
}

fn build_block(lines: &[&str], slice: &block::BlockSlice) -> Block {
    match &slice.kind {
        BlockKind::Heading { level } => {
            let line = lines[slice.start].trim_start();
            let text = line.trim_start_matches('#').trim_start();
            let children = inline::parse_inline(text);
            Block::Heading { level: *level, children }
        }

        BlockKind::Paragraph => {
            let text = lines[slice.start..=slice.end].join("\n");
            let children = inline::parse_inline(&text);
            Block::Paragraph(children)
        }

        BlockKind::CodeBlock { language } => {
            let first_line = lines[slice.start];
            // skip first (opening fence) and last (closing fence) lines
            let code_start = slice.start + 1;
            let code_end = if slice.end > slice.start { slice.end } else { slice.start + 1 };
            // check if last line is a closing fence
            let actual_end = if code_end > code_start && is_fence_line(lines[code_end]) {
                code_end
            } else {
                code_end + 1
            };
            let code = if code_start < actual_end && code_start < lines.len() {
                lines[code_start..actual_end.min(lines.len())].join("\n")
            } else {
                String::new()
            };
            Block::CodeBlock {
                language: language.clone(),
                code,
            }
        }

        BlockKind::List { first_marker: _ } => {
            let items = parse_list_items(lines, slice.start, slice.end);
            Block::List { items }
        }

        BlockKind::BlockQuote => {
            let quote_lines: Vec<&str> = lines[slice.start..=slice.end]
                .iter()
                .map(|l| l.strip_prefix("> ").unwrap_or(l.strip_prefix('>').unwrap_or(l)))
                .collect();
            let inner_text = quote_lines.join("\n");
            let inner_doc = parse(&inner_text);
            Block::BlockQuote(inner_doc.blocks)
        }

        BlockKind::Table => {
            parse_table(lines, slice.start, slice.end)
        }

        BlockKind::ThematicBreak => Block::ThematicBreak,

        BlockKind::MathBlock => {
            let content = if slice.end > slice.start + 1 {
                lines[slice.start + 1..slice.end].join("\n")
            } else {
                String::new()
            };
            Block::MathBlock { content }
        }

        BlockKind::HtmlBlock => {
            let content = lines[slice.start..=slice.end].join("\n");
            Block::HtmlBlock { content }
        }

        BlockKind::FootnoteDef { label } => {
            let first_line = lines[slice.start];
            let rest = first_line.find("]:").map(|pos| &first_line[pos + 2..]).unwrap_or("");
            let mut content_text = rest.trim().to_string();
            if slice.end > slice.start {
                content_text.push('\n');
                content_text.push_str(&lines[slice.start + 1..=slice.end].join("\n"));
            }
            let inner_doc = parse(&content_text);
            Block::FootnoteDefinition {
                label: label.clone(),
                content: inner_doc.blocks,
            }
        }
    }
}

fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn parse_list_items(lines: &[&str], start: usize, end: usize) -> Vec<ListItem> {
    let mut items = Vec::new();
    let mut i = start;

    while i <= end {
        let line = lines[i];
        if let Some(marker) = block::parse_list_marker(line) {
            let text = extract_list_text(line);
            let children = inline::parse_inline(&text);

            // collect continuation lines
            let mut item_end = i + 1;
            while item_end <= end {
                let next = lines[item_end];
                if block::parse_list_marker(next).is_some() { break; }
                if next.trim().is_empty() { break; }
                item_end += 1;
            }

            // check for sub-items (indented list)
            let mut sub_items = Vec::new();
            let mut j = i + 1;
            while j < item_end {
                let next = lines[j];
                if next.starts_with("  ") || next.starts_with("\t") {
                    if block::parse_list_marker(next.trim_start()).is_some() {
                        // found sub-list, collect it
                        let sub_start = j;
                        while j < item_end {
                            if block::parse_list_marker(lines[j].trim_start()).is_some() || lines[j].starts_with("  ") || lines[j].starts_with("\t") {
                                j += 1;
                            } else {
                                break;
                            }
                        }
                        let sub = parse_list_items(lines, sub_start, j - 1);
                        sub_items.extend(sub);
                        continue;
                    }
                }
                j += 1;
            }

            items.push(ListItem {
                marker,
                children,
                sub_items,
            });
            i = item_end;
        } else {
            i += 1;
        }
    }

    items
}

fn extract_list_text(line: &str) -> String {
    let trimmed = line.trim_start();
    // skip marker
    if let Some(pos) = trimmed.find(' ') {
        trimmed[pos + 1..].to_string()
    } else if let Some(pos) = trimmed.find(". ") {
        trimmed[pos + 2..].to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_table(lines: &[&str], start: usize, end: usize) -> Block {
    // first line: headers
    let headers = parse_table_row(lines[start]);
    // second line: separator (parse alignment)
    let aligns = parse_table_aligns(lines[start + 1]);
    // remaining: data rows
    let rows: Vec<Vec<Vec<Inline>>> = (start + 2..=end)
        .map(|i| parse_table_row(lines[i]))
        .collect();

    Block::Table { headers, aligns, rows }
}

fn parse_table_row(line: &str) -> Vec<Vec<Inline>> {
    let trimmed = line.trim();
    let inner = trimmed.trim_matches('|');
    inner.split('|')
        .map(|cell| inline::parse_inline(cell.trim()))
        .collect()
}

fn parse_table_aligns(line: &str) -> Vec<Option<TableAlign>> {
    let trimmed = line.trim();
    let inner = trimmed.trim_matches('|');
    inner.split('|')
        .map(|cell| {
            let cell = cell.trim();
            if cell.starts_with(':') && cell.ends_with(':') { Some(TableAlign::Center) }
            else if cell.ends_with(':') { Some(TableAlign::Right) }
            else if cell.starts_with(':') { Some(TableAlign::Left) }
            else { None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_heading() {
        let doc = parse("# Hello World");
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn parse_paragraph() {
        let doc = parse("Hello world\nSecond line");
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0], Block::Paragraph(_)));
    }

    #[test]
    fn parse_code_block() {
        let doc = parse("```rust\nfn main() {}\n```");
        assert_eq!(doc.blocks.len(), 1);
        if let Block::CodeBlock { language, code } = &doc.blocks[0] {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(code.contains("fn main()"));
        } else {
            panic!("expected CodeBlock");
        }
    }

    #[test]
    fn parse_list() {
        let doc = parse("- item 1\n- item 2\n- item 3");
        assert_eq!(doc.blocks.len(), 1);
        if let Block::List { items } = &doc.blocks[0] {
            assert_eq!(items.len(), 3);
        }
    }

    #[test]
    fn parse_block_quote() {
        let doc = parse("> quoted text");
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0], Block::BlockQuote(_)));
    }

    #[test]
    fn parse_table() {
        let doc = parse("| a | b |\n| --- | --- |\n| 1 | 2 |");
        assert_eq!(doc.blocks.len(), 1);
        if let Block::Table { headers, rows, .. } = &doc.blocks[0] {
            assert_eq!(headers.len(), 2);
            assert_eq!(rows.len(), 1);
        }
    }

    #[test]
    fn parse_math_block() {
        let doc = parse("$$\nE = mc^2\n$$");
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0], Block::MathBlock { .. }));
    }

    #[test]
    fn parse_frontmatter() {
        let doc = parse("---\ntitle: Test\n---\n\n# Hello");
        assert!(doc.frontmatter.is_some());
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn parse_mixed() {
        let doc = parse("# Title\n\nParagraph with **bold**.\n\n- list item\n\n```\ncode\n```");
        assert_eq!(doc.blocks.len(), 4);
    }

    #[test]
    fn parse_thematic_break() {
        let doc = parse("---");
        // This might be parsed as frontmatter delimiter... need to check
        // Actually "---" alone should be thematic break since there's no closing "---"
        // But our frontmatter split will try to find a closing ---
        // Let's check: with just "---", split_frontmatter returns (None, "---")
        // Then block detection should see it as thematic break
        if doc.frontmatter.is_some() {
            // If frontmatter consumed it, blocks might be empty
            // This is a known edge case
        } else {
            assert!(matches!(&doc.blocks[0], Block::ThematicBreak));
        }
    }
}
