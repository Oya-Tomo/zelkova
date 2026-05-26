# Zelkova User Guide

## Getting Started

Start the daemon and launch the GUI:

```sh
zelkovad &   # start background daemon
zelkova      # launch GUI
```

## Interface Layout

```
+-------------------+----------------------------------------+
| Zelkova    [+]    |  tab1.md  tab2.md                      |
|                   |----------------------------------------|
| ▾ Work            |                                        |
|   My Note         |  # My Note                             |
|   Another Note    |                                        |
| ▸ Personal        |  Note content here...                  |
| Untitled          |                                        |
|                   |                                        |
+-------------------+----------------------------------------+
```

- **Sidebar** (left): folder tree and note list. Click `+` or use `Ctrl+N` to create a note.
- **Editor** (right): Markdown editor with syntax highlighting.
- **Command Palette**: press `Ctrl+P` to open.

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `Ctrl+P` | Open command palette |
| `Ctrl+N` | Create new note |
| `Ctrl+S` | Save note |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+Q` | Quit |
| `Escape` | Close palette / cancel |

### Editor

| Key | Action |
|-----|--------|
| `Arrow keys` | Move cursor |
| `Shift+Arrow` | Select text |
| `Ctrl+A` | Select all |
| `Backspace` | Delete character |
| `Enter` | New line |
| `Ctrl+Z` | Undo |
| `Ctrl+Shift+Z` | Redo |

### Pane

| Key | Action |
|-----|--------|
| `Ctrl+Alt+→` | Next tab |
| `Ctrl+Alt+←` | Previous tab |
| `Ctrl+Alt+V` | Toggle view mode (editor → split → preview) |

## Command Palette

Press `Ctrl+P` to open. Type to fuzzy-search commands, then press `Enter` to execute.

### Available Commands

| Command | Description |
|---------|-------------|
| Create Note | Create a new note (optional title + folder) |
| Create Folder | Create a folder (name + optional parent) |
| Move Note to Folder | Move a note into a folder |
| Move Folder to Folder | Move a folder into another folder |
| Rename Note | Change a note's title |
| Rename Folder | Change a folder's name |
| Delete Note | Delete a note (with confirmation) |
| Delete Folder | Delete a folder (choose: move notes to root or delete notes too) |
| Toggle Sidebar | Show/hide the sidebar |
| Toggle View Mode | Cycle editor/split/preview view |
| Save Note | Save current note |
| Quit | Exit Zelkova |

### Multi-step Commands

Some commands require multiple inputs. The palette walks through each argument:

1. **Select** args: type to fuzzy-filter the list, `↑`/`↓` to navigate, `Enter` to confirm
2. **Text** args: type your input, `Enter` to confirm
3. **Confirmation** args: select "Yes, delete" or "Cancel"

Press `Escape` to go back one step or close the palette.

## Folder Management

Folders are virtual (stored in `.zelkova/structure.toml`). Notes are not moved on disk.

- Click a folder `▸`/`▾` to expand/collapse
- Use the command palette for create, rename, move, and delete operations

## Configuration

Config files live in `~/.config/zelkova/`:

| File | Purpose |
|------|---------|
| `config.toml` | General settings (vault path, daemon socket) |
| `keymap.toml` | Custom keybindings |
| `theme.toml` | Color theme |

### Custom Keybindings

Edit `~/.config/zelkova/keymap.toml`:

```toml
leader = "space"

[[bindings]]
key = "ctrl-p"
action = "open_command_palette"

[[bindings]]
key = "ctrl-n"
action = "create_note"
```

Available actions: `open_command_palette`, `search_notes`, `create_note`, `save_note`, `toggle_sidebar`, `quit`, `undo`, `redo`
