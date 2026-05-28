use gpui::{
    AnyElement, App, Bounds, ContentMask, DispatchPhase, Element, ElementId, Entity,
    GlobalElementId, Hitbox, InspectorElementId, IntoElement, LayoutId, Pixels, Point, ScrollDelta,
    ScrollWheelEvent, Style, Window, px, relative,
};

use super::Editor;

/// Custom element that prevents horizontal container expansion while allowing
/// vertical scroll via the parent's `overflow_y_scroll`.
///
/// When `wrap=false`, the content can be wider than the viewport. Standard GPUI divs
/// expand to fit their children, which makes `content_size == bounds.size` and prevents
/// horizontal scrolling. This element uses `relative(1.)` width in `request_layout()` to
/// fix the element's width to the parent's width, breaking the expansion chain.
///
/// Horizontal scrolling is managed manually:
/// - `scroll_x` offset is applied during prepaint via `window.with_element_offset()`
/// - Content is clipped to viewport bounds via `window.with_content_mask()` during paint
/// - Scroll wheel handler registered via `window.on_mouse_event()` on the hitbox
pub struct EditorContentElement {
    child: AnyElement,
    scroll_x: f32,
    editor: Entity<Editor>,
}

impl EditorContentElement {
    pub fn new(child: AnyElement, scroll_x: f32, editor: Entity<Editor>) -> Self {
        Self {
            child,
            scroll_x,
            editor,
        }
    }
}

impl IntoElement for EditorContentElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorContentElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let child_layout_id = self.child.request_layout(window, cx);

        let mut style = Style::default();
        style.size.width = relative(1.).into();

        let layout_id = window.request_layout(style, [child_layout_id], cx);
        let child = std::mem::replace(&mut self.child, gpui::Empty.into_any_element());
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Hitbox {
        let hitbox = window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal);

        let scroll_x = self.scroll_x;
        if scroll_x > 0.0 {
            window.with_element_offset(Point::new(px(-scroll_x), px(0.0)), |window| {
                child.prepaint(window, cx);
            });
        } else {
            child.prepaint(window, cx);
        }

        hitbox
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Register scroll wheel handler via window.on_mouse_event (same pattern as GPUI div).
        let editor = self.editor.clone();
        let hitbox = hitbox.clone();
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            if !hitbox.should_handle_scroll(_window) {
                return;
            }
            let dx = match &event.delta {
                ScrollDelta::Pixels(p) => f32::from(p.x),
                ScrollDelta::Lines(l) => l.x * 22.0,
            };
            // Also treat Shift+vertical scroll as horizontal scroll
            let dx = if dx == 0.0 && event.modifiers.shift {
                match &event.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y),
                    ScrollDelta::Lines(l) => l.y * 22.0,
                }
            } else {
                dx
            };
            let dx = -dx;
            if dx == 0.0 {
                return;
            }
            editor.update(cx, |this, cx| {
                let max_width = this
                    .cached_lines
                    .iter()
                    .map(|l| l.chars().count() as f32 * 7.2)
                    .fold(0.0_f32, f32::max);
                let viewport_width = f32::from(this.scroll_handle.bounds().size.width);
                let max_scroll_x = (max_width - viewport_width).max(0.0);
                this.scroll_x = (this.scroll_x + dx).clamp(0.0, max_scroll_x);
                cx.notify();
            });
        });

        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            child.paint(window, cx);
        });
    }
}
