//! Pure data models for notes — no filesystem access.
//!
//! This crate is safe to import from any client (gui, cli, rpc, daemon).
//! All `std::fs` operations live in `zelkova-vault`, which is a daemon-only
//! dependency per ADR-0001.

pub mod directory;
pub mod note;
pub mod parse;

pub use directory::{DirectoryStructure, Folder, FolderTree, NoteMapping};
pub use note::{Frontmatter, Note};
pub use parse::{extract_title_from_body, format_note_file, parse_frontmatter, parse_note_content};
