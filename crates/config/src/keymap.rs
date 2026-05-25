use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapConfig {
    #[serde(default = "default_leader")]
    pub leader: String,
    #[serde(default)]
    pub bindings: Vec<BindingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingConfig {
    pub key: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

fn default_leader() -> String {
    "space".to_string()
}

impl Default for KeymapConfig {
    fn default() -> Self {
        Self {
            leader: default_leader(),
            bindings: default_bindings(),
        }
    }
}

fn default_bindings() -> Vec<BindingConfig> {
    vec![
        BindingConfig {
            key: "ctrl-p".into(),
            action: "open_command_palette".into(),
            context: None,
        },
        BindingConfig {
            key: "ctrl-shift-f".into(),
            action: "search_notes".into(),
            context: None,
        },
        BindingConfig {
            key: "ctrl-n".into(),
            action: "create_note".into(),
            context: None,
        },
        BindingConfig {
            key: "ctrl-s".into(),
            action: "save_note".into(),
            context: None,
        },
        BindingConfig {
            key: "ctrl-b".into(),
            action: "toggle_sidebar".into(),
            context: None,
        },
        BindingConfig {
            key: "ctrl-q".into(),
            action: "quit".into(),
            context: None,
        },
    ]
}

impl KeymapConfig {
    pub fn load() -> Result<Self> {
        let keymap_path = Self::keymap_path()?;
        if !keymap_path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&keymap_path)
            .with_context(|| format!("failed to read keymap from {}", keymap_path.display()))?;
        let keymap: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse keymap at {}", keymap_path.display()))?;
        Ok(keymap)
    }

    pub fn keymap_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("cannot determine XDG config directory")?;
        Ok(config_dir.join("zelkova").join("keymap.toml"))
    }

    /// Replace "leader" in key strings with the actual leader key
    pub fn resolved_bindings(&self) -> Vec<BindingConfig> {
        self.bindings
            .iter()
            .map(|b| BindingConfig {
                key: b.key.replace("leader", &self.leader),
                action: b.action.clone(),
                context: b.context.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keymap_is_valid() {
        let km = KeymapConfig::default();
        assert_eq!(km.leader, "space");
        assert!(!km.bindings.is_empty());
        // No space-prefixed bindings in defaults
        assert!(!km.bindings.iter().any(|b| b.key.contains("leader")));
    }

    #[test]
    fn parse_custom_keymap() {
        let toml = r#"
leader = "ctrl-x"
[[bindings]]
key = "ctrl-p"
action = "open_command_palette"
"#;
        let km: KeymapConfig = toml::from_str(toml).unwrap();
        assert_eq!(km.leader, "ctrl-x");
        assert_eq!(km.bindings.len(), 1);
    }

    #[test]
    fn resolve_leader_key() {
        let mut km = KeymapConfig::default();
        km.leader = "ctrl-x".to_string();
        km.bindings = vec![BindingConfig {
            key: "leader f".into(),
            action: "search".into(),
            context: None,
        }];
        let resolved = km.resolved_bindings();
        assert_eq!(resolved[0].key, "ctrl-x f");
    }

    #[test]
    fn empty_bindings_default() {
        let toml = r#"leader = "space""#;
        let km: KeymapConfig = toml::from_str(toml).unwrap();
        assert!(km.bindings.is_empty());
    }

    #[test]
    fn roundtrip_default() {
        let km = KeymapConfig::default();
        let toml_str = toml::to_string_pretty(&km).unwrap();
        let parsed: KeymapConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(km.leader, parsed.leader);
        assert_eq!(km.bindings.len(), parsed.bindings.len());
    }
}
