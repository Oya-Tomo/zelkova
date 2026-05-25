# CLAUDE.md — Claude Code 開発ルール

## プロジェクト概要

Zelkova: GPUI 0.2 ベースの Markdown ノートアプリ。10 crate のワークスペース構成。

## 開発フロー

### ブランチ戦略: Release Flow

```
main (protected, always stable)
  └─ develop (integration branch)
       └─ feature/42-add-math-preview
       └─ bugfix/346-fix-range-selection
       └─ docs/12-update-architecture
       └─ refactor/55-extract-render-method
       └─ chore/10-bump-tree-sitter
```

- **ブランチ名**: `<prefix>/<Issue番号>-<タスク内容>`
- **プレフィックス**: feature, bugfix, docs, refactor, chore
- **ベースブランチ**: 必ず `develop` から切る
- **マージ**: PR 経由で Squash Merge

### バージョニング: SemVer

`vMAJOR.MINOR.PATCH`。まだ `v1.0.0` ではないので breaking change は minor で扱う。

### コミットメッセージ: Conventional Commits

```
type(scope): description
```

- **type**: feat, fix, docs, refactor, chore
- **scope**: crate 名 (gui, highlight, config, daemon, cli, rpc, search, note_core, markdown, rope)
- **例**: `feat(highlight): add Go language support`

## Issue 駆動開発

1. Issue を受け取ったら内容を確認
2. `/grill-me` で仕様の齟齬がなくなるまで議論
3. 合意した仕様を Issue 本文の `## 仕様（確定）` セクションに追記
4. 適切なブランチを作成して実装開始
5. 実装状況は Issue コメントで逐一報告
6. PR 作成時に Issue をクローズ

## コーディングルール

### Rust 品質ルール

| ルール | 詳細 |
|---|---|
| **`unwrap()` 禁止** | `clippy::unwrap_used` を警告。代わりに `expect("理由")` を使う。CI で deny 予定（既存コードの修正後） |
| **`expect()` には理由を書く** | `expect("index is valid because len was checked")` のように「なぜ安全か」を書く |
| **`let _ = ...` でエラーを握りつぶさない** | エラーは必ずログ出力または伝播。`clippy::let_underscore_untyped = "warn"` |
| **`unsafe` には SAFETY コメント** | `// SAFETY: ...` で安全性の根拠を必ず書く |
| **`clone()` の乱用に注意** | 不要なアロケーションを避ける。`&str` で済むところは `String` を返さない |
| **TODO/FIXME には Issue 番号** | `// TODO(#42): handle edge case` の形式 |

### コード変更時の義務

1. **リファクタリング** — 実装後に冗長性・重複・可読性を確認し整理する
2. **ドキュメント更新** — `docs/architecture.md` および `crates/*/docs/architecture.md` も併せて見直す
3. **テスト** — 新しい関数・ロジックには `#[cfg(test)]` でテストを書く
4. **`cargo test` + `cargo clippy`** — 変更後に必ず両方を通す

### PR のサイズ

1 PR は **400行以内**を目安。超えそうなら Issue にサブタスクを列挙して分割する。

## セキュリティ

- **機密情報はコミットしない** — `.env`, API key, トークン, パスワードは例外なし
- **破壊的変更は事前確認** — 既存 API の変更・ファイル削除はユーザーに確認してから実行
- **force push しない** — 特に `develop`, `main` には絶対に force push しない

## コミュニケーション

- コミットメッセージ・PR 本文・Issue 本文: **英語**
- 会話・コメント・仕様議論: **日本語**

## プロジェクト構造

```
crates/
├── gui/           GPUI エディタ (bin: zelkova)
├── daemon/        バックグラウンドデーモン (bin: zelkovad)
├── cli/           CLI ツール (bin: zelkova-cli)
├── markdown/      Markdown パーサー
├── highlight/     Tree-sitter コードハイライト
├── rope/          B-tree テキストバッファ
├── note_core/     Note データモデル・Vault CRUD
├── rpc/           JSON-RPC 2.0 (Unix socket)
├── search/        全文検索 (Tantivy)
└── config/        TOML 設定 (app/keymap/theme)
```
