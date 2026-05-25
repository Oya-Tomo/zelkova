# gui

## Role

GPUI 0.2ベースのGUIアプリケーション。Markdown編集、シンタックスハイライト、プレビュー、タブ管理を提供。

## Module Layout

```
src/
├── main.rs              ZelkovaApp, actions!マクロ, エントリポイント
├── keymap.rs            KeyBinding構築, アクション名マッピング
├── pane.rs              PaneManager (タブ, ViewMode切替)
├── command_palette.rs   CommandPalette (ファジーマッチ)
├── preview.rs           Markdownプレビュー (zelkova-markdown AST → GPUI要素)
└── editor/
    ├── mod.rs           Editor本体, アクションハンドラ, EntityInputHandler, Render
    ├── highlight.rs     ResolvedColors, 行単位ハイライト, インラインスキャン
    └── ime.rs           IME状態管理
```

## Dependencies

- `gpui 0.2` — UI framework
- `zelkova-config` — テーマ・キーマップ設定
- `zelkova-note-core` — Frontmatter構造体
- `zelkova-rpc` — デーモン通信
- `zelkova-rope` — テキストバッファ (undo/redo付き)
- `zelkova-markdown` — プレビュー用パーサー
- `zelkova-highlight` — Tree-sitterコードハイライト

## Key Components

### ZelkovaApp (main.rs)

アプリケーションのルート。サイドバー（ノート一覧）とメインコンテンツ（PaneManager）を管理。

- ノート一覧をRPCで取得
- 選択中ノートのタブを開く
- コマンドパレットのオーバーレイ表示
- テーマ設定の読み込みと伝播

### PaneManager (pane.rs)

タブ付きエディタ/プレビュー管理。ViewMode (Editor/Split/Preview) を切替。

- タブの開閉、アクティブタブ切替
- エディタにsocket pathとテーマを伝播
- フォーカス管理

### Editor (editor/mod.rs)

メインエディタコンポーネント。以下の責務を持つ：

**データ管理:**
- `buffer: Buffer` — Ropeベースのテキストバッファ
- `cached_text: String` — バッファのキャッシュ (O(n)のRope traversalを回避)
- `cached_lines: Vec<String>` — 行分割キャッシュ
- `cached_highlights: Vec<HighlightedLine>` — ハイライト結果キャッシュ
- `resolved_colors: ResolvedColors` — テーマ色の事前パース結果

**カーソル・選択:**
- `cursor_pos: usize` — バイトオフセット
- `selection: Option<Range<usize>>` — バイト範囲
- `edit_zone: EditZone` — Title/Content切替 (frontmatterヘッダー内のタイトル編集)

**アクションハンドラ:**
- カーソル移動 (矢印キー、Title↔Content境界越え)
- 選択拡張 (Shift+矢印)
- 文字入力 (EntityInputHandler経由、IME対応)
- Backspace, Enter, Undo/Redo, Save

**レンダリング (renderメソッド):**
1. `render_frontmatter_header()` — タイトル、タグ、日付のヘッダー
2. `build_highlights()` — ハイライトキャッシュの構築
3. 行ループ: `render_highlighted_line()` で各行をレンダリング
4. カーソル行はテキストをbefore/afterに分割し、2pxバーを挿入

**位置計算:**
- `byte_to_line_col()` — バイトオフセット → (行, 桁)
- `line_col_to_byte()` — (行, 桁) → バイトオフセット
- `pixel_to_col()` — マウスピクセル位置 → 桁 (固定幅フォント前提、7.2px/字)

### ResolvedColors (editor/highlight.rs)

全テーマ色を一度だけHslaにパースして保持。毎フレームの文字列パースを排除。

フィールド:
- Markdown色: heading_marker, heading_fg, list_marker, quote_fg, text_dim, bold_fg, italic_fg, strikethrough_fg, image_marker, link_fg, math_fg
- コードブロック色: code_bg, code_fg, code_keyword
- `code_syntax: [Hsla; 12]` — Tree-sitter 12ハイライトクラス (attribute, comment, constant, function, keyword, number, operator, property, punctuation, string, tag, type)

### ハイライトパイプライン

```
cached_lines
    │
    build_highlights(lines, &resolved_colors)
    │
    ├─ 行が "```" で始まる → コードブロックモード
    │   ├─ highlight_fence_line() — ```行のスタイル
    │   ├─ zelkova_highlight::highlight_code() — Tree-sitterで構文解析
    │   ├─ resolved_colors.syntax_color(idx) — ハイライトインデックス → Hsla
    │   └─ HighlightedLine { line_bg: Some(code_bg) }
    │
    ├─ 行が "$$" で始まる → 数式ブロックモード
    │   └─ math_delim_line() + math_fg で統一スタイル
    │
    └─ 通常行
        ├─ detect_line_context() — Heading/ListItem/BlockQuote/Table/Normal
        ├─ highlight_line() — ブロックコンテキストに応じたスタイル
        │   └─ scan_inline() — Bold/Italic/Strikethrough/Code/Link/Image/Math
        └─ HighlightedLine
```

### 選択背景 (overlay_selection)

`combine_highlights` (HashSetで非決定的) の代わりに、自前の決定的オーバーレイを使用:
1. 既存ハイライトを選択境界で分割
2. 選択範囲内のbackground_colorをsel_bgで上書き
3. 選択範囲外は元のスタイルを保持
4. 選択内のギャップをsel_bgのみのハイライトで埋める

### CommandPalette (command_palette.rs)

オーバーレイ検索UI。キーアクションをファジーマッチで絞り込み。

### Preview (preview.rs)

zelkova-markdownのAST → GPUI要素ツリーのレンダリング。editorとは独立したエンティティ。

## Data Flow

```
User Input (keyboard/mouse)
    │
    ├─ EntityInputHandler → buffer.edit() → cache_edit()
    │                                    → highlights_dirty = true
    │
    └─ cx.notify() → render()
        │
        ├─ if highlights_dirty:
        │   build_highlights() → cached_highlights
        │
        ├─ render_frontmatter_header()
        │
        └─ for each line:
            ├─ render_highlighted_line()
            │   ├─ overlay_selection() — 選択背景適用
            │   └─ line_bg — コードブロック全幅背景
            │
            └─ StyledText::with_highlights()
```

## Known Limitations

- **固定幅フォント前提**: pixel_to_col()が7.2px/字で計算。プロポーショナルフォントでずれる。
- **ハイライト遅延**: 初回フレームはプレーンテキスト。次フレームでハイライト表示。
- **ポーリングベースのファイル監視**: inotify/FSEventsではなく2秒ポーリング。
- **プレーンテキストのfrontmatterヘッダー**: タイトル・タグ・日付の色がハードコード（テーマ未対応）。
