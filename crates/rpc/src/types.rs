use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32001,
            message: message.into(),
            data: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(id.into())),
            method: method.to_string(),
            params,
        }
    }

    pub fn notification(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params,
        }
    }
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

// IPC method parameters and results

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub results: Vec<SearchHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: Uuid,
    pub title: String,
    pub path: PathBuf,
    pub score: f32,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNotesParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNotesResult {
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: Uuid,
    pub title: String,
    pub path: PathBuf,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNoteParams {
    pub id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNoteResult {
    pub id: Uuid,
    pub title: String,
    pub path: PathBuf,
    pub tags: Vec<String>,
    pub content: String,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNoteParams {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNoteResult {
    pub id: Uuid,
    pub title: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsResult {
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildIndexResult {
    pub indexed_count: usize,
}

pub const METHOD_SEARCH: &str = "search";
pub const METHOD_LIST_NOTES: &str = "list_notes";
pub const METHOD_GET_NOTE: &str = "get_note";
pub const METHOD_CREATE_NOTE: &str = "create_note";
pub const METHOD_TAGS: &str = "tags";
pub const METHOD_REBUILD_INDEX: &str = "rebuild_index";
pub const METHOD_NOTE_UPDATED: &str = "note_updated";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteUpdatedParams {
    pub path: PathBuf,
}
