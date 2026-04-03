use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub expansions: HashMap<String, String>,
}

impl Config {
    pub fn path() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                PathBuf::from(home).join(".config")
            });
        base.join("srkt").join("expansions.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(content) => Ok(toml::from_str(&content)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let content = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, &content)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn add(&mut self, trigger: &str, expansion: &str) -> Result<()> {
        if trigger.is_empty() {
            anyhow::bail!("trigger must not be empty");
        }
        // Overwriting the same trigger is always allowed — skip conflict check for it.
        for existing in self.expansions.keys() {
            if existing == trigger {
                // Same key: overwrite, no conflict.
                continue;
            }
            if existing.starts_with(trigger) || trigger.starts_with(existing.as_str()) {
                bail!(
                    "prefix conflict: '{}' conflicts with existing trigger '{}'",
                    trigger,
                    existing
                );
            }
        }
        self.expansions
            .insert(trigger.to_string(), expansion.to_string());
        Ok(())
    }

    pub fn remove(&mut self, trigger: &str) -> bool {
        self.expansions.remove(trigger).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_path(dir: &TempDir, filename: &str) -> PathBuf {
        dir.path().join(filename)
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir, "nonexistent.toml");
        let config = Config::load(&path).unwrap();
        assert!(config.expansions.is_empty());
    }

    #[test]
    fn test_save_and_reload() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir, "expansions.toml");

        let mut config = Config::default();
        config.add("/mail", "user@example.com").unwrap();
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.expansions.get("/mail").map(String::as_str),
            Some("user@example.com")
        );
    }

    #[test]
    fn test_multiline_expansion_survives_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = temp_path(&dir, "expansions.toml");

        let multiline = "Best regards,\nJakub";
        let mut config = Config::default();
        config.add("/sig", multiline).unwrap();
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.expansions.get("/sig").map(String::as_str),
            Some(multiline)
        );
    }

    #[test]
    fn test_remove_existing_trigger() {
        let mut config = Config::default();
        config.add("/mail", "user@example.com").unwrap();
        assert!(config.remove("/mail"));
        assert!(!config.expansions.contains_key("/mail"));
    }

    #[test]
    fn test_remove_missing_trigger_returns_false() {
        let mut config = Config::default();
        assert!(!config.remove("/nonexistent"));
    }

    #[test]
    fn test_prefix_conflict_rejected() {
        let mut config = Config::default();
        config.add("/mail", "user@example.com").unwrap();
        // "/m" is a proper prefix of "/mail" — should be rejected.
        let result = config.add("/m", "something");
        assert!(result.is_err(), "expected prefix conflict error");
    }

    #[test]
    fn test_no_conflict_when_overwriting_same_trigger() {
        let mut config = Config::default();
        config.add("/mail", "user@example.com").unwrap();
        // Overwriting the same trigger should succeed and update the value.
        let result = config.add("/mail", "new@example.com");
        assert!(result.is_ok(), "overwriting same trigger should be Ok");
        assert_eq!(
            config.expansions.get("/mail").map(String::as_str),
            Some("new@example.com")
        );
    }

    #[test]
    fn test_empty_trigger_rejected() {
        let mut cfg = Config::default();
        assert!(cfg.add("", "value").is_err());
    }
}
