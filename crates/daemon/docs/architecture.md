# daemon

## Role

バックグラウンドデーモンプロセス。RPCサーバー経由でノートCRUD・検索を提供し、ファイル監視でインデックスを自動更新する。

## Module Layout

```
src/
├── main.rs      エントリポイント, DaemonState, メインループ
├── handlers.rs  RPCメソッドのハンドラ
├── indexer.rs   インデックス再構築・単一ノート再インデックス
└── watcher.rs   ポーリングベースのファイル監視
```

## Dependencies

- `zelkova-config` — 設定読み込み
- `zelkova-note-core` — Vault, Note, Frontmatter
- `zelkova-rpc` — RPC型・サーバー
- `zelkova-search` — 検索インデックス
- `anyhow` — エラーハンドリング

## Key Types / APIs

### DaemonState (main.rs)

デーモンのグローバル状態。`Arc<DaemonState>`でスレッド間共有。

```rust
struct DaemonState {
    vault: Vault,                       // ファイルシステム上のノート集
    search_index: Box<dyn SearchIndex>, // 検索インデックス
    config: AppConfig,                  // アプリケーション設定
}
```

### メインループ (main.rs)

```
1. AppConfig::load()                        → 設定読み込み
2. Vault::new(vault_path)                   → Vault初期化
3. default_search_index(&index_path)        → 検索インデックス初期化
4. DaemonStateをArcでラップ
5. index_on_startがtrueならrebuild_index()  → 初回インデックス構築
6. RpcServer::bind(&socket_path)            → ソケットバインド
7. write_pid_file()                         → PIDファイル書き込み
8. start_watcher(state.clone())             → ファイル監視開始
9. loop { server.accept_one(&handler) }     → 接続を待ち受け
```

**PIDファイル:** `{vault_path}/.zelkova/daemon.pid` にプロセスIDを書き込み。CLIからデーモンの生死確認に使用。

**インデックスパス:** `{vault_path}/.zelkova/index/`

### RPCハンドラ (handlers.rs)

`handle_request(request, state)` がメソッド名でディスパッチ:

| RPCメソッド | ハンドラ | 処理内容 |
|---|---|---|
| `search` | `handle_search` | SearchQueryでインデックス検索 → SearchHit[] |
| `list_notes` | `handle_list_notes` | Vaultから全ノート取得、タグフィルタ適用 → NoteSummary[] |
| `get_note` | `handle_get_note` | IDでノート検索 → GetNoteResult |
| `create_note` | `handle_create_note` | Vaultに新規ノート作成 → CreateNoteResult |
| `tags` | `handle_tags` | 全タグをソートして返す → TagsResult |
| `rebuild_index` | `handle_rebuild_index` | インデックス全体を再構築 → RebuildIndexResult |
| `note_updated` | `handle_note_updated` | 指定パスのノートを再インデックス |

**パラメータパース:** `parse_params<T>(request)` で `request.params` を型付きでデシリアライズ。失敗時は`invalid_params`エラー。

**エラー処理:** 全ハンドラは `Result<Value, JsonRpcError>` を返し、`handle_request` でsuccess/errorレスポンスに変換。

### Indexer (indexer.rs)

| 関数 | シグネチャ | 説明 |
|---|---|---|
| `rebuild_index` | `&DaemonState -> Result<usize>` | 全ノートをインデックスに再登録 (既存を全削除) |
| `reindex_note` | `&Path, &DaemonState -> Result<()>` | 指定パスのノートを削除→再登録 |

**rebuild_index:**
1. `vault.list_notes()` で全ノート取得
2. 各NoteをSearchDocumentに変換
3. `search_index.rebuild(&docs)` でインデックス再構築
4. 登録件数を返す

**reindex_note:**
1. `vault.list_notes()` から該当パスのノートを検索
2. `search_index.remove_document(&id)` → `search_index.add_document(&doc)`

### Watcher (watcher.rs)

ポーリングベースのファイル監視。inotify等に依存しない。

```rust
fn start_watcher(state: Arc<DaemonState>) -> Result<()>
```

**動作:**
1. 別スレッドで起動
2. `scan_files(vault_path)` でスナップショット取得 (HashMap<PathBuf, SystemTime>)
3. 2秒間スリープ
4. 新しいスナップショットを取得
5. 前回スナップショットと比較:
   - 変更があったファイル → `reindex_note()` で再インデックス
   - 新規ファイル → `reindex_note()` でインデックス追加
   - 削除されたファイル → ログ出力のみ (インデックスからの削除は未実装)
6. スナップショットを更新して3に戻る

**scan_files:** 再帰的に.mdファイルを収集。隠しディレクトリはスキップ。

## Data Flow

```
┌──────────────────────────────────────────────────────┐
│                     デーモンプロセス                    │
│                                                      │
│  main()                                              │
│    ├── Vault::new(vault_path)                        │
│    ├── TantivyIndex::open(index_path)                │
│    ├── rebuild_index()                               │
│    ├── RpcServer::bind(socket_path)                  │
│    ├── write_pid_file()                              │
│    ├── start_watcher() ──┐                           │
│    │                     │                           │
│    └── loop {            │   別スレッド:              │
│          accept_one()    │   每2秒:                   │
│          │               │   scan_files()            │
│          handlers:       │   差分検出                 │
│          ├─ search       │   → reindex_note()        │
│          ├─ list_notes   │                           │
│          ├─ get_note     │                           │
│          ├─ create_note  │                           │
│          ├─ tags         │                           │
│          ├─ rebuild      │                           │
│          └─ note_updated │                           │
│        }                │                           │
│                         └── (ファイルシステム監視)     │
└──────────────────────────────────────────────────────┘
        │
        │ Unix Domain Socket
        │
┌───────┴──────┐
│  CLI / GUI   │
└──────────────┘
```
