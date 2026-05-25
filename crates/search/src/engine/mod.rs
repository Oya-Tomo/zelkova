use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocument {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub text: String,
    pub limit: Option<usize>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: Uuid,
    pub title: String,
    pub path: PathBuf,
    pub score: f32,
    pub snippet: String,
}

pub trait SearchIndex: Send + Sync {
    fn add_document(&self, doc: &SearchDocument) -> Result<()>;
    fn remove_document(&self, id: &Uuid) -> Result<()>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    fn rebuild(&self, docs: &[SearchDocument]) -> Result<()>;
}
