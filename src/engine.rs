use std::{net::TcpStream, path::PathBuf, process::Command, sync::Arc, time::Duration};

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::{
    bundle::{self, BundleAudit},
    config::{CaptureRequestState, ConfigStore, ExtensionState, LocalConfig},
    embedding::{Embedder, FastEmbedder},
    index::Index,
    model::{
        CaptureInput, DoctorCheck, DoctorReport, GetResult, Record, SearchHit, SearchScope,
        SetupReport, Status,
    },
    public_fetch, sensor_assets,
    vault::Vault,
    youtube::enrich_youtube,
};

pub struct Engine {
    home: PathBuf,
    vault: Vault,
    index: Index,
    embedder: Arc<dyn Embedder>,
}

impl Engine {
    pub fn new(home: PathBuf) -> Self {
        let embedder = Arc::new(FastEmbedder::new(home.join("models")));
        Self::with_embedder(home, embedder)
    }

    pub fn with_embedder(home: PathBuf, embedder: Arc<dyn Embedder>) -> Self {
        let vault_path = home.join("vault");
        let index_path = home.join("index.db");
        Self {
            home,
            vault: Vault::new(vault_path),
            index: Index::new(index_path),
            embedder,
        }
    }

    pub fn setup(&self) -> Result<Status> {
        std::fs::create_dir_all(&self.home)?;
        self.vault.init()?;
        self.index.init()?;
        ConfigStore::new(&self.home).load_or_create()?;
        self.embedder.embed_query("Starlee setup health check")?;
        self.status()
    }

    pub fn onboarding(&self) -> Result<SetupReport> {
        let status = self.setup()?;
        let config = self.local_config()?;
        let extension_path = sensor_assets::install(&self.home, &config)?;
        Ok(SetupReport {
            status,
            bookmarklet: crate::config::bookmarklet(&config),
            extension_path: extension_path.display().to_string(),
            extension_token: config.capture_token,
            example_queries: vec![
                "What do I know about this topic?".into(),
                "What have I captured recently?".into(),
                "Do my sources agree with this claim?".into(),
            ],
        })
    }

    pub fn capture(&self, mut input: CaptureInput) -> Result<Record> {
        self.setup()?;
        let config = self.local_config()?;
        if matches!(input.source_type, crate::model::SourceType::Youtube)
            && let Some(key) = config.youtube_api_key.as_deref()
        {
            let _ = enrich_youtube(&mut input, key);
        }
        let existing = input
            .url
            .as_deref()
            .map(|url| self.index.get_by_url(url))
            .transpose()?
            .flatten()
            .map(|path| self.vault.read(&path))
            .transpose()?;
        let record = if let Some(existing) = existing {
            self.vault.replace(&existing, input)?
        } else {
            self.vault.write(input)?
        };
        self.index.upsert(&record, self.embedder.as_ref())?;
        Ok(record)
    }

    pub fn capture_public_url(&self, url: &str) -> Result<Record> {
        self.capture(public_fetch::fetch_explicitly_public(url)?)
    }

    pub fn search_scoped(
        &self,
        query: &str,
        limit: usize,
        scope: SearchScope,
    ) -> Result<Vec<SearchHit>> {
        let mut hits = Vec::new();
        if matches!(scope, SearchScope::Own | SearchScope::Both) {
            hits.extend(self.index.search(query, limit, self.embedder.as_ref())?);
        }
        if matches!(scope, SearchScope::Borrowed | SearchScope::Both) {
            let config = self.local_config()?;
            let paths = config
                .borrowed_bundles
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                let query_embedding = self.embedder.embed_query(query)?;
                hits.extend(bundle::search(&paths, query, &query_embedding, limit)?);
            }
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        hits.truncate(limit);
        Ok(hits)
    }
    pub fn recent(&self, limit: usize) -> Result<Vec<SearchHit>> {
        self.index.recent(limit)
    }

    pub fn get(&self, id: &str) -> Result<Option<Record>> {
        self.index
            .get(id)?
            .map(|path| self.vault.read(&path))
            .transpose()
    }

    pub fn get_any(&self, id: &str) -> Result<Option<GetResult>> {
        if let Some(record) = self.get(id)? {
            return Ok(Some(GetResult::Own { record }));
        }
        let config = self.local_config()?;
        let paths = config
            .borrowed_bundles
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        Ok(bundle::get(&paths, id)?.map(|record| GetResult::Borrowed { record }))
    }

    pub fn reindex(&self) -> Result<Status> {
        let records = self.vault.records()?;
        self.index.rebuild(&records, self.embedder.as_ref())?;
        self.status()
    }

    pub fn status(&self) -> Result<Status> {
        let (capture_count, chunk_count) = self.index.counts()?;
        let store = ConfigStore::new(&self.home);
        let config = store.load_or_create()?;
        Ok(Status {
            home: self.home.display().to_string(),
            vault: self.home.join("vault").display().to_string(),
            index: self.home.join("index.db").display().to_string(),
            capture_count,
            chunk_count,
            retrieval: format!("hybrid FTS5 + sqlite-vec ({})", self.embedder.name()),
            capture_endpoint: format!("http://127.0.0.1:{}", config.capture_port),
            capture_token_path: store.path().display().to_string(),
            youtube_metadata_configured: config.youtube_api_key.is_some(),
            borrowed_bundle_count: config.borrowed_bundles.len(),
        })
    }

    pub fn local_config(&self) -> Result<LocalConfig> {
        ConfigStore::new(&self.home).load_or_create()
    }

    pub fn doctor(&self) -> Result<DoctorReport> {
        let status = self.status()?;
        let config = self.local_config()?;
        let extension_path = self.home.join("sensor-extension");
        let user_home = home_dir();
        let launch_agent_path = user_home.join("Library/LaunchAgents/com.starlee.capture.plist");
        let app_path = user_home.join("Applications/Starlee.app");
        let plugin_path = user_home.join("plugins/starlee");
        let marketplace_path = user_home.join(".agents/plugins/marketplace.json");
        let mut checks = vec![
            DoctorCheck {
                name: "vault".into(),
                ok: self.home.join("vault").is_dir(),
                detail: self.home.join("vault").display().to_string(),
            },
            DoctorCheck {
                name: "index".into(),
                ok: self.home.join("index.db").exists(),
                detail: self.home.join("index.db").display().to_string(),
            },
            DoctorCheck {
                name: "extension_assets".into(),
                ok: extension_path.join("manifest.json").exists(),
                detail: extension_path.display().to_string(),
            },
            DoctorCheck {
                name: "token".into(),
                ok: config.capture_token.len() == 64,
                detail: format!("sha256:{}", token_fingerprint(&config.capture_token)),
            },
            DoctorCheck {
                name: "launch_agent".into(),
                ok: launch_agent_path.exists(),
                detail: launch_agent_path.display().to_string(),
            },
            DoctorCheck {
                name: "capture_service".into(),
                ok: capture_service_reachable(config.capture_port),
                detail: format!("127.0.0.1:{}", config.capture_port),
            },
            DoctorCheck {
                name: "mac_app_installed".into(),
                ok: app_path.join("Contents/MacOS/StarleeMenuBar").exists(),
                detail: app_path.display().to_string(),
            },
            DoctorCheck {
                name: "mac_app_running".into(),
                ok: process_running("Starlee.app/Contents/MacOS/StarleeMenuBar"),
                detail: "StarleeMenuBar process".into(),
            },
            DoctorCheck {
                name: "codex_plugin_source".into(),
                ok: plugin_path.exists() && marketplace_path.exists(),
                detail: format!(
                    "{} via {}",
                    plugin_path.display(),
                    marketplace_path.display()
                ),
            },
        ];
        let extension_seen = config.extension.last_handshake_at.is_some();
        checks.push(DoctorCheck {
            name: "extension_handshake".into(),
            ok: extension_seen,
            detail: config
                .extension
                .last_handshake_at
                .clone()
                .unwrap_or_else(|| "no extension handshake recorded".into()),
        });

        let next_actions = checks
            .iter()
            .filter(|check| !check.ok)
            .map(|check| match check.name.as_str() {
                "extension_assets" => "Run `starlee setup` to generate browser extension assets.",
                "launch_agent" => "Run `scripts/install-service.sh` from the Starlee repository.",
                "capture_service" => {
                    "Run `starlee serve` or reinstall Starlee to restart the capture service."
                }
                "mac_app_installed" => "Run `./scripts/install.sh` to install Starlee.app.",
                "mac_app_running" => "Open `~/Applications/Starlee.app`.",
                "codex_plugin_source" => {
                    "Run `./scripts/install.sh` to install the Codex plugin source."
                }
                "extension_handshake" => {
                    "Load or reload ~/Starlee/sensor-extension in your browser."
                }
                _ => "Run `starlee setup` and then `starlee doctor` again.",
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        Ok(DoctorReport {
            ok: checks.iter().all(|check| check.ok),
            status,
            checks,
            next_actions,
        })
    }

    pub fn record_extension_hello(
        &self,
        browser: Option<String>,
        extension_version: Option<String>,
        can_capture_active_tab: bool,
    ) -> Result<ExtensionState> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        config.extension = ExtensionState {
            browser,
            extension_version,
            can_capture_active_tab,
            last_handshake_at: Some(Utc::now().to_rfc3339()),
        };
        store.save(&config)?;
        Ok(config.extension)
    }

    pub fn create_capture_request(&self, source: impl Into<String>) -> Result<CaptureRequestState> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let source = source.into();
        let id_material = format!(
            "{}:{}:{}",
            config.capture_token,
            Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            source
        );
        let id = token_fingerprint(&id_material);
        let request = CaptureRequestState {
            id,
            requested_at: Utc::now().to_rfc3339(),
            source,
        };
        config.pending_capture_request = Some(request.clone());
        store.save(&config)?;
        Ok(request)
    }

    pub fn take_capture_request(&self) -> Result<Option<CaptureRequestState>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let request = config.pending_capture_request.take();
        store.save(&config)?;
        Ok(request)
    }

    pub fn configure_youtube_api_key(&self, api_key: String) -> Result<()> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        config.youtube_api_key = Some(api_key);
        store.save(&config)
    }

    pub fn export_bundle(
        &self,
        path: &std::path::Path,
        include_public_bodies: bool,
    ) -> Result<BundleAudit> {
        self.index.export_bundle(path, include_public_bodies)
    }

    pub fn ingest_bundle(&self, path: &std::path::Path) -> Result<BundleAudit> {
        let audit = bundle::validate(path)?;
        let canonical = std::fs::canonicalize(path)?;
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let value = canonical.display().to_string();
        if !config.borrowed_bundles.contains(&value) {
            config.borrowed_bundles.push(value);
            store.save(&config)?;
        }
        Ok(audit)
    }
}

fn token_fingerprint(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher.finalize()[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn capture_service_reachable(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(250),
    )
    .is_ok()
}

fn process_running(pattern: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
