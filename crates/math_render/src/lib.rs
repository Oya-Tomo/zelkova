use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::math_style::MathStyle;

/// Renders LaTeX math expressions to PNG temp files with caching.
pub struct MathRenderer {
    cache: HashMap<String, PathBuf>,
    render_opts: RenderOptions,
}

impl MathRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            render_opts: RenderOptions {
                font_size: 24.0,
                padding: 4.0,
                background_color: ratex_types::color::Color::new(0.0, 0.0, 0.0, 0.0),
                font_dir: String::new(),
                device_pixel_ratio: 2.0,
            },
        }
    }

    /// Render block math to PNG file. Returns cached path or None on parse failure.
    pub fn render_block(&mut self, latex: &str) -> Option<PathBuf> {
        self.render(latex, false)
    }

    /// Get cached PNG path for block math.
    pub fn get_block(&self, latex: &str) -> Option<&PathBuf> {
        self.cache.get(&format!("b:{}", latex))
    }

    /// Render inline math to PNG file. Returns cached path or None on parse failure.
    pub fn render_inline(&mut self, latex: &str) -> Option<PathBuf> {
        self.render(latex, true)
    }

    /// Get cached PNG path for inline math.
    pub fn get_inline(&self, latex: &str) -> Option<&PathBuf> {
        self.cache.get(&format!("i:{}", latex))
    }

    fn render(&mut self, latex: &str, inline: bool) -> Option<PathBuf> {
        let key = if inline {
            format!("i:{}", latex)
        } else {
            format!("b:{}", latex)
        };
        if let Some(cached) = self.cache.get(&key) {
            return Some(cached.clone());
        }
        let style = if inline {
            MathStyle::Text
        } else {
            MathStyle::Display
        };
        let layout_opts = LayoutOptions::default().with_style(style);
        let ast = parse(latex).ok()?;
        let lbox = layout(&ast, &layout_opts);
        let display_list = to_display_list(&lbox);
        let png_bytes = render_to_png(&display_list, &self.render_opts).ok()?;
        let path = write_png_to_temp(&png_bytes, &key);
        self.cache.insert(key, path.clone());
        Some(path)
    }
}

impl Default for MathRenderer {
    fn default() -> Self {
        Self::new()
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
        let mut renderer = MathRenderer::new();
        let path = renderer.render_block(r"\frac{1}{2}");
        assert!(path.is_some());
        assert!(path.expect("path").exists());
    }

    #[test]
    fn render_inline_x() {
        let mut renderer = MathRenderer::new();
        let path = renderer.render_inline("x^2 + y^2 = z^2");
        assert!(path.is_some());
    }

    #[test]
    fn cache_hit() {
        let mut renderer = MathRenderer::new();
        let _ = renderer.render_block("E = mc^2");
        let path2 = renderer.render_block("E = mc^2");
        assert!(path2.is_some());
        assert_eq!(renderer.cache.len(), 1);
    }

    #[test]
    fn invalid_latex_returns_none() {
        let mut renderer = MathRenderer::new();
        let result = renderer.render_block(r"\frac{");
        assert!(result.is_none());
    }
}
