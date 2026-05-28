# Scroll Experiments — Issue #82

GPUI 0.2 (v0.2.2) / Taffy v0.9.0 でのスクロール実装の試行記録。

## 前提知識

### GPUI 0.2 の Overflow API

`gpui-macros` の `overflow_style_methods!()` マクロで生成（`gpui-macros/src/styles.rs`）:

| メソッド | 効果 |
|----------|------|
| `overflow_hidden()` | `overflow.x = Hidden`, `overflow.y = Hidden` |
| `overflow_x_hidden()` | `overflow.x = Hidden` |
| `overflow_y_hidden()` | `overflow.y = Hidden` |
| `overflow_scroll()` | `overflow.x = Scroll`, `overflow.y = Scroll` |
| `overflow_x_scroll()` | `overflow.x = Scroll` |
| `overflow_y_scroll()` | `overflow.y = Scroll` |

### Taffy バージョンの差異（重要）

| プロジェクト | GPUI | Taffy | 横スクロール |
|-------------|------|-------|-------------|
| **Zelkova** | 0.2.2 (crates.io) | **0.9.0**（`"=0.9.0"`で固定） | 不可 |
| **Zed** | 0.2.2 (git fork) | **0.10.1**（`"=0.10.1"`） | 可能 |

Taffy 0.9.0 → 0.10.1 の間に overflow 処理の改善があった可能性が高い。
GPUI 0.2.2 は Taffy `"=0.9.0"` で固定しているため、Taffy 単体のアップグレードは不可。

### GPUI の content_size 計算

`div.rs:1371-1394`: スクロール範囲 = `content_size - bounds.size`。

`content_size` は**直接の子の layout bounds** の min/max から計算:
```rust
for child_layout_id in &request_layout.child_layout_ids {
    let child_bounds = window.layout_bounds(*child_layout_id);
    child_min = child_min.min(&child_bounds.origin);
    child_max = child_max.max(&child_bounds.bottom_right());
}
(child_max - child_min).into()
```

content_size == bounds.size の場合、スクロール範囲 = 0（スクロール不可）。

### GPUI の横スクロールホイール処理

`div.rs:2424-2430`: `overflow.x == Overflow::Scroll` の時、ホイールの `delta.x` を `scroll_offset.x` に加算。
→ GPUI 側のイベント処理は正しく実装されている。問題はスクロール範囲が 0 になること。

### Zed のアプローチ

1. **エディタ本文**: 独自の `Element` トレイト実装でカスタムペイント。水平スクロールは `clamp_scroll_left()` でスクロール位置を制限。
2. **UI コンポーネント**: `div` + `overflow_scroll()` を使用。Taffy 0.10.1 では正常動作。
3. **GPUI scrollable.rs サンプル**: ネストした div で縦横スクロールが動作確認されている。

## 試行記録

### Approach 1: `uniform_list` (archived)

**Commits**: `c3bdf86` → `516e161`

```
div (flex_col, size_full)
  ├── header (fixed)
  └── uniform_list (track_scroll, size_full)
       └── 各行 22px 固定
```

- 縦スクロール: OK
- 横スクロール: **不可**（uniform_list は縦専用）
- 固定アイテム高（22px）で折返し行に対応できない

### Approach 2: `div` + `overflow_scroll()` (content_div wrapper)

```
div (size_full, flex_col, overflow_hidden)
  ├── header (fixed)
  └── scroll_div (id, flex_1, w_full, overflow_y_scroll [+ overflow_x_scroll])
       └── content_div (flex_col, flex_shrink_0)
            └── lines...
```

- 縦スクロール: OK
- 横スクロール: **不可**（コンテナがコンテンツ幅に広がる）

バリエーション:

| # | 変更 | 結果 |
|---|------|------|
| 2a | `overflow_scroll()` のみ | 広がる、スクロールしない |
| 2b | `overflow_y_scroll` + `overflow_x_scroll` | 縦OK、横不可 |
| 2c | 外側 div に `overflow_hidden()` | 広がりは防がれるがスクロール不可 |
| 2d | 行の `.w_full()` を外す | テキストはみ出すがスクロールしない |

### Approach 3: 手動スクロール（set_offset）

`overflow_hidden()` でクリップ、`on_scroll_wheel` で縦横両方 `set_offset()` に渡す。

- **結果: ガクガクで実用不可**
- `set_offset()` は即時ジャンプのみ、スムーススクロールの最適化なし

### Approach 4: 明示的幅制約 (content_div に computed width)

`wrap=false` 時、`content_div` に `.w(px(max_line_width))` で最長行の文字幅を設定。

- **結果: 効果なし**（コンテナ依然として広がる）

### Approach 5: 行をスクロールコンテナの直接の子に

content_div wrapper を削除。`items_start()` で行幅をコンテナに制限しない。

- **結果: 効果なし**（コンテナが広がる）

### Approach 6: クリップ wrapper（Zed 方式）

各行を outer_div（`w_full`, `overflow_x_hidden`）でラップし、inner_div を `ml(-scroll_x)` でオフセット。

- **結果: 効果なし**（行自体が広がり、outer_div も広がる）

### Approach 7: ネストしたDiv（content_sizing_div）

スクロールDivの中にコンテンツ最大幅を明示したDivを入れる。

```
scroll_div (w_full, max_w_full, overflow_y_scroll, overflow_x_scroll)
  └── content_div (flex_col, w(px(max_line_width)))
       ├── line 0 (whitespace_nowrap)
       └── ...
```

- **結果: 効果なし**（コンテナが広がる）

### Approach 8: Taffy 0.10.1 へのアップグレード（2025-05-28）

GPUI 0.2.2 をローカルにコピーし、`Cargo.toml` の Taffy 要求を `"=0.9.0"` → `"0.10"` に変更。
`[patch.crates-io]` で GPUI をローカル版に置き換え。

```toml
# Cargo.toml (workspace root)
[patch.crates-io]
gpui = { path = "crates/gpui-patched" }
```

- コンパイル: **OK**
- テスト: **OK**
- 縦スクロール: **OK**
- 横スクロール: **不可**（コンテナがコンテンツ幅に広がる、スクロール範囲 = 0）

**結論: Taffy 0.10.1 だけでは解決しない。Zed が横スクロールできるのは Taffy のバージョン差ではなく、独自の `Element` トレイト実装によるもの。**

### 失敗の根本分析

**なぜ content_size == bounds.size になるか:**

1. **GPUI の content_size 計算方式（主因）**: `div.rs` の content_size は直接の子の layout bounds から計算。Taffy が overflow を設定しても、GPUI は子の実際のレイアウトサイズをそのまま使うため、コンテナが広がれば content_size も広がる。**これは Taffy のバージョンに関係ない**（Approach 8 で検証済み）。

2. **Flex stretch アライメント**: GPUI の div はデフォルト `Display::Flex`。子は `align_items: stretch` でコンテナ幅に引き伸ばされる。明示的 `w(px(...))` も stretch で上書きされる可能性。

3. **制約の連鎖崩壊**: `max_w_full()` を設定しても、上流の親が子に引っ張られて広がるため、最終的に `max_width` も広い値に解決されてしまう。

**Zed が横スクロールできる理由**: Taffy バージョンではなく、独自の `Element` トレイト実装でスクロールコンテナを自前管理しているため（`clamp_scroll_left()` 等）。

## 解決策の検討

### ~~A. パッケージアップグレード（推奨）~~ → 却下

Taffy 0.9.0 → 0.10.1 へアップグレードしたが、横スクロールは動作しなかった。
（Approach 8 で検証済み）

根本原因は Taffy のバージョンではなく、GPUI の `content_size` 計算が子の layout bounds をそのまま使うため。

### B. カスタム Element 実装（推奨）

GPUI の `Element` トレイトを実装して、スクロールコンテナを自前で管理。

```rust
pub trait Element {
    type RequestLayoutState;
    type PrepaintState;

    fn request_layout(&mut self, ...) -> (LayoutId, Self::RequestLayoutState);
    fn prepaint(&mut self, ..., bounds: Bounds<Pixels>, ...) -> Self::PrepaintState;
    fn paint(&mut self, ..., bounds: Bounds<Pixels>, ...);
}
```

**設計案:**

```
EditorContentElement {
    scroll_y: f32,     // 縦スクロールオフセット
    scroll_x: f32,     // 横スクロールオフセット（wrap=false時）
    lines: Vec<String>,
    // ...
}

request_layout():
    - 自身のサイズを親から決定（w_full, flex_1 相当）
    - content_size = (max_line_width, total_line_height)
    - ScrollHandle に bounds と content_size を設定

paint():
    - bounds 内でクリップ（window.with_element_offset で scroll_offset を適用）
    - 可視行のみ描画（virtualization）
    - カーソル、選択範囲も paint 内で描画
```

**メリット:**
- Taffy の overflow に依存しない
- Zed と同じアプローチ
- 完全な制御が可能

**デメリット:**
- 実装コストが高い（数日の作業）
- マウスイベント（クリック、ドラッグ）の自前処理が必要
- IME 入力の統合が複雑

### C. GPUI の scroll 実装をパッチ

`div.rs` の `content_size` 計算を修正して、overflow scroll 時にコンテナが広がらないようにする。

**具体案:**
- `prepaint()` で `scroll_max` を計算した後、`bounds.size` をコンテナの本来のサイズに固定
- または、`request_layout()` 時に `overflow: scroll` のノードに min-width 制約を追加

リスク: GPUI 内部の深い理解が必要。他のコンポーネントへの副作用の可能性。

## 推奨ロードマップ

1. ~~**パッケージアップグレード**: Taffy 0.10.1 → 却下（効果なし）~~
2. **カスタム Element 実装**: Zed と同じアプローチでスクロールコンテナを自前管理（唯一の解決策）
3. **Taffy 0.10.1 パッチは維持**: レイアウト精度の向上が期待できるため

---

## Zed カスタム Element 調査（2025-05-28）

Zed のエディタは GPUI の `div` ベースのスクロールに依存せず、`Element` トレイトを直接実装してスクロールを自前管理している。

### 主要ファイル

- `crates/editor/src/element.rs` — `EditorElement` の `Element` トレイト実装
- `crates/editor/src/scroll_manager.rs` — スクロール状態管理

### request_layout(): コンテナが広がらない仕組み

```rust
// element.rs:9798-9871
fn request_layout(&mut self, ...) -> (LayoutId, EditorRequestLayoutState) {
    let mut style = Style::default();
    style.size.width = relative(1.).into();  // 親の幅いっぱい（相対指定）
    style.size.height = relative(1.).into(); // 親の高さいっぱい（相対指定）

    // コンテンツサイズは計算するが、style には反映しない
    // → コンテナが広がらない
    window.request_layout(style, None, cx)
}
```

**重要**: `relative(1.)` を使うことで、Taffy がコンテンツに引っ張られて広がるのを防ぐ。
`Style::default()` に `width: relative(1.)` / `height: relative(1.)` を設定するだけで、
レイアウトエンジンは「親のサイズいっぱい」を解決し、コンテンツ幅は無視される。

### scroll_max の計算（prepaint 内）

```rust
// element.rs:10639-10644
let scroll_max: gpui::Point<ScrollPixelOffset> = point(
    ScrollPixelOffset::from(
        ((scroll_width - editor_width) / em_layout_width).max(0.0),
    ),
    max_scroll_top,
);
```

- `scroll_width`: 全行の最大幅（コンテンツ幅）
- `editor_width`: ビューポート幅（bounds から取得）
- `scroll_max.x = max(0, scroll_width - editor_width)`

これを自前で計算するため、GPUI の `content_size` に依存しない。

### paint() でのスクロールオフセット適用

```rust
// element.rs:9380-9381
let mut fragment_origin = content_origin
    + gpui::point(
        Pixels::from(-layout.position_map.scroll_pixel_position.x),
        line_y,
    );
```

スクロール位置を **負のオフセット** として各フラグメントの描画位置に加算。
GPUI の `ScrollHandle` に頼らず、自分でオフセットを管理・適用。

### クリッピング

```rust
// element.rs:991-992, 10857-10858
window.with_content_mask(Some(ContentMask { bounds }), |window| {
    // bounds 内のみ描画（クリッピング）
});
```

`ContentMask` でビューポートの bounds にクリップ。はみ出した部分は描画されない。

### スクロールホイールイベント処理

```rust
// element.rs:7812-7821
let x = (current_scroll_position.x
    * ScrollPixelOffset::from(glyph_width)
    - ScrollPixelOffset::from(delta.x * scroll_sensitivity))
    / ScrollPixelOffset::from(glyph_width);

let mut scroll_position = point(x, y).clamp(&point(0., 0.), &position_map.scroll_max);
```

ホイールの `delta.x` をスクロール位置に加算し、`scroll_max` でクランプ。
この処理も `Element` の `paint()` 内または `on_scroll_wheel` ハンドラで実行。

### 可視行の仮想化（Virtualization）

```rust
// 行の描画範囲を計算
let start_row = DisplayRow((scroll_position.y + clipped_top_in_lines).floor() as u32);
let end_row = DisplayRow((scroll_position.y + clipped_top_in_lines + visible_height_in_lines).ceil() as u32);
```

スクロール位置に基づいて可視行だけを計算・描画。全行を描画しないのでパフォーマンスも良い。

### マウスイベント

マウスイベントの座標はスクロールオフセットで調整:
- クリック位置 → `position - scroll_offset` でコンテンツ座標に変換
- ドラッグ → 同様に変換して選択範囲を更新

### データフロー全体

```
1. request_layout()
   → style.size = relative(1.) でコンテナサイズを親に決定
   → コンテンツ幅はレイアウトに影響しない

2. prepaint()
   → bounds から editor_width を取得
   → scroll_width（最大行幅）を計算
   → scroll_max = scroll_width - editor_width
   → 可視行の範囲を計算（start_row, end_row）

3. paint()
   → ContentMask でビューポートにクリップ
   → 各行を -scroll_offset で描画位置をシフト
   → カーソル・選択範囲も同じオフセットで描画

4. on_scroll_wheel
   → delta を scroll_position に加算
   → scroll_max でクランプ
   → cx.notify() で再描画
```

### Zelkova への適用設計

Zed のパターンを Zelkova に適用する場合:

```rust
pub struct EditorContentElement {
    editor: Entity<Editor>,
}

impl Element for EditorContentElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        let layout_id = window.request_layout(style, None, cx);
        (layout_id, ())
    }

    fn prepaint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) -> () {
        // 可視行の範囲を計算
        // scroll_max を計算
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, _: &mut App) -> () {
        // ContentMask でクリップ
        // 各行を -scroll_offset で描画
        // カーソル・選択・画像も同じオフセットで描画
    }
}
```

**現在の Zelkova editor render() からの移行手順:**

1. `EditorContentElement` 構造体を定義
2. `Element` トレイトの 3 メソッドを実装
3. `Render` トレイトの `render()` で、コンテンツ行の描画を `EditorContentElement` に委譲
4. `on_scroll_wheel` を `EditorContentElement::paint()` 内に移動
5. マウスイベント（クリック、ドラッグ）の座標を scroll_offset で調整
6. 縦スクロールの `ScrollHandle` は不要になる（自前管理に移行）
7. 既存のハイライト・画像レンダリングは `paint()` 内で同じオフセットを適用
