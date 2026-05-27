use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::display_item::DisplayItem;
use ratex_types::math_style::MathStyle;

/// Rendered math image: PNG file path and total height in em units.
#[derive(Debug, Clone)]
pub struct MathImage {
    pub path: PathBuf,
    /// Total height in em (height + depth from DisplayList).
    pub em_height: f32,
}

/// Renders LaTeX math expressions to PNG temp files with caching.
pub struct MathRenderer {
    cache: HashMap<String, MathImage>,
    render_opts: RenderOptions,
    /// Hex color string used for cache key invalidation (e.g. "ffffff").
    color_hex: String,
}

impl MathRenderer {
    /// Create a new renderer.
    ///
    /// * `font_size` — base font size in logical pixels (e.g. 14.0 for GPUI text_sm).
    ///   Internally renders at `font_size * 2` pixels for HiDPI quality, then
    ///   callers display at `font_size * em_height` logical pixels.
    /// * `text_color` — hex color string for math glyphs, e.g. "#ffffff" or "ffffff".
    pub fn new(font_size: f32, text_color: &str) -> Self {
        let hex = text_color.trim_start_matches('#').to_string();
        Self {
            cache: HashMap::new(),
            render_opts: RenderOptions {
                font_size,
                padding: 4.0,
                background_color: ratex_types::color::Color::new(0.0, 0.0, 0.0, 0.0),
                font_dir: String::new(),
                device_pixel_ratio: 2.0,
            },
            color_hex: hex,
        }
    }

    /// Pre-render block math. Call during the mutable pre-render phase.
    pub fn render_block(&mut self, latex: &str) -> Option<&MathImage> {
        self.render(latex, false)
    }

    /// Get cached block math image. Call during the immutable render phase.
    pub fn get_block(&self, latex: &str) -> Option<&MathImage> {
        self.cache.get(&self.cache_key(latex, false))
    }

    /// Pre-render inline math. Call during the mutable pre-render phase.
    pub fn render_inline(&mut self, latex: &str) -> Option<&MathImage> {
        self.render(latex, true)
    }

    /// Get cached inline math image. Call during the immutable render phase.
    pub fn get_inline(&self, latex: &str) -> Option<&MathImage> {
        self.cache.get(&self.cache_key(latex, true))
    }

    /// Update the text color. Invalidates cache for future renders.
    pub fn set_text_color(&mut self, text_color: &str) {
        self.color_hex = text_color.trim_start_matches('#').to_string();
    }

    /// The logical font size in pixels (for computing display dimensions).
    pub fn font_size(&self) -> f32 {
        self.render_opts.font_size
    }

    fn cache_key(&self, latex: &str, inline: bool) -> String {
        let prefix = if inline { "i" } else { "b" };
        let fs = self.render_opts.font_size;
        format!("{}:{}:{}:{:.0}", prefix, latex, self.color_hex, fs)
    }

    fn render(&mut self, latex: &str, inline: bool) -> Option<&MathImage> {
        let key = self.cache_key(latex, inline);
        if self.cache.contains_key(&key) {
            return Some(&self.cache[&key]);
        }
        let style = if inline {
            MathStyle::Text
        } else {
            MathStyle::Display
        };
        let layout_opts = LayoutOptions::default().with_style(style);
        let ast = parse(latex).ok()?;
        let lbox = layout(&ast, &layout_opts);
        let mut display_list = to_display_list(&lbox);
        let color = parse_hex_color(&self.color_hex);
        override_colors(&mut display_list, color);
        let png_bytes = render_to_png(&display_list, &self.render_opts).ok()?;
        let path = write_png_to_temp(&png_bytes, &key);
        let em_height = (display_list.height + display_list.depth) as f32;
        self.cache.insert(key, MathImage { path, em_height });
        // Safety: just inserted, key exists.
        Some(self.cache.values().last().expect("just inserted"))
    }
}

fn parse_hex_color(hex: &str) -> ratex_types::color::Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).expect("valid hex r") as f32 / 255.0;
    let g = u8::from_str_radix(&hex[2..4], 16).expect("valid hex g") as f32 / 255.0;
    let b = u8::from_str_radix(&hex[4..6], 16).expect("valid hex b") as f32 / 255.0;
    ratex_types::color::Color::rgb(r, g, b)
}

/// Override glyph, line, and path colors to match the configured text color.
/// Rect items are skipped as they represent background fills (e.g. \colorbox).
fn override_colors(
    display_list: &mut ratex_types::display_item::DisplayList,
    color: ratex_types::color::Color,
) {
    for item in &mut display_list.items {
        match item {
            DisplayItem::GlyphPath { color: c, .. }
            | DisplayItem::Line { color: c, .. }
            | DisplayItem::Path { color: c, .. } => {
                *c = color;
            }
            DisplayItem::Rect { .. } => {}
        }
    }
}

fn write_png_to_temp(png_bytes: &[u8], key: &str) -> PathBuf {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    let dir = std::env::temp_dir().join("zelkova-math");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{hash:016x}.png"));
    if !path.exists() {
        let _ = std::fs::write(&path, png_bytes);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_fraction() {
        let mut renderer = MathRenderer::new(14.0, "#ffffff");
        let img = renderer.render_block(r"\frac{1}{2}");
        assert!(img.is_some());
        assert!(img.expect("img").path.exists());
    }

    #[test]
    fn render_inline_x() {
        let mut renderer = MathRenderer::new(14.0, "#ffffff");
        let img = renderer.render_inline("x^2 + y^2 = z^2");
        assert!(img.is_some());
        assert!(img.expect("img").em_height > 0.0);
    }

    #[test]
    fn cache_hit() {
        let mut renderer = MathRenderer::new(14.0, "#ffffff");
        let _ = renderer.render_block("E = mc^2");
        let img2 = renderer.render_block("E = mc^2");
        assert!(img2.is_some());
        assert_eq!(renderer.cache.len(), 1);
    }

    #[test]
    fn invalid_latex_returns_none() {
        let mut renderer = MathRenderer::new(14.0, "#ffffff");
        let result = renderer.render_block(r"\frac{");
        assert!(result.is_none());
    }

    #[test]
    fn color_change_invalidates_cache() {
        let mut renderer = MathRenderer::new(14.0, "#cba6f7");
        let _ = renderer.render_block("E = mc^2");
        renderer.set_text_color("#ffffff");
        let _ = renderer.render_block("E = mc^2");
        assert_eq!(renderer.cache.len(), 2);
    }
}
