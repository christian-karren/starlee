use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DEFAULT_CAPTURE_PORT: u16 = 47291;
pub const DEFAULT_SPOTIFY_REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub version: u32,
    pub capture_port: u16,
    pub capture_token: String,
    #[serde(default = "default_query_relevance_floor")]
    pub query_relevance_floor: f32,
    #[serde(default)]
    pub extension: ExtensionState,
    #[serde(default)]
    pub pending_capture_request: Option<CaptureRequestState>,
    #[serde(default)]
    pub capture_request_status: Option<CaptureRequestStatus>,
    #[serde(default)]
    pub capture_diagnostics: Vec<CaptureDiagnosticEvent>,
    #[serde(default)]
    pub youtube_api_key: Option<String>,
    #[serde(default)]
    pub spotify_client_id: Option<String>,
    #[serde(default)]
    pub spotify_redirect_uri: Option<String>,
    #[serde(default)]
    pub spotify_oauth: Option<SpotifyOAuthConfig>,
    #[serde(default)]
    pub spotify_sync: SpotifySyncConfig,
    #[serde(default)]
    pub borrowed_bundles: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionState {
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub extension_version: Option<String>,
    #[serde(default)]
    pub extension_build: Option<String>,
    #[serde(default)]
    pub can_capture_active_tab: bool,
    #[serde(default)]
    pub last_handshake_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequestState {
    pub id: String,
    pub requested_at: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target_browser: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureRequestStatus {
    pub id: String,
    pub requested_at: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub picked_up_at: Option<String>,
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub requested_browser: Option<String>,
    #[serde(default)]
    pub handling_browser: Option<String>,
    #[serde(default)]
    pub page: Option<CaptureRequestPageMetadata>,
    pub status: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptureRequestPageMetadata {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureDiagnosticEvent {
    pub timestamp: String,
    pub component: String,
    pub event: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub browser: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub page: Option<CaptureRequestPageMetadata>,
    #[serde(default)]
    pub safe_metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyOAuthConfig {
    pub client_id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpotifySyncConfig {
    #[serde(default)]
    pub last_synced_at: Option<String>,
    #[serde(default)]
    pub next_sync_at: Option<String>,
    #[serde(default)]
    pub last_result: Option<SpotifySyncLastResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncLastResult {
    pub checked_at: String,
    pub added: usize,
    pub skipped: usize,
    pub status: String,
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
            query_relevance_floor: default_query_relevance_floor(),
            extension: ExtensionState::default(),
            pending_capture_request: None,
            capture_request_status: None,
            capture_diagnostics: Vec::new(),
            youtube_api_key: None,
            spotify_client_id: None,
            spotify_redirect_uri: Some(DEFAULT_SPOTIFY_REDIRECT_URI.into()),
            spotify_oauth: None,
            spotify_sync: SpotifySyncConfig::default(),
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
        let mut config: LocalConfig =
            serde_json::from_slice(&bytes).context("parse local Starlee config")?;
        if config.version == 0 {
            config.version = 1;
        }
        if config.query_relevance_floor <= 0.0 {
            config.query_relevance_floor = default_query_relevance_floor();
        }
        if config
            .spotify_redirect_uri
            .as_deref()
            .map(str::trim)
            .is_none_or(str::is_empty)
        {
            config.spotify_redirect_uri = Some(DEFAULT_SPOTIFY_REDIRECT_URI.into());
        }
        Ok(config)
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

fn default_query_relevance_floor() -> f32 {
    0.35
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
            query_relevance_floor: default_query_relevance_floor(),
            extension: ExtensionState::default(),
            pending_capture_request: None,
            capture_request_status: None,
            capture_diagnostics: Vec::new(),
            youtube_api_key: None,
            spotify_client_id: None,
            spotify_redirect_uri: Some(DEFAULT_SPOTIFY_REDIRECT_URI.into()),
            spotify_oauth: None,
            spotify_sync: SpotifySyncConfig::default(),
            borrowed_bundles: Vec::new(),
        };
        let value = bookmarklet(&config);
        assert!(value.starts_with("javascript:"));
        assert!(value.contains("abc123"));
        assert!(value.contains("49999"));
    }

    #[test]
    fn missing_query_floor_migrates_to_default() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let config_path = temp.path().join("config.json");
        fs::write(
            &config_path,
            r#"{
              "version": 1,
              "capture_port": 47291,
              "capture_token": "abc123",
              "extension": {},
              "pending_capture_request": null,
              "capture_request_status": null,
              "youtube_api_key": null,
              "borrowed_bundles": []
            }"#,
        )?;
        let config = ConfigStore::new(temp.path()).load()?;
        assert_eq!(config.query_relevance_floor, 0.35);
        assert_eq!(
            config.spotify_redirect_uri.as_deref(),
            Some(DEFAULT_SPOTIFY_REDIRECT_URI)
        );
        Ok(())
    }
}
