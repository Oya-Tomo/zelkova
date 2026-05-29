# daemon

## Role

Background daemon process. Provides note CRUD and search via an RPC server, and automatically updates the index through file watching.

## Module Layout

```
src/
├── main.rs      Entry point, DaemonState, main loop
├── handlers.rs  RPC method handlers
├── indexer.rs   Index rebuild and single-note reindexing
└── watcher.rs   Polling-based file watcher
```

## Dependencies

- `zelkova-config` — Configuration loading
- `zelkova-note-core` — Vault, Note, Frontmatter
- `zelkova-rpc` — RPC types and server
- `zelkova-search` — Search index
- `anyhow` — Error handling

## Key Types / APIs

### DaemonState (main.rs)

Global daemon state. Shared across threads via `Arc<DaemonState>`.

```rust
struct DaemonState {
    vault: Vault,                       // Collection of notes on the filesystem
    search_index: Box<dyn SearchIndex>, // Search index
    config: AppConfig,                  // Application settings
}
```

### Main Loop (main.rs)

```
1. AppConfig::load()                        → Load configuration
2. Vault::new(vault_path)                   → Initialize Vault
3. default_search_index(&index_path)        → Initialize search index
4. Wrap DaemonState in Arc
5. If index_on_start is true, rebuild_index() → Initial index build
6. RpcServer::bind(&socket_path)            → Bind socket
7. write_pid_file()                         → Write PID file
8. start_watcher(state.clone())             → Start file watcher
9. loop { server.accept_one(&handler) }     → Accept connections
```

**PID file:** Writes the process ID to `{vault_path}/.zelkova/daemon.pid`. Used by the CLI to check if the daemon is alive.

**Index path:** `{vault_path}/.zelkova/index/`

### RPC Handlers (handlers.rs)

`handle_request(request, state)` dispatches by method name:

| RPC Method | Handler | Description |
|---|---|---|
| `search` | `handle_search` | Search index with SearchQuery → SearchHit[] |
| `list_notes` | `handle_list_notes` | Get all notes from Vault, apply tag filter → NoteSummary[] |
| `get_note` | `handle_get_note` | Find note by ID → GetNoteResult |
| `create_note` | `handle_create_note` | Create a new note in Vault → CreateNoteResult |
| `tags` | `handle_tags` | Return all tags sorted → TagsResult |
| `rebuild_index` | `handle_rebuild_index` | Rebuild entire index → RebuildIndexResult |
| `note_updated` | `handle_note_updated` | Reindex the note at the specified path |

**Parameter parsing:** `parse_params<T>(request)` deserializes `request.params` into a typed struct. Returns an `invalid_params` error on failure.

**Error handling:** All handlers return `Result<Value, JsonRpcError>`, which `handle_request` converts into success/error responses.

### Indexer (indexer.rs)

| Function | Signature | Description |
|---|---|---|
| `rebuild_index` | `&DaemonState -> Result<usize>` | Re-register all notes in the index (deletes all existing entries) |
| `reindex_note` | `&Path, &DaemonState -> Result<()>` | Delete then re-register the note at the specified path |

**rebuild_index:**
1. Get all notes via `vault.list_notes()`
2. Convert each Note to a SearchDocument
3. Rebuild index via `search_index.rebuild(&docs)`
4. Return the count of indexed documents

**reindex_note:**
1. Find the note at the specified path from `vault.list_notes()`
2. `search_index.remove_document(&id)` → `search_index.add_document(&doc)`

### Watcher (watcher.rs)

Polling-based file watcher. Does not depend on inotify or similar.

```rust
fn start_watcher(state: Arc<DaemonState>) -> Result<()>
```

**Behavior:**
1. Starts on a separate thread
2. Take a snapshot via `scan_files(vault_path)` (HashMap<PathBuf, SystemTime>)
3. Sleep for 2 seconds
4. Take a new snapshot
5. Compare with previous snapshot:
   - Modified files → reindex via `reindex_note()`
   - New files → add to index via `reindex_note()`
   - Deleted files → log only (removal from index not yet implemented)
6. Update snapshot and return to step 3

**scan_files:** Recursively collects `.md` files. Skips hidden directories.

## Data Flow

```
┌──────────────────────────────────────────────────────┐
│                     Daemon Process                     │
│                                                      │
│  main()                                              │
│    ├── Vault::new(vault_path)                        │
│    ├── TantivyIndex::open(index_path)                │
│    ├── rebuild_index()                               │
│    ├── RpcServer::bind(socket_path)                  │
│    ├── write_pid_file()                              │
│    ├── start_watcher() ──┐                           │
│    │                     │                           │
│    └── loop {            │   Separate thread:        │
│          accept_one()    │   Every 2 seconds:        │
│          │               │   scan_files()            │
│          handlers:       │   Detect changes          │
│          ├─ search       │   → reindex_note()        │
│          ├─ list_notes   │                           │
│          ├─ get_note     │                           │
│          ├─ create_note  │                           │
│          ├─ tags         │                           │
│          ├─ rebuild      │                           │
│          └─ note_updated │                           │
│        }                │                           │
│                         └── (Filesystem monitoring)   │
└──────────────────────────────────────────────────────┘
        │
        │ Unix Domain Socket
        │
┌───────┴──────┐
│  CLI / GUI   │
└──────────────┘
```
