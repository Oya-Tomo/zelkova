use crate::DaemonState;
use std::collections::HashSet;
use zelkova_rpc::*;
use zelkova_search::SearchQuery;

pub fn handle_request(request: JsonRpcRequest, state: &DaemonState) -> JsonRpcResponse {
    let result = match request.method.as_str() {
        METHOD_SEARCH => handle_search(&request, state),
        METHOD_LIST_NOTES => handle_list_notes(&request, state),
        METHOD_GET_NOTE => handle_get_note(&request, state),
        METHOD_CREATE_NOTE => handle_create_note(&request, state),
        METHOD_CREATE_FOLDER => handle_create_folder(&request, state),
        METHOD_MOVE_NOTE => handle_move_note(&request, state),
        METHOD_LIST_TREE => handle_list_tree(state),
        METHOD_DELETE_FOLDER => handle_delete_folder(&request, state),
        METHOD_RENAME_FOLDER => handle_rename_folder(&request, state),
        METHOD_TAGS => handle_tags(state),
        METHOD_REBUILD_INDEX => handle_rebuild_index(state),
        METHOD_NOTE_UPDATED => handle_note_updated(&request, state),
        _ => Err(JsonRpcError::not_found(format!(
            "unknown method: {}",
            request.method
        ))),
    };

    match result {
        Ok(value) => JsonRpcResponse::success(request.id, value),
        Err(error) => JsonRpcResponse::error(request.id, error),
    }
}

fn handle_search(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchParams = parse_params(request)?;
    let query = SearchQuery {
        text: params.query,
        limit: params.limit,
        tags: params.tags,
    };
    let results = state
        .search_index
        .search(&query)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let hits: Vec<SearchHit> = results
        .into_iter()
        .map(|r| SearchHit {
            id: r.id,
            title: r.title,
            path: r.path,
            score: r.score,
            snippet: r.snippet,
        })
        .collect();

    serde_json::to_value(SearchResults { results: hits })
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_list_notes(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ListNotesParams = parse_params(request)?;
    let notes = state
        .vault
        .list_notes()
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let mut summaries: Vec<NoteSummary> = notes
        .into_iter()
        .map(|n| NoteSummary {
            id: n.frontmatter.id,
            title: n.frontmatter.title,
            path: n.path,
            tags: n.frontmatter.tags.into_iter().collect(),
        })
        .collect();

    if let Some(tag) = &params.tag {
        summaries.retain(|s| s.tags.contains(tag));
    }

    serde_json::to_value(ListNotesResult { notes: summaries })
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_get_note(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: GetNoteParams = parse_params(request)?;
    let notes = state
        .vault
        .list_notes()
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let note = notes
        .into_iter()
        .find(|n| n.frontmatter.id == params.id)
        .ok_or_else(|| JsonRpcError::not_found("note not found"))?;

    let result = GetNoteResult {
        id: note.frontmatter.id,
        title: note.frontmatter.title,
        path: note.path,
        tags: note.frontmatter.tags.into_iter().collect(),
        content: note.content,
        created: note.frontmatter.created.to_rfc3339(),
        updated: note.frontmatter.updated.to_rfc3339(),
    };

    serde_json::to_value(result).map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_create_note(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CreateNoteParams = parse_params(request)?;
    let tags: HashSet<String> = params.tags.into_iter().collect();

    let note = state
        .vault
        .create_note(params.title.as_deref(), tags)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let result = CreateNoteResult {
        id: note.frontmatter.id,
        title: note.frontmatter.title,
        path: note.path,
    };

    serde_json::to_value(result).map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_tags(state: &DaemonState) -> Result<serde_json::Value, JsonRpcError> {
    let tags = state
        .vault
        .all_tags()
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let mut tag_list: Vec<String> = tags.into_iter().collect();
    tag_list.sort();

    serde_json::to_value(TagsResult { tags: tag_list })
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_rebuild_index(state: &DaemonState) -> Result<serde_json::Value, JsonRpcError> {
    let count =
        crate::indexer::rebuild_index(state).map_err(|e| JsonRpcError::internal(e.to_string()))?;

    serde_json::to_value(RebuildIndexResult {
        indexed_count: count,
    })
    .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_note_updated(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NoteUpdatedParams = parse_params(request)?;

    crate::indexer::reindex_note(&params.path, state)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    serde_json::to_value(serde_json::json!({"status": "ok"}))
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_create_folder(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CreateFolderParams = parse_params(request)?;
    let mut directory = state
        .directory
        .lock()
        .map_err(|e| JsonRpcError::internal(format!("lock error: {e}")))?;
    let folder = directory.create_folder(&params.name, params.parent);
    directory
        .save(&state.vault.vault_path)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    let result = CreateFolderResult {
        id: folder.id,
        name: folder.name,
        parent: folder.parent,
    };
    serde_json::to_value(result).map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_move_note(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: MoveNoteParams = parse_params(request)?;
    let mut directory = state
        .directory
        .lock()
        .map_err(|e| JsonRpcError::internal(format!("lock error: {e}")))?;
    directory.move_note_to_folder(params.note_id, params.folder_id);
    directory
        .save(&state.vault.vault_path)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    serde_json::to_value(serde_json::json!({"status": "ok"}))
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_list_tree(state: &DaemonState) -> Result<serde_json::Value, JsonRpcError> {
    let directory = state
        .directory
        .lock()
        .map_err(|e| JsonRpcError::internal(format!("lock error: {e}")))?;

    let folders: Vec<FolderInfo> = directory
        .folders
        .iter()
        .map(|f| FolderInfo {
            id: f.id,
            name: f.name.clone(),
            parent: f.parent,
        })
        .collect();

    let mappings: Vec<NoteMappingInfo> = directory
        .mappings
        .iter()
        .map(|m| NoteMappingInfo {
            note_id: m.note,
            folder_id: m.folder,
        })
        .collect();

    serde_json::to_value(ListTreeResult { folders, mappings })
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_delete_folder(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: DeleteFolderParams = parse_params(request)?;
    let mut directory = state
        .directory
        .lock()
        .map_err(|e| JsonRpcError::internal(format!("lock error: {e}")))?;

    directory
        .delete_folder(params.folder_id)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;
    directory
        .save(&state.vault.vault_path)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    serde_json::to_value(serde_json::json!({"status": "ok"}))
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn handle_rename_folder(
    request: &JsonRpcRequest,
    state: &DaemonState,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RenameFolderParams = parse_params(request)?;
    let mut directory = state
        .directory
        .lock()
        .map_err(|e| JsonRpcError::internal(format!("lock error: {e}")))?;

    if !directory.rename_folder(params.folder_id, &params.new_name) {
        return Err(JsonRpcError::not_found("folder not found"));
    }
    directory
        .save(&state.vault.vault_path)
        .map_err(|e| JsonRpcError::internal(e.to_string()))?;

    serde_json::to_value(serde_json::json!({"status": "ok"}))
        .map_err(|e| JsonRpcError::internal(e.to_string()))
}

fn parse_params<T: serde::de::DeserializeOwned>(
    request: &JsonRpcRequest,
) -> Result<T, JsonRpcError> {
    let params = request
        .params
        .as_ref()
        .ok_or_else(|| JsonRpcError::invalid_params("missing params"))?;

    serde_json::from_value(params.clone()).map_err(|e| JsonRpcError::invalid_params(e.to_string()))
}
