# cli

## Role

ClapベースのCLIインターフェース。RPCクライアント経由でデーモンと通信し、ノートの検索・一覧・表示・作成・タグ管理・デーモン操作を行う。

## Module Layout

```
src/
├── main.rs      エントリポイント, Clapコマンド定義, ensure_daemon()
└── commands.rs  各コマンドの実装 (出力フォーマット付き)
```

## Dependencies

- `clap` — CLIパーサー (derive API)
- `zelkova-config` — 設定読み込み
- `zelkova-rpc` — RPCクライアント
- `uuid` — UUIDパース
- `anyhow` — エラーハンドリング
- `libc` — デーモン停止用SIGTERM送信

## Key Types / APIs

### Commands (main.rs)

Clapのderive APIで定義されたコマンド階層。

```
zelkova
├── search <query> [--tag <TAG>] [--limit <N>]
├── list [--tag <TAG>]
├── show <id>
├── create <title> [--dir <DIR>] [--tags <TAG1,TAG2>]
├── tags
└── daemon
    ├── status
    ├── start
    ├── stop
    └── rebuild-index
```

### Commands enum

```rust
enum Commands {
    Search { query: String, tag: Option<String>, limit: usize },
    List { tag: Option<String> },
    Show { id: String },
    Create { title: String, dir: Option<String>, tags: Vec<String> },
    Tags,
    Daemon { action: DaemonAction },
}
```

### DaemonAction enum

```rust
enum DaemonAction {
    Status,       // デーモンの状態確認
    Start,        // デーモン起動
    Stop,         // デーモン停止
    RebuildIndex, // 検索インデックス再構築
}
```

### ensure_daemon() (main.rs)

デーモンが実行中か確認し、未実行なら自動起動するヘルパー。

```
1. ソケットファイルの存在確認
2. 存在しない場合:
   a. daemon_start() で起動
   b. ソケット出現を最大5秒間ポーリング (100ms間隔)
   c. タイムアウト時はエラー
3. RpcClient::new(socket) を返す
```

### コマンド実装 (commands.rs)

| 関数 | 対応コマンド | 処理内容 |
|---|---|---|
| `search(client, query, tags, limit)` | search | RPC search → 結果をID・タイトル・スコアで表示 |
| `list(client, tag)` | list | RPC list_notes → ID・タイトル・タグで表示 |
| `show(client, id)` | show | RPC get_note → タイトル・パス・タグ・日時・本文を表示 |
| `create(client, title, dir, tags)` | create | RPC create_note → 作成結果をID・タイトル・パスで表示 |
| `tags(client)` | tags | RPC tags → タグ一覧を表示 |
| `daemon_status(config)` | daemon status | PIDファイルと/procでプロセスの生死確認 |
| `daemon_start(config)` | daemon start | zelkovadバイナリをspawn (非同期) |
| `daemon_stop(config)` | daemon stop | PIDファイルからPIDを読み取り、SIGTERM送信 |
| `rebuild_index(client)` | daemon rebuild-index | RPC rebuild_index → 結果を表示 |

### 出力フォーマット

**search:**
```
550e8400-...  ノートタイトル  (score: 0.85)
  スニペットテキスト...
```

**list:**
```
550e8400-...  ノートタイトル [rust, programming]
```

**show:**
```
Title: ノートタイトル
Path:  /path/to/note.md
Tags:  rust, programming
Created: 2025-01-15T10:30:00+00:00
Updated: 2025-01-15T10:30:00+00:00
---
ノート本文
```

**create:**
```
Created note: 550e8400-...
  Title: ノートタイトル
  Path:  /path/to/note.md
```

### デーモンライフサイクル管理

```
daemon start:
  zelkovadバイナリを探索 (current_exeと同じディレクトリ)
  → Command::new().spawn() (バックグラウンド実行)

daemon stop:
  PIDファイル ({vault}/.zelkova/daemon.pid) を読み取り
  → libc::kill(pid, SIGTERM)

daemon status:
  PIDファイルを読み取り
  → /proc/{pid} の存在で実行中か判定
  → ソケットパスも表示
```

## Data Flow

```
ユーザー入力
    │
    clap::parse()
    │
    ├── Search/List/Show/Create/Tags:
    │     │
    │     ensure_daemon(config)
    │       ├── ソケット存在? ──No──> daemon_start()
    │       │                         ソケット出現をポーリング
    │       └── RpcClient::new(socket)
    │     │
    │     commands::xxx(client, ...)
    │       │
    │       client.rpc_method(...)
    │         │
    │         ── Unix Socket ──> デーモン
    │         <──────────────── レスポンス
    │       │
    │     結果をstdoutにフォーマット出力
    │
    └── Daemon:
          ├── Status → PIDファイル + /proc 確認
          ├── Start  → zelkovad をspawn
          ├── Stop   → SIGTERM送信
          └── RebuildIndex → ensure_daemon → RPC rebuild_index
```
