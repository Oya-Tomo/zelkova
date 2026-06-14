// Vault FS access is the whole point of this crate (called only by the daemon).
// The workspace-wide `disallowed_methods` lint guards front-end crates
// (gui/cli) from touching the vault directly; this crate is the legitimate
// back-end owner of those operations.
#![allow(clippy::disallowed_methods)]

pub mod directory;
pub mod note;
pub mod vault;

pub use directory::{DirectoryStructure, Folder, FolderTree, NoteMapping};
pub use note::{Frontmatter, Note};
pub use vault::Vault;
pub use vault::format_note_file;
pub use vault::parse_note_content;
