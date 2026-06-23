use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::TcpStream,
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::{
    bundle::{self, BundleAudit},
    config::{CaptureRequestState, CaptureRequestStatus, ConfigStore, ExtensionState, LocalConfig},
    embedding::{Embedder, FastEmbedder},
    index::Index,
    model::{
        CaptureInput, CorpusOverview, DoctorCheck, DoctorReport, GetResult, QueryResult, Record,
        SearchHit, SearchScope, SetupReport, SourceType, SpotifySyncEvent, SpotifySyncLog, Status,
    },
    public_fetch, sensor_assets, spotify,
    spotify::{SpotifyConfigureReport, SpotifySyncReport, SpotifySyncStatus},
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
            bookmarklet: "redacted: run `starlee bookmarklet` locally to generate the token-bearing bookmarklet".into(),
            extension_path: extension_path.display().to_string(),
            extension_token: "redacted".into(),
            extension_token_fingerprint: token_fingerprint(&config.capture_token),
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

    pub fn query(
        &self,
        question: &str,
        context: Option<&str>,
        max_chunks: usize,
    ) -> Result<QueryResult> {
        let started = Instant::now();
        let config = self.local_config()?;
        let limit = max_chunks.clamp(1, 20);
        let retrieval_query = match context.filter(|value| !value.trim().is_empty()) {
            Some(context) => format!("Question: {question}\nContext: {context}"),
            None => question.to_owned(),
        };
        let retrieved = self
            .index
            .query_chunks(&retrieval_query, limit, self.embedder.as_ref())?;
        let total_retrieved = retrieved.len();
        let mut chunks = retrieved
            .into_iter()
            .filter(|chunk| chunk.similarity >= config.query_relevance_floor)
            .collect::<Vec<_>>();
        let relevance_floor_hit = chunks.len() < 2;
        if relevance_floor_hit {
            chunks.truncate(1);
        }
        for (index, chunk) in chunks.iter_mut().enumerate() {
            chunk.index = index + 1;
        }
        Ok(QueryResult {
            chunks,
            total_retrieved,
            relevance_floor_hit,
            query_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub fn corpus_overview(&self) -> Result<CorpusOverview> {
        let started = Instant::now();
        let records = self.vault.records()?;
        let total_captures = records.len();
        let earliest_capture = records
            .iter()
            .map(|record| record.metadata.captured_at.date_naive().to_string())
            .min();
        let latest_capture = records
            .iter()
            .map(|record| record.metadata.captured_at.date_naive().to_string())
            .max();
        let mut source_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut domain_counts: HashMap<String, usize> = HashMap::new();
        let mut author_counts: HashMap<String, usize> = HashMap::new();
        let mut term_counts: HashMap<String, usize> = HashMap::new();
        for record in &records {
            let source_type = match record.metadata.source_type {
                SourceType::Article => "article",
                SourceType::Youtube => "youtube",
                SourceType::SpotifyEpisode => "spotify_episode",
                SourceType::Note => "note",
            };
            let source_type = source_type.to_owned();
            *source_counts.entry(source_type).or_insert(0) += 1;
            if let Some(domain) = record
                .metadata
                .url
                .as_deref()
                .and_then(domain_from_url)
                .or_else(|| record.metadata.site.clone())
            {
                *domain_counts.entry(domain).or_insert(0) += 1;
            }
            if let Some(author) = record
                .metadata
                .author
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            {
                *author_counts.entry(author.trim().to_owned()).or_insert(0) += 1;
            }
            add_terms(&mut term_counts, &record.metadata.title);
            add_terms(&mut term_counts, &record.metadata.summary);
            add_terms(&mut term_counts, &record.body);
        }
        let source_breakdown = source_counts
            .into_iter()
            .map(|(source, count)| {
                let ratio = if total_captures == 0 {
                    0.0
                } else {
                    count as f64 / total_captures as f64
                };
                (source, ratio)
            })
            .collect();
        Ok(CorpusOverview {
            total_captures,
            earliest_capture,
            latest_capture,
            top_topics: top_keys(term_counts, 10),
            source_breakdown,
            top_domains: top_keys(domain_counts, 5),
            top_authors: top_keys(author_counts, 5),
            overview_ms: started.elapsed().as_millis() as u64,
        })
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
            return Ok(Some(GetResult::Own {
                record: Box::new(record),
            }));
        }
        let config = self.local_config()?;
        let paths = config
            .borrowed_bundles
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        Ok(bundle::get(&paths, id)?.map(|record| GetResult::Borrowed { record }))
    }

    pub fn reindex(&self, stale_embeddings_only: bool) -> Result<Status> {
        let records = self.vault.records()?;
        if stale_embeddings_only {
            self.index.reembed_stale(&records, self.embedder.as_ref())?;
        } else {
            self.index.rebuild(&records, self.embedder.as_ref())?;
        }
        self.status()
    }

    pub fn migrate(&self) -> Result<crate::index::MigrationReport> {
        std::fs::create_dir_all(&self.home)?;
        self.index.migrate()
    }

    pub fn status(&self) -> Result<Status> {
        let (capture_count, chunk_count) = self.index.counts()?;
        let schema_version = self.index.schema_version()?;
        let chunks_stale = self.index.stale_chunk_count(self.embedder.name())?;
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
            spotify_oauth_configured: spotify::oauth_is_valid(&config),
            spotify_account: config
                .spotify_oauth
                .as_ref()
                .and_then(|oauth| oauth.display_name.clone().or_else(|| oauth.user_id.clone())),
            spotify_last_synced_at: config.spotify_sync.last_synced_at.clone(),
            spotify_next_sync_at: config
                .spotify_sync
                .next_sync_at
                .clone()
                .or_else(|| Some(spotify::next_sync_at(chrono::Local::now()).to_rfc3339())),
            schema_version,
            embedding_model_current: self.embedder.name().into(),
            chunks_stale,
        })
    }

    pub fn local_config(&self) -> Result<LocalConfig> {
        ConfigStore::new(&self.home).load_or_create()
    }

    pub fn doctor(&self) -> Result<DoctorReport> {
        let status = self.status()?;
        let config = self.local_config()?;
        let coverage_gap = self.spotify_coverage_gap()?;
        if let Some(gap) = coverage_gap.as_deref() {
            self.record_spotify_sync_event(spotify_event(
                "detected",
                "failed",
                "serve_not_running",
                gap,
            ))?;
        }
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
        if cfg!(target_os = "macos") {
            let safari_app_path = user_home.join("Applications/Starlee Safari.app");
            let safari_extension_path =
                safari_app_path.join("Contents/PlugIns/Starlee Safari Extension.appex");
            checks.push(DoctorCheck {
                name: "safari_extension_installed".into(),
                ok: safari_extension_path.exists(),
                detail: safari_app_path.display().to_string(),
            });
        }
        let extension_seen = config.extension.last_handshake_at.is_some();
        checks.push(DoctorCheck {
            name: "schema_version".into(),
            ok: status.schema_version >= crate::index::CURRENT_SCHEMA_VERSION,
            detail: status.schema_version.to_string(),
        });
        checks.push(DoctorCheck {
            name: "embedding_model_current".into(),
            ok: true,
            detail: status.embedding_model_current.clone(),
        });
        checks.push(DoctorCheck {
            name: "chunks_stale".into(),
            ok: status.chunks_stale == 0,
            detail: status.chunks_stale.to_string(),
        });
        checks.push(DoctorCheck {
            name: "spotify_oauth".into(),
            ok: spotify::oauth_is_valid(&config),
            detail: config
                .spotify_oauth
                .as_ref()
                .and_then(|oauth| oauth.display_name.clone().or_else(|| oauth.user_id.clone()))
                .unwrap_or_else(|| "not configured or expired".into()),
        });
        checks.push(DoctorCheck {
            name: "spotify_episode_history_api".into(),
            ok: true,
            detail: spotify::SPOTIFY_SYNC_DETAIL.into(),
        });
        let spotify_poller_running = capture_service_reachable(config.capture_port);
        checks.push(DoctorCheck {
            name: "spotify_poller_running".into(),
            ok: spotify_poller_running,
            detail: if spotify_poller_running {
                "capture service is reachable; in-process Spotify poller can run when wired into serve".into()
            } else {
                "capture service is not reachable; Spotify playback during this window is invisible to Starlee".into()
            },
        });
        checks.push(DoctorCheck {
            name: "spotify_last_successful_poll".into(),
            ok: self.index.spotify_last_successful_poll_at()?.is_some(),
            detail: self
                .index
                .spotify_last_successful_poll_at()?
                .unwrap_or_else(|| "no successful Spotify poll recorded".into()),
        });
        checks.push(DoctorCheck {
            name: "spotify_last_capture".into(),
            ok: true,
            detail: self
                .index
                .spotify_last_capture_at()?
                .unwrap_or_else(|| "no Spotify capture recorded".into()),
        });
        let recent_counts = self
            .index
            .spotify_recent_reason_counts(Utc::now() - chrono::TimeDelta::days(7))?;
        let has_recent_hard_failure = recent_counts.iter().any(|count| {
            matches!(
                count.reason_code.as_str(),
                "spotify_not_connected" | "serve_not_running" | "network_error" | "capture_error"
            )
        });
        checks.push(DoctorCheck {
            name: "spotify_recent_skips_failures".into(),
            ok: !has_recent_hard_failure,
            detail: if recent_counts.is_empty() {
                "no Spotify skips or failures recorded in the last 7 days".into()
            } else {
                recent_counts
                    .iter()
                    .map(|count| format!("{}={}", count.reason_code, count.count))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        });
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
                "safari_extension_installed" => {
                    "Run `./scripts/install-safari-extension.sh`, then enable Starlee in Safari Settings > Extensions."
                }
                "codex_plugin_source" => {
                    "Run `./scripts/install.sh` to install the Codex plugin source."
                }
                "spotify_oauth" => "Run `starlee configure-spotify` to connect Spotify.",
                "spotify_episode_history_api" => {
                    "Choose a Spotify capture strategy that does not depend on recently-played podcast episodes."
                }
                "spotify_poller_running" => {
                    "Run `starlee serve` or reinstall Starlee to restart the capture service and Spotify poller."
                }
                "spotify_last_successful_poll" => {
                    "Run `starlee sync-spotify` or start `starlee serve` to record Spotify poller liveness."
                }
                "spotify_recent_skips_failures" => {
                    "Run `starlee sync-log --show-skips` to inspect recent Spotify skip and failure reasons."
                }
                "schema_version" => "Run `starlee migrate` to apply pending database migrations.",
                "chunks_stale" => "Run `starlee reindex --stale-embeddings-only` to refresh stale embeddings.",
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
        config.capture_request_status = Some(CaptureRequestStatus {
            id: request.id.clone(),
            requested_at: request.requested_at.clone(),
            source: request.source.clone(),
            status: "queued".into(),
            completed_at: None,
            message: None,
        });
        config.pending_capture_request = Some(request.clone());
        store.save(&config)?;
        Ok(request)
    }

    pub fn take_capture_request(&self) -> Result<Option<CaptureRequestState>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let request = config.pending_capture_request.take();
        if let Some(request) = request.as_ref()
            && let Some(status) = config.capture_request_status.as_mut()
            && status.id == request.id
        {
            status.status = "picked_up".into();
            status.message = Some("Browser extension picked up the capture request.".into());
        }
        store.save(&config)?;
        Ok(request)
    }

    pub fn capture_request_status(&self, id: &str) -> Result<Option<CaptureRequestStatus>> {
        let config = self.local_config()?;
        Ok(config
            .capture_request_status
            .filter(|status| status.id == id))
    }

    pub fn record_capture_request_result(
        &self,
        id: &str,
        status: impl Into<String>,
        message: Option<String>,
    ) -> Result<Option<CaptureRequestStatus>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let Some(mut request_status) = config.capture_request_status.clone() else {
            return Ok(None);
        };
        if request_status.id != id {
            return Ok(None);
        }
        request_status.status = status.into();
        request_status.completed_at = Some(Utc::now().to_rfc3339());
        request_status.message = message;
        config.capture_request_status = Some(request_status.clone());
        store.save(&config)?;
        Ok(Some(request_status))
    }

    pub fn configure_youtube_api_key(&self, api_key: String) -> Result<()> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        config.youtube_api_key = Some(api_key);
        store.save(&config)
    }

    pub fn configure_spotify(&self, client_id: Option<String>) -> Result<SpotifyConfigureReport> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        config.spotify_sync.next_sync_at =
            Some(spotify::next_sync_at(chrono::Local::now()).to_rfc3339());
        let client_id_stored = if let Some(client_id) = client_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            config.spotify_client_id = Some(client_id.to_owned());
            config.spotify_oauth = None;
            true
        } else {
            config
                .spotify_client_id
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        };
        let oauth = spotify::configure_oauth(&config)?;
        config.spotify_oauth = Some(oauth);
        store.save(&config)?;
        Ok(spotify::configure_report(client_id_stored))
    }

    pub fn spotify_sync_status(&self) -> Result<SpotifySyncStatus> {
        Ok(spotify::status(&self.local_config()?))
    }

    pub fn spotify_sync_log(
        &self,
        limit: usize,
        show_skips: bool,
        since: Option<chrono::DateTime<Utc>>,
    ) -> Result<SpotifySyncLog> {
        let coverage_gap = self.spotify_coverage_gap()?;
        if let Some(gap) = coverage_gap.as_deref() {
            self.record_spotify_sync_event(spotify_event(
                "detected",
                "failed",
                "serve_not_running",
                gap,
            ))?;
        }
        Ok(SpotifySyncLog {
            events: self.index.spotify_sync_events(limit, show_skips, since)?,
            coverage_gap,
        })
    }

    pub fn sync_spotify(&self) -> Result<SpotifySyncReport> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let Some(oauth) = config.spotify_oauth.clone() else {
            let event = spotify_event(
                "detected",
                "failed",
                "spotify_not_connected",
                "Spotify is not connected; run `starlee configure-spotify` before syncing.",
            );
            self.record_spotify_sync_event(event)?;
            anyhow::bail!("Spotify is not connected; run `starlee configure-spotify` first");
        };
        let oauth = if spotify::oauth_is_valid(&config) {
            oauth
        } else {
            let refreshed = match spotify::refresh_oauth(&oauth) {
                Ok(refreshed) => refreshed,
                Err(error) => {
                    let mut event = spotify_event(
                        "detected",
                        "failed",
                        "spotify_not_connected",
                        "Spotify token refresh failed; reconnect Spotify before syncing.",
                    );
                    event.underlying_error = Some(error.to_string());
                    self.record_spotify_sync_event(event)?;
                    return Err(error);
                }
            };
            config.spotify_oauth = Some(refreshed.clone());
            store.save(&config)?;
            refreshed
        };
        let plan = match spotify::recently_played_sync_plan(&oauth.access_token) {
            Ok(plan) => plan,
            Err(error) => {
                let mut event = spotify_event(
                    "detected",
                    "failed",
                    "network_error",
                    "Spotify poll failed before Starlee could inspect playback history.",
                );
                event.underlying_error = Some(error.to_string());
                self.record_spotify_sync_event(event)?;
                return Err(error);
            }
        };
        let skipped = plan
            .events
            .iter()
            .filter(|event| event.outcome == "skipped")
            .count();
        for event in plan.events {
            self.record_spotify_sync_event(event)?;
        }
        let mut added = 0;
        let mut failed = 0;
        for episode in plan.episodes {
            let input = episode.input;
            let episode_id = input.spotify_episode_id.clone();
            let stable_id = episode_id
                .as_deref()
                .map(|episode_id| format!("spotify:episode:{episode_id}"));
            if let Some(stable_id) = stable_id.as_deref()
                && self.get(stable_id)?.is_some()
            {
                self.record_spotify_sync_event(event_for_input(
                    &input,
                    "deduped",
                    "skipped",
                    "duplicate_already_captured",
                    "Not captured: this Spotify episode is already in the Starlee vault.",
                    None,
                ))?;
                continue;
            }
            match self.capture(input.clone()) {
                Ok(_) => {
                    added += 1;
                    let (reason_code, explanation) = if input
                        .transcript_status
                        .as_deref()
                        .is_some_and(|status| {
                            status == "missing" || status.starts_with("unavailable")
                        }) {
                        (
                            "no_feed_transcript",
                            "Captured without transcript because no feed transcript was available.",
                        )
                    } else {
                        (
                            "captured_ok",
                            "Captured Spotify episode into the Starlee vault.",
                        )
                    };
                    self.record_spotify_sync_event(event_for_input(
                        &input,
                        "captured",
                        "ok",
                        reason_code,
                        explanation,
                        None,
                    ))?;
                }
                Err(error) => {
                    failed += 1;
                    self.record_spotify_sync_event(event_for_input(
                        &input,
                        "captured",
                        "failed",
                        "capture_error",
                        "Starlee saw the episode but failed while writing it to the vault.",
                        Some(error.to_string()),
                    ))?;
                }
            }
        }
        let checked_at = Utc::now().to_rfc3339();
        let status = if failed > 0 {
            "failed"
        } else if added == 0 {
            "no_qualifying_episodes"
        } else {
            "captured"
        };
        let report = SpotifySyncReport {
            ok: true,
            checked_at: checked_at.clone(),
            added,
            skipped: skipped + failed,
            status: status.into(),
            api_limitation: spotify::SPOTIFY_SYNC_DETAIL.into(),
            next_action:
                "Run `starlee list` or `starlee recent` to review synced Spotify episodes.".into(),
        };
        config.spotify_sync.next_sync_at =
            Some(spotify::next_sync_at(chrono::Local::now()).to_rfc3339());
        config.spotify_sync.last_synced_at = Some(checked_at);
        config.spotify_sync.last_result = Some(crate::config::SpotifySyncLastResult {
            checked_at: report.checked_at.clone(),
            added: report.added,
            skipped: report.skipped,
            status: report.status.clone(),
        });
        store.save(&config)?;
        Ok(report)
    }

    pub fn record_spotify_sync_event(&self, event: SpotifySyncEvent) -> Result<()> {
        self.index.insert_spotify_sync_event(&event)?;
        self.append_spotify_log_line(&event)
    }

    fn append_spotify_log_line(&self, event: &SpotifySyncEvent) -> Result<()> {
        let logs = self.home.join("logs");
        std::fs::create_dir_all(&logs)?;
        let line = format!(
            "{} spotify_sync reason={} outcome={} stage={} episode_id={} title={}\n",
            event.timestamp,
            event.reason_code,
            event.outcome,
            event.stage_reached,
            event.episode_id.as_deref().unwrap_or("-"),
            event
                .episode_title
                .as_deref()
                .unwrap_or("-")
                .replace('\n', " ")
        );
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs.join("serve.log"))?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    fn spotify_coverage_gap(&self) -> Result<Option<String>> {
        let last_poll = self.index.spotify_last_successful_poll_at()?;
        let service_running = self
            .local_config()
            .map(|config| capture_service_reachable(config.capture_port))
            .unwrap_or(false);
        if service_running {
            return Ok(None);
        }
        let Some(last_poll) = last_poll else {
            return Ok(Some(
                "the sync service is not running and Starlee has never recorded a successful Spotify poll; anything played in this window was not seen".into(),
            ));
        };
        Ok(Some(format!(
            "the sync service is not running; last successful Spotify poll was {last_poll}, so anything played after that may not have been seen"
        )))
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

fn spotify_event(
    stage_reached: &str,
    outcome: &str,
    reason_code: &str,
    explanation: &str,
) -> SpotifySyncEvent {
    SpotifySyncEvent {
        id: 0,
        timestamp: Utc::now().to_rfc3339(),
        episode_id: None,
        episode_title: None,
        show_name: None,
        stage_reached: stage_reached.into(),
        outcome: outcome.into(),
        reason_code: reason_code.into(),
        explanation: explanation.into(),
        underlying_error: None,
        listen_duration_s: None,
        threshold_s: None,
    }
}

fn event_for_input(
    input: &CaptureInput,
    stage_reached: &str,
    outcome: &str,
    reason_code: &str,
    explanation: &str,
    underlying_error: Option<String>,
) -> SpotifySyncEvent {
    SpotifySyncEvent {
        id: 0,
        timestamp: Utc::now().to_rfc3339(),
        episode_id: input.spotify_episode_id.clone(),
        episode_title: Some(input.title.clone()),
        show_name: input.show.clone(),
        stage_reached: stage_reached.into(),
        outcome: outcome.into(),
        reason_code: reason_code.into(),
        explanation: explanation.into(),
        underlying_error,
        listen_duration_s: input.listen_duration_s,
        threshold_s: Some(10 * 60),
    }
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

fn domain_from_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_owned())
}

fn add_terms(counts: &mut HashMap<String, usize>, text: &str) {
    let stopwords = stopwords();
    let mut document_terms = HashSet::new();
    for term in text
        .split(|character: char| !character.is_ascii_alphanumeric())
        .map(str::to_lowercase)
        .filter(|term| term.len() >= 4 && !stopwords.contains(term.as_str()))
    {
        document_terms.insert(term);
    }
    for term in document_terms {
        *counts.entry(term).or_insert(0) += 1;
    }
}

fn top_keys(mut counts: HashMap<String, usize>, limit: usize) -> Vec<String> {
    let mut values = counts.drain().collect::<Vec<_>>();
    values.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    values
        .into_iter()
        .take(limit)
        .map(|(value, _)| value)
        .collect()
}

fn stopwords() -> HashSet<&'static str> {
    [
        "about", "after", "again", "also", "because", "been", "being", "between", "could", "does",
        "down", "from", "have", "into", "just", "like", "more", "most", "much", "only", "over",
        "said", "same", "some", "such", "than", "that", "their", "them", "then", "there", "these",
        "they", "this", "through", "what", "when", "where", "which", "while", "with", "would",
        "your",
    ]
    .into_iter()
    .collect()
}
