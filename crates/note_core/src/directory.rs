use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Folder {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteMapping {
    pub note: Uuid,
    pub folder: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DirectoryStructure {
    #[serde(default)]
    pub folders: Vec<Folder>,
    #[serde(default)]
    pub mappings: Vec<NoteMapping>,
}

#[derive(Debug, Clone)]
pub struct FolderTree {
    pub folder: Folder,
    pub children: Vec<FolderTree>,
    pub notes: Vec<Uuid>,
}

impl DirectoryStructure {
    pub fn load(vault_path: &PathBuf) -> Result<Self> {
        let structure_path = vault_path.join(".zelkova").join("structure.toml");
        if !structure_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&structure_path)
            .with_context(|| format!("failed to read {}", structure_path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("failed to parse {}", structure_path.display()))
    }

    pub fn save(&self, vault_path: &PathBuf) -> Result<()> {
        let zelkova_dir = vault_path.join(".zelkova");
        fs::create_dir_all(&zelkova_dir)
            .with_context(|| format!("failed to create {}", zelkova_dir.display()))?;
        let structure_path = zelkova_dir.join("structure.toml");
        let content =
            toml::to_string_pretty(self).context("failed to serialize directory structure")?;
        fs::write(&structure_path, &content)
            .with_context(|| format!("failed to write {}", structure_path.display()))
    }

    pub fn create_folder(&mut self, name: &str, parent: Option<Uuid>) -> Folder {
        let folder = Folder {
            id: Uuid::new_v4(),
            name: name.to_string(),
            parent,
        };
        self.folders.push(folder.clone());
        folder
    }

    pub fn move_note_to_folder(&mut self, note_id: Uuid, folder_id: Option<Uuid>) {
        // Remove existing mapping for this note
        self.mappings.retain(|m| m.note != note_id);

        // Add new mapping (None = root, no mapping needed)
        if let Some(fid) = folder_id {
            self.mappings.push(NoteMapping {
                note: note_id,
                folder: fid,
            });
        }
    }

    pub fn get_folder_for_note(&self, note_id: Uuid) -> Option<Uuid> {
        self.mappings
            .iter()
            .find(|m| m.note == note_id)
            .map(|m| m.folder)
    }

    pub fn get_folder(&self, folder_id: Uuid) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == folder_id)
    }

    pub fn build_tree(&self) -> Vec<FolderTree> {
        let root_folders: Vec<&Folder> =
            self.folders.iter().filter(|f| f.parent.is_none()).collect();
        root_folders
            .into_iter()
            .map(|f| self.build_subtree(f))
            .collect()
    }

    fn build_subtree(&self, folder: &Folder) -> FolderTree {
        let children: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent == Some(folder.id))
            .collect();

        let notes: Vec<Uuid> = self
            .mappings
            .iter()
            .filter(|m| m.folder == folder.id)
            .map(|m| m.note)
            .collect();

        FolderTree {
            folder: folder.clone(),
            children: children
                .into_iter()
                .map(|c| self.build_subtree(c))
                .collect(),
            notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_folder_root() {
        let mut ds = DirectoryStructure::default();
        let folder = ds.create_folder("Work", None);
        assert_eq!(folder.name, "Work");
        assert!(folder.parent.is_none());
        assert_eq!(ds.folders.len(), 1);
    }

    #[test]
    fn create_folder_nested() {
        let mut ds = DirectoryStructure::default();
        let parent = ds.create_folder("Work", None);
        let child = ds.create_folder("Projects", Some(parent.id));
        assert_eq!(child.parent, Some(parent.id));
    }

    #[test]
    fn move_note_to_folder() {
        let mut ds = DirectoryStructure::default();
        let folder = ds.create_folder("Work", None);
        let note_id = Uuid::new_v4();

        ds.move_note_to_folder(note_id, Some(folder.id));
        assert_eq!(ds.get_folder_for_note(note_id), Some(folder.id));
    }

    #[test]
    fn move_note_to_root() {
        let mut ds = DirectoryStructure::default();
        let folder = ds.create_folder("Work", None);
        let note_id = Uuid::new_v4();

        ds.move_note_to_folder(note_id, Some(folder.id));
        ds.move_note_to_folder(note_id, None);
        assert_eq!(ds.get_folder_for_note(note_id), None);
        assert!(ds.mappings.is_empty());
    }

    #[test]
    fn build_tree_nested() {
        let mut ds = DirectoryStructure::default();
        let work = ds.create_folder("Work", None);
        let personal = ds.create_folder("Personal", None);
        let projects = ds.create_folder("Projects", Some(work.id));

        let note1 = Uuid::new_v4();
        let note2 = Uuid::new_v4();
        ds.move_note_to_folder(note1, Some(work.id));
        ds.move_note_to_folder(note2, Some(projects.id));

        let tree = ds.build_tree();
        assert_eq!(tree.len(), 2); // Work, Personal

        let work_tree = tree
            .iter()
            .find(|t| t.folder.id == work.id)
            .expect("Work folder in tree");
        assert_eq!(work_tree.notes, vec![note1]);
        assert_eq!(work_tree.children.len(), 1);
        assert_eq!(work_tree.children[0].notes, vec![note2]);
    }

    #[test]
    fn save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_path = tmp.path().to_path_buf();

        let mut ds = DirectoryStructure::default();
        let folder = ds.create_folder("Work", None);
        let note_id = Uuid::new_v4();
        ds.move_note_to_folder(note_id, Some(folder.id));
        ds.save(&vault_path).unwrap();

        let loaded = DirectoryStructure::load(&vault_path).unwrap();
        assert_eq!(loaded.folders.len(), 1);
        assert_eq!(loaded.folders[0].name, "Work");
        assert_eq!(loaded.mappings.len(), 1);
        assert_eq!(loaded.mappings[0].note, note_id);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_path = tmp.path().to_path_buf();
        let ds = DirectoryStructure::load(&vault_path).unwrap();
        assert!(ds.folders.is_empty());
        assert!(ds.mappings.is_empty());
    }
}
