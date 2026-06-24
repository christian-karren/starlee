use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    process::Command,
    sync::Arc,
    time::Instant,
};

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::{
    bundle::{self, BundleAudit},
    config::{
        CaptureDiagnosticEvent, CaptureRequestPageMetadata, CaptureRequestState,
        CaptureRequestStatus, ConfigStore, ExtensionState, LocalConfig,
    },
    embedding::{Embedder, FastEmbedder},
    index::Index,
    model::{
        BridgeHealth, CaptureInput, CaptureTraceReport, CorpusOverview, DoctorCheck, DoctorReport,
        GetResult, QueryResult, Record, RuntimeIdentity, SearchHit, SearchScope, SetupReport,
        SourceType, SpotifySyncEvent, SpotifySyncLog, Status,
    },
    public_fetch, sensor_assets, spotify,
    spotify::{SpotifyConfigureReport, SpotifySyncReport, SpotifySyncStatus},
    vault::Vault,
    youtube::enrich_youtube,
};

mod bridge;

#[allow(unused_imports)]
pub use bridge::{
    CAPTURE_REQUEST_TTL, CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE,
    CAPTURE_STATUS_EXTENSION_UNAVAILABLE, CAPTURE_STATUS_EXTRACTING, CAPTURE_STATUS_FAILED,
    CAPTURE_STATUS_PERMISSION_DENIED, CAPTURE_STATUS_PICKED_UP, CAPTURE_STATUS_POSTED,
    CAPTURE_STATUS_QUEUED, CAPTURE_STATUS_SAVED, CAPTURE_STATUS_TIMED_OUT,
    CAPTURE_STATUS_UNSUPPORTED_PAGE, EXTENSION_HEARTBEAT_FRESHNESS,
};

use bridge::*;

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
        self.setup()?;
        let config = self.local_config()?;
        let extension_path = sensor_assets::install(&self.home, &config)?;
        let status = self.status()?;
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
        let mut existing = input
            .url
            .as_deref()
            .map(|url| self.index.get_by_url(url))
            .transpose()?
            .flatten()
            .map(|path| self.vault.read(&path))
            .transpose()?;
        if existing.is_none()
            && matches!(input.source_type, SourceType::Youtube)
            && let Some(video_id) = input.video_id.as_deref()
        {
            existing = self.youtube_record_by_video_id(video_id)?;
        }
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

    fn youtube_record_by_video_id(&self, video_id: &str) -> Result<Option<Record>> {
        let video_id = video_id.trim();
        if video_id.is_empty() {
            return Ok(None);
        }
        Ok(self.vault.records()?.into_iter().find(|record| {
            matches!(record.metadata.source_type, SourceType::Youtube)
                && record.metadata.video_id.as_deref() == Some(video_id)
        }))
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
        let mut config = store.load_or_create()?;
        let changed = expire_stale_capture_request(&mut config);
        if changed {
            store.save(&config)?;
        }
        let bridge_health = self.bridge_health_from_config(&config);
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
            bridge_health,
        })
    }

    pub fn local_config(&self) -> Result<LocalConfig> {
        ConfigStore::new(&self.home).load_or_create()
    }

    pub fn capture_diagnostics(&self, limit: usize) -> Result<Vec<CaptureDiagnosticEvent>> {
        let mut events = self.local_config()?.capture_diagnostics;
        events.reverse();
        events.truncate(limit);
        Ok(events)
    }

    pub fn record_capture_diagnostic_event(
        &self,
        event: CaptureDiagnosticEvent,
    ) -> Result<CaptureDiagnosticEvent> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let event = sanitize_capture_diagnostic_event(event);
        append_capture_diagnostic(&mut config, event.clone());
        store.save(&config)?;
        Ok(event)
    }

    pub fn last_capture_trace(&self) -> Result<CaptureTraceReport> {
        let config = self.local_config()?;
        let request_id = config
            .capture_request_status
            .as_ref()
            .map(|status| status.id.clone())
            .or_else(|| {
                config
                    .capture_diagnostics
                    .iter()
                    .rev()
                    .find_map(|event| event.request_id.clone())
            });
        let mut events = match request_id.as_deref() {
            Some(id) => config
                .capture_diagnostics
                .iter()
                .filter(|event| event.request_id.as_deref() == Some(id))
                .cloned()
                .collect::<Vec<_>>(),
            None => config.capture_diagnostics.clone(),
        };
        events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        let request_status = config.capture_request_status.clone().filter(|status| {
            request_id
                .as_deref()
                .is_none_or(|request_id| status.id == request_id)
        });
        let terminal_status = request_status
            .as_ref()
            .and_then(|status| {
                capture_status_is_terminal(&status.status).then(|| status.status.clone())
            })
            .or_else(|| {
                events
                    .iter()
                    .rev()
                    .filter_map(|event| event.status.as_deref())
                    .find(|status| capture_status_is_terminal(status))
                    .map(str::to_owned)
            });
        let started_at = request_status
            .as_ref()
            .map(|status| status.requested_at.clone())
            .or_else(|| events.first().map(|event| event.timestamp.clone()));
        let completed_at = request_status
            .as_ref()
            .and_then(|status| status.completed_at.clone())
            .or_else(|| {
                terminal_status
                    .as_ref()
                    .and_then(|terminal| {
                        events
                            .iter()
                            .rev()
                            .find(|event| event.status.as_ref() == Some(terminal))
                    })
                    .map(|event| event.timestamp.clone())
            });
        Ok(CaptureTraceReport {
            trace_version: 1,
            request_id,
            started_at,
            completed_at,
            terminal_status: terminal_status.clone(),
            recommended_next_action: bridge_next_action(
                self.home.join("sensor-extension/manifest.json").exists(),
                self.home
                    .join("sensor-extension/starlee-config.json")
                    .exists(),
                extension_heartbeat_is_fresh(&config.extension, EXTENSION_HEARTBEAT_FRESHNESS),
                config.extension.can_capture_active_tab,
                terminal_status
                    .as_deref()
                    .or_else(|| request_status.as_ref().map(|status| status.status.as_str())),
            ),
            runtime: self.runtime_identity(&config),
            request_status,
            events,
        })
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
        checks.push(DoctorCheck {
            name: "browser_bridge".into(),
            ok: status.bridge_health.ok,
            detail: status.bridge_health.recommended_next_action.clone(),
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
                "browser_bridge" => status.bridge_health.recommended_next_action.as_str(),
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

    pub fn bridge_health(&self) -> Result<BridgeHealth> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let changed = expire_stale_capture_request(&mut config);
        if changed {
            store.save(&config)?;
        }
        Ok(self.bridge_health_from_config(&config))
    }

    fn runtime_identity(&self, config: &LocalConfig) -> RuntimeIdentity {
        RuntimeIdentity {
            starlee_version: env!("CARGO_PKG_VERSION").into(),
            app_build_identifier: std::env::var("STARLEE_APP_BUILD_IDENTIFIER").ok(),
            browser: config.extension.browser.clone(),
            extension_version: config.extension.extension_version.clone(),
            extension_build: config.extension.extension_build.clone(),
            git_commit: git_commit()
                .or_else(|| option_env!("STARLEE_GIT_COMMIT").map(str::to_owned)),
            app_path: user_app_path(),
            binary_path: std::env::current_exe()
                .ok()
                .map(|path| path.display().to_string()),
            source_repo_path: source_repo_path(),
        }
    }

    fn bridge_health_from_config(&self, config: &LocalConfig) -> BridgeHealth {
        let extension_path = self.home.join("sensor-extension");
        let extension_setup_present = extension_path.join("manifest.json").exists();
        let extension_config_present = extension_path.join("starlee-config.json").exists();
        let checked_in_recently =
            extension_heartbeat_is_fresh(&config.extension, EXTENSION_HEARTBEAT_FRESHNESS);
        let last_request_status = config
            .capture_request_status
            .as_ref()
            .map(|status| status.status.clone());
        let last_failure = config
            .capture_request_status
            .as_ref()
            .filter(|status| capture_status_is_terminal(&status.status))
            .filter(|status| status.status != CAPTURE_STATUS_SAVED);
        let last_failure_reason = last_failure.map(|status| status.status.clone());
        let last_failure_message = last_failure.and_then(|status| {
            safe_bridge_failure_message(&status.status, status.message.as_deref())
        });
        let ok = extension_setup_present
            && extension_config_present
            && checked_in_recently
            && config.extension.can_capture_active_tab;
        BridgeHealth {
            ok,
            extension_setup_present,
            extension_config_present,
            checked_in_recently,
            browser: config.extension.browser.clone(),
            extension_version: config.extension.extension_version.clone(),
            can_capture_active_tab: config.extension.can_capture_active_tab,
            last_hello_at: config.extension.last_handshake_at.clone(),
            last_request_status,
            last_failure_reason,
            last_failure_message,
            recommended_next_action: bridge_next_action(
                extension_setup_present,
                extension_config_present,
                checked_in_recently,
                config.extension.can_capture_active_tab,
                config
                    .capture_request_status
                    .as_ref()
                    .map(|status| status.status.as_str()),
            ),
            recent_diagnostics: recent_diagnostics(config, 8),
        }
    }

    pub fn record_extension_hello(
        &self,
        browser: Option<String>,
        extension_version: Option<String>,
        extension_build: Option<String>,
        can_capture_active_tab: bool,
    ) -> Result<ExtensionState> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        config.extension = ExtensionState {
            browser,
            extension_version,
            extension_build,
            can_capture_active_tab,
            last_handshake_at: Some(Utc::now().to_rfc3339()),
        };
        store.save(&config)?;
        Ok(config.extension)
    }

    pub fn create_capture_request(
        &self,
        source: impl Into<String>,
    ) -> Result<CaptureRequestStatus> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        expire_stale_capture_request(&mut config);
        let source = source.into();
        let id_material = format!(
            "{}:{}:{}",
            config.capture_token,
            Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            source
        );
        let id = token_fingerprint(&id_material);
        let requested_at = Utc::now().to_rfc3339();
        let browser = config.extension.browser.clone();
        let request = CaptureRequestState {
            id: id.clone(),
            requested_at: requested_at.clone(),
            source: source.clone(),
        };
        let mut status = CaptureRequestStatus {
            id,
            requested_at,
            source,
            picked_up_at: None,
            browser,
            page: None,
            status: CAPTURE_STATUS_QUEUED.into(),
            completed_at: None,
            message: Some("Capture request queued for the browser extension.".into()),
        };
        if extension_is_fresh(&config.extension, EXTENSION_HEARTBEAT_FRESHNESS) {
            config.pending_capture_request = Some(request);
            if status.source == "menu-bar" {
                append_capture_diagnostic(
                    &mut config,
                    diagnostic_event(DiagnosticEventInput {
                        component: "menu_bar",
                        event: "menu_bar_capture_clicked",
                        request_id: Some(&status.id),
                        status: Some(&status.status),
                        source: Some(&status.source),
                        browser: status.browser.as_deref(),
                        message: Some("Menu-bar capture requested."),
                        page: None,
                    }),
                );
            }
            append_capture_diagnostic(
                &mut config,
                diagnostic_event(DiagnosticEventInput {
                    component: "engine",
                    event: "capture_request_queued",
                    request_id: Some(&status.id),
                    status: Some(&status.status),
                    source: Some(&status.source),
                    browser: status.browser.as_deref(),
                    message: status.message.as_deref(),
                    page: None,
                }),
            );
        } else {
            status.status = CAPTURE_STATUS_EXTENSION_UNAVAILABLE.into();
            status.completed_at = Some(Utc::now().to_rfc3339());
            status.message = default_capture_status_message(CAPTURE_STATUS_EXTENSION_UNAVAILABLE);
            config.pending_capture_request = None;
            append_capture_diagnostic(
                &mut config,
                diagnostic_event(DiagnosticEventInput {
                    component: "engine",
                    event: "capture_request_rejected",
                    request_id: Some(&status.id),
                    status: Some(&status.status),
                    source: Some(&status.source),
                    browser: status.browser.as_deref(),
                    message: status.message.as_deref(),
                    page: None,
                }),
            );
        }
        config.capture_request_status = Some(status.clone());
        store.save(&config)?;
        Ok(status)
    }

    pub fn take_capture_request(&self) -> Result<Option<CaptureRequestState>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        expire_stale_capture_request(&mut config);
        let request = config.pending_capture_request.take();
        let picked_up_event = if let Some(request) = request.as_ref()
            && let Some(status) = config.capture_request_status.as_mut()
            && status.id == request.id
            && !capture_status_is_terminal(&status.status)
        {
            status.status = CAPTURE_STATUS_PICKED_UP.into();
            status.picked_up_at = Some(Utc::now().to_rfc3339());
            status.message = Some("Browser extension picked up the capture request.".into());
            Some(diagnostic_event(DiagnosticEventInput {
                component: "engine",
                event: "capture_request_picked_up",
                request_id: Some(&status.id),
                status: Some(&status.status),
                source: Some(&status.source),
                browser: status.browser.as_deref(),
                message: status.message.as_deref(),
                page: status.page.clone(),
            }))
        } else {
            None
        };
        if let Some(event) = picked_up_event {
            append_capture_diagnostic(&mut config, event);
        }
        store.save(&config)?;
        Ok(request)
    }

    pub fn capture_request_status(&self, id: &str) -> Result<Option<CaptureRequestStatus>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let changed = expire_stale_capture_request(&mut config);
        if changed {
            store.save(&config)?;
        }
        Ok(config
            .capture_request_status
            .filter(|status| status.id == id))
    }

    pub fn record_capture_request_result(
        &self,
        id: &str,
        status: impl Into<String>,
        message: Option<String>,
        page: Option<CaptureRequestPageMetadata>,
    ) -> Result<Option<CaptureRequestStatus>> {
        let store = ConfigStore::new(&self.home);
        let mut config = store.load_or_create()?;
        let expired = expire_stale_capture_request(&mut config);
        let Some(mut request_status) = config.capture_request_status.clone() else {
            if expired {
                store.save(&config)?;
            }
            return Ok(None);
        };
        if request_status.id != id {
            if expired {
                store.save(&config)?;
            }
            return Ok(None);
        }
        if capture_status_is_terminal(&request_status.status) {
            store.save(&config)?;
            return Ok(Some(request_status));
        }
        let status = normalize_capture_request_status(&status.into());
        request_status.status = status.clone();
        if status == CAPTURE_STATUS_PICKED_UP && request_status.picked_up_at.is_none() {
            request_status.picked_up_at = Some(Utc::now().to_rfc3339());
        }
        if capture_status_is_terminal(&status) {
            request_status.completed_at = Some(Utc::now().to_rfc3339());
            config.pending_capture_request = None;
        }
        if let Some(page) = page {
            request_status.page = Some(sanitize_page_metadata(page));
        }
        request_status.message = message.or_else(|| default_capture_status_message(&status));
        let diagnostic_message =
            safe_bridge_failure_message(&request_status.status, request_status.message.as_deref());
        append_capture_diagnostic(
            &mut config,
            diagnostic_event(DiagnosticEventInput {
                component: "browser_bridge",
                event: "capture_request_status",
                request_id: Some(&request_status.id),
                status: Some(&request_status.status),
                source: Some(&request_status.source),
                browser: request_status.browser.as_deref(),
                message: diagnostic_message.as_deref(),
                page: request_status.page.clone(),
            }),
        );
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

fn domain_from_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_owned())
}

fn user_app_path() -> Option<String> {
    let path = home_dir().join("Applications/Starlee.app");
    path.exists().then(|| path.display().to_string())
}

fn source_repo_path() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git_commit() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::EMBEDDING_DIMENSION;

    struct StaticTestEmbedder;

    impl Embedder for StaticTestEmbedder {
        fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|_| vec![1.0; EMBEDDING_DIMENSION])
                .collect())
        }

        fn embed_query(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(vec![1.0; EMBEDDING_DIMENSION])
        }

        fn name(&self) -> &'static str {
            "engine-test"
        }
    }

    #[test]
    fn capture_request_lifecycle_reaches_saved_with_safe_metadata() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(StaticTestEmbedder));
        engine.record_extension_hello(
            Some("Chrome".into()),
            Some("0.1.0".into()),
            Some("codex/youtube-capture-diagnostic-harness@b965131".into()),
            true,
        )?;

        let request = engine.create_capture_request("test")?;
        assert_eq!(request.status, CAPTURE_STATUS_QUEUED);

        let picked_up = engine.take_capture_request()?.expect("request available");
        assert_eq!(picked_up.id, request.id);
        let status = engine
            .capture_request_status(&request.id)?
            .expect("status available");
        assert_eq!(status.status, CAPTURE_STATUS_PICKED_UP);
        assert!(status.picked_up_at.is_some());

        let extracting = engine
            .record_capture_request_result(&request.id, CAPTURE_STATUS_EXTRACTING, None, None)?
            .expect("extracting status");
        assert_eq!(extracting.status, CAPTURE_STATUS_EXTRACTING);
        assert!(extracting.completed_at.is_none());

        let posted = engine
            .record_capture_request_result(
                &request.id,
                CAPTURE_STATUS_POSTED,
                None,
                Some(CaptureRequestPageMetadata {
                    title: Some("A useful page".into()),
                    url: Some("https://example.com/article".into()),
                    domain: Some("example.com".into()),
                }),
            )?
            .expect("posted status");
        assert_eq!(posted.status, CAPTURE_STATUS_POSTED);
        assert_eq!(
            posted.page.as_ref().and_then(|page| page.domain.as_deref()),
            Some("example.com")
        );

        let saved = engine
            .record_capture_request_result(
                &request.id,
                CAPTURE_STATUS_SAVED,
                Some("Saved to Starlee.".into()),
                None,
            )?
            .expect("saved status");
        assert_eq!(saved.status, CAPTURE_STATUS_SAVED);
        assert!(saved.completed_at.is_some());
        Ok(())
    }

    #[test]
    fn capture_request_diagnostics_record_safe_lifecycle_trace() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(StaticTestEmbedder));
        engine.record_extension_hello(Some("Safari".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("menu-bar")?;

        assert!(engine.take_capture_request()?.is_some());
        engine.record_capture_request_result(&request.id, CAPTURE_STATUS_EXTRACTING, None, None)?;
        engine.record_capture_request_result(
            &request.id,
            CAPTURE_STATUS_POSTED,
            Some("Browser extension posted the capture to Starlee.".into()),
            Some(CaptureRequestPageMetadata {
                title: Some("A very useful page".into()),
                url: Some("https://example.com/story?private=query".into()),
                domain: Some("example.com".into()),
            }),
        )?;
        engine.record_capture_request_result(&request.id, CAPTURE_STATUS_SAVED, None, None)?;

        let diagnostics = engine.capture_diagnostics(10)?;
        let statuses = diagnostics
            .iter()
            .rev()
            .filter_map(|event| event.status.as_deref())
            .collect::<Vec<_>>();

        assert_eq!(
            statuses,
            vec![
                CAPTURE_STATUS_QUEUED,
                CAPTURE_STATUS_QUEUED,
                CAPTURE_STATUS_PICKED_UP,
                CAPTURE_STATUS_EXTRACTING,
                CAPTURE_STATUS_POSTED,
                CAPTURE_STATUS_SAVED
            ]
        );
        assert!(
            diagnostics
                .iter()
                .any(|event| event.event == "menu_bar_capture_clicked")
        );
        assert!(diagnostics.iter().all(|event| event.request_id.is_some()));
        assert!(diagnostics.iter().any(|event| {
            event.page.as_ref().and_then(|page| page.domain.as_deref()) == Some("example.com")
        }));
        let serialized = serde_json::to_string(&diagnostics)?;
        assert!(!serialized.contains("capture_token"));
        assert!(!serialized.contains("Transcript unavailable"));
        Ok(())
    }

    #[test]
    fn capture_request_diagnostics_are_bounded() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Safari".into()), Some("0.1.0".into()), None, true)?;

        for _ in 0..150 {
            let request = engine.create_capture_request("menu-bar")?;
            engine.record_capture_request_result(&request.id, CAPTURE_STATUS_FAILED, None, None)?;
        }

        assert_eq!(engine.local_config()?.capture_diagnostics.len(), 120);
        assert_eq!(engine.capture_diagnostics(5)?.len(), 5);
        Ok(())
    }

    #[test]
    fn last_capture_trace_groups_request_events_and_runtime_identity() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(StaticTestEmbedder));
        engine.record_extension_hello(
            Some("Chrome".into()),
            Some("0.1.0".into()),
            Some("codex/youtube-capture-diagnostic-harness@b965131".into()),
            true,
        )?;
        let request = engine.create_capture_request("menu-bar")?;
        engine.take_capture_request()?;
        engine.record_capture_diagnostic_event(CaptureDiagnosticEvent {
            timestamp: Utc::now().to_rfc3339(),
            component: "youtube_extractor".into(),
            event: "youtube_segments_extracted".into(),
            request_id: Some(request.id.clone()),
            status: Some("unavailable".into()),
            source: Some("menu-bar".into()),
            browser: Some("Chrome".into()),
            message: Some("No rendered transcript segments found.".into()),
            page: Some(CaptureRequestPageMetadata {
                title: Some("Fixture video".into()),
                url: Some("https://www.youtube.com/watch?v=abc123".into()),
                domain: Some("youtube.com".into()),
            }),
            safe_metadata: BTreeMap::from([("segment_count".into(), "0".into())]),
        })?;
        engine.record_capture_request_result(&request.id, CAPTURE_STATUS_FAILED, None, None)?;

        let trace = engine.last_capture_trace()?;
        assert_eq!(trace.trace_version, 1);
        assert_eq!(trace.request_id.as_deref(), Some(request.id.as_str()));
        assert_eq!(
            trace.terminal_status.as_deref(),
            Some(CAPTURE_STATUS_FAILED)
        );
        assert_eq!(trace.runtime.starlee_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(trace.runtime.browser.as_deref(), Some("Chrome"));
        assert_eq!(
            trace.runtime.extension_build.as_deref(),
            Some("codex/youtube-capture-diagnostic-harness@b965131")
        );
        assert!(
            trace
                .events
                .iter()
                .all(|event| { event.request_id.as_deref() == Some(request.id.as_str()) })
        );
        assert!(trace.events.iter().any(|event| {
            event.event == "youtube_segments_extracted"
                && event.safe_metadata.get("segment_count").map(String::as_str) == Some("0")
        }));
        Ok(())
    }

    #[test]
    fn diagnostic_event_intake_sanitizes_metadata_and_forbidden_content() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(StaticTestEmbedder));
        let stored = engine.record_capture_diagnostic_event(CaptureDiagnosticEvent {
            timestamp: "not-a-date".into(),
            component: "extension".into(),
            event: "youtube_segments_extracted".into(),
            request_id: Some("request-fingerprint".into()),
            status: Some("unavailable".into()),
            source: Some("menu-bar".into()),
            browser: Some("Chrome".into()),
            message: Some("No rendered transcript segments found.".repeat(20)),
            page: Some(CaptureRequestPageMetadata {
                title: Some("Video".into()),
                url: Some("https://www.youtube.com/watch?v=abc123".into()),
                domain: Some("youtube.com".into()),
            }),
            safe_metadata: BTreeMap::from([
                ("segment_count".into(), "0".into()),
                ("bad key".into(), "should be dropped".into()),
                (
                    "transcript_text".into(),
                    "never store transcript text here".into(),
                ),
            ]),
        })?;

        assert_eq!(stored.component, "extension");
        assert!(parse_rfc3339_utc(&stored.timestamp).is_some());
        assert!(stored.message.as_deref().unwrap_or("").len() <= 240);
        assert!(stored.safe_metadata.contains_key("segment_count"));
        assert!(!stored.safe_metadata.contains_key("bad key"));
        let serialized = serde_json::to_string(&engine.capture_diagnostics(10)?)?;
        assert!(!serialized.contains("capture_token"));
        assert!(!serialized.contains("Bearer "));
        assert!(!serialized.contains("OAuth"));
        assert!(!serialized.contains("<html"));
        assert!(!serialized.contains("never store transcript text here"));
        Ok(())
    }

    #[test]
    fn capture_request_failure_states_are_terminal() -> Result<()> {
        for status in [
            CAPTURE_STATUS_FAILED,
            CAPTURE_STATUS_PERMISSION_DENIED,
            CAPTURE_STATUS_UNSUPPORTED_PAGE,
            CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE,
        ] {
            let temp = tempfile::tempdir()?;
            let engine = Engine::new(temp.path().to_owned());
            engine.record_extension_hello(
                Some("Chrome".into()),
                Some("0.1.0".into()),
                None,
                true,
            )?;
            let request = engine.create_capture_request("test")?;
            let failed = engine
                .record_capture_request_result(&request.id, status, None, None)?
                .expect("terminal status");
            assert_eq!(failed.status, status);
            assert!(failed.completed_at.is_some());
            let ignored = engine
                .record_capture_request_result(&request.id, CAPTURE_STATUS_SAVED, None, None)?
                .expect("terminal status remains available");
            assert_eq!(ignored.status, status);
        }
        Ok(())
    }

    #[test]
    fn bridge_health_recommends_reload_when_content_script_is_unreachable() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Safari".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("menu-bar")?;
        engine.record_capture_request_result(
            &request.id,
            CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE,
            Some("Private page body should not be reflected".into()),
            None,
        )?;

        let health = engine.bridge_health()?;

        assert_eq!(
            health.last_request_status.as_deref(),
            Some(CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE)
        );
        assert_eq!(
            health.last_failure_message.as_deref(),
            Some("Safari extension could not reach the page content script.")
        );
        assert_eq!(
            health.recommended_next_action,
            "Open Safari, enable the Starlee Safari extension, allow it on youtube.com, reload the YouTube tab, then try capture again."
        );
        Ok(())
    }

    #[test]
    fn stale_capture_request_times_out_and_is_not_served() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("test")?;

        age_capture_request(temp.path(), 30)?;

        assert!(engine.take_capture_request()?.is_none());
        let status = engine
            .capture_request_status(&request.id)?
            .expect("timed out status");
        assert_eq!(status.status, CAPTURE_STATUS_TIMED_OUT);
        assert!(status.completed_at.is_some());

        let ignored = engine
            .record_capture_request_result(&request.id, CAPTURE_STATUS_SAVED, None, None)?
            .expect("timed out status remains available");
        assert_eq!(ignored.status, CAPTURE_STATUS_TIMED_OUT);
        Ok(())
    }

    #[test]
    fn stale_capture_result_with_wrong_id_persists_timeout() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("test")?;
        age_capture_request(temp.path(), 30)?;

        assert!(
            engine
                .record_capture_request_result("wrong-id", CAPTURE_STATUS_SAVED, None, None)?
                .is_none()
        );
        let status = engine
            .capture_request_status(&request.id)?
            .expect("timed out status");
        assert_eq!(status.status, CAPTURE_STATUS_TIMED_OUT);
        assert!(status.completed_at.is_some());
        Ok(())
    }

    #[test]
    fn status_lookup_persists_pending_cleanup_for_terminal_request() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("test")?;

        let store = ConfigStore::new(temp.path());
        let mut config = store.load_or_create()?;
        if let Some(status) = config.capture_request_status.as_mut() {
            status.status = CAPTURE_STATUS_FAILED.into();
            status.completed_at = Some(Utc::now().to_rfc3339());
        }
        assert!(config.pending_capture_request.is_some());
        store.save(&config)?;

        let status = engine
            .capture_request_status(&request.id)?
            .expect("terminal status");
        assert_eq!(status.status, CAPTURE_STATUS_FAILED);
        assert!(
            ConfigStore::new(temp.path())
                .load_or_create()?
                .pending_capture_request
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn page_metadata_is_trimmed_limited_and_body_free() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("test")?;
        let long_title = format!("  {}  ", "A".repeat(300));
        let long_url = format!("https://example.com/{}", "b".repeat(2100));

        let posted = engine
            .record_capture_request_result(
                &request.id,
                CAPTURE_STATUS_POSTED,
                None,
                Some(CaptureRequestPageMetadata {
                    title: Some(long_title),
                    url: Some(long_url),
                    domain: Some("  example.com  ".into()),
                }),
            )?
            .expect("posted status");
        let page = posted.page.expect("safe page metadata");
        assert_eq!(page.title.expect("title").chars().count(), 240);
        assert_eq!(page.url.expect("url").chars().count(), 2048);
        assert_eq!(page.domain.as_deref(), Some("example.com"));
        Ok(())
    }

    #[test]
    fn wrong_request_id_and_duplicate_pickup_do_not_progress_request() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("test")?;

        assert!(
            engine
                .record_capture_request_result("wrong-id", CAPTURE_STATUS_SAVED, None, None)?
                .is_none()
        );
        assert!(engine.take_capture_request()?.is_some());
        assert!(engine.take_capture_request()?.is_none());
        let status = engine
            .capture_request_status(&request.id)?
            .expect("status remains for original id");
        assert_eq!(status.status, CAPTURE_STATUS_PICKED_UP);
        Ok(())
    }

    #[test]
    fn stale_extension_heartbeat_prevents_queueing() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::new(temp.path().to_owned());
        let request = engine.create_capture_request("menu-bar")?;
        assert_eq!(request.status, CAPTURE_STATUS_EXTENSION_UNAVAILABLE);
        assert!(request.completed_at.is_some());
        assert!(engine.take_capture_request()?.is_none());
        Ok(())
    }

    #[test]
    fn bridge_health_reports_fresh_extension_heartbeat_as_ready() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;

        let health = engine.bridge_health()?;

        assert!(health.ok);
        assert!(health.extension_setup_present);
        assert!(health.extension_config_present);
        assert!(health.checked_in_recently);
        assert_eq!(health.browser.as_deref(), Some("Chrome"));
        assert!(health.can_capture_active_tab);
        assert_eq!(health.last_request_status, None);
        assert!(health.recommended_next_action.contains("Bridge is ready"));
        Ok(())
    }

    #[test]
    fn bridge_health_reports_missing_or_stale_heartbeat_with_next_action() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());

        let missing = engine.bridge_health()?;
        assert!(!missing.ok);
        assert!(!missing.checked_in_recently);
        assert!(
            missing
                .recommended_next_action
                .contains("Load or reload the Starlee browser extension")
        );

        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        age_extension_hello(temp.path(), 10 * 60)?;
        let stale = engine.bridge_health()?;
        assert!(!stale.ok);
        assert!(!stale.checked_in_recently);
        assert!(
            stale
                .recommended_next_action
                .contains("Load or reload the Starlee browser extension")
        );
        Ok(())
    }

    #[test]
    fn bridge_health_includes_last_request_failure_without_sensitive_payload() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("menu-bar")?;
        engine.record_capture_request_result(
            &request.id,
            CAPTURE_STATUS_PERMISSION_DENIED,
            Some("PRIVATE SELECTED TEXT and transcript should not leak".into()),
            Some(CaptureRequestPageMetadata {
                title: Some("Private title".into()),
                url: Some("https://private.example/article".into()),
                domain: Some("private.example".into()),
            }),
        )?;

        let health = engine.bridge_health()?;
        let serialized = serde_json::to_string(&health)?;

        assert_eq!(
            health.last_request_status.as_deref(),
            Some(CAPTURE_STATUS_PERMISSION_DENIED)
        );
        assert_eq!(
            health.last_failure_reason.as_deref(),
            Some(CAPTURE_STATUS_PERMISSION_DENIED)
        );
        assert!(
            health
                .last_failure_message
                .as_deref()
                .is_some_and(|message| message.contains("Grant Starlee site access"))
        );
        assert!(!serialized.contains(&request.id));
        assert!(!serialized.contains("PRIVATE SELECTED TEXT"));
        assert!(!serialized.contains("transcript"));
        assert!(!serialized.contains("Private title"));
        assert!(!serialized.contains("private.example"));
        Ok(())
    }

    #[test]
    fn bridge_health_output_does_not_expose_tokens_or_content() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());
        let store = ConfigStore::new(temp.path());
        let config = store.load_or_create()?;
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("menu-bar")?;
        engine.record_capture_request_result(
            &request.id,
            CAPTURE_STATUS_FAILED,
            Some("restricted body transcript selected text capture token".into()),
            None,
        )?;

        let serialized = serde_json::to_string(&engine.bridge_health()?)?;

        assert!(!serialized.contains(&config.capture_token));
        assert!(!serialized.contains(&request.id));
        assert!(!serialized.contains("restricted body"));
        assert!(!serialized.contains("selected text"));
        assert!(!serialized.contains("capture token"));
        Ok(())
    }

    #[test]
    fn youtube_capture_recaptures_existing_video_id() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(StaticTestEmbedder));
        let mut first = CaptureInput::new(
            "Original lecture",
            "[00:01] First captured transcript",
            SourceType::Youtube,
            crate::model::Access::Restricted,
        );
        first.site = Some("youtube.com".into());
        first.source = Some("manual_capture".into());
        first.url = Some("https://www.youtube.com/watch?v=abc123".into());
        first.video_id = Some("abc123".into());
        first.transcript_status = Some("full".into());
        first.transcript_source = Some("rendered_dom".into());
        let original = engine.capture(first)?;

        let mut recapture = CaptureInput::new(
            "Updated lecture",
            "[00:01] Updated captured transcript",
            SourceType::Youtube,
            crate::model::Access::Restricted,
        );
        recapture.site = Some("youtube.com".into());
        recapture.source = Some("manual_capture".into());
        recapture.url = Some("https://youtu.be/abc123".into());
        recapture.video_id = Some("abc123".into());
        recapture.transcript_status = Some("full".into());
        recapture.transcript_source = Some("rendered_dom".into());
        let updated = engine.capture(recapture)?;

        assert_eq!(updated.metadata.id, original.metadata.id);
        assert_eq!(engine.vault.records()?.len(), 1);
        assert_eq!(updated.body, "[00:01] Updated captured transcript");
        Ok(())
    }

    #[test]
    fn youtube_request_lifecycle_reaches_saved_without_transcript_status_leak() -> Result<()> {
        let temp = tempfile::tempdir()?;
        install_extension_setup(temp.path())?;
        let engine = Engine::new(temp.path().to_owned());
        engine.record_extension_hello(Some("Chrome".into()), Some("0.1.0".into()), None, true)?;
        let request = engine.create_capture_request("menu-bar")?;
        assert_eq!(request.status, CAPTURE_STATUS_QUEUED);

        assert!(engine.take_capture_request()?.is_some());
        engine.record_capture_request_result(&request.id, CAPTURE_STATUS_EXTRACTING, None, None)?;
        engine.record_capture_request_result(
            &request.id,
            CAPTURE_STATUS_POSTED,
            Some("Posted YouTube capture.".into()),
            Some(CaptureRequestPageMetadata {
                title: Some("A lecture".into()),
                url: Some("https://www.youtube.com/watch?v=abc123".into()),
                domain: Some("youtube.com".into()),
            }),
        )?;
        let saved = engine
            .record_capture_request_result(&request.id, CAPTURE_STATUS_SAVED, None, None)?
            .expect("saved status");

        let serialized = serde_json::to_string(&saved)?;
        assert_eq!(saved.status, CAPTURE_STATUS_SAVED);
        assert!(saved.completed_at.is_some());
        assert!(serialized.contains("A lecture"));
        assert!(!serialized.contains("timestamped transcript text"));
        assert!(!serialized.contains("rendered_dom"));
        assert!(!serialized.contains("transcript_status"));
        Ok(())
    }

    fn age_capture_request(home: &std::path::Path, seconds: i64) -> Result<()> {
        let store = ConfigStore::new(home);
        let mut config = store.load_or_create()?;
        let old = (Utc::now() - chrono::TimeDelta::seconds(seconds)).to_rfc3339();
        if let Some(status) = config.capture_request_status.as_mut() {
            status.requested_at = old.clone();
        }
        if let Some(pending) = config.pending_capture_request.as_mut() {
            pending.requested_at = old;
        }
        store.save(&config)
    }

    fn age_extension_hello(home: &std::path::Path, seconds: i64) -> Result<()> {
        let store = ConfigStore::new(home);
        let mut config = store.load_or_create()?;
        config.extension.last_handshake_at =
            Some((Utc::now() - chrono::TimeDelta::seconds(seconds)).to_rfc3339());
        store.save(&config)
    }

    fn install_extension_setup(home: &std::path::Path) -> Result<()> {
        let extension_path = home.join("sensor-extension");
        std::fs::create_dir_all(&extension_path)?;
        std::fs::write(extension_path.join("manifest.json"), "{}")?;
        std::fs::write(extension_path.join("starlee-config.json"), "{}")?;
        Ok(())
    }
}
