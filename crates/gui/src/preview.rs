use gpui::{
    App, Context, FocusHandle, IntoElement, Render, SharedString, StyledText, Window, div, img,
    prelude::*, px, rgb,
};
use zelkova_config::EditorColors;
use zelkova_markdown::{Block, Inline, ListMarker, MarkdownDoc, TableAlign, parse};

pub struct Preview {
    doc: MarkdownDoc,
    theme: EditorColors,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            doc: MarkdownDoc {
                frontmatter: None,
                blocks: Vec::new(),
            },
            theme: EditorColors::default(),
        }
    }

    pub fn from_markdown(text: &str) -> Self {
        Self {
            doc: parse(text),
            theme: EditorColors::default(),
        }
    }

    pub fn set_theme(&mut self, theme: EditorColors) {
        self.theme = theme;
    }

    pub fn update_content(&mut self, text: &str) {
        self.doc = parse(text);
    }
}

impl Render for Preview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let children: Vec<_> = self
            .doc
            .blocks
            .iter()
            .map(|block| render_block(block, &self.theme))
            .collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .p(px(16.0))
            .text_color(rgb(0xcdd6f4))
            .text_sm()
            .children(children)
    }
}

fn render_block(block: &Block, theme: &EditorColors) -> gpui::AnyElement {
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
                .text_color(rgb(0x89b4fa))
                .font_weight(gpui::FontWeight::BOLD)
                .child(text)
                .into_any_element()
        }
        Block::Paragraph(inlines) => {
            let rendered = render_inlines(inlines, theme);
            div()
                .mb(px(8.0))
                .flex()
                .flex_row()
                .flex_wrap()
                .children(rendered)
                .into_any_element()
        }
        Block::CodeBlock { language, code } => {
            let lang_label = language.clone().unwrap_or_default();
            let code_text = SharedString::from(code.to_string());
            div()
                .mb(px(8.0))
                .bg(rgb(0x313244))
                .rounded(px(4.0))
                .p(px(8.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xa6adc8))
                        .mb(px(4.0))
                        .child(lang_label),
                )
                .child(StyledText::new(code_text).with_highlights(vec![]))
                .into_any_element()
        }
        Block::List { items } => {
            let children: Vec<_> = items
                .iter()
                .map(|item| render_list_item(item, 0, theme))
                .collect();
            div()
                .mb(px(8.0))
                .flex()
                .flex_col()
                .children(children)
                .into_any_element()
        }
        Block::BlockQuote(blocks) => {
            let children: Vec<_> = blocks.iter().map(|b| render_block(b, theme)).collect();
            div()
                .mb(px(8.0))
                .pl(px(12.0))
                .border_l_2()
                .border_color(rgb(0x585b70))
                .text_color(rgb(0x9399b2))
                .flex()
                .flex_col()
                .children(children)
                .into_any_element()
        }
        Block::Table {
            headers,
            aligns,
            rows,
        } => render_table(headers, aligns, rows, theme),
        Block::ThematicBreak => div()
            .my(px(12.0))
            .w_full()
            .h(px(1.0))
            .bg(rgb(0x313244))
            .into_any_element(),
        Block::MathBlock { content } => div()
            .mb(px(8.0))
            .bg(rgb(0x313244))
            .rounded(px(4.0))
            .p(px(8.0))
            .text_color(rgb(0xcba6f7))
            .child(content.clone())
            .into_any_element(),
        Block::HtmlBlock { content } => div()
            .mb(px(8.0))
            .text_color(rgb(0xa6adc8))
            .child(content.clone())
            .into_any_element(),
        Block::FootnoteDefinition { label, content } => {
            let blocks: Vec<_> = content.iter().map(|b| render_block(b, theme)).collect();
            div()
                .mb(px(4.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(rgb(0x89b4fa))
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
    theme: &EditorColors,
) -> gpui::AnyElement {
    let marker_text = match &item.marker {
        ListMarker::Dash => "- ".to_string(),
        ListMarker::Plus => "+ ".to_string(),
        ListMarker::Star => "* ".to_string(),
        ListMarker::Number(n) => format!("{n}. "),
    };

    let inline = render_inlines(&item.children, theme);
    let sub_children: Vec<_> = item
        .sub_items
        .iter()
        .map(|sub| render_list_item(sub, depth + 1, theme))
        .collect();

    div()
        .pl(px(depth as f32 * 16.0))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_row()
                .child(div().text_color(rgb(0xf9e2af)).child(marker_text))
                .children(inline),
        )
        .children(sub_children)
        .into_any_element()
}

fn render_table(
    headers: &[Vec<Inline>],
    _aligns: &[Option<TableAlign>],
    rows: &[Vec<Vec<Inline>>],
    theme: &EditorColors,
) -> gpui::AnyElement {
    let col_count = headers.len().max(1);

    let mut table_div = div()
        .mb(px(8.0))
        .flex()
        .flex_col()
        .border_1()
        .border_color(rgb(0x313244))
        .rounded(px(4.0));

    // Header row
    let header_cells: Vec<_> = headers
        .iter()
        .map(|h| {
            let text = inline_to_string(h);
            div()
                .flex()
                .flex_1()
                .p(px(6.0))
                .bg(rgb(0x313244))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xcdd6f4))
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
                    .border_color(rgb(0x313244))
                    .text_color(rgb(0xcdd6f4))
                    .child(text)
            })
            .collect();
        table_div = table_div.child(div().flex().flex_row().w_full().children(cells));
    }

    table_div.into_any_element()
}

fn render_inlines(inlines: &[Inline], theme: &EditorColors) -> Vec<gpui::AnyElement> {
    inlines
        .iter()
        .map(|inline| render_inline(inline, theme))
        .collect()
}

fn render_inline(inline: &Inline, theme: &EditorColors) -> gpui::AnyElement {
    match inline {
        Inline::Text(t) => div().child(t.clone()).into_any_element(),
        Inline::Bold(children) => div()
            .font_weight(gpui::FontWeight::BOLD)
            .children(render_inlines(children, theme))
            .into_any_element(),
        Inline::Italic(children) => div()
            .italic()
            .children(render_inlines(children, theme))
            .into_any_element(),
        Inline::Strikethrough(children) => div()
            .line_through()
            .text_color(rgb(0x7f849c))
            .children(render_inlines(children, theme))
            .into_any_element(),
        Inline::Code(code) => div()
            .bg(rgb(0x313244))
            .rounded(px(3.0))
            .px(px(4.0))
            .text_color(rgb(0xa6e3a1))
            .child(code.clone())
            .into_any_element(),
        Inline::Link { text, url, .. } => div()
            .text_color(rgb(0x89b4fa))
            .underline()
            .cursor(gpui::CursorStyle::PointingHand)
            .child(inline_to_string(text))
            .into_any_element(),
        Inline::Image { alt, url, .. } => {
            let img_url = SharedString::from(url.to_string());
            let alt_text = alt.clone();
            div()
                .flex()
                .flex_col()
                .child(
                    img(img_url)
                        .object_fit(gpui::ObjectFit::Contain)
                        .max_h(px(300.0)),
                )
                .child(div().text_xs().text_color(rgb(0xa6adc8)).child(alt_text))
                .into_any_element()
        }
        Inline::Math(content) => div()
            .text_color(rgb(0xcba6f7))
            .child(content.clone())
            .into_any_element(),
        Inline::FootnoteRef(label) => div()
            .text_color(rgb(0x89b4fa))
            .text_xs()
            .child(format!("[^{label}]"))
            .into_any_element(),
        Inline::HtmlTag(tag) => div()
            .text_color(rgb(0xa6adc8))
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
