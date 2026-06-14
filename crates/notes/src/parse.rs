use crate::note::Frontmatter;

/// Parse YAML frontmatter from a note file's raw content.
///
/// Returns `(Some(frontmatter), body)` when frontmatter is present and parses
/// cleanly; `(None, original_content)` otherwise. The body is trimmed of the
/// leading `---\n...\n---\n` block when frontmatter is present.
pub fn parse_frontmatter(content: &str) -> (Option<Frontmatter>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    let rest = &trimmed[3..];
    let Some(end_idx) = rest.find("---") else {
        return (None, content.to_string());
    };

    let yaml_str = &rest[..end_idx];
    let body = rest[end_idx + 3..].trim_start().to_string();

    let frontmatter: Frontmatter = match serde_yaml::from_str(yaml_str) {
        Ok(fm) => fm,
        Err(_) => return (None, content.to_string()),
    };
    (Some(frontmatter), body)
}

/// Parse optional YAML frontmatter from raw note content.
///
/// Functionally equivalent to [`parse_frontmatter`]; both are kept because
/// historical callers referred to each by name. Prefer `parse_note_content`
/// in new code — the name documents intent more clearly.
pub fn parse_note_content(raw: &str) -> (Option<Frontmatter>, String) {
    parse_frontmatter(raw)
}

/// Serialize a frontmatter + body pair back into the on-disk note format.
pub fn format_note_file(frontmatter: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

/// Extract a title from the first Markdown heading in the body.
pub fn extract_title_from_body(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn parse_frontmatter_basic() {
        let content = "---\nid: \"00000000-0000-0000-0000-000000000001\"\ntitle: Test\ntags:\n  - rust\ncreated: 2025-01-01T00:00:00Z\nupdated: 2025-01-01T00:00:00Z\n---\nHello world\n";
        let (fm, body) = parse_frontmatter(content);
        let fm = fm.expect("frontmatter should parse");
        assert_eq!(fm.title, "Test");
        assert!(fm.tags.contains("rust"));
        assert_eq!(body, "Hello world\n");
    }

    #[test]
    fn parse_frontmatter_missing_returns_none() {
        let content = "just text";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_none());
        assert_eq!(body, "just text");
    }

    #[test]
    fn parse_frontmatter_unclosed_returns_none() {
        let content = "---\nid: broken\nHello world\n";
        let (fm, body) = parse_frontmatter(content);
        assert!(fm.is_none());
        assert_eq!(body, content);
    }

    #[test]
    fn extract_title_from_heading() {
        assert_eq!(
            extract_title_from_body("# My Title\nSome text"),
            Some("My Title".to_string())
        );
        assert_eq!(
            extract_title_from_body("## Sub Title\n"),
            Some("Sub Title".to_string())
        );
        assert_eq!(extract_title_from_body("No heading here"), None);
    }

    #[test]
    fn format_roundtrip() {
        let id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("valid UUID");
        let now: chrono::DateTime<Utc> = "2025-01-01T00:00:00Z".parse().expect("valid timestamp");
        let mut tags = std::collections::HashSet::new();
        tags.insert("test".to_string());

        let fm = Frontmatter {
            id,
            title: "Round".to_string(),
            tags,
            created: now,
            updated: now,
        };

        let s = format_note_file(&fm, "body text");
        assert!(s.starts_with("---"));
        assert!(s.contains("title: Round"));
        assert!(s.contains("body text"));
    }
}
