use crate::engine::{SearchDocument, SearchIndex, SearchQuery, SearchResult};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::{Index, IndexWriter, ReloadPolicy, doc};
use uuid::Uuid;

pub struct TantivyIndex {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
}

impl TantivyIndex {
    pub fn open(path: &Path) -> Result<Self> {
        let schema = build_schema();
        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path)
                .with_context(|| format!("failed to open index at {}", path.display()))?
        } else {
            std::fs::create_dir_all(path).with_context(|| {
                format!("failed to create index directory at {}", path.display())
            })?;
            Index::create_in_dir(path, schema.clone()).with_context(|| "failed to create index")?
        };

        let writer = index
            .writer(15_000_000)
            .context("failed to create index writer")?;

        Ok(Self {
            index,
            schema: schema.clone(),
            writer: Mutex::new(writer),
        })
    }

    fn field_id(&self) -> Field {
        self.schema
            .get_field("id")
            .expect("id field exists in schema built by build_schema()")
    }

    fn field_title(&self) -> Field {
        self.schema
            .get_field("title")
            .expect("title field exists in schema built by build_schema()")
    }

    fn field_content(&self) -> Field {
        self.schema
            .get_field("content")
            .expect("content field exists in schema built by build_schema()")
    }

    fn field_tags(&self) -> Field {
        self.schema
            .get_field("tags")
            .expect("tags field exists in schema built by build_schema()")
    }

    fn field_path(&self) -> Field {
        self.schema
            .get_field("path")
            .expect("path field exists in schema built by build_schema()")
    }
}

impl SearchIndex for TantivyIndex {
    fn add_document(&self, doc_input: &SearchDocument) -> Result<()> {
        let id_str = doc_input.id.to_string();
        let doc = doc!(
            self.field_id() => id_str.as_str(),
            self.field_title() => doc_input.title.as_str(),
            self.field_content() => doc_input.content.as_str(),
            self.field_tags() => doc_input.tags.join(" "),
            self.field_path() => doc_input.path.to_string_lossy().as_ref(),
        );
        let mut writer = self.writer.lock().expect("index writer mutex not poisoned");
        writer
            .add_document(doc)
            .context("failed to add document to index")?;
        writer.commit().context("failed to commit index")?;
        Ok(())
    }

    fn remove_document(&self, id: &Uuid) -> Result<()> {
        let id_str = id.to_string();
        let term = tantivy::Term::from_field_text(self.field_id(), &id_str);
        let mut writer = self.writer.lock().expect("index writer mutex not poisoned");
        writer.delete_term(term);
        writer.commit().context("failed to commit after delete")?;
        Ok(())
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .context("failed to create index reader")?;

        let searcher = reader.searcher();
        let limit = query.limit.unwrap_or(20);

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.field_title(), self.field_content(), self.field_tags()],
        );

        let mut queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

        if !query.text.is_empty() {
            let parsed = query_parser
                .parse_query(&query.text)
                .context("failed to parse search query")?;
            queries.push((Occur::Must, parsed));
        }

        for tag in &query.tags {
            let term = tantivy::Term::from_field_text(self.field_tags(), tag);
            let q: Box<dyn tantivy::query::Query> =
                Box::new(TermQuery::new(term, IndexRecordOption::Basic));
            queries.push((Occur::Must, q));
        }

        let combined: Box<dyn tantivy::query::Query> = if queries.is_empty() {
            query_parser
                .parse_query("*")
                .context("failed to parse wildcard query")?
        } else {
            Box::new(BooleanQuery::new(queries))
        };

        let top_docs = searcher
            .search(&combined, &TopDocs::with_limit(limit))
            .context("failed to execute search")?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher
                .doc(doc_address)
                .context("failed to retrieve document")?;
            let id_str = doc
                .get_first(self.field_id())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = doc
                .get_first(self.field_title())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path_str = doc
                .get_first(self.field_path())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            results.push(SearchResult {
                id: Uuid::parse_str(id_str).unwrap_or_default(),
                title: title.to_string(),
                path: PathBuf::from(path_str),
                score,
                snippet: String::new(),
            });
        }

        Ok(results)
    }

    fn rebuild(&self, docs: &[SearchDocument]) -> Result<()> {
        {
            let mut writer = self.writer.lock().expect("index writer mutex not poisoned");
            writer
                .delete_all_documents()
                .context("failed to clear index")?;
            writer.commit().context("failed to commit after clear")?;
        }

        for doc_input in docs {
            self.add_document(doc_input)?;
        }
        Ok(())
    }
}

fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("id", STRING | STORED);
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT | STORED);
    schema_builder.add_text_field("tags", TEXT | STORED);
    schema_builder.add_text_field("path", TEXT | STORED);
    schema_builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SearchDocument;

    fn test_index() -> TantivyIndex {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let path = tmp.path().to_path_buf();
        // hold onto tmp so it doesn't get dropped and deleted
        std::mem::forget(tmp);
        TantivyIndex::open(&path).expect("open test index")
    }

    fn sample_doc(id: &str, title: &str, content: &str, tags: &[&str]) -> SearchDocument {
        SearchDocument {
            id: Uuid::parse_str(id).expect("valid UUID in test"),
            title: title.to_string(),
            content: content.to_string(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            path: PathBuf::from(format!("/tmp/{title}.md")),
        }
    }

    #[test]
    fn add_and_search() {
        let index = test_index();
        let doc = sample_doc(
            "00000000-0000-0000-0000-000000000001",
            "Rust Guide",
            "Learn Rust programming language",
            &["rust", "programming"],
        );
        index.add_document(&doc).expect("add document in test");

        let results = index
            .search(&SearchQuery {
                text: "Rust".to_string(),
                limit: None,
                tags: vec![],
            })
            .expect("search in test");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Guide");
    }

    #[test]
    fn search_by_tag() {
        let index = test_index();
        let doc = sample_doc(
            "00000000-0000-0000-0000-000000000002",
            "Note",
            "content",
            &["important"],
        );
        index.add_document(&doc).expect("add document in test");

        let results = index
            .search(&SearchQuery {
                text: String::new(),
                limit: None,
                tags: vec!["important".to_string()],
            })
            .expect("search in test");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn remove_document() {
        let index = test_index();
        let id =
            Uuid::parse_str("00000000-0000-0000-0000-000000000003").expect("valid UUID in test");
        let doc = sample_doc(&id.to_string(), "To Remove", "gone", &[]);
        index.add_document(&doc).expect("add document in test");
        index.remove_document(&id).expect("remove document in test");

        let results = index
            .search(&SearchQuery {
                text: "Remove".to_string(),
                limit: None,
                tags: vec![],
            })
            .expect("search in test");
        assert!(results.is_empty());
    }

    #[test]
    fn rebuild_clears_and_reindexes() {
        let index = test_index();
        let doc1 = sample_doc(
            "00000000-0000-0000-0000-000000000004",
            "Old",
            "old content",
            &[],
        );
        index.add_document(&doc1).expect("add document in test");

        let doc2 = sample_doc(
            "00000000-0000-0000-0000-000000000005",
            "New",
            "new content",
            &[],
        );
        index.rebuild(&[doc2]).expect("rebuild in test");

        let results = index
            .search(&SearchQuery {
                text: "Old".to_string(),
                limit: None,
                tags: vec![],
            })
            .expect("search in test");
        assert!(results.is_empty());

        let results = index
            .search(&SearchQuery {
                text: "New".to_string(),
                limit: None,
                tags: vec![],
            })
            .expect("search in test");
        assert_eq!(results.len(), 1);
    }
}
