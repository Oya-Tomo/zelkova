use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub ui: UiColors,
    #[serde(default)]
    pub editor: EditorColors,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiColors {
    #[serde(default = "default_bg")]
    pub bg: String,
    #[serde(default = "default_sidebar_bg")]
    pub sidebar_bg: String,
    #[serde(default = "default_border")]
    pub border: String,
    #[serde(default = "default_text")]
    pub text: String,
    #[serde(default = "default_text_dim")]
    pub text_dim: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorColors {
    #[serde(default = "default_heading_fg")]
    pub heading_fg: String,
    #[serde(default = "default_heading_marker")]
    pub heading_marker: String,
    #[serde(default = "default_list_marker")]
    pub list_marker: String,
    #[serde(default = "default_code_bg")]
    pub code_bg: String,
    #[serde(default = "default_code_fg")]
    pub code_fg: String,
    #[serde(default = "default_link_fg")]
    pub link_fg: String,
    #[serde(default = "default_image_marker")]
    pub image_marker: String,
    #[serde(default = "default_quote_fg")]
    pub quote_fg: String,
    #[serde(default = "default_quote_border")]
    pub quote_border: String,
    #[serde(default = "default_math_fg")]
    pub math_fg: String,
    #[serde(default = "default_strikethrough_fg")]
    pub strikethrough_fg: String,
    #[serde(default = "default_bold_fg")]
    pub bold_fg: String,
    #[serde(default = "default_italic_fg")]
    pub italic_fg: String,
    #[serde(default = "default_bold_weight")]
    pub bold_weight: u32,
    #[serde(default = "default_text_dim")]
    pub text_dim: String,
    #[serde(default = "default_code_keyword")]
    pub code_keyword: String,
    #[serde(default = "default_code_function")]
    pub code_function: String,
    #[serde(default = "default_code_string")]
    pub code_string: String,
    #[serde(default = "default_code_number")]
    pub code_number: String,
    #[serde(default = "default_code_comment")]
    pub code_comment: String,
    #[serde(default = "default_code_type")]
    pub code_type: String,
    #[serde(default = "default_code_constant")]
    pub code_constant: String,
    #[serde(default = "default_code_operator")]
    pub code_operator: String,
    #[serde(default = "default_code_property")]
    pub code_property: String,
    #[serde(default = "default_code_tag")]
    pub code_tag: String,
    #[serde(default = "default_code_punctuation")]
    pub code_punctuation: String,
    #[serde(default = "default_code_attribute")]
    pub code_attribute: String,
}

fn default_bg() -> String {
    "#1e1e2e".into()
}
fn default_sidebar_bg() -> String {
    "#181825".into()
}
fn default_border() -> String {
    "#313244".into()
}
fn default_text() -> String {
    "#cdd6f4".into()
}
fn default_text_dim() -> String {
    "#a6adc8".into()
}
fn default_heading_fg() -> String {
    "#89b4fa".into()
}
fn default_heading_marker() -> String {
    "#89b4fa".into()
}
fn default_list_marker() -> String {
    "#f9e2af".into()
}
fn default_code_bg() -> String {
    "#313244".into()
}
fn default_code_fg() -> String {
    "#a6e3a1".into()
}
fn default_link_fg() -> String {
    "#89b4fa".into()
}
fn default_image_marker() -> String {
    "#7f849c".into()
}
fn default_quote_fg() -> String {
    "#9399b2".into()
}
fn default_quote_border() -> String {
    "#585b70".into()
}
fn default_math_fg() -> String {
    "#ffffff".into()
}
fn default_strikethrough_fg() -> String {
    "#7f849c".into()
}
fn default_bold_fg() -> String {
    "#f9e2af".into()
}
fn default_italic_fg() -> String {
    "#f5c2e7".into()
}
fn default_bold_weight() -> u32 {
    700
}
fn default_code_keyword() -> String {
    "#cba6f7".into()
}
fn default_code_function() -> String {
    "#89b4fa".into()
}
fn default_code_string() -> String {
    "#a6e3a1".into()
}
fn default_code_number() -> String {
    "#fab387".into()
}
fn default_code_comment() -> String {
    "#6c7086".into()
}
fn default_code_type() -> String {
    "#f9e2af".into()
}
fn default_code_constant() -> String {
    "#fab387".into()
}
fn default_code_operator() -> String {
    "#89dceb".into()
}
fn default_code_property() -> String {
    "#89b4fa".into()
}
fn default_code_tag() -> String {
    "#f38ba8".into()
}
fn default_code_punctuation() -> String {
    "#6c7086".into()
}
fn default_code_attribute() -> String {
    "#f9e2af".into()
}


impl Default for UiColors {
    fn default() -> Self {
        Self {
            bg: default_bg(),
            sidebar_bg: default_sidebar_bg(),
            border: default_border(),
            text: default_text(),
            text_dim: default_text_dim(),
        }
    }
}

impl Default for EditorColors {
    fn default() -> Self {
        Self {
            heading_fg: default_heading_fg(),
            heading_marker: default_heading_marker(),
            list_marker: default_list_marker(),
            code_bg: default_code_bg(),
            code_fg: default_code_fg(),
            link_fg: default_link_fg(),
            image_marker: default_image_marker(),
            quote_fg: default_quote_fg(),
            quote_border: default_quote_border(),
            math_fg: default_math_fg(),
            strikethrough_fg: default_strikethrough_fg(),
            bold_fg: default_bold_fg(),
            italic_fg: default_italic_fg(),
            bold_weight: default_bold_weight(),
            text_dim: default_text_dim(),
            code_keyword: default_code_keyword(),
            code_function: default_code_function(),
            code_string: default_code_string(),
            code_number: default_code_number(),
            code_comment: default_code_comment(),
            code_type: default_code_type(),
            code_constant: default_code_constant(),
            code_operator: default_code_operator(),
            code_property: default_code_property(),
            code_tag: default_code_tag(),
            code_punctuation: default_code_punctuation(),
            code_attribute: default_code_attribute(),
        }
    }
}

impl EditorColors {
    /// Parse "#RRGGBB" to `(r, g, b)` u8 tuple.
    pub fn parse_hex(hex: &str) -> (u8, u8, u8) {
        let hex = hex.trim_start_matches('#');
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    }
}

impl UiColors {
    pub fn parse_hex(hex: &str) -> (u8, u8, u8) {
        EditorColors::parse_hex(hex)
    }
}

impl ThemeConfig {
    pub fn load() -> Result<Self> {
        let path = Self::theme_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read theme from {}", path.display()))?;
        let theme: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse theme at {}", path.display()))?;
        Ok(theme)
    }

    pub fn theme_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("cannot determine XDG config directory")?;
        Ok(config_dir.join("zelkova").join("theme.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_is_valid() {
        let theme = ThemeConfig::default();
        assert!(theme.ui.bg.starts_with('#'));
        assert!(theme.editor.heading_fg.starts_with('#'));
        assert_eq!(theme.editor.bold_weight, 700);
    }

    #[test]
    fn parse_partial_theme() {
        let toml = r##"
[ui]
bg = "#ffffff"
"##;
        let theme: ThemeConfig = toml::from_str(toml).expect("valid TOML in test");
        assert_eq!(theme.ui.bg, "#ffffff");
        assert_eq!(theme.editor.heading_fg, "#89b4fa");
    }

    #[test]
    fn roundtrip_default() {
        let theme = ThemeConfig::default();
        let toml_str = toml::to_string_pretty(&theme).expect("default theme serializes");
        let parsed: ThemeConfig = toml::from_str(&toml_str).expect("roundtrip TOML parses");
        assert_eq!(theme.ui.bg, parsed.ui.bg);
        assert_eq!(theme.editor.heading_fg, parsed.editor.heading_fg);
    }
}
