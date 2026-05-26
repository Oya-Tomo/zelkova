use gpui::KeyBinding;
use zelkova_config::{BindingConfig, KeymapConfig};

/// Build GPUI KeyBindings from KeymapConfig.
/// Maps action name strings to concrete action types.
pub fn build_bindings(keymap_config: &KeymapConfig) -> Vec<KeyBinding> {
    let mut bindings: Vec<KeyBinding> = Vec::new();

    // Editor key bindings (always active)
    bindings.push(KeyBinding::new("left", crate::MoveLeft, None));
    bindings.push(KeyBinding::new("right", crate::MoveRight, None));
    bindings.push(KeyBinding::new("up", crate::MoveUp, None));
    bindings.push(KeyBinding::new("down", crate::MoveDown, None));
    bindings.push(KeyBinding::new("shift-left", crate::SelectLeft, None));
    bindings.push(KeyBinding::new("shift-right", crate::SelectRight, None));
    bindings.push(KeyBinding::new("shift-up", crate::SelectUp, None));
    bindings.push(KeyBinding::new("shift-down", crate::SelectDown, None));
    bindings.push(KeyBinding::new("backspace", crate::Backspace, None));
    bindings.push(KeyBinding::new("enter", crate::InsertNewline, None));

    // Undo/Redo
    bindings.push(KeyBinding::new("ctrl-z", crate::Undo, None));
    bindings.push(KeyBinding::new("ctrl-shift-z", crate::Redo, None));

    // Pane key bindings
    bindings.push(KeyBinding::new("ctrl-alt-right", crate::NextPane, None));
    bindings.push(KeyBinding::new("ctrl-alt-left", crate::PrevPane, None));
    bindings.push(KeyBinding::new("ctrl-alt-v", crate::ToggleViewMode, None));

    // Global
    bindings.push(KeyBinding::new("escape", crate::Cancel, None));

    // SelectAll
    bindings.push(KeyBinding::new("ctrl-a", crate::SelectAll, None));

    // User-defined bindings from config
    let resolved = keymap_config.resolved_bindings();
    for binding in resolved {
        if let Some(kb) = binding_to_key_binding(&binding) {
            bindings.push(kb);
        }
    }

    bindings
}

fn binding_to_key_binding(binding: &BindingConfig) -> Option<KeyBinding> {
    let context = binding.context.as_deref();
    match binding.action.as_str() {
        "open_command_palette" => Some(KeyBinding::new(
            &binding.key,
            crate::OpenCommandPalette,
            context,
        )),
        "search_notes" => Some(KeyBinding::new(&binding.key, crate::SearchNotes, context)),
        "create_note" | "new_note" => {
            Some(KeyBinding::new(&binding.key, crate::CreateNote, context))
        }
        "list_notes" => Some(KeyBinding::new(&binding.key, crate::ListNotes, context)),
        "show_tags" => Some(KeyBinding::new(&binding.key, crate::ShowTags, context)),
        "toggle_sidebar" => Some(KeyBinding::new(&binding.key, crate::ToggleSidebar, context)),
        "save_note" => Some(KeyBinding::new(&binding.key, crate::SaveNote, context)),
        "quit" => Some(KeyBinding::new(&binding.key, crate::Quit, context)),
        "move_up" => Some(KeyBinding::new(&binding.key, crate::MoveUp, context)),
        "move_down" => Some(KeyBinding::new(&binding.key, crate::MoveDown, context)),
        "move_left" => Some(KeyBinding::new(&binding.key, crate::MoveLeft, context)),
        "move_right" => Some(KeyBinding::new(&binding.key, crate::MoveRight, context)),
        "backspace" => Some(KeyBinding::new(&binding.key, crate::Backspace, context)),
        "insert_newline" => Some(KeyBinding::new(&binding.key, crate::InsertNewline, context)),
        "next_pane" => Some(KeyBinding::new(&binding.key, crate::NextPane, context)),
        "prev_pane" => Some(KeyBinding::new(&binding.key, crate::PrevPane, context)),
        "toggle_view_mode" => Some(KeyBinding::new(
            &binding.key,
            crate::ToggleViewMode,
            context,
        )),
        "undo" => Some(KeyBinding::new(&binding.key, crate::Undo, context)),
        "redo" => Some(KeyBinding::new(&binding.key, crate::Redo, context)),
        "confirm" => Some(KeyBinding::new(&binding.key, crate::Confirm, context)),
        "cancel" => Some(KeyBinding::new(&binding.key, crate::Cancel, context)),
        "select_all" => Some(KeyBinding::new(&binding.key, crate::SelectAll, context)),
        _ => {
            eprintln!("warning: unknown action in keymap: {}", binding.action);
            None
        }
    }
}

/// All action names with their display labels, for the command palette.
pub fn all_action_entries() -> Vec<(String, String)> {
    vec![
        ("OpenCommandPalette".into(), "Open Command Palette".into()),
        ("SearchNotes".into(), "Search Notes".into()),
        ("CreateNote".into(), "Create Note".into()),
        ("CreateFolder".into(), "Create Folder".into()),
        ("MoveToFolder".into(), "Move to Folder".into()),
        ("DeleteFolder".into(), "Delete Folder".into()),
        ("RenameFolder".into(), "Rename Folder".into()),
        ("ListNotes".into(), "List Notes".into()),
        ("ShowTags".into(), "Show Tags".into()),
        ("ToggleSidebar".into(), "Toggle Sidebar".into()),
        ("SaveNote".into(), "Save Note".into()),
        ("Quit".into(), "Quit".into()),
    ]
}

/// Command specs with argument definitions for the command palette.
/// `folder_names` are the current folder names from the daemon, used to
/// populate Select-style argument options dynamically.
pub fn all_command_specs(folder_names: &[String]) -> Vec<super::command_palette::CommandSpec> {
    use super::command_palette::{ArgSpec, ArgType, CommandSpec};

    let folder_options = {
        let mut opts = vec!["(root)".into()];
        opts.extend(folder_names.iter().cloned());
        opts
    };
    let folder_only_options: Vec<String> = folder_names.iter().cloned().collect();

    vec![
        CommandSpec::no_arg("Open Command Palette"),
        CommandSpec::no_arg("Search Notes"),
        CommandSpec::with_args(
            "Create Note",
            vec![
                ArgSpec {
                    prompt: "Note title".into(),
                    arg_type: ArgType::FreeText { default: None },
                    optional: true,
                },
                ArgSpec {
                    prompt: "Folder".into(),
                    arg_type: ArgType::Select {
                        options: folder_options.clone(),
                    },
                    optional: true,
                },
            ],
        ),
        CommandSpec::with_args(
            "Create Folder",
            vec![
                ArgSpec {
                    prompt: "Folder name".into(),
                    arg_type: ArgType::FreeText { default: None },
                    optional: false,
                },
                ArgSpec {
                    prompt: "Parent folder".into(),
                    arg_type: ArgType::Select {
                        options: folder_options.clone(),
                    },
                    optional: true,
                },
            ],
        ),
        CommandSpec::with_args(
            "Move to Folder",
            vec![ArgSpec {
                prompt: "Target folder".into(),
                arg_type: ArgType::Select {
                    options: folder_options,
                },
                optional: true,
            }],
        ),
        CommandSpec::with_args(
            "Delete Folder",
            vec![
                ArgSpec {
                    prompt: "Folder".into(),
                    arg_type: ArgType::Select {
                        options: folder_only_options.clone(),
                    },
                    optional: false,
                },
                ArgSpec {
                    prompt: "Contents".into(),
                    arg_type: ArgType::Select {
                        options: vec!["Move notes to root".into(), "Delete notes too".into()],
                    },
                    optional: false,
                },
                ArgSpec {
                    prompt: "Confirm".into(),
                    arg_type: ArgType::Select {
                        options: vec!["Cancel".into(), "Yes, delete".into()],
                    },
                    optional: false,
                },
            ],
        ),
        CommandSpec::with_args(
            "Rename Folder",
            vec![
                ArgSpec {
                    prompt: "Folder".into(),
                    arg_type: ArgType::Select {
                        options: folder_only_options,
                    },
                    optional: false,
                },
                ArgSpec {
                    prompt: "New name".into(),
                    arg_type: ArgType::FreeText { default: None },
                    optional: false,
                },
            ],
        ),
        CommandSpec::no_arg("List Notes"),
        CommandSpec::no_arg("Show Tags"),
        CommandSpec::no_arg("Toggle Sidebar"),
        CommandSpec::no_arg("Save Note"),
        CommandSpec::no_arg("Quit"),
    ]
}
