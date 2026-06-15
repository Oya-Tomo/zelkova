# ADR-0002: Editor text layout via a custom Element

Date: 2026-06-15

## Status

Accepted (decisions confirmed via grill-me on 2026-06-15)

## Context and Problem Statement

`Editor::render` (in `crates/gui/src/editor/render.rs`) emits each logical line as `div().child(StyledText::new(text).with_highlights(h))`. GPUI 0.2 internally builds a `TextLayout` per styled text — including font-size-aware wrap positions, glyph positions, and per-row boundaries — but this layout is **private to the element**. The Editor has no way to read it back.

The result is that cursor math is approximate:

- `pixel_to_col(line, pixel_x, ascii_w)` walks the whole logical line and returns the closest character index. Clicks on the 2nd visual row of a wrapped line land on the closest character of the *entire* line, not within the clicked visual row.
- `handle_move_up` / `handle_move_down` use `byte_to_line_col`, which only knows logical lines. There is no concept of "visual row" inside a wrapped logical line.
- Heading / list / code blocks use different font sizes, so a character-count × pixel-width approximation is incorrect.

Issue #98 tracks the user-visible breakage; sub-issues #162 (this one), #163, #164 break the fix into sequenced steps.

### What GPUI 0.2 actually exposes

Confirmed by source inspection at `~/.cargo/registry/src/.../gpui-0.2.2/src/`:

- `TextSystem::shape_line(text, font_size, runs, force_width) -> ShapedLine` — public (`text_system.rs:365`)
  - **`force_width` is NOT a wrap width** — it's "redistribute glyphs to fit exactly this width" (line_layout.rs:572-573 rewrites `glyph.position.x = glyph_pos * force_width`). It does **not** split the line into visual rows.
- `TextSystem::layout_line(text, font_size, runs, force_width) -> Arc<LineLayout>` — public (line_layout.rs:533), same caveat.
- `ShapedLine::paint(origin, line_height, window, cx)` — public (`text_system/line.rs:63`)
- `LineLayout::index_for_x(x) -> Option<usize>` and `x_for_index(index) -> Pixels` — public (line_layout.rs:58, :105)
- `WrappedLineLayout` — public type but populated only inside `Text` element state; no public API to obtain one out-of-band
- **Wrap calculation is private to the `Text` element.** GPUI's `elements/text.rs` performs soft-wrapping internally (driven by `wrap_width` style); the resulting `WrappedLineLayout` is stored in element-local `TextLayoutInner` and only reachable from inside the element's own paint closure.

The conclusion: the data we need **is not directly available**. We have to compute it ourselves by:

1. Calling `shape_line` on the full logical line (no `force_width`) to get a `ShapedLine` whose `x_for_index(byte_idx)` tells us the pixel width up to each byte.
2. Walking the byte positions, asking `x_for_index(i) > wrap_width?`, and splitting at the largest `i` that still fits — respecting word boundaries (ASCII spaces, CJK character boundaries, etc.).
3. For each visual-row byte range, calling `shape_line` again on the substring to get a separate `ShapedLine` for that row.

Zed (which publishes the crates.io `gpui = "0.2.2"`) does exactly this in its `LineWithInvisibles::from_chunks` (`crates/editor/src/element.rs:7006`).

## Decision Drivers

- **Accuracy** — heading / list / code font-size differences, proportional fonts, CJK width, ligatures must all be respected. Character-count heuristics cannot do this.
- **No private fork** — Zelkova must keep using `gpui = "0.2"` from crates.io.
- **Preserve existing Editor features** — syntax highlighting (`StyledText::with_highlights`), inline images, line background, heading-level rendering, selection overlay.
- **Testability** — the wrap-position math (byte ↔ visual row) must be pure functions that can be unit tested without a window.

## Considered Options

### Option A: Editor measures lines directly via `TextSystem::layout_line`

In `Editor::render`, before emitting children, call `cx.text_system().layout_line(line_text, font_size, runs, Some(wrap_width))` for each logical line and cache the `Arc<LineLayout>` in `cached_line_layouts`. Keep using `StyledText` for actual rendering.

- ✅ Small change (~80 lines)
- ✅ No custom Element
- ❌ The `wrap_width` Editor picks must exactly match the width GPUI uses when laying out `StyledText`. Any mismatch (padding, scroll bar, font-size fallback) means the cached layout disagrees with what's painted.
- ❌ Hard to detect drift; would need an assertion that the painted text bounds match the measured width.

### Option B (chosen): Custom `EditorLineElement` that owns the layout

Introduce a new element in `crates/gui/src/editor/layout.rs` that wraps a single logical line. It:

1. **Wrap-calculate** by calling `shape_line(text, font_size, runs, None)` on the full logical line, walking byte positions via `x_for_index`, and producing a `Vec<WrapSegment>` where each segment is a `(byte_start, byte_end)` for a visual row. Word-boundary-aware (ASCII spaces, CJK boundaries).
2. **Per-row shape** by calling `shape_line(substr, font_size, sub_runs, None)` for each visual row's slice. Each `ShapedLine` knows its own width and is paintable.
3. Implements GPUI's `Element` trait with `RequestLayoutState = ()` and `PrepaintState = EditorLineLayout { rows: Vec<ShapedLine>, wrap_width: Pixels }`.
4. In `paint`, draws each row's `ShapedLine` at `origin + Point { y: row_index * line_height }`.
5. Writes the `EditorLineLayout` back to the `Editor` entity via `cx.update(entity, ...)` so cursor math in #163 / #164 can read it.

`Editor::render` emits one `EditorLineElement` per logical line, passing the line's text + highlights + font-size + wrap_width + the Editor's entity handle.

- ✅ Layout that Editor reads is *exactly* the layout that was painted — no drift possible.
- ✅ All public API, no fork.
- ✅ Sets up #163 (cache wrap positions) and #164 (cursor ops) cleanly.
- ✅ Wrap algorithm is unit-testable as a pure function (`wrap_line_bytes(text, runs, wrap_width, &ShapedLine) -> Vec<WrapSegment>`).
- ❌ ~500-700 lines: implements the Element trait, the wrap algorithm (largest single piece), re-implements the `StyledText`-with-highlights rendering path using `TextRun` arrays, handles wrap_width propagation, image inline rendering, selection overlay.
- ❌ Styled text features Editor currently relies on (e.g., `StyledText`'s own wrap handling) need explicit re-implementation.
- ❌ Wrap algorithm duplicates work GPUI's `Text` element already does internally. If a future GPUI release exposes wrap calculation publicly, we can simplify.

### Option C: Fork GPUI to expose `WrappedLineLayout` from `StyledText`

Patch `elements/text.rs` to make `TextLayout::line_layout_for_index` reachable from outside.

- ✅ Minimal Editor changes.
- ❌ Adds a forked dependency. Ongoing maintenance, version drift, security review.
- ❌ Goes against the project's policy of using crates.io dependencies.

## Decision Outcome

Chosen: **Option B — Custom `EditorLineElement`**.

### Rationale

1. The whole point of #98 is accuracy. Option A's "measure then trust" approach is fundamentally fragile — wrap width depends on the element's resolved bounds, which the Editor doesn't know at measure time.
2. Zed's Editor uses the same pattern (`EditorElement` with `PrepaintState = EditorLayout`). It's a proven model on the same GPUI source we depend on.
3. Wrap calculation, although we have to write it ourselves, is a pure function that takes a `ShapedLine` and a `wrap_width` and returns `Vec<WrapSegment>`. It is unit-testable without a window.
4. The cost (~500-700 lines) is bounded and split across three sub-issues (#162 plumbing + wrap algorithm, #163 cache + pure conversions, #164 cursor ops). Each sub-issue ships independently and each is testable.

### Architecture

```
Editor (Entity)
  ├── cached_text: String
  ├── cached_lines: Vec<String>               # logical lines
  ├── cached_highlights: Vec<HighlightedLine> # per logical line
  ├── cached_line_layouts: Vec<Option<Arc<WrappedLineLayout>>>
  │                                           # NEW (#162). Index = logical line.
  │                                           # Populated by EditorLineElement::paint
  │                                           # via cx.update. Invalidated on text /
  │                                           # highlights / width change.
  └── render()
        ├── for each logical line:
        │     EditorLineElement::new(text, highlights, font_size, wrap_width, entity, line_idx)
        └── canvas() — same pattern as #146, no layout role

EditorLineElement (NEW, crates/gui/src/editor/layout.rs)
  Implements gpui::Element
  ├── text: SharedString
  ├── highlights: Vec<(Range<usize>, HighlightStyle)>
  ├── font_size: Pixels
  ├── wrap_width: Option<Pixels>
  ├── editor_entity: Entity<Editor>
  ├── line_idx: usize
  ├── request_layout(window, cx)
  │     → shape_line → size
  ├── prepaint(bounds, window, cx)
  │     → layout_line(text, font_size, runs, wrap_width)
  │     → store Arc<WrappedLineLayout> in PrepaintState
  └── paint(bounds, prestate, window, cx)
        → ShapedLine::paint for each visual row
        → cx.update(editor_entity, |ed, _| {
              ed.cached_line_layouts[line_idx] = Some(prestate.layout.clone());
          })
```

### Sub-issue mapping

- **#162 (this ADR)** — Land `EditorLineElement` + `Editor::cached_line_layouts`. Cursor math still approximate, but the layout is captured.
- **#163** — Introduce `WrapSegment` model + pure `byte ↔ visual_row` conversions in `util.rs`, fed from `cached_line_layouts`.
- **#164** — Replace `pixel_to_col` with layout-based hit test; switch `handle_move_up/down` to (line, visual_row, col).

### Phase 1 scope (this Issue #162)

- `EditorLineElement` with text-only rendering (highlights preserved; images and selection-overlay temporarily still drawn by `render_highlighted_line` on top — they are out-of-band, not inside the shaped line).
- Wrap algorithm as a pure function `wrap_line_bytes(text, &ShapedLine, wrap_width) -> Vec<WrapSegment>` in `util.rs`, with unit tests (no window needed for the algorithm itself; only the `ShapedLine` construction does).
- Wire into `Editor::render` replacing `StyledText` for the body text of each logical line.
- `cached_line_layouts: Vec<Option<Arc<EditorLineLayout>>>` where `EditorLineLayout = { rows: Vec<ShapedLine>, wrap_width: Pixels }`.
- No cursor behavior change yet (deferred to #164).

### Out of scope for #162

- Inline images inside a logical line (they will continue to render via the existing `img()` path; full integration into `EditorLineElement` is a follow-up if needed).
- Wrapping at non-ASCII boundaries (handled by `TextSystem`, no Editor work).
- Performance optimisation (caching shape results across frames — GPUI's text system already does this internally).

## Consequences

**Good:**

- Editor's wrap data is *the same* as what was painted — no drift, ever.
- Heading / code / list sizes are respected because `TextSystem::shape_line` uses the actual font metrics.
- Future cursor work (#163, #164) becomes straightforward: read the layout, do pure math.
- Removes the dependency on `StyledText`'s internal layout behaviour, decoupling us from future GPUI changes to that element.

**Bad:**

- ~300-500 lines of new code in the GUI crate.
- Re-implements what `StyledText` did for us for free (wrap, paint, basic selection). If GPUI improves `StyledText` later, we don't automatically benefit.
- Inline images are not part of the shaped line; they're still rendered as sibling elements. If we ever want images to participate in wrap layout, more work is needed.

**Neutral:**

- `EditorLineElement` becomes the single source of truth for "how Editor lays out a line". Anything that needs line geometry goes through it.
- `render_highlighted_line` shrinks (the paint path moves into the Element), but the highlight *building* pipeline (`build_highlights`, `scan_inline`) stays put.

## Resolved decisions (grill-me 2026-06-15)

1. **Approach** — Option B (custom Element + self-implemented wrap algorithm). GPUI 0.2 does not expose wrap calculation publicly; Zed's pattern is the proven model on the same source.
2. **Layout handoff from Element to Editor** — `Rc<RefCell<Vec<Option<EditorLineLayout>>>>` shared between Editor and each `EditorLineElement`. Avoids `cx.update` from paint, mirrors Zed's `PositionMap` pattern.
3. **`wrap_width` source** — a hidden `canvas` at the top of the Editor's root div captures `bounds.size.width` during its paint callback and writes it to `cached_wrap_width: Rc<RefCell<Option<Pixels>>>`. GPUI 0.2 has no `on_layout` API; canvas is the established workaround (Editor already uses this pattern for `window.handle_input()` since PR #146). Paint order parent→child guarantees `EditorLineElement` reads the fresh value same-frame. First frame: `None` → no wrap (one visual row per logical line), corrects on frame 2.
4. **Wrap boundary rule** — word-boundary aware: ASCII whitespace (`' '`, `'\t'`) creates break opportunities; CJK characters (Unicode ideographic blocks) break per-character. Mirrors Zed default behaviour. No `word-break` configurability in this phase.
5. **Images in lines** — logical lines containing inline image URLs are excluded from wrap calculation (rendered as a single visual row, may overflow `wrap_width`). Inline-image-and-text mixing on the same visual row is deferred.

## Implementation notes (not decisions, just notes for the implementer)

- **UTF-8 boundaries** — `LineLayout::index_for_x` returns UTF-8 byte boundaries (`text_system/line_layout.rs:58`). Wrap algorithm and cursor math must never split a multi-byte char.
- **TextRun slicing** — when a logical line wraps inside a `TextRun`, slice the run across the boundary using the same logic as `util.rs::adjust_highlight_offsets` (already used for selection overlay).
- **Performance** — wrap calculation calls `shape_line` once for the full logical line plus once per visual row. Mitigations: cache the full-line `ShapedLine` in `cached_line_layouts` (one per logical line, invalidated on text change); rely on GPUI's internal text-system cache for the per-row shape calls (keyed by content + font + size).

## More Information

- Parent issue: #98
- Sub-issues: #162 (this), #163, #164
- Predecessor: ADR-0001 (daemon-only vault access) — unrelated, listed for cross-reference
- Zed reference: `crates/editor/src/element.rs:190` (`EditorElement`), `:10053` (`point_for_position`)
- GPUI reference: `text_system/line_layout.rs:533` (`layout_line`), `text_system/line.rs:31` (`ShapedLine`)
