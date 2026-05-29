# Horizontal Scroll Survey — Issue #126

GPUI 0.2 / Taffy 0.9.0 における横スクロールの実現可能性に関する技術調査。

## 目次

1. [問題の概要](#1-問題の概要)
2. [Taffy の scrollable overflow 処理](#2-taffy-scrollable-overflow)
3. [GPUI の scroll コンテナ実装](#3-gpui-scroll-コンテナ実装)
4. [Zed の横スクロールアプローチ](#4-zed-横スクロールアプローチ)
5. [gpui-component Scrollbar の仕組み](#5-gpui-component-scrollbar)
6. [バグか仕様か](#6-バグか仕様か)
7. [現在の Zelkova の実装](#7-現在の-zelkova-実装)
8. [解決策の評価](#8-解決策の評価)
9. [参考文献](#9-参考文献)

---

## 1. 問題の概要

`wrap=false` モードのエディタで、行が `whitespace_nowrap` によりコンテナ幅を超える場合、GPUI の `overflow_scroll()` を使ったネイティブな横スクロールが動作しない。

**現象**: `overflow_x_scroll()` または `overflow_scroll()` を設定した `div` に `whitespace_nowrap` の行を入れると、スクロールコンテナ自体がコンテンツ幅に広がってしまい、スクロール範囲が 0 になる。

**根本原因**: 二段構え。
1. **Taffy 側**: flex コンテナの `min-content` サイズが `whitespace_nowrap` 子要素の内容幅に拡張される。`overflow: scroll` が設定されていても、Taffy 0.9.0 ではコンテナの intrinsic size をコンテンツ幅に制限する処理が未実装。
2. **GPUI 側**: `content_size` を直接の子の layout bounds の min/max から計算しており、Taffy の overflow 設定を反映していない。

---

## 2. Taffy の scrollable overflow 処理

### 2.1 Taffy の content_size 機能

Taffy は `content_size` feature flag (`#[cfg(feature = "content_size")]`) でスクロール可能領域の出力を提供する。各ノードの `Layout` 構造体に `content_size: Size<f32>` を格納し、スクロール範囲は `content_size - bounds.size` で計算される。

この機能は Issue [#470](https://github.com/DioxusLabs/taffy/issues/470) で提案され、PR [#573](https://github.com/DioxusLabs/taffy/pull/573) で v0.4 として実装された。

### 2.2 既知の問題

**Issue [#696](https://github.com/DioxusLabs/taffy/issues/696) — "Flexbox container isn't shrinking to be smaller than its content size + margins"**

`overflow: hidden` を設定した flex コンテナが、子要素の内容サイズに合わせて広がってしまう問題。Zelkova が遭遇している問題と同一。PR [#728](https://github.com/DioxusLabs/taffy/pull/728) で修正されたが、この修正はコンテナの intrinsic size を制限するものであり、GPUI が使用する Taffy 0.9.0 には含まれていない。

**Issue [#871](https://github.com/DioxusLabs/taffy/issues/871) — "`content_size` for internal nodes incorrectly includes the node's left/top border"**

内部ノードの `content_size` 計算にノード自身の left/top border が誤って含まれるバグ。

**Issue [#954](https://github.com/DioxusLabs/taffy/issues/954) — "Incorrect grid content size computation"**

グリッドレイアウトでの content_size 計算の不正確さ（2026年5月時点で Open）。

### 2.3 scrollable_overflow PR（将来の修正）

**PR [#875](https://github.com/DioxusLabs/taffy/pull/875) — "scrollable_overflow implementation"**（Draft）

`content_size: Size<f32>` を `scrollable_overflow: Rect<f32>` に置き換える大規模な PR。[CSS Overflow 3 §2.2](https://www.w3.org/TR/css-overflow-3/#scrollable) の "Scrollable Overflow Region" 定義に準拠することを目指している。

主な変更点:
- `Layout` 構造体に `scrollable_overflow: Rect<f32>` を追加（`content_size` は feature flag で残すか削除するか議論中）
- `LayoutOutput` に `descendent_scrollable_overflow` を追加
- flexbox・grid の各レイアウトアルゴリズムで scrollable overflow を伝播
- scroll コンテナ（`overflow: scroll/hidden/auto`）では子の overflow を padding で拡張してからクリップ
- `overflow: clip` と `overflow: visible` の区別を明確化

この PR がマージされれば、`overflow: scroll` コンテナの intrinsic size がコンテンツに引っ張られる問題が解決する可能性が高い。ただし、**2026年5月時点で Draft** であり、マージ時期は未定。

### 2.4 Taffy の flex レイアウトにおける overflow 処理

Taffy 0.9.0 の flex レイアウト（`src/compute/flexbox.rs`）では:

1. 子要素の `min_content` サイズを計算
2. `whitespace_nowrap` の場合、テキストノードの `min_content` が全文幅になる
3. 親コンテナの `min_content` は子の `min_content` の合計
4. `overflow: scroll` はコンテナの **表示サイズ** を制限するが、**intrinsic size** は制限しない
5. 結果として、コンテナは利用可能な幅を無視して子の幅に広がる

CSS 仕様では `overflow: scroll` は scroll container を作成し、コンテナの auto size は overflow したコンテンツを無視するべきだが、Taffy 0.9.0 はこの仕様を完全には実装していない。

---

## 3. GPUI の scroll コンテナ実装

### 3.1 GPUI が使用する Taffy バージョン

| プロジェクト | GPUI | Taffy | 横スクロール |
|-------------|------|-------|-------------|
| **Zelkova** | 0.2.2 (crates.io) | **0.9.0**（`"=0.9.0"` 固定） | 不可 |
| **Zed** | 0.2.2 (git fork) | **0.10.1**（`"=0.10.1"`） | カスタム Element でのみ可能 |

GPUI 0.2.2 は Taffy `"=0.9.0"` で固定しているため、Taffy 単体のアップグレードは不可。また、Taffy 0.10.1 へのアップグレードだけでは横スクロール問題は解決しない（`scroll-experiments.md` Approach 8 で検証済み）。

### 3.2 Overflow API

`gpui-macros` の `overflow_style_methods!()` マクロで生成:

| メソッド | 効果 |
|----------|------|
| `overflow_hidden()` | `overflow.x = Hidden`, `overflow.y = Hidden` |
| `overflow_scroll()` | `overflow.x = Scroll`, `overflow.y = Scroll` |
| `overflow_x_scroll()` | `overflow.x = Scroll` |
| `overflow_y_scroll()` | `overflow.y = Scroll` |

### 3.3 content_size の計算

GPUI の `div.rs` における `content_size` 計算（prepaint 時）:

```rust
// div.rs の content_size 計算（概要）
for child_layout_id in &request_layout.child_layout_ids {
    let child_bounds = window.layout_bounds(*child_layout_id);
    child_min = child_min.min(&child_bounds.origin);
    child_max = child_max.max(&child_bounds.bottom_right());
}
content_size = (child_max - child_min).into();
```

**問題点**: Taffy が `overflow: scroll` を設定しても、GPUI は子の実際の layout bounds をそのまま使う。コンテナが広がっていれば `content_size` も広がり、`content_size == bounds.size` となりスクロール範囲が 0 になる。

### 3.4 ScrollHandle の仕組み

`ScrollHandle` が管理する状態:
- `bounds()` — スクロールコンテナのビューポートサイズ（Taffy レイアウト結果から取得）
- `offset()` — 現在のスクロール位置（負の Point）
- `max_offset()` — 最大スクロールオフセット（`content_size - bounds.size` 相当）
- `content_size()` — `max_offset() + bounds().size`（gpui-component の ScrollbarHandle trait 実装）

スクロールホイールイベントは `div.rs` の prepaint/paint で処理され、`overflow == Scroll` の場合に `delta` を `offset` に加算して `max_offset` でクランプする。横スクロールのイベント処理自体は実装済みだが、`max_offset.x` が 0 になるため機能しない。

### 3.5 element_offset

`window.with_element_offset(Point, closure)` は描画時の視覚的なオフセットを適用する。レイアウトには影響しない。`EditorContentElement` で横スクロールの視覚的シフトに使用。

---

## 4. Zed の横スクロールアプローチ

Zed は GPUI の `div` + `overflow_scroll()` を使わず、**完全にカスタムの `Element` トレイト実装**でスクロールを自前管理している。

### 4.1 全体アーキテクチャ

| ファイル | 役割 |
|---------|------|
| `crates/editor/src/element.rs` (~13,600行) | `EditorElement` の `Element` トレイト実装。レイアウト・描画・スクロールバー全てを管理 |
| `crates/editor/src/scroll.rs` (955行) | `ScrollManager` — スクロール状態（`ScrollAnchor` + offset）の管理 |
| `crates/editor/src/display_map.rs` | `longest_row()` — 最長行の特定 |

### 4.2 request_layout: コンテナが広がらない仕組み

```rust
// element.rs request_layout（概要）
fn request_layout(&mut self, ...) -> (LayoutId, EditorRequestLayoutState) {
    let mut style = Style::default();
    style.size.width = relative(1.).into();   // 親の幅いっぱい
    style.size.height = relative(1.).into();  // 親の高さいっぱい
    window.request_layout(style, None, cx)
}
```

`relative(1.)` で Taffy に「親サイズいっぱい」を指示。子のコンテンツ幅は Taffy レイアウトに影響しない。overflow 設定は一切使わない。

### 4.3 コンテンツ幅の計算

Zed は `window.text_system().shape_line()` でテキストをシェイプし、ピクセル単位の正確な幅を取得する:

```rust
// element.rs prepaint 内
let longest_line_width = layout_line(
    snapshot.longest_row(),
    &snapshot,
    style,
    editor_width,
    is_row_soft_wrapped,
    window,
    cx,
).width;
```

`longest_row()` は `DisplaySnapshot` から最長の表示行を特定。`shape_line()` はフォントメトリクスを考慮したピクセル精度の幅を返す。

### 4.4 scroll_max の計算

```rust
// element.rs prepaint 内
let scroll_max = point(
    ((scroll_width - editor_width) / em_layout_width).max(0.0),
    max_scroll_top,
);
```

- `scroll_width` = 最長行のピクセル幅
- `editor_width` = ビューポート幅（`bounds.size.width`）
- `scroll_max.x` = `max(0, scroll_width - editor_width)`

GPUI の `ScrollHandle` に依存せず、自前で計算。

### 4.5 描画とクリッピング

```rust
// 各行フラグメントの描画位置にスクロールオフセットを適用
let fragment_origin = content_origin
    + point(-scroll_pixel_position.x, line_y);

// ContentMask でビューポートにクリップ
window.with_content_mask(Some(ContentMask { bounds }), |window| {
    // 行・カーソル・選択範囲の描画
});
```

### 4.6 スクロール状態管理

```rust
// scroll.rs
pub struct ScrollAnchor {
    pub offset: gpui::Point<ScrollOffset>,
    pub anchor: Anchor,  // バッファ内のアンカーポイント
}
```

`ScrollManager` は `scroll_max_x` を持ち、ホイールイベントで `offset.x` を更新して `scroll_max_x` でクランプする。

### 4.7 Zed のスクロールバー

Zed のスクロールバーもカスタム描画（`EditorScrollbars` 構造体）。`gpui-component` の `Scrollbar` は使用しない。

UI コンポーネント（エディタ以外）では `gpui-component` 相当の `Scrollbars<T: ScrollableHandle>` を使用:

```rust
// crates/ui/src/components/scrollbar.rs
pub trait ScrollableHandle: 'static + Any + Sized + Clone {
    fn max_offset(&self) -> Point<Pixels>;
    fn set_offset(&self, point: Point<Pixels>);
    fn offset(&self) -> Point<Pixels>;
    fn viewport(&self) -> Bounds<Pixels>;

    fn content_size(&self) -> Size<Pixels> {
        self.viewport().size + self.max_offset().into()
    }
}
```

`ScrollAxes::Both` の場合、`overflow_scroll()` を設定して GPUI ネイティブの両軸スクロールを有効化する。ただし、これはエディタ以外のコンポーネント（データテーブル等）での使用に限られる。

### 4.8 データフロー全体

```
1. request_layout()
   → style.size = relative(1.) でコンテナサイズを親に決定
   → コンテンツ幅はレイアウトに影響しない

2. prepaint()
   → bounds から editor_width を取得
   → shape_line() で longest_line_width を計算
   → scroll_max = longest_line_width - editor_width
   → 可視行の範囲を計算（start_row, end_row）

3. paint()
   → ContentMask でビューポートにクリップ
   → 各行を -scroll_offset で描画位置をシフト
   → カーソル・選択範囲も同じオフセットで描画
   → スクロールバーをカスタム描画

4. on_scroll_wheel
   → delta を scroll_position に加算
   → scroll_max でクランプ
   → cx.notify() で再描画
```

---

## 5. gpui-component Scrollbar

### 5.1 ScrollbarHandle trait

```rust
// gpui-component-0.5.1/src/scroll/scrollbar.rs
pub trait ScrollbarHandle: 'static {
    fn offset(&self) -> Point<Pixels>;
    fn set_offset(&self, offset: Point<Pixels>);
    fn content_size(&self) -> Size<Pixels>;
    fn start_drag(&self) {}
    fn end_drag(&self) {}
}
```

`ScrollHandle` のデフォルト実装:
```rust
impl ScrollbarHandle for ScrollHandle {
    fn content_size(&self) -> Size<Pixels> {
        self.max_offset() + self.bounds().size
    }
}
```

### 5.2 つまみサイズ計算

```rust
// scrollbar.rs prepaint 内
let scroll_size = self.scroll_handle.content_size();

for axis in self.axis.all() {
    let (scroll_area_size, container_size, scroll_position) = match axis { ... };

    if scroll_area_size <= container_size {
        continue; // スクロールバー非表示
    }

    let thumb_length = (container_size / scroll_area_size * container_size)
        .max(px(MIN_THUMB_SIZE)); // MIN_THUMB_SIZE = 48px

    let thumb_start = -(scroll_position / (scroll_area_size - container_size)
        * (container_size - margin_end - thumb_length));
}
```

`content_size()` が正確でなければ、つまみのサイズと位置がずれる。

### 5.3 ScrollbarAxis

```rust
pub enum ScrollbarAxis {
    Vertical,
    Horizontal,
    Both,
}
```

`Both` の場合、水平→垂直の順で処理。水平スクロールバーは垂直スクロールバーと重ならないよう `margin_end = WIDTH` (16px) を確保。

---

## 6. バグか仕様か

### 結論: バグに近い仕様ギャップ

Taffy 0.9.0 の振る舞いは以下の理由で「バグ」と言える:

1. **CSS仕様違反**: CSS仕様では `overflow: scroll` は scroll container を作成し、コンテナの auto size は overflow したコンテンツを無視するべき。Taffy はこの振る舞いを v0.4 の段階で不完全にしか実装していない。

2. **Taffy 側で認識済み**: Issue [#696](https://github.com/DioxusLabs/taffy/issues/696) で同一の問題が報告され、修正済み（ただし GPUI が使う 0.9.0 には未取り込み）。

3. **PR #875 で根本修正予定**: `content_size` を `scrollable_overflow` に置き換える PR が進行中。これにより、scroll container の intrinsic size と overflow region が正しく分離される。

GPUI 側の問題:

1. **content_size の独自計算**: GPUI は Taffy の `content_size` 出力を使わず、子の layout bounds から独自に計算している。Taffy 側で `scrollable_overflow` が正しく実装されても、GPUI がそれを使わなければ意味がない。

2. **Taffy バージョン固定**: GPUI 0.2.2 が Taffy 0.9.0 に固定しているため、たとえ Taffy 側で修正がマージされても GPUI 側でアップグレードが必要。

**Zelkova に取っての意味**: Taffy/GPUI の修正を待つのは現実的ではない。Zed と同様にカスタム Element で自前管理するのが確実な道。

---

## 7. 現在の Zelkova 実装

### 7.1 スクロール構造（PR #125 revert 後）

```
div (size_full, flex_col, overflow_hidden)
  ├── header (frontmatter)
  └── scroll_container (flex_1, relative, overflow_hidden)
       ├── scroll_div (absolute, size_full, overflow_y_scroll, track_scroll)
       │    └── content_div (flex_col)
       │         ├── line 0 (whitespace_nowrap, h=22px)
       │         ├── line 1
       │         └── ...
       └── scrollbar_overlay (absolute, top_0, left_0, right_0, bottom_0)
            └── Scrollbar (axis=Both or Vertical)
```

### 7.2 wrap=false の横スクロール

現在、`overflow_y_scroll()` のみ使用。横スクロールは `EditorContentElement`（カスタム Element）で `element_offset` により視覚的にシフト。`HScrollHandle` が `scroll_x` と `content_width` を手動管理。

コンテンツ幅の計算:
```rust
let max_line_width = lines
    .iter()
    .map(|l| l.chars().count() as f32 * 7.2) // 7.2px per ASCII char
    .fold(0.0_f32, f32::max);
```

**問題**: `chars().count() * 7.2` は近似値。CJK 文字（~2x 幅）、タブ、フォントメトリクスの違いを考慮していない。

---

## 8. 解決策の評価

### Option A: GPUI ネイティブ横スクロールの改善を待つ

**前提**: Taffy PR #875 がマージ → GPUI が Taffy をアップグレード → GPUI の content_size 計算を修正

**メリット**: 最もシンプル。div + overflow_scroll() で両軸スクロールが動作する。

**デメリット**: 時期未定（Taffy PR #875 は Draft）。GPUI 0.2 の更新頻度も不明。Zelkova は制御外。

**評価**: 待つべきではない。数ヶ月〜数年かかる可能性。

### Option B: Zed 方式のカスタム Element（推奨）

Zed と同様に `EditorContentElement` を完全なカスタムスクロールコンテナとして実装。

**設計**:
```rust
pub struct EditorContentElement {
    editor: Entity<Editor>,
}

impl Element for EditorContentElement {
    fn request_layout(...) -> (LayoutId, ()) {
        // relative(1.) でコンテナを親サイズに固定
        // コンテンツ幅はレイアウトに影響しない
    }

    fn prepaint(...) {
        // bounds から viewport 幅を取得
        // shape_line() または推定で content_width を計算
        // scroll_max = content_width - viewport_width
        // 可視行の範囲を計算
    }

    fn paint(...) {
        // ContentMask でクリップ
        // 各行を -scroll_offset で描画
        // ホイールイベントで scroll_x を更新
    }
}
```

**メリット**:
- Taffy/GPUI の制約に依存しない
- Zed で実証済みのアプローチ
- ピクセル精度のスクロールが可能（`shape_line()` 使用時）
- 仮想化（可視行のみ描画）も可能

**デメリット**:
- 実装コストが高い（数百行のカスタム Element コード）
- マウスイベント（クリック、ドラッグ）の自前処理
- IME 入力の統合
- 画像行の描画もオフセット管理が必要

### Option C: 現在のアプローチの改善（応急処置）

`HScrollHandle` + `EditorContentElement` を維持しつつ、`chars().count() * 7.2` をより正確な計算に置き換える。

**改善案**:
- `window.text_system().shape_line()` で各行のピクセル幅を計算
- または `canvas()` の paint callback で実際の描画幅を測定

**メリット**: 実装コストが低い。

**デメリット**: 依然としてカスタム Element の複雑さを抱える。`shape_line()` を使うなら Option B と変わらない。

### 推奨ロードマップ

1. **短期**: 現在の `overflow_y_scroll()` + Scrollbar(Vertical) を維持。横スクロールは一時停止。
2. **中期**: Option B（カスタム Element）を実装。縦横両方のスクロールを自前管理。`shape_line()` でピクセル精度のコンテンツ幅を計算。
3. **長期**: GPUI のネイティブ横スクロールが改善されたら、カスタム Element を段階的に削除。

---

## 9. 参考文献

### Taffy Issues & PRs

- [#470](https://github.com/DioxusLabs/taffy/issues/470) — Output "scroll width/height" for each node（content_size 機能の起源）
- [#573](https://github.com/DioxusLabs/taffy/pull/573) — Compute content size, padding and border for each node（content_size 実装）
- [#696](https://github.com/DioxusLabs/taffy/issues/696) — Flexbox container isn't shrinking to be smaller than its content size + margins（Zelkova と同一の問題）
- [#871](https://github.com/DioxusLabs/taffy/issues/871) — `content_size` for internal nodes incorrectly includes the node's left/top border
- [#875](https://github.com/DioxusLabs/taffy/pull/875) — scrollable_overflow implementation（Draft、将来の根本修正）
- [#885](https://github.com/DioxusLabs/taffy/issues/885) — absolute positioned node causes scroll width/height on parent
- [#954](https://github.com/DioxusLabs/taffy/issues/954) — Incorrect grid content size computation

### CSS 仕様

- [CSS Overflow 3 §2.2 — Scrollable Overflow](https://www.w3.org/TR/css-overflow-3/#scrollable)

### Zed ソースコード

- [`crates/editor/src/element.rs`](https://github.com/zed-industries/zed/blob/main/crates/editor/src/element.rs) — EditorElement の Element 実装
- [`crates/editor/src/scroll.rs`](https://github.com/zed-industries/zed/blob/main/crates/editor/src/scroll.rs) — ScrollManager
- [`crates/editor/src/display_map.rs`](https://github.com/zed-industries/zed/blob/main/crates/editor/src/display_map.rs) — longest_row()
- [`crates/ui/src/components/scrollbar.rs`](https://github.com/zed-industries/zed/blob/main/crates/ui/src/components/scrollbar.rs) — UI コンポーネント用 Scrollbar

### 社内ドキュメント

- [`crates/gui/docs/scroll-experiments.md`](scroll-experiments.md) — Issue #82 の試行記録（8つのアプローチと検証結果）
