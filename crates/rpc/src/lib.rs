pub mod client;
pub mod server;
pub mod types;

pub use types::*;

// Re-export the pure model types from zelkova-notes so front-end crates
// (gui, cli) can depend on `zelkova-rpc` alone and not pull in the FS
// layer. Per ADR-0001, the daemon is the only crate that should touch
// `zelkova-vault` directly.
pub use zelkova_notes::{DirectoryStructure, Folder, FolderTree, Frontmatter, Note, NoteMapping};
