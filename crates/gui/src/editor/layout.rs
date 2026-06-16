//! Custom Element that renders a single logical line of the Editor and
//! captures the resulting layout for downstream cursor math.
//!
//! See ADR-0002 for the overall design and the alternatives that were
//! considered.
//!
//! NOTE: This module is introduced in PR 2a (#162 split) but is not yet
//! wired into `render.rs`. PR 2b will replace `StyledText` with
//! `EditorLineElement` and remove the `dead_code` allows below.

#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Bounds, Element, GlobalElementId, HighlightStyle, IntoElement, LayoutId, Pixels, Point,
    ShapedLine, SharedString, Size, Style, Window, px,
};

use super::util::{WrapSegment, build_runs_from_highlights, wrap_line_bytes};

/// Layout captured during paint of one logical line.
///
/// Stored in the `Editor` via the shared `Rc<RefCell<...>>` handle so that
/// cursor / click math in #163 and #164 can read the exact pixel positions
/// of every visual row.
#[derive(Clone)]
pub struct EditorLineLayout {
    /// One `ShapedLine` per visual row, in order.
    pub rows: Vec<ShapedLine>,
    /// The wrap segments used to split the logical line, parallel to `rows`.
    pub segments: Vec<WrapSegment>,
    /// The wrap width used for this layout (for invalidation checks).
    pub wrap_width: Option<Pixels>,
    /// Line height in pixels.
    pub line_height: Pixels,
}

/// Shared handle that lets `Editor` read the most recent layout for each
/// logical line without forcing the Element to call `cx.update` from paint.
pub type LayoutHandle = Rc<RefCell<Option<EditorLineLayout>>>;

/// Shared handle for the wrap-width value captured by the hidden canvas.
/// `Editor` reads this each frame; the canvas writes `Some(width)` during
/// its paint callback.
pub type WrapWidthHandle = Rc<RefCell<Option<Pixels>>>;

/// Custom GPUI Element that renders a single logical line.
///
/// The Element receives the raw text and highlight list (not pre-built
/// TextRuns) because font resolution must happen *inside* request_layout /
/// prepaint — that's the only point where `window.text_style()` reflects
/// the parent div's actual style (Heading rows with `text_2xl()`, etc.).
/// Building TextRuns earlier captures the wrong style.
///
/// Pipeline:
/// 1. request_layout: shape the full line to compute width / height.
/// 2. prepaint: shape again, split into visual rows via `wrap_line_bytes`,
///    re-shape each row's substring.
/// 3. paint: paint each row at the correct Y offset, write layout into
///    the shared handle.
pub struct EditorLineElement {
    text: SharedString,
    highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
    layout_handle: LayoutHandle,
}

impl EditorLineElement {
    pub fn new(
        text: SharedString,
        highlights: Vec<(std::ops::Range<usize>, HighlightStyle)>,
        line_height: Pixels,
        wrap_width: Option<Pixels>,
        layout_handle: LayoutHandle,
    ) -> Self {
        Self {
            text,
            highlights,
            line_height,
            wrap_width,
            layout_handle,
        }
    }

    /// Build TextRuns using the *current* window text style. Must be called
    /// inside request_layout / prepaint where the style stack is correct.
    fn build_runs(&self, window: &Window) -> (Vec<gpui::TextRun>, Pixels) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let runs = build_runs_from_highlights(&style, self.text.len(), &self.highlights);
        // Debug: emit one DBG line per run so `grep DBG` captures all of them.
        eprintln!(
            "DBG build_runs START text={:?} highlights={}",
            self.text,
            self.highlights.len()
        );
        for (i, r) in runs.iter().enumerate() {
            eprintln!(
                "DBG run[{}] len={} color={:?} bg={:?}",
                i, r.len, r.color, r.background_color
            );
        }
        eprintln!("DBG build_runs END");
        (runs, font_size)
    }
}

impl Element for EditorLineElement {
    type RequestLayoutState = ();
    type PrepaintState = EditorLineLayout;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let (runs, font_size) = self.build_runs(window);
        let shaped = window
            .text_system()
            .shape_line(self.text.clone(), font_size, &runs, None);

        let line_height = self.line_height;
        let size = if let Some(ww) = self.wrap_width {
            let segs = wrap_line_bytes(&self.text, ww, |i| shaped.x_for_index(i));
            let rows = segs.len().max(1);
            Size {
                width: ww,
                height: line_height * rows as f32,
            }
        } else {
            let width = shaped.x_for_index(self.text.len());
            Size {
                width,
                height: line_height,
            }
        };

        let mut style = Style::default();
        style.size.width = gpui::Length::Definite(size.width.into());
        style.size.height = gpui::Length::Definite(size.height.into());

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let (runs, font_size) = self.build_runs(window);
        let full = window
            .text_system()
            .shape_line(self.text.clone(), font_size, &runs, None);

        let (segments, rows): (Vec<WrapSegment>, Vec<ShapedLine>) =
            if let Some(ww) = self.wrap_width {
                let segs = wrap_line_bytes(&self.text, ww, |i| full.x_for_index(i));
                let mut row_shaped = Vec::with_capacity(segs.len());
                for seg in &segs {
                    let substr: SharedString =
                        self.text[seg.start_byte..seg.end_byte].to_string().into();
                    let sub_runs = slice_runs(&runs, seg.start_byte, seg.end_byte);
                    let shaped = window
                        .text_system()
                        .shape_line(substr, font_size, &sub_runs, None);
                    row_shaped.push(shaped);
                }
                (segs, row_shaped)
            } else {
                (
                    vec![WrapSegment {
                        start_byte: 0,
                        end_byte: self.text.len(),
                    }],
                    vec![full],
                )
            };

        EditorLineLayout {
            rows,
            segments,
            wrap_width: self.wrap_width,
            line_height: self.line_height,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let origin = bounds.origin;
        let line_h = prepaint.line_height;

        for (i, row) in prepaint.rows.iter().enumerate() {
            let row_origin = origin
                + Point {
                    x: px(0.0),
                    y: line_h * i as f32,
                };
            eprintln!(
                "DBG paint row={} origin={:?} line_h={:?} row_len={}",
                i, row_origin, line_h, row.len()
            );
            let _ = row.paint_background(row_origin, line_h, window, cx);
            let _ = row.paint(row_origin, line_h, window, cx);
        }

        *self.layout_handle.borrow_mut() = Some(EditorLineLayout {
            rows: prepaint.rows.clone(),
            segments: prepaint.segments.clone(),
            wrap_width: prepaint.wrap_width,
            line_height: prepaint.line_height,
        });
    }
}

impl IntoElement for EditorLineElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

/// Slice a `TextRun` array to the byte range [start, end). Each output run
/// has its `len` clamped to fit within the slice, and any run fully outside
/// the range is dropped.
pub(crate) fn slice_runs(runs: &[gpui::TextRun], start: usize, end: usize) -> Vec<gpui::TextRun> {
    let mut out = Vec::new();
    let mut cursor = 0usize;
    for r in runs {
        let run_start = cursor;
        let run_end = cursor + r.len;
        cursor = run_end;

        if run_end <= start {
            continue;
        }
        if run_start >= end {
            break;
        }
        let clip_start = start.max(run_start);
        let clip_end = end.min(run_end);
        if clip_end <= clip_start {
            continue;
        }
        let mut sliced = r.clone();
        sliced.len = clip_end - clip_start;
        out.push(sliced);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Font, FontStyle, FontWeight, Hsla, TextRun};

    fn run(start: usize, end: usize) -> TextRun {
        TextRun {
            len: end - start,
            font: Font {
                family: "monospace".into(),
                weight: FontWeight::default(),
                style: FontStyle::default(),
                features: Default::default(),
                fallbacks: None,
            },
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn slice_runs_fully_inside() {
        let runs = vec![run(0, 5), run(5, 10)];
        let out = slice_runs(&runs, 2, 7);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].len, 3);
        assert_eq!(out[1].len, 2);
    }

    #[test]
    fn slice_runs_drops_outside() {
        let runs = vec![run(0, 5), run(5, 10), run(10, 15)];
        let out = slice_runs(&runs, 6, 9);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len, 3);
    }

    #[test]
    fn slice_runs_empty_range() {
        let runs = vec![run(0, 5)];
        let out = slice_runs(&runs, 3, 3);
        assert!(out.is_empty());
    }
}
