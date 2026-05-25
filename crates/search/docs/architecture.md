# search

## Role

ノートの全文検索インデックスを抽象化し、Tantivyベースのバックエンド実装を提供するcrate。

## Module Layout

```
src/
├── lib.rs               モジュール宣言, default_search_index ファクトリ関数
├── engine/
│   └── mod.rs           SearchDocument, SearchQuery, SearchResult, SearchIndex trait
└── tantivy_backend.rs   TantivyIndex (feature "tantivy" で有効化)
```

## Dependencies

- `uuid` — ドキュメントID
- `anyhow` — エラーハンドリング
- `tantivy` (feature-gated) — 全文検索エンジン

## Key Types / APIs

### SearchDocument (engine/mod.rs)

インデックスに登録するドキュメント。

```rust
struct SearchDocument {
    id: Uuid,
    title: String,
    content: String,
    tags: Vec<String>,
    path: PathBuf,
}
```

### SearchQuery (engine/mod.rs)

検索クエリ。

```rust
struct SearchQuery {
    text: String,           // 全文検索テキスト
    limit: Option<usize>,   // 結果の最大数 (デフォルト20)
    tags: Vec<String>,      // タグフィルタ (AND条件)
}
```

### SearchResult (engine/mod.rs)

検索結果の1件。

```rust
struct SearchResult {
    id: Uuid,
    title: String,
    path: PathBuf,
    score: f32,         // 関連度スコア
    snippet: String,    // スニペット (現在未実装: 空文字列)
}
```

### SearchIndex trait (engine/mod.rs)

検索バックエンドの抽象インターフェース。

```rust
trait SearchIndex: Send + Sync {
    fn add_document(&self, doc: &SearchDocument) -> Result<()>;
    fn remove_document(&self, id: &Uuid) -> Result<()>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;
    fn rebuild(&self, docs: &[SearchDocument]) -> Result<()>;
}
```

### default_search_index (lib.rs)

```rust
fn default_search_index(path: &Path) -> Result<Box<dyn SearchIndex>>
```

Feature flagに基づいてバックエンドを選択:
- `"tantivy"` feature有効 → `TantivyIndex::open(path)` を返す
- feature無効 → コンパイルエラー

### TantivyIndex (tantivy_backend.rs)

TantivyベースのSearchIndex実装。feature `"tantivy"` で有効化。

```rust
struct TantivyIndex {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
}
```

**インデックススキーマ (5フィールド):**

| フィールド | 型 | オプション | 用途 |
|---|---|---|---|
| `id` | STRING | STORED | ドキュメントの一意識別子 |
| `title` | TEXT | STORED | タイトル (トークナイズ済み) |
| `content` | TEXT | STORED | 本文 (トークナイズ済み) |
| `tags` | TEXT | STORED | タグ (スペース区切りで結合して格納) |
| `path` | TEXT | STORED | ファイルパス |

**SearchIndex実装:**

| メソッド | 実装詳細 |
|---|---|
| `add_document` | `doc!`マクロでTantivyドキュメントを構築し、writerに追加→commit |
| `remove_document` | idフィールドのTermで削除→commit |
| `search` | QueryParser (title, content, tags対象) + タグTermQuery → BooleanQuery → TopDocs |
| `rebuild` | `delete_all_documents()` → commit → 全ドキュメントをadd_document |

**検索クエリ構築:**
1. `text`が非空 → QueryParserでパース → `Occur::Must`として追加
2. 各`tag` → TermQueryでタグフィールドを検索 → `Occur::Must`として追加
3. 全条件をBooleanQueryで結合 (AND条件)
4. 条件が空 → ワイルドカードクエリ (`*`)

**スレッド安全性:** `IndexWriter`は`Mutex`で保護。`SearchIndex: Send + Sync`を満たす。

**インデックスの永続化:** 指定されたディレクトリにTantivyインデックスを保存。`meta.json`の有無で新規作成か既存オープンかを判定。

## Data Flow

```
ノート追加:
  SearchDocument { id, title, content, tags, path }
      │
      TantivyIndex::add_document()
      │
      ├── doc! マクロでフィールドをマッピング
      │     tags: Vec<String> → スペース区切りの単一テキスト
      │
      ├── writer.add_document(doc)
      └── writer.commit()

検索:
  SearchQuery { text: "Rust", tags: ["programming"], limit: Some(10) }
      │
      TantivyIndex::search()
      │
      ├── QueryParser: title, content, tagsフィールドで "Rust" を検索
      ├── TermQuery: tagsフィールドで "programming" を検索
      ├── BooleanQuery: Must (AND) で結合
      ├── TopDocs::with_limit(10) で取得
      └── SearchResult[] に変換 (id, title, path, score)

インデックス再構築:
  rebuild(docs)
      │
      ├── delete_all_documents() → commit
      └── for each doc: add_document(doc)
```
