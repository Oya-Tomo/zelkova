# ADR-0001: Centralize vault access through daemon RPC

Date: 2026-06-02

## Status

Proposed

## Context and Problem Statement

Zelkova has a multi-client architecture: GUI (GPUI), CLI, and MCP all need to read and write notes in a shared vault. Currently, the GUI bypasses the daemon for file reads and writes — `Editor::load()` and `Editor::save_to_disk()` access the filesystem directly via `std::fs`. Only metadata operations (list, create, move, delete) go through the daemon via JSON-RPC.

This causes two problems:

1. **Existing Markdown files are invisible** — `parse_frontmatter()` in `note_core` requires YAML frontmatter. Plain `.md` files are silently skipped during `list_notes`, so the user's existing notes don't appear in the sidebar.
2. **Inconsistent access paths** — Two routes to the same vault (RPC vs direct fs) mean the data model, validation, and event propagation diverge between clients.

Additionally, future requirements demand a centralized process:

- **Collaborative editing** (CRDT/OT) — requires a central coordinator to order operations, resolve conflicts, and broadcast changes to all connected clients.
- **Vector/semantic search** — embedding models are memory-heavy (hundreds of MB to GB). A long-running daemon can keep the model loaded, while spawning it per CLI invocation is impractical.

### Precedent research

| Application | File I/O pattern | Process model | Concurrency |
|---|---|---|---|
| **Zed** | `Fs` trait, in-process | Single process | Single process |
| **Obsidian** | `Vault` API + `DataAdapter`, in-process | Single process (Electron) | Single instance |
| **Helix** | Direct `std::fs` | Single process | Single process |
| **Kakoune** | Direct POSIX I/O | Client-Server (Unix Socket) | Multiple clients, shared buffers |
| **Inkdrop** | PouchDB (LevelDB) | Single process (Electron) | Single instance (DB lock) |

Most editors use single-process in-process I/O. Kakoune's client-server model is closest to Zelkova's daemon approach, but serves a different purpose (shared editing buffers, not API gateway). No precedent uses a daemon as API gateway for file I/O — Zelkova's approach is unusual but justified by its multi-client (GUI + CLI + MCP) and future collab/search requirements.

## Decision Drivers

- **Consistency**: All clients must interpret the vault the same way. A single code path (note_core inside the daemon) prevents drift.
- **Future collab readiness**: Writes must be observable by a coordinator for CRDT merge/conflict resolution.
- **Search index freshness**: Changes should update the search index immediately, not via polling with seconds of lag.
- **Startup cost**: CLI and MCP must not scan the entire vault on every invocation. A long-running daemon with in-memory state solves this.
- **Simplicity for client authors**: CLI and MCP should only need an RPC client, not a vault scanning engine.

## Considered Options

### Option A: Distribute note_core — daemon for indexing only

Each crate (GUI, CLI, MCP) imports `note_core` directly. Daemon handles file watching and search indexing.

```
GUI  ──→ note_core ←── CLI
Daemon ──→ note_core     ←── MCP
              │
         Vault (filesystem)
```

### Option B: Distribute note_core — daemon with event notifications

Same as Option A, but after writing, clients notify the daemon via RPC (`note_updated`) so it can update the search index immediately.

```
GUI ──→ note_core ──→ write to disk
                    ──→ notify daemon → reindex
```

### Option C: Daemon as centralized API gateway

All vault operations (read, write, list, move, delete) go through daemon RPC. Only the daemon imports `note_core`. Clients are thin RPC consumers.

```
GUI  ──→ RPC ──→ Daemon ──→ note_core ──→ Vault
CLI  ──→ RPC ──┘
MCP  ──→ RPC ──┘
```

## Decision Outcome

Chosen: **Option C — Daemon as centralized API gateway**.

### Justification

1. **Collaborative editing requires centralized writes.** CRDT coordination is impossible if clients write directly to the filesystem without the daemon observing the change. Even with event notifications (Option B), a missed notification breaks consistency.
2. **Vector search requires a long-running process.** Embedding models are too expensive to load per CLI invocation. The daemon keeps them resident.
3. **CLI/MCP startup cost is negligible.** The daemon already has the vault state in memory; an RPC call returns results in milliseconds versus a full filesystem scan.
4. **note_core becomes the single source of truth.** Only the daemon imports it, so there is one interpretation of the vault format, one validation path, and one place to evolve the schema.

### Consequences

**Good:**
- Consistency: one code path for all vault mutations.
- Collab-ready: daemon observes every write, can coordinate CRDT operations.
- Search index stays fresh: daemon updates index synchronously on write.
- CLI/MCP are lightweight: only need `rpc` crate, not `note_core`.
- Testability: daemon can be tested with integration tests against `note_core`; clients can be tested against a mock RPC server.

**Bad:**
- Daemon is a single point of failure. If it crashes, all clients lose access to the vault.
  - Mitigation: auto-restart via systemd/supervise; graceful degradation is a future consideration.
- IPC overhead on every read/write. For large notes, this adds latency compared to direct filesystem access.
  - Mitigation: RPC responses return metadata + content; benchmark and optimize transport if needed.
- GUI Editor currently does direct file I/O. Refactoring to use RPC for reads and writes is a breaking change to the editor's save/load pipeline.

**Neutral:**
- `note_core` is no longer imported by GUI, CLI, or MCP. It becomes a daemon-internal dependency.
- The daemon's RPC API becomes the public contract for all vault operations. API changes must be backward-compatible.

## Confirmation

- [ ] GUI Editor no longer calls `std::fs` directly for vault files — all file I/O goes through RPC.
- [ ] CLI uses `rpc` crate exclusively for vault operations.
- [ ] `note_core` is only a dependency of the `daemon` crate (verify via `Cargo.toml`).
- [ ] Integration tests cover the full RPC round-trip: create note → read note → update note → list notes → delete note.

## More Information

- Related Issue: #142 (fix(note_core): tolerate existing markdown files without frontmatter and refactor vault access)
- Reference: `docs/architecture.md` for current crate graph and workspace members.
- ADR template: [MADR](https://adr.github.io/adr-templates/)
