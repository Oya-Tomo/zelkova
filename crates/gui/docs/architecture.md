# gui

## Role

GPUI 0.2-based GUI application. Provides Markdown editing, syntax highlighting, preview, and tab management.

## Module Layout

```
src/
├── main.rs              ZelkovaApp, actions! macro, entry point
├── keymap.rs            KeyBinding construction, action name mapping
├── pane.rs              PaneManager (tabs, ViewMode switching)
├── command_palette.rs   CommandPalette (fuzzy match)
├── preview.rs           Markdown preview (zelkova-markdown AST → GPUI elements)
└── editor/
    ├── mod.rs           Editor body, action handlers, EntityInputHandler, Render
    ├── highlight.rs     ResolvedColors, line-level highlighting, inline scanning
    └── ime.rs           IME state management
```

## Dependencies

- `gpui 0.2` — UI framework
- `zelkova-config` — Theme and keymap configuration
- `zelkova-note-core` — Frontmatter struct
- `zelkova-rpc` — Daemon communication
- `zelkova-rope` — Text buffer (with undo/redo)
- `zelkova-markdown` — Parser for preview
- `zelkova-highlight` — Tree-sitter code highlighting

## Key Components

### ZelkovaApp (main.rs)

Application root. Manages the sidebar (note list) and main content (PaneManager).

- Fetches note list via RPC
- Opens a tab for the selected note
- Displays command palette overlay
- Loads and propagates theme settings

### PaneManager (pane.rs)

Tabbed editor/preview management. Switches between ViewMode (Editor/Split/Preview).

- Opens and closes tabs, switches active tab
- Propagates socket path and theme to the editor
- Focus management

### Editor (editor/mod.rs)

Main editor component. Responsibilities:

**Data Management:**
- `buffer: Buffer` — Rope-based text buffer
- `cached_text: String` — Buffer cache (avoids O(n) Rope traversal)
- `cached_lines: Vec<String>` — Line split cache
- `cached_highlights: Vec<HighlightedLine>` — Highlight result cache
- `resolved_colors: ResolvedColors` — Pre-parsed theme colors

**Cursor & Selection:**
- `cursor_pos: usize` — Byte offset
- `selection: Option<Range<usize>>` — Byte range
- `edit_zone: EditZone` — Title/Content switching (title editing within frontmatter header)

**Action Handlers:**
- Cursor movement (arrow keys, Title↔Content boundary crossing)
- Selection expansion (Shift+arrow)
- Character input (via EntityInputHandler, IME support)
- Backspace, Enter, Undo/Redo, Save

**Rendering (render method):**
1. `render_frontmatter_header()` — Header for title, tags, and date
2. `build_highlights()` — Build highlight cache
3. Line loop: render each line with `render_highlighted_line()`
4. Cursor line splits text into before/after, inserting a 2px bar

**Position Calculation:**
- `byte_to_line_col()` — Byte offset → (line, column)
- `line_col_to_byte()` — (line, column) → byte offset
- `pixel_to_col()` — Mouse pixel position → column (assumes monospace font, 7.2px/char)

### ResolvedColors (editor/highlight.rs)

Parses all theme colors into Hsla once and holds them. Eliminates per-frame string parsing.

Fields:
- Markdown colors: heading_marker, heading_fg, list_marker, quote_fg, text_dim, bold_fg, italic_fg, strikethrough_fg, image_marker, link_fg, math_fg
- Code block colors: code_bg, code_fg, code_keyword
- `code_syntax: [Hsla; 12]` — Tree-sitter 12 highlight classes (attribute, comment, constant, function, keyword, number, operator, property, punctuation, string, tag, type)

### Highlight Pipeline

```
cached_lines
    │
    build_highlights(lines, &resolved_colors)
    │
    ├─ Line starts with "```" → Code block mode
│   ├─ highlight_fence_line() — style for ``` lines
│   ├─ zelkova_highlight::highlight_code() — Tree-sitter syntax parsing
│   ├─ resolved_colors.syntax_color(idx) — highlight index → Hsla
│   └─ HighlightedLine { line_bg: Some(code_bg) }
│
├─ Line starts with "$$" → Math block mode
│   └─ math_delim_line() + math_fg for unified style
│
└─ Normal line
    ├─ detect_line_context() — Heading/ListItem/BlockQuote/Table/Normal
    ├─ highlight_line() — Style based on block context
    │   └─ scan_inline() — Bold/Italic/Strikethrough/Code/Link/Image/Math
    └─ HighlightedLine
```

### Selection Background (overlay_selection)

Uses a custom deterministic overlay instead of `combine_highlights` (which is non-deterministic via HashSet):
1. Split existing highlights at selection boundaries
2. Overwrite `background_color` within selection range with `sel_bg`
3. Preserve original style outside selection range
4. Fill gaps within selection with sel_bg-only highlights

### CommandPalette (command_palette.rs)

Overlay search UI. Filters key actions via fuzzy matching.

### Preview (preview.rs)

Renders zelkova-markdown AST → GPUI element tree. An entity independent of the editor.

## Data Flow

```
User Input (keyboard/mouse)
    │
    ├─ EntityInputHandler → buffer.edit() → cache_edit()
    │                                    → highlights_dirty = true
    │
    └─ cx.notify() → render()
        │
        ├─ if highlights_dirty:
        │   build_highlights() → cached_highlights
        │
        ├─ render_frontmatter_header()
        │
        └─ for each line:
            ├─ render_highlighted_line()
            │   ├─ overlay_selection() — apply selection background
            │   └─ line_bg — code block full-width background
            │
            └─ StyledText::with_highlights()
```

## Known Limitations

- **Monospace font assumption**: `pixel_to_col()` calculates at 7.2px/char. Will be off with proportional fonts.
- **Highlight delay**: First frame shows plain text. Highlighting appears on the next frame.
- **Polling-based file watching**: 2-second polling instead of inotify/FSEvents.
- **Plain text frontmatter header**: Title, tag, and date colors are hardcoded (no theme support).
