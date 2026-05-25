use crate::note::{Frontmatter, Note};
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct Vault {
    pub vault_path: PathBuf,
}

impl Vault {
    pub fn new(vault_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&vault_path).with_context(|| {
            format!(
                "failed to create vault directory at {}",
                vault_path.display()
            )
        })?;
        Ok(Self { vault_path })
    }

    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let mut notes = Vec::new();
        self.collect_notes(&self.vault_path, &mut notes)?;
        Ok(notes)
    }

    pub fn get_note(&self, relative_path: &Path) -> Result<Option<Note>> {
        let full_path = self.vault_path.join(relative_path);
        if !full_path.exists() {
            return Ok(None);
        }
        Ok(Some(self.parse_note_file(&full_path)?))
    }

    pub fn create_note(
        &self,
        title: Option<&str>,
        parent_dir: Option<&Path>,
        tags: HashSet<String>,
    ) -> Result<Note> {
        let dir = match parent_dir {
            Some(p) => self.vault_path.join(p),
            None => self.vault_path.clone(),
        };
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create directory at {}", dir.display()))?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let frontmatter = Frontmatter {
            id,
            title: title.unwrap_or("").to_string(),
            tags,
            created: now,
            updated: now,
        };

        let filename = format!("{id}.md");
        let path = dir.join(&filename);
        let content = format_note_file(&frontmatter, "");
        fs::write(&path, &content)
            .with_context(|| format!("failed to write note to {}", path.display()))?;

        Ok(Note {
            frontmatter,
            content: String::new(),
            path,
        })
    }

    pub fn delete_note(&self, relative_path: &Path) -> Result<()> {
        let full_path = self.vault_path.join(relative_path);
        if full_path.exists() {
            fs::remove_file(&full_path)
                .with_context(|| format!("failed to delete note at {}", full_path.display()))?;
        }
        Ok(())
    }

    pub fn all_tags(&self) -> Result<HashSet<String>> {
        let notes = self.list_notes()?;
        Ok(notes.into_iter().flat_map(|n| n.frontmatter.tags).collect())
    }

    fn collect_notes(&self, dir: &Path, notes: &mut Vec<Note>) -> Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // skip hidden directories like .zelkova
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        continue;
                    }
                }
                self.collect_notes(&path, notes)?;
            } else if path.extension().is_some_and(|e| e == "md") {
                match self.parse_note_file(&path) {
                    Ok(note) => notes.push(note),
                    Err(e) => eprintln!("warning: failed to parse {}: {e}", path.display()),
                }
            }
        }
        Ok(())
    }

    fn parse_note_file(&self, path: &Path) -> Result<Note> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let (frontmatter, body) = parse_frontmatter(&content)?;

        Ok(Note {
            frontmatter,
            content: body,
            path: path.to_path_buf(),
        })
    }
}

fn parse_frontmatter(content: &str) -> Result<(Frontmatter, String)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        anyhow::bail!("note does not start with YAML frontmatter");
    }

    let rest = &trimmed[3..];
    let Some(end_idx) = rest.find("---") else {
        anyhow::bail!("unclosed YAML frontmatter");
    };

    let yaml_str = &rest[..end_idx];
    let body = rest[end_idx + 3..].trim_start().to_string();

    let frontmatter: Frontmatter =
        serde_yaml::from_str(yaml_str).context("failed to parse YAML frontmatter")?;

    Ok((frontmatter, body))
}

fn format_note_file(frontmatter: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();
    format!("---\n{yaml}---\n{body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn vault_create_with_empty_title() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = Vault::new(tmp.path().to_path_buf()).unwrap();

        let note = vault.create_note(None, None, HashSet::new()).unwrap();
        assert!(note.path.exists());
        assert_eq!(note.frontmatter.title, "");

        let notes = vault.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].frontmatter.title, "");
    }

    #[test]
    fn vault_create_no_duplicate_filenames() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = Vault::new(tmp.path().to_path_buf()).unwrap();

        let note1 = vault
            .create_note(Some("Same Title"), None, HashSet::new())
            .unwrap();
        let note2 = vault
            .create_note(Some("Same Title"), None, HashSet::new())
            .unwrap();

        assert_ne!(note1.path, note2.path, "UUID filenames must differ");
        assert!(note1.path.exists());
        assert!(note2.path.exists());

        let notes = vault.list_notes().unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn parse_frontmatter_basic() {
        let content = "---\nid: \"00000000-0000-0000-0000-000000000001\"\ntitle: Test\ntags:\n  - rust\ncreated: 2025-01-01T00:00:00Z\nupdated: 2025-01-01T00:00:00Z\n---\nHello world\n";
        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm.title, "Test");
        assert!(fm.tags.contains("rust"));
        assert_eq!(body, "Hello world\n");
    }

    #[test]
    fn parse_frontmatter_missing() {
        let content = "just text";
        assert!(parse_frontmatter(content).is_err());
    }

    #[test]
    fn format_roundtrip() {
        let id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let now = "2025-01-01T00:00:00Z".parse().unwrap();
        let mut tags = HashSet::new();
        tags.insert("test".to_string());

        let fm = Frontmatter {
            id,
            title: "Round".to_string(),
            tags,
            created: now,
            updated: now,
        };

        let s = format_note_file(&fm, "body text");
        assert!(s.starts_with("---"));
        assert!(s.contains("title: Round"));
        assert!(s.contains("body text"));
    }

    #[test]
    fn vault_create_and_list() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = Vault::new(tmp.path().to_path_buf()).unwrap();

        let mut tags = HashSet::new();
        tags.insert("demo".to_string());
        let note = vault.create_note(Some("Test Note"), None, tags).unwrap();

        assert!(note.path.exists());
        assert!(
            note.path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(".md")
        );
        assert_ne!(
            note.path.file_stem().unwrap(),
            "Test Note",
            "filename should be UUID, not title"
        );

        let notes = vault.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].frontmatter.title, "Test Note");

        let all_tags = vault.all_tags().unwrap();
        assert!(all_tags.contains("demo"));
    }

    #[test]
    fn vault_create_in_subdirectory() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = Vault::new(tmp.path().to_path_buf()).unwrap();

        let note = vault
            .create_note(Some("Sub Note"), Some(Path::new("sub/dir")), HashSet::new())
            .unwrap();

        assert!(note.path.to_string_lossy().contains("sub/dir"));
        assert!(note.path.exists());

        let notes = vault.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn vault_delete_note() {
        let tmp = tempfile::tempdir().unwrap();
        let vault = Vault::new(tmp.path().to_path_buf()).unwrap();

        let note = vault
            .create_note(Some("To Delete"), None, HashSet::new())
            .unwrap();
        let rel = note
            .path
            .strip_prefix(&vault.vault_path)
            .unwrap()
            .to_path_buf();

        vault.delete_note(&rel).unwrap();
        assert!(!note.path.exists());
    }
}
