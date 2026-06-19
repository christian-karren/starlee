use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_CAPTURE_PORT: u16 = 47291;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub version: u32,
    pub capture_port: u16,
    pub capture_token: String,
    #[serde(default)]
    pub youtube_api_key: Option<String>,
    #[serde(default)]
    pub borrowed_bundles: Vec<String>,
}

pub struct ConfigStore {
    path: PathBuf,
}

pub fn bookmarklet(config: &LocalConfig) -> String {
    let script = include_str!("../assets/bookmarklet.js")
        .replace(
            "__TOKEN__",
            &serde_json::to_string(&config.capture_token).expect("token serializes"),
        )
        .replace("__PORT__", &config.capture_port.to_string());
    format!("javascript:{}", script.trim())
}

impl ConfigStore {
    pub fn new(home: &Path) -> Self {
        Self {
            path: home.join("config.json"),
        }
    }

    pub fn load_or_create(&self) -> Result<LocalConfig> {
        if self.path.exists() {
            return self.load();
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let config = LocalConfig {
            version: 1,
            capture_port: DEFAULT_CAPTURE_PORT,
            capture_token: generate_token()?,
            youtube_api_key: None,
            borrowed_bundles: Vec::new(),
        };
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&config)?)?;
        restrict_permissions(&temporary)?;
        fs::rename(&temporary, &self.path)?;
        restrict_permissions(&self.path)?;
        Ok(config)
    }

    pub fn load(&self) -> Result<LocalConfig> {
        let bytes = fs::read(&self.path)
            .with_context(|| format!("read local config {}", self.path.display()))?;
        serde_json::from_slice(&bytes).context("parse local Starlee config")
    }

    pub fn save(&self, config: &LocalConfig) -> Result<()> {
        let temporary = self.path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(config)?)?;
        restrict_permissions(&temporary)?;
        fs::rename(&temporary, &self.path)?;
        restrict_permissions(&self.path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn generate_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("generate capture token")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reuses_a_256_bit_token() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let store = ConfigStore::new(temp.path());
        let first = store.load_or_create()?;
        let second = store.load_or_create()?;
        assert_eq!(first.capture_token, second.capture_token);
        assert_eq!(first.capture_token.len(), 64);
        Ok(())
    }

    #[test]
    fn bookmarklet_embeds_local_configuration() {
        let config = LocalConfig {
            version: 1,
            capture_port: 49999,
            capture_token: "abc123".into(),
            youtube_api_key: None,
            borrowed_bundles: Vec::new(),
        };
        let value = bookmarklet(&config);
        assert!(value.starts_with("javascript:"));
        assert!(value.contains("abc123"));
        assert!(value.contains("49999"));
    }
}
