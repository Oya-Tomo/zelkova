// Config TOML read/write is this crate's purpose. FS access here touches
// GUI/CLI-owned config files (~/.config/zelkova/*), never vault files.
#![allow(clippy::disallowed_methods)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod keymap;
pub use keymap::{BindingConfig, KeymapConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub note: NoteConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub editor: EditorBehavior,
    #[serde(default)]
    pub preview: PreviewBehavior,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub override_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorBehavior {
    #[serde(default = "default_true")]
    pub wrap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewBehavior {
    #[serde(default = "default_true")]
    pub wrap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteConfig {
    #[serde(default = "default_vault_path")]
    pub vault_path: PathBuf,
    #[serde(default = "default_extension")]
    pub default_extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,
    #[serde(default = "default_true")]
    pub index_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_vault_path() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join("Notes")
}

fn default_extension() -> String {
    "md".to_string()
}

fn default_socket_path() -> PathBuf {
    PathBuf::from("/tmp/zelkova.sock")
}

fn default_true() -> bool {
    true
}

fn default_mode() -> String {
    "dark".to_string()
}

impl Default for NoteConfig {
    fn default() -> Self {
        Self {
            vault_path: default_vault_path(),
            default_extension: default_extension(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            index_on_start: true,
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl Default for EditorBehavior {
    fn default() -> Self {
        Self { wrap: true }
    }
}

impl Default for PreviewBehavior {
    fn default() -> Self {
        Self { wrap: true }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "catppuccin".to_string(),
            mode: default_mode(),
            override_path: None,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config from {}", config_path.display()))?;
        let mut config: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse config at {}", config_path.display()))?;
        config.expand_paths();
        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("cannot determine XDG config directory")?;
        Ok(config_dir.join("zelkova").join("config.toml"))
    }

    /// Expand `~` / `~/foo` in every PathBuf field that may legitimately
    /// carry a user-facing path. Rust's `PathBuf` does not perform tilde
    /// expansion the way a POSIX shell does, so without this any `~/...`
    /// value in the TOML gets treated as relative to the current working
    /// directory — which then quietly creates `./~/...` directories on
    /// whatever directory the user happened to launch the binary from.
    fn expand_paths(&mut self) {
        let home = dirs::home_dir();
        self.note.vault_path = expand_tilde_with(&self.note.vault_path, home.as_deref());
        self.daemon.socket_path = expand_tilde_with(&self.daemon.socket_path, home.as_deref());
        if let Some(p) = self.ui.override_path.take() {
            self.ui.override_path = expand_tilde_str(&p, home.as_deref());
        }
    }
}

/// Expand a leading `~` or `~/...` to `home`. Leaves every other shape
/// (absolute paths, relative paths, empty paths, `~user`) untouched.
///
/// `home = None` is treated as "no home directory known": the path is
/// returned unchanged. This is the safe behaviour when `$HOME` is unset
/// (CI sandboxes, certain container runtimes) — expanding `~` to nothing
/// would silently break user config.
fn expand_tilde_with(path: &Path, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return path.to_path_buf();
    };
    // We need byte-level access to detect the `~` prefix. Paths that aren't
    // valid UTF-8 fall through unchanged.
    let Some(s) = path.to_str() else {
        return path.to_path_buf();
    };
    if s == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return home.join(rest);
    }
    path.to_path_buf()
}

/// Same as [`expand_tilde_with`], but for `String` fields that store a path
/// the user typed in config (e.g. theme override_path).
fn expand_tilde_str(s: &str, home: Option<&Path>) -> Option<String> {
    let path = PathBuf::from(s);
    let expanded = expand_tilde_with(&path, home);
    expanded.to_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.note.vault_path.to_string_lossy().ends_with("Notes"));
        assert_eq!(config.note.default_extension, "md");
        assert_eq!(
            config.daemon.socket_path,
            PathBuf::from("/tmp/zelkova.sock")
        );
        assert!(config.daemon.index_on_start);
        assert!(config.mcp.enabled);
        assert!(config.editor.wrap);
        assert!(config.preview.wrap);
        assert_eq!(config.ui.theme, "catppuccin");
        assert_eq!(config.ui.mode, "dark");
    }

    #[test]
    fn config_path_is_under_xdg() {
        let path = AppConfig::config_path().expect("config path is valid in test env");
        assert!(path.to_string_lossy().contains("zelkova"));
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn parse_partial_toml() {
        let toml = r#"
[note]
vault_path = "/tmp/test-vault"
"#;
        let config: AppConfig = toml::from_str(toml).expect("valid TOML in test");
        assert_eq!(config.note.vault_path, PathBuf::from("/tmp/test-vault"));
        assert!(config.daemon.index_on_start);
    }

    #[test]
    fn parse_ui_section() {
        let toml = r#"
[ui]
theme = "tokyonight"
mode = "light"
override_path = "my-theme.json"
"#;
        let config: AppConfig = toml::from_str(toml).expect("valid TOML in test");
        assert_eq!(config.ui.theme, "tokyonight");
        assert_eq!(config.ui.mode, "light");
        assert_eq!(config.ui.override_path, Some("my-theme.json".to_string()));
    }

    #[test]
    fn roundtrip_default() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).expect("default config serializes");
        let parsed: AppConfig = toml::from_str(&toml_str).expect("roundtrip TOML parses");
        assert_eq!(config.note.vault_path, parsed.note.vault_path);
        assert_eq!(config.daemon.socket_path, parsed.daemon.socket_path);
        assert_eq!(config.ui.theme, parsed.ui.theme);
    }

    // --- expand_tilde_with coverage ---

    #[test]
    fn expand_tilde_bare() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde_with(Path::new("~"), Some(home)), home);
    }

    #[test]
    fn expand_tilde_single_segment() {
        let home = Path::new("/home/test");
        assert_eq!(
            expand_tilde_with(Path::new("~/Notes"), Some(home)),
            Path::new("/home/test/Notes")
        );
    }

    #[test]
    fn expand_tilde_nested() {
        let home = Path::new("/Users/alice");
        assert_eq!(
            expand_tilde_with(Path::new("~/Documents/Notes/2024"), Some(home)),
            Path::new("/Users/alice/Documents/Notes/2024")
        );
    }

    #[test]
    fn expand_tilde_absolute_unchanged() {
        let home = Path::new("/home/test");
        assert_eq!(
            expand_tilde_with(Path::new("/tmp/zelkova.sock"), Some(home)),
            Path::new("/tmp/zelkova.sock")
        );
        assert_eq!(
            expand_tilde_with(Path::new("/home/oyatomo/Notes"), Some(home)),
            Path::new("/home/oyatomo/Notes")
        );
    }

    #[test]
    fn expand_tilde_relative_unchanged() {
        let home = Path::new("/home/test");
        // Bare names, ./foo, ../bar — none of these start with `~`, so they must
        // stay relative. Otherwise we'd reintroduce the very bug we're fixing.
        assert_eq!(
            expand_tilde_with(Path::new("Notes"), Some(home)),
            Path::new("Notes")
        );
        assert_eq!(
            expand_tilde_with(Path::new("./Notes"), Some(home)),
            Path::new("./Notes")
        );
        assert_eq!(
            expand_tilde_with(Path::new("../Notes"), Some(home)),
            Path::new("../Notes")
        );
    }

    #[test]
    fn expand_tilde_empty_unchanged() {
        let home = Path::new("/home/test");
        assert_eq!(expand_tilde_with(Path::new(""), Some(home)), Path::new(""));
    }

    #[test]
    fn expand_tilde_no_home_unchanged() {
        // When $HOME can't be determined (rare: CI sandboxes, chroot, etc.),
        // expanding `~` would silently produce a broken path. Better to leave
        // it alone and let the caller's fs operation fail loudly.
        assert_eq!(
            expand_tilde_with(Path::new("~/Notes"), None),
            Path::new("~/Notes")
        );
        assert_eq!(expand_tilde_with(Path::new("~"), None), Path::new("~"));
    }

    #[test]
    fn expand_tilde_other_user_unchanged() {
        // `~alice` is POSIX shell tilde-expansion for alice's home dir.
        // We deliberately don't support this (would require passwd lookup);
        // leave the path alone so the user sees a clear "directory not found".
        let home = Path::new("/home/test");
        assert_eq!(
            expand_tilde_with(Path::new("~alice/Notes"), Some(home)),
            Path::new("~alice/Notes")
        );
    }

    // --- AppConfig::expand_paths integration ---

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn expand_paths_expands_vault_and_socket_and_override() {
        let mut config = AppConfig::default();
        config.note.vault_path = PathBuf::from("~/Notes");
        config.daemon.socket_path = PathBuf::from("~/run/zelkova.sock");
        config.ui.override_path = Some("~/theme.json".to_string());

        // We can't easily mock dirs::home_dir in a unit test, so the assertion
        // is conditional: if $HOME is set, the expansion must land there; if
        // not, the values are unchanged. Both outcomes are correct.
        if let Some(home) = dirs::home_dir() {
            config.expand_paths();
            assert_eq!(config.note.vault_path, home.join("Notes"));
            assert_eq!(config.daemon.socket_path, home.join("run/zelkova.sock"));
            assert_eq!(
                config.ui.override_path,
                Some(home.join("theme.json").to_string_lossy().to_string())
            );
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn expand_paths_leaves_absolute_unchanged() {
        let mut config = AppConfig::default();
        config.note.vault_path = PathBuf::from("/var/lib/zelkova");
        config.daemon.socket_path = PathBuf::from("/tmp/zelkova.sock");
        config.ui.override_path = Some("/etc/zelkova/theme.json".to_string());

        config.expand_paths();

        assert_eq!(config.note.vault_path, PathBuf::from("/var/lib/zelkova"));
        assert_eq!(
            config.daemon.socket_path,
            PathBuf::from("/tmp/zelkova.sock")
        );
        assert_eq!(
            config.ui.override_path,
            Some("/etc/zelkova/theme.json".to_string())
        );
    }
}
