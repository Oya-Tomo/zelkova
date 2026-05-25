use crate::DaemonState;
use anyhow::Result;
use std::path::Path;
use zelkova_search::SearchDocument;

pub fn rebuild_index(state: &DaemonState) -> Result<usize> {
    let notes = state.vault.list_notes()?;
    let docs: Vec<SearchDocument> = notes
        .into_iter()
        .map(|note| SearchDocument {
            id: note.frontmatter.id,
            title: note.frontmatter.title,
            content: note.content,
            tags: note.frontmatter.tags.into_iter().collect(),
            path: note.path,
        })
        .collect();

    let count = docs.len();
    state.search_index.rebuild(&docs)?;
    Ok(count)
}

pub fn reindex_note(path: &Path, state: &DaemonState) -> Result<()> {
    let notes = state.vault.list_notes()?;
    let note = notes
        .into_iter()
        .find(|n| n.path == path)
        .ok_or_else(|| anyhow::anyhow!("note not found at {}", path.display()))?;

    let doc = SearchDocument {
        id: note.frontmatter.id,
        title: note.frontmatter.title,
        content: note.content,
        tags: note.frontmatter.tags.into_iter().collect(),
        path: note.path,
    };

    state.search_index.remove_document(&doc.id)?;
    state.search_index.add_document(&doc)?;
    Ok(())
}
