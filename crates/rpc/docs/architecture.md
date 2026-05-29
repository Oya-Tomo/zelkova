# rpc

## Role

A crate providing inter-process communication (CLI/GUI ↔ daemon) via JSON-RPC 2.0 over Unix domain sockets.

## Module Layout

```
src/
├── lib.rs      Module declarations, re-exports of types
├── types.rs    JSON-RPC types, method constants, parameter/result structs
├── server.rs   RpcServer — socket bind, accept, connection handling
└── client.rs   RpcClient — high-level API (wrapper for each RPC method)
```

## Dependencies

- `serde` / `serde_json` — JSON-RPC message serialization
- `uuid` — Note ID
- `anyhow` — Error handling

## Key Types / APIs

### JSON-RPC Base Types (types.rs)

```rust
struct JsonRpcRequest {
    jsonrpc: String,              // Always "2.0"
    id: Option<Value>,            // Request ID (None for notifications)
    method: String,
    params: Option<Value>,
}

struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<Value>,
}
```

**JsonRpcError helpers:**

| Method | Code | Purpose |
|---|---|---|
| `not_found(msg)` | `-32001` | Resource not found |
| `invalid_params(msg)` | `-32602` | Invalid parameters |
| `internal(msg)` | `-32603` | Internal error |

**JsonRpcRequest helpers:**
- `new(id, method, params)` — Regular request (with ID)
- `notification(method, params)` — Notification (without ID)

**JsonRpcResponse helpers:**
- `success(id, result)` — Success response
- `error(id, error)` — Error response

### Method Constants

| Constant | Value | Parameters | Response |
|---|---|---|---|
| `METHOD_SEARCH` | `"search"` | `SearchParams` | `SearchResults` |
| `METHOD_LIST_NOTES` | `"list_notes"` | `ListNotesParams` | `ListNotesResult` |
| `METHOD_GET_NOTE` | `"get_note"` | `GetNoteParams` | `GetNoteResult` |
| `METHOD_CREATE_NOTE` | `"create_note"` | `CreateNoteParams` | `CreateNoteResult` |
| `METHOD_TAGS` | `"tags"` | None | `TagsResult` |
| `METHOD_REBUILD_INDEX` | `"rebuild_index"` | None | `RebuildIndexResult` |
| `METHOD_NOTE_UPDATED` | `"note_updated"` | `NoteUpdatedParams` | `{status: "ok"}` |

### Parameter/Result Types

```rust
struct SearchParams {
    query: String,
    tags: Vec<String>,          // Default: empty
    limit: Option<usize>,       // Default: None
}

struct SearchResults {
    results: Vec<SearchHit>,
}

struct SearchHit {
    id: Uuid,
    title: String,
    path: PathBuf,
    score: f32,
    snippet: String,
}

struct ListNotesParams {
    tag: Option<String>,
}

struct ListNotesResult {
    notes: Vec<NoteSummary>,
}

struct NoteSummary {
    id: Uuid,
    title: String,
    path: PathBuf,
    tags: Vec<String>,
}

struct GetNoteParams {
    id: Uuid,
}

struct GetNoteResult {
    id: Uuid,
    title: String,
    path: PathBuf,
    tags: Vec<String>,
    content: String,
    created: String,      // RFC 3339
    updated: String,      // RFC 3339
}

struct CreateNoteParams {
    title: String,
    directory: Option<String>,
    tags: Vec<String>,
}

struct CreateNoteResult {
    id: Uuid,
    title: String,
    path: PathBuf,
}

struct TagsResult {
    tags: Vec<String>,
}

struct RebuildIndexResult {
    indexed_count: usize,
}

struct NoteUpdatedParams {
    path: PathBuf,
}
```

### RpcServer (server.rs)

Server for the daemon side.

```rust
struct RpcServer {
    listener: UnixListener,
    socket_path: PathBuf,
}
```

| Method | Description |
|---|---|
| `bind(socket_path)` | Create and bind socket (deletes existing file) |
| `accept_one(handler)` | Accept one connection and process with handler |
| `socket_path()` | Reference to socket path |

**Drop implementation:** Automatically deletes the socket file.

**Protocol:** One line = one JSON-RPC message. Connection closes after a single request-response round trip.

### RpcClient (client.rs)

Client for the CLI side.

```rust
struct RpcClient {
    socket_path: PathBuf,
}
```

| Method | Corresponding RPC | Description |
|---|---|---|
| `new(socket_path)` | — | Initialize client |
| `send_request(request)` | — | Low-level send (connect → write → read → disconnect) |
| `search(query, tags, limit)` | search | Search notes |
| `list_notes(tag)` | list_notes | List notes (with tag filter) |
| `get_note(id)` | get_note | Get note details |
| `create_note(title, dir, tags)` | create_note | Create a note |
| `tags()` | tags | List all tags |
| `note_updated(path)` | note_updated | Notify file update |

**ID generation:** Incremental counter via `AtomicU64` (Relaxed ordering).

## Data Flow

```
CLI                              Daemon
  │                                 │
  │  ── UnixStream::connect() ──>  │
  │                                 │
  │  ── JSON request + "\n" ────>  │  accept_one()
  │                                 │  │
  │                                 │  handler(request)
  │                                 │  → JsonRpcResponse
  │                                 │
  │  <── JSON response + "\n" ──  │
  │                                 │
  │  (Connection closed)            │
```
