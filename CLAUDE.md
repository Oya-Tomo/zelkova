# CLAUDE.md — Claude Code Development Rules

## Project Overview

Zelkova: A GPUI 0.2-based Markdown note-taking application. 10-crate workspace.

## Development Flow

### Branch Strategy: Release Flow

```
main (protected, always stable)
  └─ develop (integration branch)
       └─ feature/42-add-math-preview
       └─ bugfix/346-fix-range-selection
       └─ docs/12-update-architecture
       └─ refactor/55-extract-render-method
       └─ chore/10-bump-tree-sitter
```

- **Branch naming**: `<prefix>/<issue-number>-<task-description>`
- **Prefixes**: feature, bugfix, docs, refactor, chore
- **Base branch**: Always branch from `develop`
- **Merge**: PR via Squash Merge

### Versioning: SemVer

`vMAJOR.MINOR.PATCH`. Pre-`v1.0.0`, so breaking changes are treated as minor bumps.

### Commit Messages: Conventional Commits

```
type(scope): description
```

- **type**: feat, fix, docs, refactor, chore
- **scope**: crate name (gui, highlight, config, daemon, cli, rpc, search, note_core, markdown, rope)
- **Example**: `feat(highlight): add Go language support`

## Issue-Driven Development

1. Read the Issue and understand the requirements
2. Use `/grill-me` to discuss the spec until there is no ambiguity
3. Add the agreed spec to the Issue body under `## Spec (Confirmed)`
4. Create an appropriate branch and start implementing
5. Report progress in Issue comments as you go
6. Close the Issue when creating the PR

## Coding Rules

### Rust Quality Rules

| Rule | Detail |
|---|---|
| **No `unwrap()`** | `clippy::unwrap_used` is a warning. Use `expect("reason")` instead. CI will deny this once existing code is fixed |
| **`expect()` requires a reason** | `expect("index is valid because len was checked")` — explain *why* it is safe |
| **No silent error suppression** | Don't use `let _ = ...` to ignore errors. Log or propagate instead. `clippy::let_underscore_untyped = "warn"` |
| **`unsafe` requires SAFETY comment** | `// SAFETY: ...` explaining why the unsafe block is sound |
| **Avoid unnecessary `clone()`** | Don't return `String` where `&str` suffices |
| **TODO/FIXME with Issue number** | `// TODO(#42): handle edge case` format |

### Code Change Checklist

1. **Refactor** — After implementing, review for redundancy, duplication, and readability
2. **Update docs** — Review `docs/architecture.md` and `crates/*/docs/architecture.md` if architecture changed
3. **Add tests** — Write `#[cfg(test)]` tests for new functions and logic
4. **Run checks** — Always run `cargo test` and `cargo clippy` after changes

### PR Size

Aim for **under 400 lines** per PR. If it exceeds that, split into sub-tasks in the Issue.

## Security

- **No secrets in commits** — `.env`, API keys, tokens, passwords — no exceptions
- **No destructive changes without confirmation** — Ask before modifying existing APIs or deleting files
- **No force pushes** — Especially on `develop` and `main`

## Communication

- Commit messages, PR descriptions, Issue text: **English**
- Discussions, spec reviews, comments: **Japanese**

## Project Structure

```
crates/
├── gui/           GPUI editor (bin: zelkova)
├── daemon/        Background daemon (bin: zelkovad)
├── cli/           CLI tool (bin: zelkova-cli)
├── markdown/      Markdown parser
├── highlight/     Tree-sitter code highlighting
├── rope/          B-tree text buffer
├── note_core/     Note data model, vault CRUD
├── rpc/           JSON-RPC 2.0 (Unix socket)
├── search/        Full-text search (Tantivy)
└── config/        TOML configuration (app/keymap/theme)
```
