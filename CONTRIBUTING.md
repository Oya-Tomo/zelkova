# Contributing to Zelkova

Thank you for your interest in contributing to Zelkova! This document describes the development workflow and coding standards.

## Development Flow

### Issue-Driven Development

All work starts with an Issue. Before writing any code:

1. **Check the Issue** — Read the spec and acceptance criteria
2. **Discuss the spec** — Resolve any ambiguity before implementing
3. **Document the agreed spec** — Add a `## Spec (Confirmed)` section to the Issue body

### Branch Strategy (Release Flow)

```
main (protected, stable releases)
  └─ develop (integration branch)
       └─ <prefix>/<issue-number>-<description>
```

**Branch prefixes:**

| Prefix | Purpose |
|---|---|
| `feature/` | New functionality |
| `bugfix/` | Bug fixes |
| `docs/` | Documentation changes |
| `refactor/` | Code cleanup without spec changes |
| `chore/` | Build, dependencies, tooling |

**Branch naming:** `<prefix>/<issue-number>-<description>`
Example: `feature/42-add-math-preview`, `bugfix/346-fix-range-selection`

Always branch from `develop`.

### Commit Messages (Conventional Commits)

```
type(scope): description
```

- **type:** `feat`, `fix`, `docs`, `refactor`, `chore`
- **scope:** crate name (`gui`, `highlight`, `config`, `daemon`, `cli`, `rpc`, `search`, `note_core`, `markdown`, `rope`)
- **Example:** `feat(highlight): add Go language support`

### Pull Requests

- Target branch: `develop`
- Merge strategy: **Squash Merge** (1 Issue = 1 commit)
- PR size guideline: **under 400 lines of diff**
- CI must pass before merge

### Versioning (SemVer)

`vMAJOR.MINOR.PATCH`. Pre-`v1.0.0`, breaking changes are treated as minor bumps.

Releases are cut as milestones: when all Issues in a milestone are closed, create a `release/vX.Y.Z` branch from `develop`, then merge into `main`.

## Coding Standards

### Rust Quality Rules

| Rule | Detail |
|---|---|
| **No `unwrap()`** | Use `expect("reason")` instead. CI enforces `clippy::unwrap_used = "deny"` |
| **`expect()` requires a reason** | `expect("index is valid because len was checked")` — explain *why* it's safe |
| **No silent error suppression** | Don't use `let _ = ...` to ignore errors. Log or propagate instead |
| **`unsafe` requires SAFETY comment** | `// SAFETY: ...` explaining why the unsafe block is sound |
| **Avoid unnecessary `clone()`** | Use `&str` where `String` isn't needed |
| **TODO/FIXME with Issue number** | `// TODO(#42): handle edge case` |

### Code Change Checklist

- [ ] Refactor for clarity after implementing
- [ ] Update `docs/architecture.md` if architecture changed
- [ ] Add tests for new functions and logic
- [ ] Run `cargo test` and `cargo clippy` — both must pass

## Security

- **No secrets in commits** — `.env`, API keys, tokens, passwords are never committed
- **No destructive changes without confirmation** — API changes, file deletions require explicit approval
- **No force pushes** — Especially on `develop` and `main`

## Language Convention

- Commit messages, PR descriptions, Issue text: **English**
- Discussions, spec reviews, comments: **Japanese**

## CI

All PRs run through:

- `cargo test` — All tests must pass
- `cargo clippy` — No warnings with deny-level lints
- `cargo fmt --check` — Code must be formatted
- `cargo audit` — No known vulnerabilities in dependencies
