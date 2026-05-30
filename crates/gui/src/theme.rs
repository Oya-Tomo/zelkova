use anyhow::{Context, Result};
use gpui::{App, Hsla, rgba};
use gpui_component::{Theme, ThemeMode, ThemeSet};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Bundled themes — compiled into binary via include_str!
// ---------------------------------------------------------------------------

const BUNDLED_THEME_JSON: &[&str] = &[
    include_str!("../themes/adventure.json"),
    include_str!("../themes/alduin.json"),
    include_str!("../themes/asciinema.json"),
    include_str!("../themes/ayu.json"),
    include_str!("../themes/catppuccin.json"),
    include_str!("../themes/everforest.json"),
    include_str!("../themes/fahrenheit.json"),
    include_str!("../themes/gruvbox.json"),
    include_str!("../themes/harper.json"),
    include_str!("../themes/hybrid.json"),
    include_str!("../themes/jellybeans.json"),
    include_str!("../themes/kibble.json"),
    include_str!("../themes/macos-classic.json"),
    include_str!("../themes/matrix.json"),
    include_str!("../themes/mellifluous.json"),
    include_str!("../themes/molokai.json"),
    include_str!("../themes/solarized.json"),
    include_str!("../themes/spaceduck.json"),
    include_str!("../themes/tokyonight.json"),
    include_str!("../themes/twilight.json"),
];

// ---------------------------------------------------------------------------
// Markdown colors — Zelkova extension to gpui-component theme JSON
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MarkdownColorEntry {
    pub color: String,
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
    #[serde(rename = "math.foreground")]
    pub math_fg: String,
    #[serde(rename = "math.background")]
    pub math_bg: String,
    pub strikethrough: MarkdownColorEntry,
    pub bold: MarkdownColorEntry,
    #[serde(rename = "bold.marker")]
    pub bold_marker: MarkdownColorEntry,
    pub italic: MarkdownColorEntry,
    #[serde(rename = "italic.marker")]
    pub italic_marker: MarkdownColorEntry,
    pub tag: MarkdownColorEntry,
    #[serde(rename = "code.marker")]
    pub code_marker: MarkdownColorEntry,
    #[serde(rename = "math.marker")]
    pub math_marker: MarkdownColorEntry,
}

impl Default for MarkdownColors {
    fn default() -> Self {
        Self {
            heading: MarkdownColorEntry {
                color: String::new(),
            },
            heading_marker: MarkdownColorEntry {
                color: String::new(),
            },
            list_marker: MarkdownColorEntry {
                color: String::new(),
            },
            link: MarkdownColorEntry {
                color: String::new(),
            },
            image_marker: MarkdownColorEntry {
                color: String::new(),
            },
            quote: MarkdownColorEntry {
                color: String::new(),
            },
            quote_border: MarkdownColorEntry {
                color: String::new(),
            },
            code_bg: String::new(),
            code_fg: String::new(),
            math_fg: String::new(),
            math_bg: String::new(),
            strikethrough: MarkdownColorEntry {
                color: String::new(),
            },
            bold: MarkdownColorEntry {
                color: String::new(),
            },
            bold_marker: MarkdownColorEntry {
                color: String::new(),
            },
            italic: MarkdownColorEntry {
                color: String::new(),
            },
            italic_marker: MarkdownColorEntry {
                color: String::new(),
            },
            tag: MarkdownColorEntry {
                color: String::new(),
            },
            code_marker: MarkdownColorEntry {
                color: String::new(),
            },
            math_marker: MarkdownColorEntry {
                color: String::new(),
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
    pub math_fg: Hsla,
    pub math_bg: Hsla,
    pub strikethrough: Hsla,
    pub bold: Hsla,
    pub bold_marker: Hsla,
    pub italic: Hsla,
    pub italic_marker: Hsla,
    pub tag: Hsla,
    pub code_marker: Hsla,
    pub math_marker: Hsla,
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
            math_fg: white,
            math_bg: gray,
            strikethrough: rgba(0xFF888888).into(),
            bold: white,
            bold_marker: white,
            italic: white,
            italic_marker: white,
            tag: white,
            code_marker: white,
            math_marker: white,
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
    // 1. Find bundled theme by matching config value against variant names in each JSON
    tracing::info!("Loading theme: name={theme_name}, mode={mode}");
    let mut raw_json: Option<serde_json::Value> = None;
    let theme_lower = theme_name.to_lowercase();

    for json_str in BUNDLED_THEME_JSON {
        let val: serde_json::Value = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(themes) = val.get("themes").and_then(|t| t.as_array()) {
            for variant in themes {
                if let Some(name) = variant.get("name").and_then(|n| n.as_str())
                    && (name == theme_name || name.to_lowercase() == theme_lower)
                {
                    raw_json = Some(val);
                    break;
                }
            }
        }
        if raw_json.is_some() {
            break;
        }
    }

    let mut theme_set = raw_json.ok_or_else(|| anyhow::anyhow!("unknown theme: {theme_name}"))?;

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
    tracing::info!(
        "Applying theme variant: {} (mode={:?})",
        variant.name,
        variant.mode,
    );
    Theme::global_mut(cx).apply_config(&theme_config);
    tracing::info!(
        "After apply_config: bg={:?}, fg={:?}",
        Theme::global(cx).background,
        Theme::global(cx).foreground,
    );

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
    let mut resolved = ResolvedMarkdownColors {
        heading: resolve_or(raw.heading.color.as_str(), theme.foreground),
        heading_marker: resolve_or(raw.heading_marker.color.as_str(), theme.foreground),
        list_marker: resolve_or(raw.list_marker.color.as_str(), theme.foreground),
        link: resolve_or(raw.link.color.as_str(), theme.primary),
        image_marker: resolve_or(raw.image_marker.color.as_str(), theme.muted_foreground),
        quote: resolve_or(raw.quote.color.as_str(), theme.muted_foreground),
        quote_border: resolve_or(raw.quote_border.color.as_str(), theme.border),
        code_bg,
        code_fg: resolve_or(raw.code_fg.as_str(), theme.foreground),
        math_fg: resolve_or(raw.math_fg.as_str(), theme.foreground),
        math_bg: resolve_or(raw.math_bg.as_str(), code_bg),
        strikethrough: resolve_or(raw.strikethrough.color.as_str(), theme.muted_foreground),
        bold: resolve_or(raw.bold.color.as_str(), theme.foreground),
        bold_marker: theme.foreground,
        italic: resolve_or(raw.italic.color.as_str(), theme.foreground),
        italic_marker: theme.foreground,
        tag: resolve_or(raw.tag.color.as_str(), theme.primary),
        code_marker: theme.foreground,
        math_marker: theme.foreground,
    };
    let bold_color = resolved.bold;
    let italic_color = resolved.italic;
    let code_fg_color = resolved.code_fg;
    let math_fg_color = resolved.math_fg;
    resolved.bold_marker = resolve_or(raw.bold_marker.color.as_str(), bold_color);
    resolved.italic_marker = resolve_or(raw.italic_marker.color.as_str(), italic_color);
    resolved.code_marker = resolve_or(raw.code_marker.color.as_str(), code_fg_color);
    resolved.math_marker = resolve_or(raw.math_marker.color.as_str(), math_fg_color);
    resolved
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

/// Convert an Hsla color to a "#RRGGBB" hex string.
pub fn hsla_to_hex(color: Hsla) -> String {
    let rgba: gpui::Rgba = color.into();
    let r = (rgba.r * 255.0) as u8;
    let g = (rgba.g * 255.0) as u8;
    let b = (rgba.b * 255.0) as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
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
