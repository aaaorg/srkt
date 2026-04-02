use anyhow::Result;
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

    pub fn load(_path: &Path) -> Result<Self> { todo!() }
    pub fn save(&self, _path: &Path) -> Result<()> { todo!() }
    pub fn add(&mut self, _trigger: &str, _expansion: &str) -> Result<()> { todo!() }
    pub fn remove(&mut self, _trigger: &str) -> bool { todo!() }
}
