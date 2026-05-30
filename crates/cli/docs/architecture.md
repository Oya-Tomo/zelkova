# cli

## Role

A Clap-based CLI interface. Communicates with the daemon via an RPC client to perform note search, listing, display, creation, tag management, and daemon operations.

## Module Layout

```
src/
├── main.rs      Entry point, Clap command definitions, ensure_daemon()
└── commands.rs  Command implementations (with output formatting)
```

## Dependencies

- `clap` — CLI parser (derive API)
- `zelkova-config` — Configuration loading
- `zelkova-rpc` — RPC client
- `uuid` — UUID parsing
- `anyhow` — Error handling
- `libc` — Sending SIGTERM to stop the daemon

## Key Types / APIs

### Commands (main.rs)

Command hierarchy defined via Clap's derive API.

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
    Status,       // Check daemon status
    Start,        // Start the daemon
    Stop,         // Stop the daemon
    RebuildIndex, // Rebuild the search index
}
```

### ensure_daemon() (main.rs)

Helper that checks whether the daemon is running and starts it automatically if not.

```
1. Check if the socket file exists
2. If not:
   a. Start via daemon_start()
   b. Poll for socket appearance (up to 5 seconds, 100ms intervals)
   c. Return error on timeout
3. Return RpcClient::new(socket)
```

### Command Implementations (commands.rs)

| Function | Command | Description |
|---|---|---|
| `search(client, query, tags, limit)` | search | RPC search → display results with ID, title, score |
| `list(client, tag)` | list | RPC list_notes → display ID, title, tags |
| `show(client, id)` | show | RPC get_note → display title, path, tags, timestamps, body |
| `create(client, title, dir, tags)` | create | RPC create_note → display result with ID, title, path |
| `tags(client)` | tags | RPC tags → display tag list |
| `daemon_status(config)` | daemon status | Check process liveness via PID file and /proc |
| `daemon_start(config)` | daemon start | Spawn the zelkovad binary (async) |
| `daemon_stop(config)` | daemon stop | Read PID from file, send SIGTERM |
| `rebuild_index(client)` | daemon rebuild-index | RPC rebuild_index → display result |

### Output Formats

**search:**
```
550e8400-...  Note Title  (score: 0.85)
  Snippet text...
```

**list:**
```
550e8400-...  Note Title [rust, programming]
```

**show:**
```
Title: Note Title
Path:  /path/to/note.md
Tags:  rust, programming
Created: 2025-01-15T10:30:00+00:00
Updated: 2025-01-15T10:30:00+00:00
---
Note body
```

**create:**
```
Created note: 550e8400-...
  Title: Note Title
  Path:  /path/to/note.md
```

### Daemon Lifecycle Management

```
daemon start:
  Locate the zelkovad binary (same directory as current_exe)
  → Command::new().spawn() (run in background)

daemon stop:
  Read PID file ({vault}/.zelkova/daemon.pid)
  → libc::kill(pid, SIGTERM)

daemon status:
  Read PID file
  → Check /proc/{pid} existence to determine if running
  → Also display socket path
```

## Data Flow

```
User input
    │
    clap::parse()
    │
    ├── Search/List/Show/Create/Tags:
    │     │
    │     ensure_daemon(config)
    │       ├── Socket exists? ──No──> daemon_start()
    │       │                         Poll for socket appearance
    │       └── RpcClient::new(socket)
    │     │
    │     commands::xxx(client, ...)
    │       │
    │       client.rpc_method(...)
    │         │
    │         ── Unix Socket ──> Daemon
    │         <──────────────── Response
    │       │
    │     Format result to stdout
    │
    └── Daemon:
          ├── Status → PID file + /proc check
          ├── Start  → spawn zelkovad
          ├── Stop   → send SIGTERM
          └── RebuildIndex → ensure_daemon → RPC rebuild_index
```
