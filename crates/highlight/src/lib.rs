mod theme;

use std::ops::Range;
use once_cell::sync::Lazy;
use tree_sitter_highlight::{HighlightConfiguration, Highlighter as TsHighlighter, HighlightEvent};

pub use theme::CodeTheme;

static CONFIGS: Lazy<std::collections::HashMap<&'static str, HighlightConfiguration>> =
    Lazy::new(|| {
        let mut map = std::collections::HashMap::new();

        if let Ok(mut cfg) = rust_config() {
            let _ = cfg.configure(theme::HIGHLIGHT_NAMES);
            map.insert("rust", cfg);
        }
        if let Ok(mut cfg) = javascript_config() {
            let _ = cfg.configure(theme::HIGHLIGHT_NAMES);
            map.insert("javascript", cfg);
        }
        if let Ok(mut cfg) = python_config() {
            let _ = cfg.configure(theme::HIGHLIGHT_NAMES);
            map.insert("python", cfg);
        }
        if let Ok(mut cfg) = go_config() {
            let _ = cfg.configure(theme::HIGHLIGHT_NAMES);
            map.insert("go", cfg);
        }
        if let Ok(mut cfg) = c_config() {
            let _ = cfg.configure(theme::HIGHLIGHT_NAMES);
            map.insert("c", cfg);
        }

        map
    });

fn rust_config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {
    HighlightConfiguration::new(
        tree_sitter_rust::LANGUAGE.into(),
        "rust",
        tree_sitter_rust::HIGHLIGHTS_QUERY,
        tree_sitter_rust::INJECTIONS_QUERY,
        "",
    )
}

fn javascript_config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {
    HighlightConfiguration::new(
        tree_sitter_javascript::LANGUAGE.into(),
        "javascript",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::INJECTIONS_QUERY,
        tree_sitter_javascript::LOCALS_QUERY,
    )
}

fn python_config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {
    HighlightConfiguration::new(
        tree_sitter_python::LANGUAGE.into(),
        "python",
        tree_sitter_python::HIGHLIGHTS_QUERY,
        "",
        "",
    )
}

fn go_config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {
    HighlightConfiguration::new(
        tree_sitter_go::LANGUAGE.into(),
        "go",
        tree_sitter_go::HIGHLIGHTS_QUERY,
        "",
        "",
    )
}

fn c_config() -> Result<HighlightConfiguration, tree_sitter::QueryError> {
    HighlightConfiguration::new(
        tree_sitter_c::LANGUAGE.into(),
        "c",
        tree_sitter_c::HIGHLIGHT_QUERY,
        "",
        "",
    )
}

/// Resolve a fence info string (e.g. "rust", "js", "typescript") to a known language key.
pub fn resolve_language(info: &str) -> Option<&'static str> {
    let lower = info.trim().to_lowercase();
    match lower.as_str() {
        "rust" | "rs" => Some("rust"),
        "javascript" | "js" => Some("javascript"),
        "typescript" | "ts" => Some("javascript"),
        "python" | "py" => Some("python"),
        "go" | "golang" => Some("go"),
        "c" | "h" => Some("c"),
        _ => None,
    }
}

/// A styled range for rendering code.
#[derive(Debug, Clone)]
pub struct StyledRange {
    pub range: Range<usize>,
    pub highlight_index: usize,
}

/// Highlight a code block, returning styled ranges.
///
/// `code` is the raw source text inside the fenced block.
/// `language` should be a resolved key from `resolve_language()`.
pub fn highlight_code(
    code: &str,
    language: &str,
) -> Vec<StyledRange> {
    let config = match CONFIGS.get(language) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut highlighter = TsHighlighter::new();
    let events = match highlighter.highlight(config, code.as_bytes(), None, |lang| {
        CONFIGS.get(lang)
    }) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut ranges = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for event in events.flatten() {
        match event {
            HighlightEvent::HighlightStart(h) => {
                stack.push(h.0);
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                if start < end {
                    if let Some(&hi) = stack.last() {
                        ranges.push(StyledRange {
                            range: start..end,
                            highlight_index: hi,
                        });
                    }
                }
            }
        }
    }

    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_known_languages() {
        assert_eq!(resolve_language("rust"), Some("rust"));
        assert_eq!(resolve_language("rs"), Some("rust"));
        assert_eq!(resolve_language("js"), Some("javascript"));
        assert_eq!(resolve_language("typescript"), Some("javascript"));
        assert_eq!(resolve_language("ts"), Some("javascript"));
        assert_eq!(resolve_language("python"), Some("python"));
        assert_eq!(resolve_language("py"), Some("python"));
        assert_eq!(resolve_language("go"), Some("go"));
        assert_eq!(resolve_language("golang"), Some("go"));
        assert_eq!(resolve_language("c"), Some("c"));
    }

    #[test]
    fn resolve_unknown_language() {
        assert_eq!(resolve_language("brainfuck"), None);
        assert_eq!(resolve_language(""), None);
    }

    #[test]
    fn highlight_rust_code() {
        let code = "fn main() { let x = 42; }";
        let ranges = highlight_code(code, "rust");
        assert!(!ranges.is_empty());
        // Should have at least keyword (fn), function (main), keyword (let), number (42)
        let indices: Vec<usize> = ranges.iter().map(|r| r.highlight_index).collect();
        assert!(indices.contains(&4), "should have keyword highlight");
    }

    #[test]
    fn highlight_python_code() {
        let code = "def hello():\n    print('world')";
        let ranges = highlight_code(code, "python");
        assert!(!ranges.is_empty());
    }

    #[test]
    fn highlight_unknown_language_returns_empty() {
        let code = "some code";
        let ranges = highlight_code(code, "unknown");
        assert!(ranges.is_empty());
    }

    #[test]
    fn highlight_empty_code() {
        let ranges = highlight_code("", "rust");
        assert!(ranges.is_empty());
    }

    #[test]
    fn code_theme_color_by_index() {
        let theme = CodeTheme::default();
        assert!(theme.color_by_index(0).is_some());
        assert!(theme.color_by_index(11).is_some());
        assert!(theme.color_by_index(12).is_none());
    }
}
