use gpui::{
    Context, FontWeight, HighlightStyle, SharedString, StyledText, div, img, prelude::*, px,
};

use super::{EditZone, Editor};
use crate::editor::util::{
    adjust_highlight_offsets, char_idx_to_byte, overlay_selection, resolve_image_path,
    split_at_char_col,
};
use crate::editor::{
    HighlightedLine, ResolvedColors, detect_line_context, highlight_fence_line, highlight_line,
};

impl Editor {
    pub(super) fn render_frontmatter_header(
        &self,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let colors = &self.resolved_colors;
        let mut children = Vec::new();

        let Some(fm) = &self.frontmatter else {
            return children;
        };

        let title = fm.title.clone();
        let tags: Vec<String> = fm.tags.iter().cloned().collect();
        let created = fm.created.format("%Y-%m-%d %H:%M").to_string();
        let updated = fm.updated.format("%Y-%m-%d %H:%M").to_string();

        let is_title_zone = self.edit_zone == EditZone::Title;
        let tc = self.title_cursor;
        let title_for_cursor = title.clone();
        let title_div = {
            let mut container = div()
                .w_full()
                .py(px(4.0))
                .flex()
                .flex_row()
                .items_center()
                .cursor(gpui::CursorStyle::IBeam)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(move |this, _ev, _window, cx| {
                        this.edit_zone = EditZone::Title;
                        this.selection = None;
                        this.title_cursor = this
                            .frontmatter
                            .as_ref()
                            .map(|f| f.title.chars().count())
                            .unwrap_or(0);
                        cx.notify();
                    }),
                );

            if is_title_zone {
                let (before, after) = split_at_char_col(&title_for_cursor, tc);
                container = container
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(colors.text)
                            .child(before),
                    )
                    .child(div().w(px(2.0)).h(px(24.0)).bg(colors.text).flex_shrink_0())
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(colors.text)
                            .child(if after.is_empty() {
                                " ".to_string()
                            } else {
                                after
                            }),
                    );
            } else {
                let title_color = if title.is_empty() {
                    colors.text_dim
                } else {
                    colors.text
                };
                container = container.child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(title_color)
                        .child(if title.is_empty() {
                            "Untitled".to_string()
                        } else {
                            title
                        }),
                );
            }
            container
        };
        children.push(title_div.into_any_element());

        let mut tag_elements = Vec::new();
        for tag in &tags {
            let tag_for_remove = tag.clone();
            tag_elements.push(
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded_md()
                    .bg(colors.selection_bg)
                    .text_color(colors.tag_fg)
                    .text_xs()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(tag.clone())
                    .child(
                        div()
                            .cursor(gpui::CursorStyle::PointingHand)
                            .text_color(colors.text_muted)
                            .child("x")
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(move |this, _ev, _window, cx| {
                                    this.remove_tag(&tag_for_remove);
                                    cx.notify();
                                }),
                            ),
                    )
                    .into_any_element(),
            );
        }

        let is_tag_zone = self.edit_zone == EditZone::TagInput;
        if is_tag_zone {
            let input_text = self.tag_input.clone();
            let tc = self.tag_input_cursor;
            let (before, after) = split_at_char_col(&input_text, tc);
            children.push(
                div()
                    .w_full()
                    .py(px(4.0))
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded_md()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.bg)
                            .text_xs()
                            .flex()
                            .flex_row()
                            .items_center()
                            .child(div().text_color(colors.text).child(if before.is_empty() {
                                SharedString::from("")
                            } else {
                                SharedString::from(before.clone())
                            }))
                            .child(div().w(px(2.0)).h(px(14.0)).bg(colors.text).flex_shrink_0())
                            .child(div().text_color(colors.text).child(if after.is_empty() {
                                if before.is_empty() {
                                    SharedString::from("Type #tag ...")
                                } else {
                                    SharedString::from("")
                                }
                            } else {
                                SharedString::from(after)
                            })),
                    )
                    .into_any_element(),
            );
        } else {
            children.push(
                div()
                    .w_full()
                    .py(px(4.0))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(4.0))
                    .cursor(gpui::CursorStyle::IBeam)
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _ev, _window, cx| {
                            this.edit_zone = EditZone::TagInput;
                            this.populate_tag_input();
                            cx.notify();
                        }),
                    )
                    .children(tag_elements)
                    .into_any_element(),
            );
        }

        children.push(
            div()
                .w_full()
                .py(px(4.0))
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(format!("Created: {created}")),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(format!("Updated: {updated}")),
                )
                .into_any_element(),
        );

        children.push(
            div()
                .w_full()
                .h(px(1.0))
                .bg(colors.border)
                .my(px(4.0))
                .into_any_element(),
        );

        children
    }

    pub(super) fn render_highlighted_line(
        &self,
        line_idx: usize,
        line_text: &str,
        display_text: String,
        mut line_div: gpui::Div,
        cursor_line: usize,
        cursor_col: usize,
    ) -> gpui::Div {
        let mut highlighted = self
            .cached_highlights
            .get(line_idx)
            .cloned()
            .unwrap_or_else(|| HighlightedLine {
                highlights: vec![],
                image_urls: Vec::new(),
                line_height: 22.0,
                heading_level: None,
                line_bg: None,
            });
        let lh = highlighted.line_height;

        // Table header bold
        if line_text.starts_with('|')
            && line_text
                .chars()
                .all(|c| c == '|' || c == '-' || c == ':' || c == ' ' || c == '\t')
            && !line_text.contains('-')
        {
            let next_is_sep = self
                .cached_lines
                .get(line_idx + 1)
                .map(|l| {
                    l.starts_with('|')
                        && l.chars()
                            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ' || c == '\t')
                        && l.contains('-')
                })
                .unwrap_or(false);
            if next_is_sep {
                highlighted.highlights.insert(
                    0,
                    (
                        0..line_text.len().max(1),
                        HighlightStyle {
                            font_weight: Some(FontWeight::BOLD),
                            ..Default::default()
                        },
                    ),
                );
            }
        }

        line_div = line_div
            .when(!self.wrap, |el| el.h(px(lh)))
            .when(self.wrap, |el| el.min_h(px(lh)).whitespace_normal())
            .when_some(highlighted.heading_level, |el, level| match level {
                1 => el.text_2xl(),
                2 => el.text_xl(),
                3 => el.text_lg(),
                4 => el.text_base(),
                _ => el.text_sm(),
            });

        // Apply line-level background (e.g. code block bg extending to right edge)
        if let Some(bg) = highlighted.line_bg {
            line_div = line_div.bg(bg);
        }

        // Merge selection background using deterministic overlay
        let sel_byte_range = self.selection.as_ref().and_then(|sel| {
            let (sl, sc) = self.byte_to_line_col(sel.start);
            let (el, ec) = self.byte_to_line_col(sel.end);
            if line_idx >= sl && line_idx <= el {
                let from = if line_idx == sl { sc } else { 0 };
                let to = if line_idx == el {
                    ec
                } else {
                    line_text.chars().count()
                };
                if from < to {
                    return Some(
                        char_idx_to_byte(line_text, from)..char_idx_to_byte(line_text, to),
                    );
                }
            }
            None
        });
        if let Some(sel_range) = sel_byte_range {
            let sel_bg = self.resolved_colors.selection_bg;
            highlighted.highlights = overlay_selection(highlighted.highlights, sel_range, sel_bg);
        }

        // Render with or without cursor
        if line_idx == cursor_line && self.edit_zone == EditZone::Content {
            let (before, after) = split_at_char_col(&display_text, cursor_col);
            let before_len = before.len();
            let display_len = display_text.len();
            let before_styled = StyledText::new(SharedString::from(before)).with_highlights(
                adjust_highlight_offsets(&highlighted.highlights, 0, before_len),
            );
            let after_styled = if after.is_empty() {
                StyledText::new(SharedString::from(" ")).with_highlights(vec![])
            } else {
                StyledText::new(SharedString::from(after)).with_highlights(
                    adjust_highlight_offsets(&highlighted.highlights, before_len, display_len),
                )
            };
            line_div = line_div
                .child(before_styled)
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(lh - 4.0))
                        .bg(self.resolved_colors.text)
                        .flex_shrink_0(),
                )
                .child(after_styled);
        } else {
            line_div = line_div.child(
                StyledText::new(SharedString::from(display_text))
                    .with_highlights(highlighted.highlights),
            );
        }

        line_div
    }

    /// Render images for a group of consecutive lines with image URLs.
    /// Images are displayed horizontally in a flex row.
    pub(super) fn render_image_row(&self, urls: &[String]) -> gpui::AnyElement {
        let mut img_elements = Vec::new();
        for url in urls {
            let resolved = resolve_image_path(self.file_path.as_deref(), url);
            if resolved.exists() {
                img_elements.push(
                    div()
                        .px(px(4.0))
                        .child(
                            img(resolved)
                                .object_fit(gpui::ObjectFit::Contain)
                                .max_h(px(200.0)),
                        )
                        .into_any_element(),
                );
            } else {
                img_elements.push(
                    div()
                        .px(px(4.0))
                        .py(px(4.0))
                        .px(px(8.0))
                        .rounded_md()
                        .bg(self.resolved_colors.border)
                        .text_xs()
                        .text_color(self.resolved_colors.text_muted)
                        .child(format!("[image not found: {url}]"))
                        .into_any_element(),
                );
            }
        }
        div()
            .ml(px(16.0))
            .py(px(4.0))
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(4.0))
            .children(img_elements)
            .into_any_element()
    }
}

/// Style a math block delimiter line ($$).
pub(super) fn math_delim_line(line: &str, math_fg: gpui::Hsla) -> HighlightedLine {
    let dollar_count = line.bytes().take_while(|&b| b == b'$').count();
    let mut highlights = vec![(
        0..dollar_count,
        HighlightStyle {
            color: Some(math_fg),
            fade_out: Some(0.4),
            ..Default::default()
        },
    )];
    if dollar_count < line.len() {
        highlights.push((
            dollar_count..line.len(),
            HighlightStyle {
                color: Some(math_fg),
                ..Default::default()
            },
        ));
    }
    HighlightedLine {
        highlights,
        image_urls: Vec::new(),
        line_height: 22.0,
        heading_level: None,
        line_bg: None,
    }
}

/// Build per-line highlights, using Tree-sitter for fenced code blocks.
pub(super) fn build_highlights(lines: &[String], colors: &ResolvedColors) -> Vec<HighlightedLine> {
    let mut result = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        if line.starts_with("```") {
            let lang = line.trim_start_matches('`').trim().to_string();
            let lang_key = zelkova_highlight::resolve_language(&lang);
            result.push(highlight_fence_line(line, colors));
            i += 1;

            let mut code_lines = Vec::new();
            while i < lines.len() && !lines[i].starts_with("```") {
                code_lines.push(lines[i].as_str());
                i += 1;
            }

            if let Some(lang_name) = lang_key {
                let code_text = code_lines.join("\n");
                let styled = zelkova_highlight::highlight_code(&code_text, lang_name);

                let mut line_byte_start = 0usize;
                for &code_line in &code_lines {
                    let line_byte_end = line_byte_start + code_line.len();

                    let mut syntax_hl = Vec::new();
                    for sr in &styled {
                        if sr.range.end <= line_byte_start || sr.range.start >= line_byte_end {
                            continue;
                        }
                        let clamp_start = sr.range.start.max(line_byte_start) - line_byte_start;
                        let clamp_end = sr.range.end.min(line_byte_end) - line_byte_start;
                        if let Some(color) = colors.syntax_color(sr.highlight_index) {
                            syntax_hl.push((
                                clamp_start..clamp_end,
                                HighlightStyle {
                                    color: Some(color),
                                    ..Default::default()
                                },
                            ));
                        }
                    }

                    result.push(HighlightedLine {
                        highlights: syntax_hl,
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: Some(colors.code_bg),
                    });
                    line_byte_start = line_byte_end + 1;
                }
            } else {
                for &code_line in &code_lines {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..code_line.len().max(1),
                            HighlightStyle {
                                color: Some(colors.code_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: Some(colors.code_bg),
                    });
                }
            }

            if i < lines.len() {
                result.push(highlight_fence_line(&lines[i], colors));
                i += 1;
            }
        } else if line.trim_start().starts_with("$$") {
            let math_fg = colors.math_fg;
            result.push(math_delim_line(line, math_fg));
            i += 1;

            while i < lines.len() {
                let ml = lines[i].len();
                if lines[i].trim_end().ends_with("$$") && lines[i].trim() != "$$" {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..ml.max(1),
                            HighlightStyle {
                                color: Some(math_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: None,
                    });
                    i += 1;
                    break;
                } else if lines[i].trim() == "$$" {
                    result.push(math_delim_line(&lines[i], math_fg));
                    i += 1;
                    break;
                } else {
                    result.push(HighlightedLine {
                        highlights: vec![(
                            0..ml.max(1),
                            HighlightStyle {
                                color: Some(math_fg),
                                ..Default::default()
                            },
                        )],
                        image_urls: Vec::new(),
                        line_height: 22.0,
                        heading_level: None,
                        line_bg: None,
                    });
                    i += 1;
                }
            }
        } else {
            let context = detect_line_context(line, false);
            result.push(highlight_line(line, &context, colors));
            i += 1;
        }
    }

    result
}
