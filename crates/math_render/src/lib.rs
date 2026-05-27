use std::collections::HashMap;

use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_svg::{render_to_svg, SvgOptions};
use ratex_types::math_style::MathStyle;

/// Renders LaTeX math expressions to SVG strings with caching.
pub struct MathRenderer {
    cache: HashMap<String, String>,
    svg_opts: SvgOptions,
}

impl MathRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            svg_opts: SvgOptions {
                font_size: 20.0,
                padding: 4.0,
                stroke_width: 0.8,
                embed_glyphs: true,
                font_dir: String::new(),
            },
        }
    }

    /// Render a block math expression ($$...$$). Returns SVG string or None on parse failure.
    pub fn render_block(&mut self, latex: &str) -> Option<String> {
        self.render(latex, false)
    }

    /// Get a cached SVG for block math. Must have been pre-rendered via `render_block`.
    pub fn get_block(&self, latex: &str) -> Option<&str> {
        self.cache.get(&format!("b:{}", latex)).map(|s| s.as_str())
    }

    /// Get a cached SVG for inline math. Must have been pre-rendered via `render_inline`.
    pub fn get_inline(&self, latex: &str) -> Option<&str> {
        self.cache.get(&format!("i:{}", latex)).map(|s| s.as_str())
    }
    pub fn render_inline(&mut self, latex: &str) -> Option<String> {
        self.render(latex, true)
    }

    fn render(&mut self, latex: &str, inline: bool) -> Option<String> {
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
        let svg = render_to_svg(&display_list, &self.svg_opts);
        self.cache.insert(key, svg.clone());
        Some(svg)
    }
}

impl Default for MathRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_fraction() {
        let mut renderer = MathRenderer::new();
        let svg = renderer.render_block(r"\frac{1}{2}");
        assert!(svg.is_some());
        assert!(svg.expect("svg").contains("<svg"));
    }

    #[test]
    fn render_inline_x() {
        let mut renderer = MathRenderer::new();
        let svg = renderer.render_inline("x^2 + y^2 = z^2");
        assert!(svg.is_some());
    }

    #[test]
    fn cache_hit() {
        let mut renderer = MathRenderer::new();
        let _ = renderer.render_block("E = mc^2");
        let svg2 = renderer.render_block("E = mc^2");
        assert!(svg2.is_some());
        assert_eq!(renderer.cache.len(), 1);
    }

    #[test]
    fn invalid_latex_returns_none() {
        let mut renderer = MathRenderer::new();
        let result = renderer.render_block(r"\frac{");
        assert!(result.is_none());
    }
}
