use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use toml;
use zelkova_notes::DirectoryStructure;

/// Load the directory structure from `<vault>/.zelkova/structure.toml`.
///
/// Returns `DirectoryStructure::default()` when the file is missing — this
/// is the normal state for a freshly initialized vault.
pub fn load_directory_structure(vault_path: &Path) -> Result<DirectoryStructure> {
    let structure_path = vault_path.join(".zelkova").join("structure.toml");
    if !structure_path.exists() {
        return Ok(DirectoryStructure::default());
    }
    let content = fs::read_to_string(&structure_path)
        .with_context(|| format!("failed to read {}", structure_path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("failed to parse {}", structure_path.display()))
}

/// Persist the directory structure to `<vault>/.zelkova/structure.toml`,
/// creating the `.zelkova` directory if necessary.
pub fn save_directory_structure(structure: &DirectoryStructure, vault_path: &Path) -> Result<()> {
    let zelkova_dir = vault_path.join(".zelkova");
    fs::create_dir_all(&zelkova_dir)
        .with_context(|| format!("failed to create {}", zelkova_dir.display()))?;
    let structure_path = zelkova_dir.join("structure.toml");
    let content =
        toml::to_string_pretty(structure).context("failed to serialize directory structure")?;
    fs::write(&structure_path, &content)
        .with_context(|| format!("failed to write {}", structure_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let vault_path = tmp.path().to_path_buf();

        let mut ds = DirectoryStructure::default();
        let folder = ds.create_folder("Work", None);
        let note_id = Uuid::new_v4();
        ds.move_note_to_folder(note_id, Some(folder.id));
        save_directory_structure(&ds, &vault_path).expect("save directory structure");

        let loaded = load_directory_structure(&vault_path).expect("load directory structure");
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.folders[0].name, "Work");
        assert_eq!(loaded.mappings.len(), 1);
        assert_eq!(loaded.mappings[0].note, note_id);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let vault_path = tmp.path().to_path_buf();
        let ds = load_directory_structure(&vault_path).expect("load returns default");
        assert!(ds.folders.is_empty());
        assert!(ds.mappings.is_empty());
    }
}
