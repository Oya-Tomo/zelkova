use anyhow::{Context, Result};
use gpui::{App, Hsla, rgba};
use gpui_component::{Theme, ThemeMode, ThemeSet};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Bundled themes — compiled into binary via include_str!
// ---------------------------------------------------------------------------

const BUNDLED_THEME_JSON: &[(&str, &str)] = &[
    ("adventure", include_str!("../themes/adventure.json")),
    ("alduin", include_str!("../themes/alduin.json")),
    ("asciinema", include_str!("../themes/asciinema.json")),
    ("ayu", include_str!("../themes/ayu.json")),
    ("catppuccin", include_str!("../themes/catppuccin.json")),
    ("everforest", include_str!("../themes/everforest.json")),
    ("fahrenheit", include_str!("../themes/fahrenheit.json")),
    ("gruvbox", include_str!("../themes/gruvbox.json")),
    ("harper", include_str!("../themes/harper.json")),
    ("hybrid", include_str!("../themes/hybrid.json")),
    ("jellybeans", include_str!("../themes/jellybeans.json")),
    ("kibble", include_str!("../themes/kibble.json")),
    (
        "macos-classic",
        include_str!("../themes/macos-classic.json"),
    ),
    ("matrix", include_str!("../themes/matrix.json")),
    ("mellifluous", include_str!("../themes/mellifluous.json")),
    ("molokai", include_str!("../themes/molokai.json")),
    ("solarized", include_str!("../themes/solarized.json")),
    ("spaceduck", include_str!("../themes/spaceduck.json")),
    ("tokyonight", include_str!("../themes/tokyonight.json")),
    ("twilight", include_str!("../themes/twilight.json")),
];

// ---------------------------------------------------------------------------
// Markdown colors — Zelkova extension to gpui-component theme JSON
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MarkdownColorEntry {
    pub color: String,
    #[serde(default)]
    pub font_weight: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MarkdownColors {
    pub heading: MarkdownColorEntry,
    #[serde(rename = "heading.marker")]
    pub heading_marker: MarkdownColorEntry,
    #[serde(rename = "list.marker")]
    pub list_marker: MarkdownColorEntry,
    pub link: MarkdownColorEntry,
    #[serde(rename = "image.marker")]
    pub image_marker: MarkdownColorEntry,
    pub quote: MarkdownColorEntry,
    #[serde(rename = "quote.border")]
    pub quote_border: MarkdownColorEntry,
    #[serde(rename = "code.background")]
    pub code_bg: String,
    #[serde(rename = "code.foreground")]
    pub code_fg: String,
    #[serde(rename = "math.color")]
    pub math_color: String,
    #[serde(rename = "math.background")]
    pub math_bg: String,
    pub strikethrough: MarkdownColorEntry,
    pub bold: MarkdownColorEntry,
    pub italic: MarkdownColorEntry,
    pub tag: MarkdownColorEntry,
}

impl Default for MarkdownColors {
    fn default() -> Self {
        Self {
            heading: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            heading_marker: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            list_marker: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            link: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            image_marker: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            quote: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            quote_border: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            code_bg: String::new(),
            code_fg: String::new(),
            math_color: String::new(),
            math_bg: String::new(),
            strikethrough: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            bold: MarkdownColorEntry {
                color: String::new(),
                font_weight: Some(700),
            },
            italic: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
            tag: MarkdownColorEntry {
                color: String::new(),
                font_weight: None,
            },
        }
    }
}

/// Resolved markdown colors as Hsla values, ready for rendering.
/// Stored as a GPUI Global so any component can access via `ResolvedMarkdownColors::global(cx)`.
#[derive(Debug, Clone)]
pub struct ResolvedMarkdownColors {
    pub heading: Hsla,
    pub heading_marker: Hsla,
    pub list_marker: Hsla,
    pub link: Hsla,
    pub image_marker: Hsla,
    pub quote: Hsla,
    pub quote_border: Hsla,
    pub code_bg: Hsla,
    pub code_fg: Hsla,
    pub math_color: Hsla,
    pub math_bg: Hsla,
    pub strikethrough: Hsla,
    pub bold: Hsla,
    pub bold_weight: u32,
    pub italic: Hsla,
    pub tag: Hsla,
}

impl Default for ResolvedMarkdownColors {
    fn default() -> Self {
        let white: Hsla = rgba(0xFFFFFFFF).into();
        let gray: Hsla = rgba(0xFF333333).into();
        Self {
            heading: white,
            heading_marker: white,
            list_marker: white,
            link: white,
            image_marker: white,
            quote: white,
            quote_border: white,
            code_bg: gray,
            code_fg: white,
            math_color: white,
            math_bg: gray,
            strikethrough: rgba(0xFF888888).into(),
            bold: white,
            bold_weight: 700,
            italic: white,
            tag: white,
        }
    }
}

impl gpui::Global for ResolvedMarkdownColors {}

impl ResolvedMarkdownColors {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }
}

// ---------------------------------------------------------------------------
// Theme loading
// ---------------------------------------------------------------------------

/// Load and apply theme from config. Returns resolved markdown colors.
pub fn load_theme(
    theme_name: &str,
    mode: &str,
    override_path: Option<&str>,
    cx: &mut App,
) -> Result<ResolvedMarkdownColors> {
    // 1. Load bundled theme JSON
    let raw_json = BUNDLED_THEME_JSON
        .iter()
        .find(|(name, _)| *name == theme_name)
        .map(|(_, json)| *json)
        .ok_or_else(|| anyhow::anyhow!("unknown theme: {theme_name}"))?;

    let mut theme_set: serde_json::Value = serde_json::from_str(raw_json)
        .with_context(|| format!("failed to parse bundled theme: {theme_name}"))?;

    // 2. Apply override if specified
    if let Some(rel_path) = override_path {
        let config_dir = dirs::config_dir().context("cannot determine XDG config directory")?;
        let override_full = config_dir.join("zelkova").join(rel_path);
        if override_full.exists() {
            let override_content = std::fs::read_to_string(&override_full)
                .with_context(|| format!("failed to read override: {}", override_full.display()))?;
            let override_set: serde_json::Value = serde_json::from_str(&override_content)
                .with_context(|| {
                    format!("failed to parse override: {}", override_full.display())
                })?;
            merge_theme_json(&mut theme_set, &override_set);
        }
    }

    // 3. Parse ThemeSet and find the matching mode variant
    let parsed_set: ThemeSet =
        serde_json::from_value(theme_set.clone()).context("failed to parse theme set")?;

    let theme_mode = match mode {
        "light" => ThemeMode::Light,
        _ => ThemeMode::Dark,
    };

    let variant = parsed_set
        .themes
        .iter()
        .find(|t| t.mode == theme_mode)
        .or_else(|| parsed_set.themes.first())
        .context("theme has no variants")?;

    // 4. Apply to gpui-component Theme system
    let theme_config = std::rc::Rc::new(variant.clone());
    Theme::global_mut(cx).apply_config(&theme_config);

    // 5. Extract and resolve markdown colors
    let markdown_raw = extract_markdown_from_json(&theme_set, &variant.name.to_string());
    let resolved = resolve_markdown_colors(&markdown_raw, &Theme::global(cx));

    // 6. Store as global for component access
    cx.set_global(resolved.clone());

    Ok(resolved)
}

/// Extract the "markdown" section from the theme JSON for the active variant.
fn extract_markdown_from_json(theme_set: &serde_json::Value, variant_name: &str) -> MarkdownColors {
    let themes = match theme_set.get("themes") {
        Some(t) => t,
        None => return MarkdownColors::default(),
    };
    let themes_arr = match themes.as_array() {
        Some(a) => a,
        None => return MarkdownColors::default(),
    };

    for variant in themes_arr {
        if variant.get("name").and_then(|v| v.as_str()) == Some(variant_name) {
            if let Some(md) = variant.get("markdown") {
                if let Ok(colors) = serde_json::from_value::<MarkdownColors>(md.clone()) {
                    return colors;
                }
            }
        }
    }

    MarkdownColors::default()
}

/// Resolve markdown colors, applying fallbacks for missing fields.
fn resolve_markdown_colors(raw: &MarkdownColors, theme: &Theme) -> ResolvedMarkdownColors {
    let code_bg = resolve_or(raw.code_bg.as_str(), theme.muted);
    ResolvedMarkdownColors {
        heading: resolve_or(raw.heading.color.as_str(), theme.foreground),
        heading_marker: resolve_or(raw.heading_marker.color.as_str(), theme.foreground),
        list_marker: resolve_or(raw.list_marker.color.as_str(), theme.foreground),
        link: resolve_or(raw.link.color.as_str(), theme.primary),
        image_marker: resolve_or(raw.image_marker.color.as_str(), theme.muted_foreground),
        quote: resolve_or(raw.quote.color.as_str(), theme.muted_foreground),
        quote_border: resolve_or(raw.quote_border.color.as_str(), theme.border),
        code_bg,
        code_fg: resolve_or(raw.code_fg.as_str(), theme.foreground),
        math_color: resolve_or(raw.math_color.as_str(), theme.foreground),
        math_bg: resolve_or(raw.math_bg.as_str(), code_bg),
        strikethrough: resolve_or(raw.strikethrough.color.as_str(), theme.muted_foreground),
        bold: resolve_or(raw.bold.color.as_str(), theme.foreground),
        bold_weight: raw.bold.font_weight.unwrap_or(700),
        italic: resolve_or(raw.italic.color.as_str(), theme.foreground),
        tag: resolve_or(raw.tag.color.as_str(), theme.primary),
    }
}

fn resolve_or(hex: &str, fallback: Hsla) -> Hsla {
    try_parse_hex(hex).unwrap_or(fallback)
}

pub fn try_parse_hex(hex: &str) -> Option<Hsla> {
    let hex = hex.trim();
    if hex.is_empty() {
        return None;
    }
    let rgba = gpui::Rgba::try_from(hex).ok()?;
    Some(rgba.into())
}

/// Merge override JSON into base theme set (shallow merge at top level).
fn merge_theme_json(base: &mut serde_json::Value, override_val: &serde_json::Value) {
    if let (serde_json::Value::Object(base_map), serde_json::Value::Object(over_map)) =
        (base, override_val)
    {
        for (key, val) in over_map {
            base_map.insert(key.clone(), val.clone());
        }
    }
}
