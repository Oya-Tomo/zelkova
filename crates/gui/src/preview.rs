use std::path::PathBuf;

use gpui::{
    App, Context, FocusHandle, Focusable, HighlightStyle, Hsla, IntoElement, Render, ScrollHandle,
    SharedString, StyledText, Window, div, img, prelude::*, px,
};
use gpui_component::ActiveTheme;
use gpui_component::scroll::{Scrollbar, ScrollbarAxis};
use zelkova_highlight::{CodeTheme, highlight_code, resolve_language};
use zelkova_markdown::{Block, Inline, ListMarker, MarkdownDoc, TableAlign, parse};
use zelkova_math_render::MathRenderer;

use crate::theme::{ResolvedMarkdownColors, hsla_to_hex};

/// GPUI `text_sm()` = 0.875rem = 14px at default 16px root.
const PREVIEW_TEXT_SIZE: f32 = 14.0;
/// Block math display multiplier — renders larger than body text for readability.
const BLOCK_MATH_SCALE: f32 = 1.8;

pub struct Preview {
    doc: MarkdownDoc,
    focus_handle: FocusHandle,
    file_path: Option<PathBuf>,
    math_renderer: MathRenderer,
    scroll_handle: ScrollHandle,
    wrap: bool,
}

impl Preview {
    #[allow(dead_code)]
    pub fn new(cx: &mut App) -> Self {
        let fg_hex = hsla_to_hex(gpui_component::Theme::global(cx).foreground);
        let math_renderer = MathRenderer::new(PREVIEW_TEXT_SIZE, &fg_hex);
        Self {
            doc: MarkdownDoc {
                frontmatter: None,
                blocks: Vec::new(),
            },
            focus_handle: cx.focus_handle(),
            file_path: None,
            math_renderer,
            scroll_handle: ScrollHandle::new(),
            wrap: true,
        }
    }

    pub fn from_markdown(text: &str, file_path: Option<PathBuf>, cx: &mut App) -> Self {
        let fg_hex = hsla_to_hex(gpui_component::Theme::global(cx).foreground);
        let math_renderer = MathRenderer::new(PREVIEW_TEXT_SIZE, &fg_hex);
        let doc = parse(text);
        let mut preview = Self {
            doc,
            focus_handle: cx.focus_handle(),
            file_path,
            math_renderer,
            scroll_handle: ScrollHandle::new(),
            wrap: true,
        };
        preview.prerender_math();
        preview
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    pub fn update_content(&mut self, text: &str) {
        self.doc = parse(text);
        self.prerender_math();
    }

    /// Pre-render all math expressions in the document to populate the SVG cache.
    fn prerender_math(&mut self) {
        fn prerender_block(block: &Block, renderer: &mut MathRenderer) {
            match block {
                Block::MathBlock { content } => {
                    if renderer.render_block(content).is_none() {
                        eprintln!("warning: failed to pre-render block math: {content}");
                    }
                }
                Block::Paragraph(inlines)
                | Block::Heading {
                    children: inlines, ..
                } => {
                    prerender_inlines(inlines, renderer);
                }
                Block::List { items } => {
                    for item in items {
                        prerender_inlines(&item.children, renderer);
                        for sub in &item.sub_items {
                            prerender_inlines(&sub.children, renderer);
                        }
                    }
                }
                Block::BlockQuote(blocks) => {
                    for b in blocks {
                        prerender_block(b, renderer);
                    }
                }
                Block::FootnoteDefinition { content, .. } => {
                    for b in content {
                        prerender_block(b, renderer);
                    }
                }
                _ => {}
            }
        }

        fn prerender_inlines(inlines: &[Inline], renderer: &mut MathRenderer) {
            for inline in inlines {
                match inline {
                    Inline::Math(content) => {
                        if renderer.render_inline(content).is_none() {
                            eprintln!("warning: failed to pre-render inline math: {content}");
                        }
                    }
                    Inline::Bold(children)
                    | Inline::Italic(children)
                    | Inline::Strikethrough(children) => {
                        prerender_inlines(children, renderer);
                    }
                    Inline::Link { text, .. } => {
                        prerender_inlines(text, renderer);
                    }
                    _ => {}
                }
            }
        }

        for block in &self.doc.blocks {
            prerender_block(block, &mut self.math_renderer);
        }
    }
}

/// Resolved preview colors derived from Theme + MarkdownColors.
struct PreviewColors {
    text: Hsla,
    heading_fg: Hsla,
    code_bg: Hsla,
    code_fg: Hsla,
    text_dim: Hsla,
    list_marker: Hsla,
    link_fg: Hsla,
    strikethrough_fg: Hsla,
    quote_fg: Hsla,
    quote_border: Hsla,
    comment_fg: Hsla,
    border: Hsla,
    math_color: Hsla,
    math_bg: Hsla,
}

impl PreviewColors {
    fn new(theme: &gpui_component::Theme, md: &ResolvedMarkdownColors) -> Self {
        Self {
            text: theme.foreground,
            heading_fg: md.heading,
            code_bg: md.code_bg,
            code_fg: md.code_fg,
            text_dim: theme.muted_foreground,
            list_marker: md.list_marker,
            link_fg: md.link,
            strikethrough_fg: theme.muted_foreground,
            quote_fg: md.quote,
            quote_border: md.quote_border,
            comment_fg: theme.muted_foreground,
            border: theme.border,
            math_color: theme.foreground,
            math_bg: md.math_bg,
        }
    }
}

impl Focusable for Preview {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Preview {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let md = ResolvedMarkdownColors::global(cx);
        let colors = PreviewColors::new(&theme, md);
        let file_path = self.file_path.clone();
        let math_renderer = &self.math_renderer;
        let text = colors.text;
        let wrap = self.wrap;
        let children: Vec<_> = self
            .doc
            .blocks
            .iter()
            .map(|block| render_block(block, &colors, file_path.as_deref(), math_renderer, wrap))
            .collect();

        // Calculate max content width across ALL block types for horizontal scroll.
        let max_content_width = if !wrap {
            let char_width = 7.2_f32;
            estimate_max_content_width(&self.doc.blocks, char_width)
        } else {
            0.0
        };

        // Build explicit scroll_size for the Scrollbar component.
        // When content width is estimated, provide it to the Scrollbar so
        // it can render the horizontal thumb even if GPUI's max_offset.x
        // lags behind the actual overflow.
        let handle = &self.scroll_handle;
        let v_content = handle.max_offset().height + handle.bounds().size.height;
        let scroll_size = if !wrap && max_content_width > 0.0 {
            gpui::size(px(max_content_width), v_content)
        } else {
            gpui::size(handle.bounds().size.width, v_content)
        };

        // Content div — identical pattern to Editor content_element.
        let content_div = div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .when(!wrap, |el| {
                el.items_start().min_w(px(max_content_width)).pb(px(16.0))
            })
            .children(children);

        let scrollbar_axis = if self.wrap {
            ScrollbarAxis::Vertical
        } else {
            ScrollbarAxis::Both
        };

        // Exact same structure as Editor scroll:
        //   root: size_full.flex.flex_col.overflow_hidden
        //     scroll_container: flex_1.relative.overflow_hidden
        //       absolute wrapper: absolute.size_full
        //         scroll_area: size_full.overflow_scroll
        //         scrollbar overlay: absolute(top_0 left_0 right_0 bottom_0)
        div()
            .id("preview-scroll")
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .track_focus(&self.focus_handle)
            .child(
                div().flex_1().relative().overflow_hidden().child(
                    div()
                        .absolute()
                        .size_full()
                        .child(
                            div()
                                .id("preview-scroll-area")
                                .size_full()
                                .when(self.wrap, |el| el.overflow_y_scroll())
                                .when(!self.wrap, |el| el.overflow_scroll())
                                .track_scroll(&self.scroll_handle)
                                .p(px(16.0))
                                .child(content_div),
                        )
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .child(
                                    Scrollbar::new(&self.scroll_handle)
                                        .id("preview-scrollbar")
                                        .axis(scrollbar_axis)
                                        .scroll_size(scroll_size),
                                ),
                        ),
                ),
            )
            .text_color(text)
            .text_sm()
    }
}

fn render_block(
    block: &Block,
    colors: &PreviewColors,
    note_path: Option<&std::path::Path>,
    math_renderer: &MathRenderer,
    wrap: bool,
) -> gpui::AnyElement {
    match block {
        Block::Heading { level, children } => {
            let font_size = match level {
                1 => 28.0,
                2 => 24.0,
                3 => 20.0,
                4 => 18.0,
                5 => 16.0,
                _ => 14.0,
            };
            let text = inline_to_string(children);
            div()
                .mt(px(if *level == 1 { 0.0 } else { 12.0 }))
                .mb(px(6.0))
                .text_size(px(font_size))
                .text_color(colors.heading_fg)
                .font_weight(gpui::FontWeight::BOLD)
                .when(!wrap, |el| el.whitespace_nowrap())
                .child(text)
                .into_any_element()
        }
        Block::Paragraph(inlines) => {
            let rendered = render_inlines(inlines, colors, note_path, math_renderer, wrap);
            div()
                .mb(px(8.0))
                .flex()
                .flex_row()
                .when(wrap, |el| el.flex_wrap())
                .when(!wrap, |el| el.flex_nowrap())
                .children(rendered)
                .into_any_element()
        }
        Block::CodeBlock { language, code } => {
            let lang_label = language.clone().unwrap_or_default();

            let styled = match language.as_deref() {
                Some(lang) if !lang.is_empty() => {
                    let code_theme = CodeTheme::default();
                    match resolve_language(lang) {
                        Some(resolved) => highlight_code(code, resolved)
                            .into_iter()
                            .filter_map(|sr| {
                                code_theme
                                    .color_by_index(sr.highlight_index)
                                    .map(|hex| (sr.range, crate::editor::parse_hex(hex)))
                            })
                            .collect::<Vec<_>>(),
                        None => Vec::new(),
                    }
                }
                _ => Vec::new(),
            };

            // Render code line by line (like Editor) so whitespace_nowrap works
            // and horizontal scroll can detect content overflow.
            let mut line_elements: Vec<gpui::AnyElement> = Vec::new();
            let mut byte_offset = 0usize;
            for line in code.lines() {
                let line_end = byte_offset + line.len();
                let line_highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = styled
                    .iter()
                    .filter(|(range, _)| range.end > byte_offset && range.start < line_end)
                    .map(|(range, color)| {
                        let clamp_start = range.start.max(byte_offset) - byte_offset;
                        let clamp_end = range.end.min(line_end) - byte_offset;
                        (
                            clamp_start..clamp_end,
                            HighlightStyle {
                                color: Some(*color),
                                ..Default::default()
                            },
                        )
                    })
                    .collect();

                let line_text = if line.is_empty() {
                    " ".to_string()
                } else {
                    line.to_string()
                };
                line_elements.push(
                    div()
                        .when(!wrap, |el| el.whitespace_nowrap())
                        .child(
                            StyledText::new(SharedString::from(line_text))
                                .with_highlights(line_highlights),
                        )
                        .into_any_element(),
                );
                byte_offset = line_end + 1; // +1 for '\n'
            }

            div()
                .mb(px(8.0))
                .bg(colors.code_bg)
                .rounded(px(4.0))
                .p(px(8.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_dim)
                        .mb(px(4.0))
                        .child(lang_label),
                )
                .children(line_elements)
                .into_any_element()
        }
        Block::List { items } => {
            let children: Vec<_> = items
                .iter()
                .map(|item| render_list_item(item, 0, colors, note_path, math_renderer, wrap))
                .collect();
            div()
                .mb(px(8.0))
                .flex()
                .flex_col()
                .children(children)
                .into_any_element()
        }
        Block::BlockQuote(blocks) => {
            let children: Vec<_> = blocks
                .iter()
                .map(|b| render_block(b, colors, note_path, math_renderer, wrap))
                .collect();
            div()
                .mb(px(8.0))
                .pl(px(12.0))
                .border_l_2()
                .border_color(colors.quote_border)
                .text_color(colors.quote_fg)
                .flex()
                .flex_col()
                .children(children)
                .into_any_element()
        }
        Block::Table {
            headers,
            aligns,
            rows,
        } => render_table(headers, aligns, rows, colors, wrap),
        Block::ThematicBreak => div()
            .my(px(12.0))
            .w_full()
            .h(px(1.0))
            .bg(colors.border)
            .into_any_element(),
        Block::MathBlock { content } => {
            let cached = math_renderer.get_block(content);
            match cached {
                Some(math_img) => {
                    let display_h =
                        math_renderer.font_size() * BLOCK_MATH_SCALE * math_img.em_height;
                    div()
                        .mb(px(8.0))
                        .bg(colors.math_bg)
                        .rounded(px(4.0))
                        .p(px(8.0))
                        .flex()
                        .justify_center()
                        .child(
                            img(math_img.path.clone())
                                .object_fit(gpui::ObjectFit::Contain)
                                .h(px(display_h)),
                        )
                        .into_any_element()
                }
                None => div()
                    .mb(px(8.0))
                    .bg(colors.math_bg)
                    .rounded(px(4.0))
                    .p(px(8.0))
                    .text_color(colors.math_color)
                    .child(content.clone())
                    .into_any_element(),
            }
        }
        Block::HtmlBlock { content } => div()
            .mb(px(8.0))
            .text_color(colors.text_dim)
            .when(!wrap, |el| el.whitespace_nowrap())
            .child(content.clone())
            .into_any_element(),
        Block::FootnoteDefinition { label, content } => {
            let blocks: Vec<_> = content
                .iter()
                .map(|b| render_block(b, colors, note_path, math_renderer, wrap))
                .collect();
            div()
                .mb(px(4.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(colors.link_fg)
                        .child(format!("[^{label}]:")),
                )
                .children(blocks)
                .into_any_element()
        }
    }
}

fn render_list_item(
    item: &zelkova_markdown::ListItem,
    depth: usize,
    colors: &PreviewColors,
    note_path: Option<&std::path::Path>,
    math_renderer: &MathRenderer,
    wrap: bool,
) -> gpui::AnyElement {
    let marker_text = match &item.marker {
        ListMarker::Dash => "- ".to_string(),
        ListMarker::Plus => "+ ".to_string(),
        ListMarker::Star => "* ".to_string(),
        ListMarker::Number(n) => format!("{n}. "),
    };

    let inline = render_inlines(&item.children, colors, note_path, math_renderer, wrap);
    let sub_children: Vec<_> = item
        .sub_items
        .iter()
        .map(|sub| render_list_item(sub, depth + 1, colors, note_path, math_renderer, wrap))
        .collect();

    div()
        .pl(px(depth as f32 * 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_row()
                .when(wrap, |el| el.flex_wrap())
                .when(!wrap, |el| el.flex_nowrap())
                .child(div().text_color(colors.list_marker).child(marker_text))
                .children(inline),
        )
        .children(sub_children)
        .into_any_element()
}

fn render_table(
    headers: &[Vec<Inline>],
    _aligns: &[Option<TableAlign>],
    rows: &[Vec<Vec<Inline>>],
    colors: &PreviewColors,
    wrap: bool,
) -> gpui::AnyElement {
    let col_count = headers.len().max(1);

    let mut table_div = div()
        .mb(px(8.0))
        .flex()
        .flex_col()
        .border_1()
        .border_color(colors.border)
        .rounded(px(4.0))
        .when(!wrap, |el| el.whitespace_nowrap());

    // Header row
    let header_cells: Vec<_> = headers
        .iter()
        .map(|h| {
            let text = inline_to_string(h);
            div()
                .flex()
                .flex_1()
                .p(px(6.0))
                .bg(colors.code_bg)
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(colors.text)
                .when(!wrap, |el| el.whitespace_nowrap())
                .child(text)
        })
        .collect();
    table_div = table_div.child(div().flex().flex_row().w_full().children(header_cells));

    // Data rows
    for row in rows {
        let cells: Vec<_> = (0..col_count)
            .map(|col| {
                let text = row
                    .get(col)
                    .map(|inline| inline_to_string(inline))
                    .unwrap_or_default();
                div()
                    .flex()
                    .flex_1()
                    .p(px(6.0))
                    .border_t_1()
                    .border_color(colors.border)
                    .text_color(colors.text)
                    .when(!wrap, |el| el.whitespace_nowrap())
                    .child(text)
            })
            .collect();
        table_div = table_div.child(div().flex().flex_row().w_full().children(cells));
    }

    table_div.into_any_element()
}

fn render_inlines(
    inlines: &[Inline],
    colors: &PreviewColors,
    note_path: Option<&std::path::Path>,
    math_renderer: &MathRenderer,
    wrap: bool,
) -> Vec<gpui::AnyElement> {
    inlines
        .iter()
        .map(|inline| render_inline(inline, colors, note_path, math_renderer, wrap))
        .collect()
}

fn render_inline(
    inline: &Inline,
    colors: &PreviewColors,
    note_path: Option<&std::path::Path>,
    math_renderer: &MathRenderer,
    wrap: bool,
) -> gpui::AnyElement {
    match inline {
        Inline::Text(t) => div()
            .when(!wrap, |el| el.whitespace_nowrap())
            .child(t.clone())
            .into_any_element(),
        Inline::Bold(children) => div()
            .font_weight(gpui::FontWeight::BOLD)
            .when(!wrap, |el| el.whitespace_nowrap())
            .children(render_inlines(
                children,
                colors,
                note_path,
                math_renderer,
                wrap,
            ))
            .into_any_element(),
        Inline::Italic(children) => div()
            .italic()
            .when(!wrap, |el| el.whitespace_nowrap())
            .children(render_inlines(
                children,
                colors,
                note_path,
                math_renderer,
                wrap,
            ))
            .into_any_element(),
        Inline::Strikethrough(children) => div()
            .line_through()
            .text_color(colors.strikethrough_fg)
            .when(!wrap, |el| el.whitespace_nowrap())
            .children(render_inlines(
                children,
                colors,
                note_path,
                math_renderer,
                wrap,
            ))
            .into_any_element(),
        Inline::Code(code) => div()
            .bg(colors.code_bg)
            .rounded(px(3.0))
            .px(px(4.0))
            .text_color(colors.code_fg)
            .when(!wrap, |el| el.whitespace_nowrap())
            .child(code.clone())
            .into_any_element(),
        Inline::Link { text, url: _, .. } => div()
            .text_color(colors.link_fg)
            .underline()
            .cursor(gpui::CursorStyle::PointingHand)
            .when(!wrap, |el| el.whitespace_nowrap())
            .child(inline_to_string(text))
            .into_any_element(),
        Inline::Image { alt, url, .. } => {
            let resolved = crate::editor::util::resolve_image_path(note_path, url);
            if resolved.exists() {
                let _alt_text = alt.clone();
                div()
                    .flex()
                    .flex_col()
                    .child(
                        img(resolved)
                            .object_fit(gpui::ObjectFit::Contain)
                            .max_h(px(300.0)),
                    )
                    .into_any_element()
            } else {
                div()
                    .py(px(4.0))
                    .px(px(8.0))
                    .rounded_md()
                    .bg(colors.code_bg)
                    .text_xs()
                    .text_color(colors.comment_fg)
                    .child(format!("[image not found: {url}]"))
                    .into_any_element()
            }
        }
        Inline::Math(content) => {
            let cached = math_renderer.get_inline(content);
            match cached {
                Some(math_img) => div()
                    .child(
                        img(math_img.path.clone())
                            .object_fit(gpui::ObjectFit::Contain)
                            .max_h(px(20.0)),
                    )
                    .into_any_element(),
                None => div()
                    .text_color(colors.math_color)
                    .child(content.clone())
                    .into_any_element(),
            }
        }
        Inline::FootnoteRef(label) => div()
            .text_color(colors.link_fg)
            .text_xs()
            .child(format!("[^{label}]"))
            .into_any_element(),
        Inline::HtmlTag(tag) => div()
            .text_color(colors.text_dim)
            .text_xs()
            .child(tag.clone())
            .into_any_element(),
        Inline::HardBreak => div().h(px(8.0)).into_any_element(),
        Inline::SoftBreak => div().child(" ").into_any_element(),
    }
}

fn inline_to_string(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Text(t) => t.clone(),
            Inline::Bold(c) | Inline::Italic(c) | Inline::Strikethrough(c) => inline_to_string(c),
            Inline::Code(code) => code.clone(),
            Inline::Link { text, .. } => inline_to_string(text),
            Inline::Image { alt, .. } => alt.clone(),
            Inline::Math(content) => content.clone(),
            Inline::FootnoteRef(label) => format!("[^{label}]"),
            Inline::HtmlTag(tag) => tag.clone(),
            Inline::HardBreak => "\n".to_string(),
            Inline::SoftBreak => " ".to_string(),
        })
        .collect()
}

fn estimate_max_content_width(blocks: &[Block], char_width: f32) -> f32 {
    let mut max_w = 0.0_f32;
    for block in blocks {
        max_w = max_w.max(estimate_block_width(block, char_width));
    }
    max_w
}

fn estimate_block_width(block: &Block, char_width: f32) -> f32 {
    match block {
        Block::CodeBlock { code, .. } => code
            .lines()
            .map(|l| l.chars().count() as f32 * char_width)
            .fold(0.0_f32, f32::max),
        Block::Heading {
            children, level, ..
        } => {
            let text = inline_to_string(children);
            let scale = match level {
                1 => 2.0,
                2 => 1.7,
                3 => 1.43,
                4 => 1.29,
                5 => 1.14,
                _ => 1.0,
            };
            text.chars().count() as f32 * char_width * scale
        }
        Block::Paragraph(inlines) => {
            let text = inline_to_string(inlines);
            text.chars().count() as f32 * char_width
        }
        Block::List { items } => items
            .iter()
            .map(|item| {
                let text = inline_to_string(&item.children);
                text.chars().count() as f32 * char_width + 32.0
            })
            .fold(0.0_f32, f32::max),
        Block::Table { headers, rows, .. } => {
            let header_w: f32 = headers
                .iter()
                .map(|h| inline_to_string(h).chars().count() as f32 * char_width)
                .sum();
            let mut max_row_w = header_w;
            for row in rows {
                let row_w: f32 = row
                    .iter()
                    .map(|cells| inline_to_string(cells).chars().count() as f32 * char_width)
                    .sum();
                max_row_w = max_row_w.max(row_w);
            }
            max_row_w
        }
        Block::BlockQuote(inner_blocks) => {
            estimate_max_content_width(inner_blocks, char_width) + 32.0
        }
        Block::FootnoteDefinition { content, .. } => {
            estimate_max_content_width(content, char_width) + 32.0
        }
        Block::MathBlock { content } => content.chars().count() as f32 * char_width,
        Block::HtmlBlock { content } => content.chars().count() as f32 * char_width,
        Block::ThematicBreak => 0.0,
    }
}
