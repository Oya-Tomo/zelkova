use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Frontmatter {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub tags: HashSet<String>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Note {
    pub frontmatter: Frontmatter,
    pub content: String,
    pub path: PathBuf,
}

impl Note {
    pub fn id(&self) -> &Uuid {
        &self.frontmatter.id
    }

    pub fn title(&self) -> &str {
        &self.frontmatter.title
    }

    pub fn tags(&self) -> &HashSet<String> {
        &self.frontmatter.tags
    }

    pub fn created(&self) -> &DateTime<Utc> {
        &self.frontmatter.created
    }

    pub fn updated(&self) -> &DateTime<Utc> {
        &self.frontmatter.updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_accessors() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let mut tags = HashSet::new();
        tags.insert("rust".to_string());
        tags.insert("note".to_string());

        let note = Note {
            frontmatter: Frontmatter {
                id,
                title: "Test".to_string(),
                tags: tags.clone(),
                created: now,
                updated: now,
            },
            content: "body".to_string(),
            path: PathBuf::from("/tmp/test.md"),
        };

        assert_eq!(note.id(), &id);
        assert_eq!(note.title(), "Test");
        assert_eq!(note.tags(), &tags);
    }
}
