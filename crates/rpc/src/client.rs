use crate::types::*;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use uuid::Uuid;

pub struct RpcClient {
    socket_path: std::path::PathBuf,
}

impl RpcClient {
    pub fn new(socket_path: &Path) -> Self {
        Self {
            socket_path: socket_path.to_path_buf(),
        }
    }

    pub fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut stream = UnixStream::connect(&self.socket_path).with_context(|| {
            format!(
                "failed to connect to socket at {}",
                self.socket_path.display()
            )
        })?;

        let json = serde_json::to_string(request).context("failed to serialize request")?;
        stream.write_all(json.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .context("failed to read response")?;

        let response: JsonRpcResponse =
            serde_json::from_str(&response_line).context("failed to parse response")?;

        Ok(response)
    }

    pub fn search(
        &self,
        query: &str,
        tags: Vec<String>,
        limit: Option<usize>,
    ) -> Result<SearchResults> {
        let params = SearchParams {
            query: query.to_string(),
            tags,
            limit,
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_SEARCH,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("search error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse search results")
    }

    pub fn list_notes(&self, tag: Option<&str>) -> Result<ListNotesResult> {
        let params = ListNotesParams {
            tag: tag.map(String::from),
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_LIST_NOTES,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("list_notes error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse list_notes result")
    }

    pub fn get_note(&self, id: &uuid::Uuid) -> Result<GetNoteResult> {
        let params = GetNoteParams { id: *id };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_GET_NOTE,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("get_note error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse get_note result")
    }

    pub fn create_note(&self, title: Option<&str>, tags: Vec<String>) -> Result<CreateNoteResult> {
        let params = CreateNoteParams {
            title: title.map(String::from),
            tags,
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_CREATE_NOTE,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("create_note error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse create_note result")
    }

    pub fn tags(&self) -> Result<TagsResult> {
        let request = JsonRpcRequest::new(next_id(), METHOD_TAGS, None);
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("tags error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse tags result")
    }

    pub fn note_updated(&self, path: &std::path::Path) -> Result<()> {
        let params = NoteUpdatedParams {
            path: path.to_path_buf(),
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_NOTE_UPDATED,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("note_updated error: {} ({})", error.message, error.code);
        }

        Ok(())
    }

    pub fn create_folder(&self, name: &str, parent: Option<Uuid>) -> Result<CreateFolderResult> {
        let params = CreateFolderParams {
            name: name.to_string(),
            parent,
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_CREATE_FOLDER,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("create_folder error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse create_folder result")
    }

    pub fn move_note(&self, note_id: Uuid, folder_id: Option<Uuid>) -> Result<()> {
        let params = MoveNoteParams { note_id, folder_id };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_MOVE_NOTE,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("move_note error: {} ({})", error.message, error.code);
        }

        Ok(())
    }

    pub fn list_tree(&self) -> Result<ListTreeResult> {
        let request = JsonRpcRequest::new(next_id(), METHOD_LIST_TREE, None);
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("list_tree error: {} ({})", error.message, error.code);
        }

        let result = response.result.context("no result in response")?;
        serde_json::from_value(result).context("failed to parse list_tree result")
    }

    pub fn delete_folder(&self, folder_id: Uuid, cascade: bool) -> Result<()> {
        let params = DeleteFolderParams { folder_id, cascade };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_DELETE_FOLDER,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("delete_folder error: {} ({})", error.message, error.code);
        }

        Ok(())
    }

    pub fn rename_folder(&self, folder_id: Uuid, new_name: &str) -> Result<()> {
        let params = RenameFolderParams {
            folder_id,
            new_name: new_name.to_string(),
        };
        let request = JsonRpcRequest::new(
            next_id(),
            METHOD_RENAME_FOLDER,
            Some(serde_json::to_value(params)?),
        );
        let response = self.send_request(&request)?;

        if let Some(error) = response.error {
            anyhow::bail!("rename_folder error: {} ({})", error.message, error.code);
        }

        Ok(())
    }
}

fn next_id() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
