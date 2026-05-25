use anyhow::{Context, Result};
use std::process::Command;
use uuid::Uuid;
use zelkova_config::AppConfig;
use zelkova_rpc::client::RpcClient;

pub fn search(
    client: &RpcClient,
    query: &str,
    tags: Vec<String>,
    limit: Option<usize>,
) -> Result<()> {
    let results = client.search(query, tags, limit).context("search failed")?;
    if results.results.is_empty() {
        println!("No results found.");
        return Ok(());
    }
    for hit in &results.results {
        println!("{}  {}  (score: {:.2})", hit.id, hit.title, hit.score);
        if !hit.snippet.is_empty() {
            println!("  {}", hit.snippet);
        }
    }
    Ok(())
}

pub fn list(client: &RpcClient, tag: Option<&str>) -> Result<()> {
    let result = client.list_notes(tag).context("list failed")?;
    if result.notes.is_empty() {
        println!("No notes found.");
        return Ok(());
    }
    for note in &result.notes {
        let tags = if note.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", note.tags.join(", "))
        };
        println!("{}  {}{}", note.id, note.title, tags);
    }
    Ok(())
}

pub fn show(client: &RpcClient, id: &Uuid) -> Result<()> {
    let result = client.get_note(id).context("get_note failed")?;
    println!("Title: {}", result.title);
    println!("Path:  {}", result.path.display());
    println!("Tags:  {}", result.tags.join(", "));
    println!("Created: {}", result.created);
    println!("Updated: {}", result.updated);
    println!("---");
    println!("{}", result.content);
    Ok(())
}

pub fn create(client: &RpcClient, title: Option<&str>, tags: Vec<String>) -> Result<()> {
    let result = client
        .create_note(title, tags)
        .context("create_note failed")?;
    println!("Created note: {}", result.id);
    let display_title = if result.title.is_empty() {
        "(untitled)"
    } else {
        &result.title
    };
    println!("  Title: {}", display_title);
    println!("  Path:  {}", result.path.display());
    Ok(())
}

pub fn tags(client: &RpcClient) -> Result<()> {
    let result = client.tags().context("tags failed")?;
    for tag in &result.tags {
        println!("{tag}");
    }
    Ok(())
}

pub fn daemon_status(config: &AppConfig) -> Result<()> {
    let pid_path = config.note.vault_path.join(".zelkova").join("daemon.pid");
    let socket = &config.daemon.socket_path;

    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        let pid: u32 = pid_str.trim().parse().context("invalid PID")?;
        // check if process is running
        let running = std::path::Path::new("/proc").join(pid.to_string()).exists();
        if running {
            println!("Daemon running (PID {pid})");
            println!("Socket: {}", socket.display());
            return Ok(());
        }
    }

    println!("Daemon not running");
    Ok(())
}

pub fn daemon_start(_config: &AppConfig) -> Result<()> {
    // find zelkovad binary
    let exe = std::env::current_exe().context("cannot determine current executable")?;
    let daemon_exe = exe.parent().unwrap().join("zelkovad");

    if !daemon_exe.exists() {
        anyhow::bail!("zelkovad binary not found at {}", daemon_exe.display());
    }

    Command::new(&daemon_exe)
        .spawn()
        .with_context(|| format!("failed to start daemon at {}", daemon_exe.display()))?;

    println!("Daemon started");
    Ok(())
}

pub fn daemon_stop(config: &AppConfig) -> Result<()> {
    let pid_path = config.note.vault_path.join(".zelkova").join("daemon.pid");

    let pid_str = std::fs::read_to_string(&pid_path).context("daemon PID file not found")?;
    let pid: u32 = pid_str.trim().parse().context("invalid PID")?;

    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    println!("Daemon stopped (PID {pid})");
    Ok(())
}

pub fn rebuild_index(client: &RpcClient) -> Result<()> {
    let request = zelkova_rpc::JsonRpcRequest::new(1, zelkova_rpc::METHOD_REBUILD_INDEX, None);
    let response = client
        .send_request(&request)
        .context("rebuild_index request failed")?;

    if let Some(error) = response.error {
        anyhow::bail!("rebuild_index error: {} ({})", error.message, error.code);
    }

    if let Some(result) = response.result {
        println!("Rebuilt index: {result}");
    }
    Ok(())
}
