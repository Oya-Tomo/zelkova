# rope

## Role

B-treeベースのRopeデータ構造による効率的なテキスト編集と、undo/redo付きのテキストバッファを提供するcrate。外部依存なし。

## Module Layout

```
src/
└── lib.rs    全実装 (Node, Rope, Buffer) 約545行
```

## Dependencies

なし (標準ライブラリのみ)

## Key Types / APIs

### 定数

| 定数 | 値 | 用途 |
|---|---|---|
| `CHUNK_SIZE` | 512 | リーフノードの最大バイト数 |
| `MIN_SPLIT` | 128 (`CHUNK_SIZE / 4`) | 分割の最小閾値 (現在未使用) |

### Node enum

Ropeの内部ノード。永続データ構造として不変 (insert/deleteは新しいNodeを返す)。

```rust
enum Node {
    Leaf {
        text: String,         // 最大CHUNK_SIZEバイトのテキスト断片
        line_count: usize,    // 改行数ベースの行数 (最低1)
    },
    Internal {
        left: Box<Node>,      // 左部分木
        right: Box<Node>,     // 右部分木
        char_count: usize,    // 左+右の合計バイト数
        line_count: usize,    // 左+右の合計行数
    },
}
```

**メソッド:**

| メソッド | シグネチャ | 計算量 | 説明 |
|---|---|---|---|
| `char_count` | `&self -> usize` | O(1) | バイト数を返す (リーフはtext.len、内部はキャッシュ) |
| `line_count` | `&self -> usize` | O(1) | 行数を返す |
| `from_str` | `&str -> Self` | O(n) | 文字列から構築。CHUNK_SIZE超過なら再帰的に分割 |
| `insert` | `&self, usize, &str -> Self` | O(log n) | 指定位置にテキストを挿入 |
| `delete` | `&self, usize, usize -> Self` | O(log n) | 指定範囲 [start, end) を削除 |
| `line` | `&self, usize -> String` | O(n) worst | 指定行のテキストを取得 |
| `text` | `&self -> String` | O(n) | 全テキストを結合して返す |
| `char_at` | `&self, usize -> Option<char>` | O(log n) | 指定位置の文字を取得 |

### 分割点の決定 (find_split_point)

`CHUNK_SIZE`を超える文字列を構築する際、中央付近で改行またはスペースを探して分割。見つからなければ中央で分割。

### リバランス (rebalance)

delete後に一方の子が空になった場合、もう一方の子を直接返す。それ以外は`merge`で両者を結合。

### Rope struct

Nodeのルートをラップした公開API。

```rust
struct Rope {
    root: Node,
}
```

| メソッド | 説明 |
|---|---|
| `new()` | 空のRopeを作成 |
| `from(text)` | 文字列からRopeを作成 |
| `char_count()` | バイト数 |
| `line_count()` | 行数 |
| `insert(pos, text)` | 位置posにテキストを挿入 |
| `delete(start, end)` | 範囲を削除 |
| `line(idx)` | 行idxの内容を取得 |
| `text()` | 全テキストを取得 |
| `char_at(pos)` | 位置posの文字を取得 |

### Buffer struct

Ropeをラップし、undo/redoスタックを管理するテキストバッファ。

```rust
struct Buffer {
    rope: Rope,
    undo_stack: Vec<Edit>,
    redo_stack: Vec<Edit>,
}
```

#### Edit enum (private)

```rust
enum Edit {
    Insert { pos: usize, text: String },
    Delete { start: usize, end: usize, text: String },
}
```

1回の編集操作は常に`[Insert, Delete]`のペアとしてundo_stackに記録される:
- `edit(start, end, new_text)` → Delete(旧テキスト) + Insert(新テキスト) をpush
- `undo()` → ペアを逆順に適用 (Insertを削除、Deleteを復元)
- `redo()` → ペアを再適用

**メソッド:**

| メソッド | 説明 |
|---|---|
| `new()` | 空バッファを作成 |
| `from(text)` | 初期テキスト付きバッファを作成 |
| `edit(start, end, new_text)` | 範囲を置換 (undo/redo記録付き) |
| `insert(pos, text)` | `edit(pos, pos, text)` の短縮形 |
| `delete(start, end)` | `edit(start, end, "")` の短縮形 |
| `undo() -> bool` | 直前の編集を取り消し |
| `redo() -> bool` | 取り消した編集を再適用 |
| `text()` | 全テキストを取得 |
| `line(idx)` | 行idxの内容を取得 |
| `line_count()` | 行数 |
| `char_count()` | バイト数 |
| `can_undo()` | undo_stackが空でないか |
| `can_redo()` | redo_stackが空でないか |

## Data Flow

```
テキスト挿入:
  Buffer::edit(start, end, "new")
    ├── rope.delete(start, end)        → 新しいNodeツリー
    ├── rope.insert(start, "new")      → 新しいNodeツリー
    ├── undo_stack.push(Insert{...})
    ├── undo_stack.push(Delete{...})
    └── redo_stack.clear()

undo:
  undo_stackから[Insert, Delete]ペアをpop
    ├── Insertのテキストをrope.delete()で削除
    ├── Deleteのテキストをrope.insert()で復元
    └── ペアをredo_stackにpush

redo:
  redo_stackから[Insert, Delete]ペアをpop
    ├── Deleteのテキストをrope.delete()で削除
    ├── Insertのテキストをrope.insert()で再挿入
    └── ペアをundo_stackにpush
```

### Rope内部構造のイメージ

```
"Hello World\nFoo\nBar" → CHUNK_SIZE=512以下なので単一リーフ:

Node::Leaf { text: "Hello World\nFoo\nBar", line_count: 3 }

大きなテキスト (>512バイト):
          Internal { char_count: 1200, line_count: 30 }
         /                                              \
  Leaf { text: "...512B...", line_count: 15 }    Internal { char_count: 688, line_count: 15 }
                                                      /                                  \
                                              Leaf { "...512B..." }              Leaf { "...176B..." }
```
