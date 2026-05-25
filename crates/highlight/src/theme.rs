/// Highlight class names used across all language grammars.
/// These must be a superset of the captures used by each grammar.
/// Index corresponds to the Highlight(usize) value returned by tree-sitter-highlight.
pub static HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "function",
    "keyword",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
];

/// Theme colors for code highlighting.
/// Each field maps to a HIGHLIGHT_NAMES entry by index.
#[derive(Debug, Clone)]
pub struct CodeTheme {
    pub attribute: String,
    pub comment: String,
    pub constant: String,
    pub function: String,
    pub keyword: String,
    pub number: String,
    pub operator: String,
    pub property: String,
    pub punctuation: String,
    pub string: String,
    pub tag: String,
    pub r#type: String,
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self {
            attribute: "#f9e2af".into(),    // yellow
            comment: "#6c7086".into(),      // dim gray
            constant: "#fab387".into(),     // peach
            function: "#89b4fa".into(),     // blue
            keyword: "#cba6f7".into(),      // mauve
            number: "#fab387".into(),       // peach
            operator: "#89dceb".into(),     // teal
            property: "#89b4fa".into(),     // blue
            punctuation: "#6c7086".into(),  // dim gray
            string: "#a6e3a1".into(),       // green
            tag: "#f38ba8".into(),          // red
            r#type: "#f9e2af".into(),       // yellow
        }
    }
}

impl CodeTheme {
    /// Construct from EditorColors code-specific fields.
    pub fn from_editor_colors(colors: &zelkova_config::EditorColors) -> Self {
        Self {
            attribute: colors.code_attribute.clone(),
            comment: colors.code_comment.clone(),
            constant: colors.code_constant.clone(),
            function: colors.code_function.clone(),
            keyword: colors.code_keyword.clone(),
            number: colors.code_number.clone(),
            operator: colors.code_operator.clone(),
            property: colors.code_property.clone(),
            punctuation: colors.code_punctuation.clone(),
            string: colors.code_string.clone(),
            tag: colors.code_tag.clone(),
            r#type: colors.code_type.clone(),
        }
    }

    /// Get color string by highlight index.
    pub fn color_by_index(&self, index: usize) -> Option<&str> {
        match index {
            0 => Some(&self.attribute),
            1 => Some(&self.comment),
            2 => Some(&self.constant),
            3 => Some(&self.function),
            4 => Some(&self.keyword),
            5 => Some(&self.number),
            6 => Some(&self.operator),
            7 => Some(&self.property),
            8 => Some(&self.punctuation),
            9 => Some(&self.string),
            10 => Some(&self.tag),
            11 => Some(&self.r#type),
            _ => None,
        }
    }
}
