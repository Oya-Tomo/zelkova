mod commands;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use zelkova_config::AppConfig;
use zelkova_rpc::client::RpcClient;

#[derive(Parser)]
#[command(name = "zelkova", version, about = "Zelkova note-taking CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search notes
    Search {
        /// Search query
        query: String,
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
        /// Maximum results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// List all notes
    List {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },
    /// Show a note by ID
    Show {
        /// Note ID (UUID)
        id: String,
    },
    /// Create a new note
    Create {
        /// Note title
        title: String,
        /// Parent directory (relative to vault)
        #[arg(long)]
        dir: Option<String>,
        /// Tags
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// List all tags
    Tags,
    /// Manage the daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Check if daemon is running
    Status,
    /// Start the daemon
    Start,
    /// Stop the daemon
    Stop,
    /// Rebuild the search index
    RebuildIndex,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load().context("failed to load config")?;

    match cli.command {
        Commands::Search { query, tag, limit } => {
            let client = ensure_daemon(&config)?;
            let tags = tag.map(|t| vec![t]).unwrap_or_default();
            commands::search(&client, &query, tags, Some(limit))?;
        }
        Commands::List { tag } => {
            let client = ensure_daemon(&config)?;
            commands::list(&client, tag.as_deref())?;
        }
        Commands::Show { id } => {
            let client = ensure_daemon(&config)?;
            let uuid = uuid::Uuid::parse_str(&id).context("invalid UUID")?;
            commands::show(&client, &uuid)?;
        }
        Commands::Create { title, dir, tags } => {
            let client = ensure_daemon(&config)?;
            commands::create(&client, &title, dir.as_deref(), tags)?;
        }
        Commands::Tags => {
            let client = ensure_daemon(&config)?;
            commands::tags(&client)?;
        }
        Commands::Daemon { action } => match action {
            DaemonAction::Status => commands::daemon_status(&config)?,
            DaemonAction::Start => commands::daemon_start(&config)?,
            DaemonAction::Stop => commands::daemon_stop(&config)?,
            DaemonAction::RebuildIndex => {
                let client = ensure_daemon(&config)?;
                commands::rebuild_index(&client)?;
            }
        },
    }

    Ok(())
}

fn ensure_daemon(config: &AppConfig) -> Result<RpcClient> {
    let socket = &config.daemon.socket_path;
    if !socket.exists() {
        commands::daemon_start(config)?;
        // wait for socket to appear
        for _ in 0..50 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !socket.exists() {
            anyhow::bail!("daemon did not start: socket not found at {}", socket.display());
        }
    }
    Ok(RpcClient::new(socket))
}
