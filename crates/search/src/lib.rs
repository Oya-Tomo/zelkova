pub mod engine;

pub use engine::{SearchDocument, SearchIndex, SearchQuery, SearchResult};

#[cfg(feature = "tantivy")]
pub mod tantivy_backend;

#[cfg(feature = "tantivy")]
pub fn default_search_index(path: &std::path::Path) -> anyhow::Result<Box<dyn SearchIndex>> {
    Ok(Box::new(tantivy_backend::TantivyIndex::open(path)?))
}

#[cfg(not(feature = "tantivy"))]
pub fn default_search_index(_path: &std::path::Path) -> anyhow::Result<Box<dyn SearchIndex>> {
    compile_error!("no search backend enabled; enable the `tantivy` feature");
}
