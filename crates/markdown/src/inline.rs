use crate::ast::Inline;

/// Parse inline elements from a text string.
/// Handles: **bold**, *italic*, ~~strikethrough~~, `code`, [link](url), ![image](url), $math$, [^ref], <html>, hard/soft breaks.
pub fn parse_inline(text: &str) -> Vec<Inline> {
    let mut result = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Hard break (two spaces + newline or backslash + newline)
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '\n' {
            result.push(Inline::HardBreak);
            i += 2;
            continue;
        }
        if chars[i] == ' ' && i + 1 < chars.len() && chars[i] == ' ' && text[i..].starts_with("  \n") {
            result.push(Inline::HardBreak);
            i += 3;
            continue;
        }

        // Soft break
        if chars[i] == '\n' {
            result.push(Inline::SoftBreak);
            i += 1;
            continue;
        }

        // Bold (** or __)
        let is_bold_marker = (chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*')
            || (chars[i] == '_' && i + 1 < chars.len() && chars[i + 1] == '_');
        if is_bold_marker {
            let marker = chars[i];
            if let Some(end) = find_closing_double(&chars, i + 2, marker) {
                let inner: String = chars[i + 2..end].iter().collect();
                let children = parse_inline(&inner);
                result.push(Inline::Bold(children));
                i = end + 2;
                continue;
            }
        }

        // Strikethrough (~~)
        if i + 1 < chars.len() && chars[i] == '~' && chars[i + 1] == '~' {
            if let Some(end) = find_closing_double(&chars, i + 2, '~') {
                let inner: String = chars[i + 2..end].iter().collect();
                let children = parse_inline(&inner);
                result.push(Inline::Strikethrough(children));
                i = end + 2;
                continue;
            }
        }

        // Italic (* or _)
        if chars[i] == '*' || chars[i] == '_' {
            let marker = chars[i];
            if let Some(end) = find_closing_single(&chars, i + 1, marker) {
                let inner: String = chars[i + 1..end].iter().collect();
                let children = parse_inline(&inner);
                result.push(Inline::Italic(children));
                i = end + 1;
                continue;
            }
        }

        // Code span (` or ``)
        if chars[i] == '`' {
            let backtick_count = count_backticks(&chars, i);
            if let Some(end) = find_closing_backticks(&chars, i + backtick_count, backtick_count) {
                let code: String = chars[i + backtick_count..end].iter().collect();
                result.push(Inline::Code(code));
                i = end + backtick_count;
                continue;
            }
        }

        // Image (![alt](url))
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((alt, url, end)) = parse_link_or_image(&chars, i + 2, true) {
                result.push(Inline::Image { alt, url, title: None });
                i = end;
                continue;
            }
        }

        // Link ([text](url))
        if chars[i] == '[' {
            if let Some((text, url, end)) = parse_link_or_image(&chars, i + 1, false) {
                let children = parse_inline(&text);
                result.push(Inline::Link { text: children, url, title: None });
                i = end;
                continue;
            }
        }

        // Math ($...$)
        if chars[i] == '$' {
            if let Some(end) = find_closing_single(&chars, i + 1, '$') {
                let math: String = chars[i + 1..end].iter().collect();
                result.push(Inline::Math(math));
                i = end + 1;
                continue;
            }
        }

        // Footnote ref ([^label])
        if chars[i] == '[' && i + 1 < chars.len() && chars[i + 1] == '^' {
            if let Some(end) = find_closing_bracket(&chars, i + 2) {
                let label: String = chars[i + 2..end].iter().collect();
                result.push(Inline::FootnoteRef(label));
                i = end + 1;
                continue;
            }
        }

        // HTML tag
        if chars[i] == '<' {
            if let Some(end) = find_tag_end(&chars, i) {
                let tag: String = chars[i..end].iter().collect();
                result.push(Inline::HtmlTag(tag));
                i = end;
                continue;
            }
        }

        // Plain text — collect until next special char
        let start = i;
        while i < chars.len() {
            let c = chars[i];
            if c == '*' || c == '_' || c == '`' || c == '[' || c == '!' || c == '$' || c == '~' || c == '<' || c == '\n' || c == '\\' {
                break;
            }
            i += 1;
        }
        if i > start {
            let text: String = chars[start..i].iter().collect();
            result.push(Inline::Text(text));
        } else {
            // unknown char, treat as text
            result.push(Inline::Text(chars[i].to_string()));
            i += 1;
        }
    }

    result
}

fn find_closing_double(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_single(chars: &[char], start: usize, marker: char) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == marker {
            return Some(i);
        }
    }
    None
}

fn count_backticks(chars: &[char], start: usize) -> usize {
    chars[start..].iter().take_while(|&&c| c == '`').count()
}

fn find_closing_backticks(chars: &[char], start: usize, count: usize) -> Option<usize> {
    let mut i = start;
    while i + count <= chars.len() {
        let all_backticks = chars[i..i + count].iter().all(|&c| c == '`');
        if all_backticks {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_link_or_image(chars: &[char], start: usize, _is_image: bool) -> Option<(String, String, usize)> {
    // find closing ]
    let mut i = start;
    while i < chars.len() && chars[i] != ']' { i += 1; }
    if i >= chars.len() { return None; }
    let text: String = chars[start..i].iter().collect();
    i += 1; // skip ]

    // expect (
    if i >= chars.len() || chars[i] != '(' { return None; }
    i += 1;

    // find closing )
    let url_start = i;
    while i < chars.len() && chars[i] != ')' { i += 1; }
    if i >= chars.len() { return None; }
    let url: String = chars[url_start..i].iter().collect();
    i += 1; // skip )

    Some((text, url, i))
}

fn find_closing_bracket(chars: &[char], start: usize) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == ']' { return Some(i); }
    }
    None
}

fn find_tag_end(chars: &[char], start: usize) -> Option<usize> {
    for i in start..chars.len() {
        if chars[i] == '>' { return Some(i + 1); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let result = parse_inline("hello world");
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Inline::Text(t) if t == "hello world"));
    }

    #[test]
    fn parse_bold() {
        let result = parse_inline("**bold**");
        assert!(matches!(&result[0], Inline::Bold(children) if children.len() == 1));
    }

    #[test]
    fn parse_italic() {
        let result = parse_inline("*italic*");
        assert!(matches!(&result[0], Inline::Italic(_)));
    }

    #[test]
    fn parse_code() {
        let result = parse_inline("`code`");
        assert!(matches!(&result[0], Inline::Code(c) if c == "code"));
    }

    #[test]
    fn parse_link() {
        let result = parse_inline("[text](http://example.com)");
        assert!(matches!(&result[0], Inline::Link { text, url, .. } if url == "http://example.com"));
    }

    #[test]
    fn parse_image() {
        let result = parse_inline("![alt](image.png)");
        assert!(matches!(&result[0], Inline::Image { alt, url, .. } if alt == "alt" && url == "image.png"));
    }

    #[test]
    fn parse_strikethrough() {
        let result = parse_inline("~~deleted~~");
        assert!(matches!(&result[0], Inline::Strikethrough(_)));
    }

    #[test]
    fn parse_math() {
        let result = parse_inline("$E=mc^2$");
        assert!(matches!(&result[0], Inline::Math(m) if m == "E=mc^2"));
    }

    #[test]
    fn parse_mixed() {
        let result = parse_inline("hello **bold** and *italic* world");
        assert!(result.len() >= 5);
    }

    #[test]
    fn parse_footnote_ref() {
        let result = parse_inline("[^label]");
        assert!(matches!(&result[0], Inline::FootnoteRef(l) if l == "label"));
    }
}
