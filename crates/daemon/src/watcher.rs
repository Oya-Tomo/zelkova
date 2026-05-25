use crate::DaemonState;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Watch vault directory for file changes and reindex affected notes.
/// Uses polling-based watching to avoid inotify dependency issues.
pub fn start_watcher(state: Arc<DaemonState>) -> Result<()> {
    let vault_path = state.config.note.vault_path.clone();

    std::thread::spawn(move || {
        let mut last_snapshot = scan_files(&vault_path);
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let current_snapshot = scan_files(&vault_path);

            // Find modified files
            for (path, modified) in &current_snapshot {
                match last_snapshot.get(path) {
                    Some(prev_modified) if prev_modified != modified => {
                        eprintln!("watcher: modified: {}", path.display());
                        if let Err(e) = crate::indexer::reindex_note(path, &state) {
                            eprintln!("watcher: reindex error: {e}");
                        }
                    }
                    None => {
                        eprintln!("watcher: new: {}", path.display());
                        if let Err(e) = crate::indexer::reindex_note(path, &state) {
                            eprintln!("watcher: reindex error: {e}");
                        }
                    }
                    _ => {}
                }
            }

            // Find deleted files
            for path in last_snapshot.keys() {
                if !current_snapshot.contains_key(path) {
                    eprintln!("watcher: deleted: {}", path.display());
                }
            }

            last_snapshot = current_snapshot;
        }
    });

    Ok(())
}

fn scan_files(dir: &Path) -> std::collections::HashMap<std::path::PathBuf, std::time::SystemTime> {
    let mut result = std::collections::HashMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden directories
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().starts_with('.') {
                        continue;
                    }
                }
                result.extend(scan_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "md") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        result.insert(path, modified);
                    }
                }
            }
        }
    }
    result
}
