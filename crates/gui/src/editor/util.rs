use gpui::{HighlightStyle, Pixels};

pub fn split_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    if text.ends_with('\n') {
        lines.push(String::new());
    }
    lines
}

pub fn split_at_char_col(s: &str, col: usize) -> (String, String) {
    let mut char_count = 0;
    for (byte_idx, _) in s.char_indices() {
        if char_count == col {
            return (s[..byte_idx].to_string(), s[byte_idx..].to_string());
        }
        char_count += 1;
    }
    (s.to_string(), String::new())
}

pub fn adjust_highlight_offsets(
    highlights: &[(std::ops::Range<usize>, HighlightStyle)],
    offset_start: usize,
    offset_end: usize,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    highlights
        .iter()
        .filter_map(|(range, style)| {
            if range.start >= offset_end || range.end <= offset_start {
                return None;
            }
            let new_start = range.start.max(offset_start) - offset_start;
            let new_end = range.end.min(offset_end) - offset_start;
            Some((new_start..new_end, *style))
        })
        .collect()
}

pub fn pixel_to_col(line: &str, pixel_x: Pixels, ascii_w: f32) -> usize {
    let target = f32::from(pixel_x);
    let mut width = 0.0;
    let mut col = 0;
    for c in line.chars() {
        let char_w = if c.is_ascii() {
            ascii_w
        } else if c as u32 > 0x2FFF {
            ascii_w * 2.0
        } else {
            ascii_w
        };
        if width + char_w / 2.0 > target {
            return col;
        }
        width += char_w;
        col += 1;
    }
    col
}

pub fn byte_to_utf16(text: &str, byte_pos: usize) -> usize {
    text[..byte_pos]
        .chars()
        .map(|c| if c as u32 > 0xFFFF { 2 } else { 1 })
        .sum()
}

pub fn utf16_to_byte(text: &str, utf16_pos: usize) -> usize {
    let mut count = 0;
    for (i, c) in text.char_indices() {
        if count >= utf16_pos {
            return i;
        }
        count += if c as u32 > 0xFFFF { 2 } else { 1 };
    }
    text.len()
}

pub fn char_idx_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Parse `#xxx` tokens from tag input text.
/// A valid tag is `#` followed by one or more non-whitespace characters.
pub fn parse_tags_from_input(input: &str) -> std::collections::HashSet<String> {
    let mut tags = std::collections::HashSet::new();
    for token in input.split_whitespace() {
        if let Some(tag) = token.strip_prefix('#')
            && !tag.is_empty()
        {
            tags.insert(tag.to_string());
        }
    }
    tags
}

/// Resolve an image URL to an absolute path.
/// - Absolute paths are kept as-is.
/// - `~/` is expanded to the home directory.
/// - Relative paths are resolved against the note file's directory.
pub fn resolve_image_path(note_path: Option<&std::path::Path>, url: &str) -> std::path::PathBuf {
    let url = url.trim();
    if url.starts_with('/') {
        return std::path::PathBuf::from(url);
    }
    if let Some(rest) = url.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
        return std::path::PathBuf::from(format!("/{rest}"));
    }
    if let Some(dir) = note_path.and_then(|p| p.parent()) {
        return dir.join(url);
    }
    std::path::PathBuf::from(url)
}

/// Deterministically overlay selection background onto existing highlights.
/// Unlike `combine_highlights`, this always lets selection background win.
pub fn overlay_selection(
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
    sel: std::ops::Range<usize>,
    sel_bg: gpui::Hsla,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    if sel.is_empty() {
        return highlights;
    }

    let mut result = Vec::new();
    let mut pos = sel.start;

    for (range, style) in highlights {
        if range.end <= sel.start || range.start >= sel.end {
            result.push((range, style));
            continue;
        }

        // Fill gap in selection before this highlight
        if pos < range.start {
            let gap_end = range.start.min(sel.end);
            if pos < gap_end {
                result.push((
                    pos..gap_end,
                    HighlightStyle {
                        background_color: Some(sel_bg),
                        ..Default::default()
                    },
                ));
            }
        }

        // Part before selection
        if range.start < sel.start {
            result.push((range.start..sel.start, style));
        }

        // Overlap — override background with selection
        let o_start = range.start.max(sel.start);
        let o_end = range.end.min(sel.end);
        let mut merged = style;
        merged.background_color = Some(sel_bg);
        result.push((o_start..o_end, merged));
        pos = o_end;

        // Part after selection
        if range.end > sel.end {
            result.push((sel.end..range.end, style));
        }
    }

    // Fill remaining gap at end of selection
    if pos < sel.end {
        result.push((
            pos..sel.end,
            HighlightStyle {
                background_color: Some(sel_bg),
                ..Default::default()
            },
        ));
    }

    result.sort_by_key(|(r, _)| r.start);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_tag() {
        let tags = parse_tags_from_input("#work");
        assert!(tags.contains("work"));
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn parse_multiple_tags() {
        let tags = parse_tags_from_input("#work #meeting #project");
        assert_eq!(tags.len(), 3);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
        assert!(tags.contains("project"));
    }

    #[test]
    fn discard_invalid_tokens() {
        let tags = parse_tags_from_input("#work garbage #meeting");
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
    }

    #[test]
    fn empty_hash_discarded() {
        let tags = parse_tags_from_input("# #valid");
        assert_eq!(tags.len(), 1);
        assert!(tags.contains("valid"));
    }

    #[test]
    fn empty_input() {
        let tags = parse_tags_from_input("");
        assert!(tags.is_empty());
    }

    #[test]
    fn full_width_space_normalization() {
        let input = "#work\u{3000}#meeting\u{3000}garbage";
        let normalized = input.replace('\u{3000}', " ");
        let tags = parse_tags_from_input(&normalized);
        assert_eq!(tags.len(), 2);
        assert!(tags.contains("work"));
        assert!(tags.contains("meeting"));
    }

    #[test]
    fn duplicate_tags_deduped() {
        let tags = parse_tags_from_input("#work #work #meeting");
        assert_eq!(tags.len(), 2);
    }

    // --- resolve_image_path tests ---

    #[test]
    fn resolve_absolute_path() {
        let resolved = resolve_image_path(None, "/absolute/path.png");
        assert_eq!(resolved, std::path::PathBuf::from("/absolute/path.png"));
    }

    #[test]
    fn resolve_relative_to_note() {
        let note = std::path::Path::new("/home/user/notes/note.md");
        let resolved = resolve_image_path(Some(note), "images/photo.png");
        assert_eq!(
            resolved,
            std::path::PathBuf::from("/home/user/notes/images/photo.png")
        );
    }

    #[test]
    fn resolve_no_note_path() {
        let resolved = resolve_image_path(None, "photo.png");
        assert_eq!(resolved, std::path::PathBuf::from("photo.png"));
    }

    // --- overlay_selection tests ---

    #[test]
    fn overlay_empty_selection() {
        let bg = gpui::Hsla::default();
        let h = vec![(
            0..5,
            HighlightStyle {
                color: Some(bg),
                ..Default::default()
            },
        )];
        let result = overlay_selection(h.clone(), 3..3, bg);
        assert_eq!(result, h);
    }

    #[test]
    fn overlay_no_highlights() {
        let bg = gpui::Hsla::default();
        let result = overlay_selection(vec![], 0..5, bg);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0..5);
    }

    #[test]
    fn overlay_full_overlap() {
        let bg = gpui::Hsla::default();
        let style = HighlightStyle {
            color: Some(bg),
            ..Default::default()
        };
        let result = overlay_selection(vec![(0..10, style)], 2..8, bg);
        // before, overlap, after
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 0..2);
        assert_eq!(result[1].0, 2..8);
        assert!(result[1].1.background_color.is_some());
        assert_eq!(result[2].0, 8..10);
    }

    #[test]
    fn overlay_highlight_outside_selection() {
        let bg = gpui::Hsla::default();
        let style = HighlightStyle {
            color: Some(bg),
            ..Default::default()
        };
        let result = overlay_selection(vec![(0..3, style), (8..12, style)], 4..7, bg);
        // highlight before + gap + gap + highlight after
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, 0..3);
        assert_eq!(result[1].0, 4..7);
        assert_eq!(result[2].0, 8..12);
    }

    // --- adjust_highlight_offsets tests ---

    #[test]
    fn adjust_keeps_overlapping() {
        let bg = gpui::Hsla::default();
        let style = HighlightStyle {
            color: Some(bg),
            ..Default::default()
        };
        let result = adjust_highlight_offsets(&[(2..8, style)], 2, 10);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0..6);
    }

    #[test]
    fn adjust_removes_non_overlapping() {
        let bg = gpui::Hsla::default();
        let style = HighlightStyle {
            color: Some(bg),
            ..Default::default()
        };
        let result = adjust_highlight_offsets(&[(0..3, style), (10..15, style)], 5, 10);
        assert!(result.is_empty());
    }

    // --- split_lines tests ---

    #[test]
    fn split_empty() {
        assert_eq!(split_lines(""), vec![""]);
    }

    #[test]
    fn split_single_line() {
        assert_eq!(split_lines("hello"), vec!["hello"]);
    }

    #[test]
    fn split_trailing_newline() {
        assert_eq!(split_lines("a\nb\n"), vec!["a", "b", ""]);
    }

    #[test]
    fn split_no_trailing_newline() {
        assert_eq!(split_lines("a\nb"), vec!["a", "b"]);
    }

    // --- split_at_char_col tests ---

    #[test]
    fn split_at_start() {
        let (before, after) = split_at_char_col("hello", 0);
        assert_eq!(before, "");
        assert_eq!(after, "hello");
    }

    #[test]
    fn split_at_middle() {
        let (before, after) = split_at_char_col("hello", 2);
        assert_eq!(before, "he");
        assert_eq!(after, "llo");
    }

    #[test]
    fn split_at_end() {
        let (before, after) = split_at_char_col("hello", 5);
        assert_eq!(before, "hello");
        assert_eq!(after, "");
    }

    #[test]
    fn split_past_end() {
        let (before, after) = split_at_char_col("hello", 10);
        assert_eq!(before, "hello");
        assert_eq!(after, "");
    }

    #[test]
    fn split_multibyte() {
        let (before, after) = split_at_char_col("aあb", 1);
        assert_eq!(before, "a");
        assert_eq!(after, "あb");
    }

    // --- byte_to_utf16 / utf16_to_byte roundtrip ---

    #[test]
    fn utf16_roundtrip_ascii() {
        let text = "hello";
        assert_eq!(utf16_to_byte(text, byte_to_utf16(text, 3)), 3);
    }

    #[test]
    fn utf16_roundtrip_multibyte() {
        let text = "aあb";
        // 'あ' is U+3042 (> 0xFFFF? no, it's in BMP so 1 UTF-16 unit)
        assert_eq!(byte_to_utf16(text, 4), 2); // "a" + "あ" = 2 UTF-16 units
        assert_eq!(utf16_to_byte(text, 2), 4); // byte 4 = start of 'b'
    }

    #[test]
    fn utf16_surrogate_pair() {
        let text = "a🎉b"; // 🎉 = U+1F389, surrogate pair (2 UTF-16 units)
        assert_eq!(byte_to_utf16(text, 1), 1); // "a"
        assert_eq!(byte_to_utf16(text, 5), 3); // "a" + 2 for surrogate
        assert_eq!(utf16_to_byte(text, 3), 5); // byte 5 = start of 'b'
    }
}
