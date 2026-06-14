// The vault crate is the legitimate owner of FS access for note storage per
// ADR-0001. The workspace-wide `disallowed_methods` lint guards front-end
// crates (gui/cli) from touching the vault directly; this crate is the
// daemon-internal implementation of those operations.
#![allow(clippy::disallowed_methods)]

pub mod directory_io;
pub mod vault;

pub use directory_io::{load_directory_structure, save_directory_structure};
pub use vault::Vault;

// Re-export the model types so daemon code can `use zelkova_vault::Frontmatter`
// instead of pulling in a second dependency. Front-end crates should still
// import them through `zelkova_rpc` so they have no direct dependency on
// this FS-touching crate.
pub use zelkova_notes::{DirectoryStructure, Folder, FolderTree, Frontmatter, Note, NoteMapping};
