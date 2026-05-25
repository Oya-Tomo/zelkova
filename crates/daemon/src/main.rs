mod handlers;
mod indexer;
mod watcher;

use anyhow::{Context, Result};
use std::sync::{Arc, Mutex};
use zelkova_config::AppConfig;
use zelkova_note_core::{DirectoryStructure, Vault};
use zelkova_rpc::server::RpcServer;
use zelkova_search::SearchIndex;

struct DaemonState {
    vault: Vault,
    search_index: Box<dyn SearchIndex>,
    config: AppConfig,
    directory: Mutex<DirectoryStructure>,
}

fn main() -> Result<()> {
    let config = AppConfig::load().context("failed to load config")?;

    let vault = Vault::new(config.note.vault_path.clone()).context("failed to open vault")?;

    let index_path = config.note.vault_path.join(".zelkova").join("index");
    let search_index =
        zelkova_search::default_search_index(&index_path).context("failed to open search index")?;

    let directory = DirectoryStructure::load(&config.note.vault_path)
        .context("failed to load directory structure")?;

    let state = Arc::new(DaemonState {
        vault,
        search_index,
        config: config.clone(),
        directory: Mutex::new(directory),
    });

    // initial index
    if config.daemon.index_on_start {
        let count = indexer::rebuild_index(&state)?;
        eprintln!("indexed {count} notes");
    }

    // start RPC server
    let server = RpcServer::bind(&config.daemon.socket_path).with_context(|| {
        format!(
            "failed to bind RPC socket at {}",
            config.daemon.socket_path.display()
        )
    })?;

    eprintln!(
        "zelkovad listening on {}",
        config.daemon.socket_path.display()
    );

    write_pid_file(&config)?;

    // Start file watcher
    if config.daemon.index_on_start {
        if let Err(e) = watcher::start_watcher(state.clone()) {
            eprintln!("warning: file watcher failed to start: {e}");
        }
    }

    loop {
        let state = state.clone();
        if let Err(e) = server.accept_one(&move |req| handlers::handle_request(req, &state)) {
            eprintln!("error handling connection: {e}");
        }
    }
}

fn write_pid_file(config: &AppConfig) -> Result<()> {
    let pid_dir = config.note.vault_path.join(".zelkova");
    std::fs::create_dir_all(&pid_dir)?;
    let pid_path = pid_dir.join("daemon.pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;
    Ok(())
}
