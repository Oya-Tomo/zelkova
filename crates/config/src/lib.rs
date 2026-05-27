use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod keymap;
pub mod theme;
pub use keymap::{BindingConfig, KeymapConfig};
pub use theme::{EditorColors, ThemeConfig, UiColors};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct AppConfig {
    #[serde(default)]
    pub note: NoteConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
    #[serde(default)]
    pub mcp: McpConfig,
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

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if !config_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("failed to read config from {}", config_path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse config at {}", config_path.display()))?;
        Ok(config)
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("cannot determine XDG config directory")?;
        Ok(config_dir.join("zelkova").join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn roundtrip_default() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).expect("default config serializes");
        let parsed: AppConfig = toml::from_str(&toml_str).expect("roundtrip TOML parses");
        assert_eq!(config.note.vault_path, parsed.note.vault_path);
        assert_eq!(config.daemon.socket_path, parsed.daemon.socket_path);
    }
}
