use gpui::{Font, HighlightStyle, Pixels, TextRun, TextStyle};
use unicode_width::UnicodeWidthChar;

/// One visual row within a wrapped logical line.
///
/// `start_byte` / `end_byte` are UTF-8 byte offsets into the logical line's
/// text. `end_byte` is exclusive. Together they identify the substring of
/// the logical line that occupies this visual row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapSegment {
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Decide where a logical line should break into visual rows.
///
/// `x_for_index(i)` must return the pixel width from the start of the line
/// up to (but not including) byte `i`. In production this comes from
/// `ShapedLine::x_for_index`; in tests we can substitute a deterministic
/// mock (e.g. each ASCII char = 1.0 px) and exercise the boundary logic
/// without rendering.
///
/// Rules (per ADR-0002):
/// - ASCII whitespace (`' '`, `'\t'`) creates a break opportunity **after**
///   the whitespace.
/// - CJK characters (Unicode ideographic / wide ranges) each create a break
///   opportunity **after** themselves.
/// - When `wrap_width` is exceeded and a break opportunity exists in the
///   current row, break at the most recent one.
/// - When no opportunity exists (a single token longer than `wrap_width`,
///   e.g. a long URL with no whitespace), force-break at the byte where
///   `x_for_index` first exceeds `wrap_width`. UTF-8 char boundaries are
///   respected — never split a multi-byte char.
/// - An empty input returns a single empty `WrapSegment` so callers always
///   have at least one row.
///
/// Returns a non-empty `Vec<WrapSegment>` whose segments are contiguous and
/// together cover the whole `text`.
pub fn wrap_line_bytes<F>(text: &str, wrap_width: Pixels, mut x_for_index: F) -> Vec<WrapSegment>
where
    F: FnMut(usize) -> Pixels,
{
    let wrap = f32::from(wrap_width);
    let mut segments = Vec::new();
    let mut row_start = 0usize; // byte offset where the current visual row starts
    let mut last_break = 0usize; // byte offset of the most recent break opportunity (exclusive: break lands *after* this byte)
    let mut row_start_x = f32::from(x_for_index(0)); // pixel x at row_start

    let bytes = text.as_bytes();
    let mut byte_idx = 0usize;
    while byte_idx < bytes.len() {
        // Determine the length of the UTF-8 char starting at byte_idx so we
        // advance a whole codepoint at a time and never split a multi-byte
        // sequence. This is safe because `text` is a valid &str.
        let ch_len = utf8_char_len(bytes[byte_idx]);
        let next_byte_idx = byte_idx + ch_len;

        // Pixel width of [row_start, next_byte_idx) — i.e. what the row's
        // width would become if we appended this char.
        let width_including = f32::from(x_for_index(next_byte_idx)) - row_start_x;

        if width_including > wrap && byte_idx > row_start {
            // Appending this char would overflow the row. Break BEFORE this
            // char: prefer the most recent break opportunity (so ASCII lines
            // break at word boundaries); fall back to a force-break at the
            // char boundary (single long token, e.g. URL).
            let break_at = if last_break > row_start {
                last_break
            } else {
                byte_idx
            };
            segments.push(WrapSegment {
                start_byte: row_start,
                end_byte: break_at,
            });
            row_start = break_at;
            row_start_x = f32::from(x_for_index(row_start));
            // Reset last_break so we don't reuse a stale opportunity from
            // before the wrap.
            last_break = row_start;
            // Re-process this char in the new row (don't advance byte_idx).
            continue;
        }

        // Char stays in the current row. Update break opportunity AFTER
        // deciding not to break: ASCII whitespace and CJK chars create a
        // break opportunity that subsequent iterations can use.
        let ch = &text[byte_idx..next_byte_idx];
        if is_break_opportunity_after(ch) {
            last_break = next_byte_idx;
        }

        byte_idx = next_byte_idx;
    }

    // Final row: from row_start to end of text.
    segments.push(WrapSegment {
        start_byte: row_start,
        end_byte: bytes.len(),
    });

    segments
}

/// UTF-8 char length from the leading byte. Mirrors the standard bit pattern.
fn utf8_char_len(first_byte: u8) -> usize {
    if first_byte < 0x80 {
        1
    } else if first_byte >> 5 == 0b110 {
        2
    } else if first_byte >> 4 == 0b1110 {
        3
    } else if first_byte >> 3 == 0b11110 {
        4
    } else {
        // Invalid UTF-8 leading byte — &str guarantees this can't happen,
        // but fall back to 1 to avoid an infinite loop if it ever did.
        1
    }
}

/// Whether the given character (as a &str slice of length 1 char) creates a
/// break opportunity *after* itself. ASCII whitespace and wide (CJK / emoji)
/// characters both qualify.
///
/// Wide-character detection uses `unicode-width` so we follow the same
/// Unicode tables as the rest of the Rust ecosystem, rather than
/// hand-maintaining codepoint ranges.
fn is_break_opportunity_after(ch: &str) -> bool {
    if ch == " " || ch == "\t" {
        return true;
    }
    if let Some(c) = ch.chars().next() {
        // Width 2 = full-width / wide character (CJK ideographs, emoji, etc.).
        // Per CJK typography these break after each character.
        c.width() == Some(2)
    } else {
        false
    }
}

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

/// Build a `TextRun` array suitable for `TextSystem::shape_line`, by
/// layering the Editor's highlight list on top of the inherited text style.
#[allow(dead_code)]
///
/// `base` is typically `window.text_style()` — so Heading rows that set
/// `text_2xl()` etc. on the parent div automatically pick up the larger
/// font here.
///
/// `text_len` is the UTF-8 byte length of the line text. Highlights that
/// fall outside `[0, text_len)` are clamped.
///
/// Later highlights paint over earlier ones, matching `StyledText`'s
/// `with_highlights` semantics.
pub fn build_runs_from_highlights(
    base: &TextStyle,
    text_len: usize,
    highlights: &[(std::ops::Range<usize>, HighlightStyle)],
) -> Vec<TextRun> {
    let base_run = TextRun {
        len: text_len,
        font: Font {
            family: base.font_family.clone(),
            weight: base.font_weight,
            style: base.font_style,
            features: base.font_features.clone(),
            fallbacks: base.font_fallbacks.clone(),
        },
        color: base.color,
        background_color: base.background_color,
        underline: base.underline,
        strikethrough: None,
    };

    if highlights.is_empty() || text_len == 0 {
        return vec![base_run];
    }

    // Collect transition points (byte offsets where a highlight starts
    // or ends) so we can split the line into a contiguous run list.
    let mut boundaries: Vec<usize> = vec![0, text_len];
    for (range, _) in highlights {
        if range.start > 0 && range.start < text_len {
            boundaries.push(range.start);
        }
        if range.end > 0 && range.end < text_len {
            boundaries.push(range.end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut runs = Vec::new();
    for win in boundaries.windows(2) {
        let start = win[0];
        let end = win[1];
        if start >= end {
            continue;
        }
        let mut run = base_run.clone();
        run.len = end - start;
        // Apply every highlight that fully covers this slice; the last one
        // in the input list wins (mirrors StyledText::with_highlights).
        for (range, style) in highlights {
            if range.start <= start && range.end >= end {
                apply_highlight_to_run(&mut run, style);
            }
        }
        runs.push(run);
    }

    if runs.is_empty() {
        vec![base_run]
    } else {
        runs
    }
}

/// Apply a `HighlightStyle` to a `TextRun`, overriding only the fields the
/// highlight specifies. Fields not set in the highlight retain the run's
/// existing value (which came from the base text style).
#[allow(dead_code)]
fn apply_highlight_to_run(run: &mut TextRun, h: &HighlightStyle) {
    if let Some(c) = h.color {
        run.color = c;
    }
    if let Some(weight) = h.font_weight {
        run.font.weight = weight;
    }
    if let Some(style) = h.font_style {
        run.font.style = style;
    }
    if let Some(bg) = h.background_color {
        run.background_color = Some(bg);
    }
    if let Some(u) = h.underline {
        run.underline = Some(u);
    }
    if let Some(s) = h.strikethrough {
        run.strikethrough = Some(s);
    }
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
    use gpui::px;

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

    // --- wrap_line_bytes tests ---

    /// Mock pixel width function: each ASCII byte = 1.0px, CJK / wide char
    /// = 2.0px (matches `UnicodeWidthChar::width`). Returns the width up to
    /// the given byte index (exclusive end), as `x_for_index` requires.
    fn mock_x(text: &str) -> impl Fn(usize) -> Pixels + '_ {
        move |i: usize| {
            let prefix = &text[..i.min(text.len())];
            let w: f32 = prefix.chars().map(|c| c.width().unwrap_or(1) as f32).sum();
            px(w)
        }
    }

    #[test]
    fn wrap_short_text_no_wrap() {
        let text = "hello";
        let segs = wrap_line_bytes(text, px(100.0), mock_x(text));
        assert_eq!(
            segs,
            vec![WrapSegment {
                start_byte: 0,
                end_byte: 5
            }]
        );
    }

    #[test]
    fn wrap_breaks_at_word_boundary() {
        // "hello world foo" with width 7.0: "hello " (6px) fits, "hello w" (7px)
        // fits exactly, "hello wo" (8px) overflows. Break after the space at
        // byte 6 → first row is "hello " (0..6).
        let text = "hello world foo";
        let segs = wrap_line_bytes(text, px(7.0), mock_x(text));
        assert_eq!(
            segs,
            vec![
                WrapSegment {
                    start_byte: 0,
                    end_byte: 6
                },
                WrapSegment {
                    start_byte: 6,
                    end_byte: 12
                },
                WrapSegment {
                    start_byte: 12,
                    end_byte: 15
                },
            ]
        );
    }

    #[test]
    fn wrap_cjk_breaks_per_character() {
        // 3 Japanese chars: each 2px wide. wrap_width = 3.0px → first row
        // fits one char (2px), second char would push to 4px > 3.0, break.
        let text = "こんにちは";
        let segs = wrap_line_bytes(text, px(3.0), mock_x(text));
        assert_eq!(
            segs,
            vec![
                WrapSegment {
                    start_byte: 0,
                    end_byte: 3
                },
                WrapSegment {
                    start_byte: 3,
                    end_byte: 6
                },
                WrapSegment {
                    start_byte: 6,
                    end_byte: 9
                },
                WrapSegment {
                    start_byte: 9,
                    end_byte: 12
                },
                WrapSegment {
                    start_byte: 12,
                    end_byte: 15
                },
            ]
        );
    }

    #[test]
    fn wrap_long_word_force_break() {
        // "abcdefghij" with width 4.0, no spaces — must force-break at the
        // byte boundary that overflows (not at a non-existent opportunity).
        let text = "abcdefghij";
        let segs = wrap_line_bytes(text, px(4.0), mock_x(text));
        // Each row is exactly 4 chars wide because there's no break
        // opportunity and the algorithm force-breaks at char boundaries.
        assert_eq!(
            segs,
            vec![
                WrapSegment {
                    start_byte: 0,
                    end_byte: 4
                },
                WrapSegment {
                    start_byte: 4,
                    end_byte: 8
                },
                WrapSegment {
                    start_byte: 8,
                    end_byte: 10
                },
            ]
        );
    }

    #[test]
    fn wrap_empty_returns_single_empty_segment() {
        let segs = wrap_line_bytes("", px(10.0), |_| px(0.0));
        assert_eq!(
            segs,
            vec![WrapSegment {
                start_byte: 0,
                end_byte: 0
            }]
        );
    }

    #[test]
    fn wrap_mixed_ascii_cjk() {
        // "ab日本" — a, b = 1px each; 日, 本 = 2px each. width=3.0
        // The algorithm prefers "don't exceed wrap_width" over "fill the
        // row greedily", so the CJK chars each go on their own row:
        //   "ab"   (2px) → fits, next char 日 would push to 4 > 3 → break.
        //   "日"   (2px) → fits, next char 本 would push to 4 > 3 → break.
        //   "本"   (2px) → final row.
        let text = "ab日本";
        let segs = wrap_line_bytes(text, px(3.0), mock_x(text));
        assert_eq!(
            segs,
            vec![
                WrapSegment {
                    start_byte: 0,
                    end_byte: 2
                },
                WrapSegment {
                    start_byte: 2,
                    end_byte: 5
                },
                WrapSegment {
                    start_byte: 5,
                    end_byte: 8
                },
            ]
        );
    }

    #[test]
    fn wrap_never_splits_multibyte_char() {
        // "aあb" with width = 0.5px (impossibly narrow) — should still
        // produce segments whose boundaries are all valid UTF-8 char starts.
        let text = "aあb";
        let segs = wrap_line_bytes(text, px(0.5), mock_x(text));
        for seg in &segs {
            // Every segment boundary must be a char boundary in the original text.
            assert!(text.is_char_boundary(seg.start_byte));
            assert!(text.is_char_boundary(seg.end_byte));
        }
        // Coverage: union of segments == full text.
        let mut covered = 0;
        for seg in &segs {
            assert_eq!(seg.start_byte, covered);
            covered = seg.end_byte;
        }
        assert_eq!(covered, text.len());
    }

    #[test]
    fn wrap_whitespace_at_row_end_goes_to_that_row() {
        // "foo bar baz" width=4: "foo " (4px) fits exactly, "foo b" (5) > 4.
        // Break opportunity after the space (byte 4). First row = "foo " (0..4).
        let text = "foo bar baz";
        let segs = wrap_line_bytes(text, px(4.0), mock_x(text));
        assert_eq!(
            segs[0],
            WrapSegment {
                start_byte: 0,
                end_byte: 4
            }
        );
    }
}
