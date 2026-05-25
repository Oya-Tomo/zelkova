# rpc

## Role

JSON-RPC 2.0 over Unix domain socketsによるプロセス間通信 (CLI/CLI ↔ デーモン) を提供するcrate。

## Module Layout

```
src/
├── lib.rs      モジュール宣言, typesのre-export
├── types.rs    JSON-RPC型, メソッド定数, パラメータ/結果構造体
├── server.rs   RpcServer — ソケットのbind, accept, 接続処理
└── client.rs   RpcClient — 高レベルAPI (各RPCメソッドのラッパー)
```

## Dependencies

- `serde` / `serde_json` — JSON-RPCメッセージのシリアライズ
- `uuid` — Note ID
- `anyhow` — エラーハンドリング

## Key Types / APIs

### JSON-RPC基本型 (types.rs)

```rust
struct JsonRpcRequest {
    jsonrpc: String,              // 常に "2.0"
    id: Option<Value>,            // リクエストID (通知の場合はNone)
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

**JsonRpcErrorヘルパー:**

| メソッド | コード | 用途 |
|---|---|---|
| `not_found(msg)` | `-32001` | リソース未検出 |
| `invalid_params(msg)` | `-32602` | パラメータ不正 |
| `internal(msg)` | `-32603` | 内部エラー |

**JsonRpcRequestヘルパー:**
- `new(id, method, params)` — 通常リクエスト (ID付き)
- `notification(method, params)` — 通知 (IDなし)

**JsonRpcResponseヘルパー:**
- `success(id, result)` — 成功レスポンス
- `error(id, error)` — エラーレスポンス

### メソッド定数

| 定数 | 値 | パラメータ | レスポンス |
|---|---|---|---|
| `METHOD_SEARCH` | `"search"` | `SearchParams` | `SearchResults` |
| `METHOD_LIST_NOTES` | `"list_notes"` | `ListNotesParams` | `ListNotesResult` |
| `METHOD_GET_NOTE` | `"get_note"` | `GetNoteParams` | `GetNoteResult` |
| `METHOD_CREATE_NOTE` | `"create_note"` | `CreateNoteParams` | `CreateNoteResult` |
| `METHOD_TAGS` | `"tags"` | なし | `TagsResult` |
| `METHOD_REBUILD_INDEX` | `"rebuild_index"` | なし | `RebuildIndexResult` |
| `METHOD_NOTE_UPDATED` | `"note_updated"` | `NoteUpdatedParams` | `{status: "ok"}` |

### パラメータ/結果型

```rust
struct SearchParams {
    query: String,
    tags: Vec<String>,          // デフォルト空
    limit: Option<usize>,       // デフォルトNone
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

デーモン側のサーバー。

```rust
struct RpcServer {
    listener: UnixListener,
    socket_path: PathBuf,
}
```

| メソッド | 説明 |
|---|---|
| `bind(socket_path)` | ソケットを作成してバインド (既存ファイルは削除) |
| `accept_one(handler)` | 1接続を受け付け、handlerで処理 |
| `socket_path()` | ソケットパスへの参照 |

**Drop実装:** ソケットファイルを自動削除。

**プロトコル:** 1行 = 1JSON-RPCメッセージ。リクエスト→レスポンスの1往復で接続を閉じる。

### RpcClient (client.rs)

CLI側のクライアント。

```rust
struct RpcClient {
    socket_path: PathBuf,
}
```

| メソッド | 対応RPC | 説明 |
|---|---|---|
| `new(socket_path)` | — | クライアントを初期化 |
| `send_request(request)` | — | 低レベル送信 (接続→書き込み→読み取り→切断) |
| `search(query, tags, limit)` | search | ノート検索 |
| `list_notes(tag)` | list_notes | ノート一覧 (タグフィルタ付き) |
| `get_note(id)` | get_note | ノート詳細取得 |
| `create_note(title, dir, tags)` | create_note | ノート作成 |
| `tags()` | tags | 全タグ一覧 |
| `note_updated(path)` | note_updated | ファイル更新通知 |

**ID生成:** `AtomicU64`のインクリメンタルカウンター (Relaxed順序)。

## Data Flow

```
CLI                              デーモン
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
  │  (接続終了)                      │
```
