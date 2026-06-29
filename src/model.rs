use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{CaptureDiagnosticEvent, CaptureRequestStatus};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Article,
    Youtube,
    #[serde(rename = "spotify_episode")]
    SpotifyEpisode,
    #[default]
    Note,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Public,
    #[default]
    Restricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub id: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    pub captured_at: DateTime<Utc>,
    #[serde(default)]
    pub consumed_at: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub video_id: Option<String>,
    #[serde(default)]
    pub word_count: Option<usize>,
    pub access: Access,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub spotify_episode_id: Option<String>,
    #[serde(default)]
    pub spotify_show_id: Option<String>,
    #[serde(default)]
    pub show: Option<String>,
    #[serde(default)]
    pub listen_duration_s: Option<u64>,
    #[serde(default)]
    pub listen_progress_pct: Option<u8>,
    #[serde(default)]
    pub transcript_status: Option<String>,
    #[serde(default)]
    pub transcript_source: Option<String>,
    #[serde(default)]
    pub transcript_reason: Option<String>,
    #[serde(default)]
    pub matched_youtube_id: Option<String>,
    #[serde(default)]
    pub linked_youtube_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureInput {
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub source_type: SourceType,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub consumed_at: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub video_id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub spotify_episode_id: Option<String>,
    #[serde(default)]
    pub spotify_show_id: Option<String>,
    #[serde(default)]
    pub show: Option<String>,
    #[serde(default)]
    pub listen_duration_s: Option<u64>,
    #[serde(default)]
    pub listen_progress_pct: Option<u8>,
    #[serde(default)]
    pub transcript_status: Option<String>,
    #[serde(default)]
    pub transcript_source: Option<String>,
    #[serde(default)]
    pub transcript_reason: Option<String>,
    #[serde(default)]
    pub matched_youtube_id: Option<String>,
    #[serde(default)]
    pub linked_youtube_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl CaptureInput {
    pub fn new(
        title: impl Into<String>,
        text: impl Into<String>,
        source_type: SourceType,
        access: Access,
    ) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            source_type,
            access,
            author: None,
            site: None,
            source: None,
            url: None,
            consumed_at: None,
            published_at: None,
            duration: None,
            video_id: None,
            summary: None,
            tags: Vec::new(),
            topics: Vec::new(),
            spotify_episode_id: None,
            spotify_show_id: None,
            show: None,
            listen_duration_s: None,
            listen_progress_pct: None,
            transcript_status: None,
            transcript_source: None,
            transcript_reason: None,
            matched_youtube_id: None,
            linked_youtube_id: None,
            description: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub metadata: Frontmatter,
    pub body: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorrowedRecord {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub captured_at: String,
    #[serde(default)]
    pub consumed_at: Option<String>,
    pub access: Access,
    pub summary: String,
    pub bundle_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum GetResult {
    Own { record: Box<Record> },
    Borrowed { record: BorrowedRecord },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    pub url: Option<String>,
    pub captured_at: String,
    #[serde(default)]
    pub consumed_at: Option<String>,
    pub access: Access,
    #[serde(default)]
    pub topics: Vec<String>,
    pub snippet: String,
    pub file_path: String,
    pub score: f64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub chunks: Vec<QueryChunk>,
    pub total_retrieved: usize,
    pub relevance_floor_hit: bool,
    pub query_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryChunk {
    pub index: usize,
    pub title: String,
    pub url: Option<String>,
    pub domain: Option<String>,
    pub captured_at: String,
    #[serde(default)]
    pub consumed_at: Option<String>,
    pub vault_path: String,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopicCount {
    pub topic: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportReport {
    pub imported: Vec<ImportedDocument>,
    pub skipped: Vec<SkippedDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    pub path: String,
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedDocument {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusOverview {
    pub total_captures: usize,
    pub earliest_capture: Option<String>,
    pub latest_capture: Option<String>,
    pub top_topics: Vec<String>,
    pub source_breakdown: std::collections::BTreeMap<String, f64>,
    pub top_domains: Vec<String>,
    pub top_authors: Vec<String>,
    pub overview_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    Own,
    Borrowed,
    #[default]
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub home: String,
    pub vault: String,
    pub index: String,
    pub capture_count: u64,
    pub chunk_count: u64,
    pub retrieval: String,
    pub capture_endpoint: String,
    pub capture_token_path: String,
    pub youtube_metadata_configured: bool,
    pub borrowed_bundle_count: usize,
    pub spotify_oauth_configured: bool,
    pub spotify_account: Option<String>,
    pub spotify_last_synced_at: Option<String>,
    pub spotify_next_sync_at: Option<String>,
    pub schema_version: i64,
    pub embedding_model_current: String,
    pub chunks_stale: u64,
    pub bridge_health: BridgeHealth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeHealth {
    pub ok: bool,
    pub chrome_setup: ChromeSetupStatus,
    pub browser_setup: ChromeSetupStatus,
    pub extension_setup_present: bool,
    pub extension_config_present: bool,
    pub checked_in_recently: bool,
    pub browser: Option<String>,
    pub extension_version: Option<String>,
    pub extension_build: Option<String>,
    pub can_capture_active_tab: bool,
    pub last_hello_at: Option<String>,
    pub last_request_status: Option<String>,
    pub last_failure_reason: Option<String>,
    pub last_failure_message: Option<String>,
    pub recommended_next_action: String,
    #[serde(default)]
    pub recent_diagnostics: Vec<CaptureDiagnosticEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromeSetupStatus {
    pub installed: bool,
    pub checked_in_recently: bool,
    pub permission_needed: bool,
    pub capture_test_passed: bool,
    pub capture_test_passed_at: Option<String>,
    pub state: String,
    pub detail: String,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureTraceReport {
    pub trace_version: u32,
    pub request_id: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub browser: Option<String>,
    #[serde(default)]
    pub requested_browser: Option<String>,
    #[serde(default)]
    pub handling_browser: Option<String>,
    pub extension_build: Option<String>,
    pub desktop_build: Option<String>,
    pub result_code: Option<String>,
    pub terminal_status: Option<String>,
    pub user_safe_message: Option<String>,
    pub failure_step: Option<String>,
    pub recommended_next_action: String,
    pub next_action: String,
    pub last_extension_check_in: LastExtensionCheckInState,
    pub runtime: RuntimeIdentity,
    #[serde(default)]
    pub request_status: Option<CaptureRequestStatus>,
    pub events: Vec<CaptureDiagnosticEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastExtensionCheckInState {
    pub checked_in_recently: bool,
    pub can_capture_active_tab: bool,
    pub last_hello_at: Option<String>,
    pub browser: Option<String>,
    pub extension_version: Option<String>,
    pub extension_build: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeIdentity {
    pub starlee_version: String,
    pub app_build_identifier: Option<String>,
    pub browser: Option<String>,
    pub extension_version: Option<String>,
    pub extension_build: Option<String>,
    pub git_commit: Option<String>,
    pub app_path: Option<String>,
    pub binary_path: Option<String>,
    pub source_repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub status: Status,
    pub checks: Vec<DoctorCheck>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncEvent {
    pub id: i64,
    pub timestamp: String,
    #[serde(default)]
    pub episode_id: Option<String>,
    #[serde(default)]
    pub episode_title: Option<String>,
    #[serde(default)]
    pub show_name: Option<String>,
    pub stage_reached: String,
    pub outcome: String,
    pub reason_code: String,
    pub explanation: String,
    #[serde(default)]
    pub underlying_error: Option<String>,
    #[serde(default)]
    pub listen_duration_s: Option<u64>,
    #[serde(default)]
    pub threshold_s: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyReasonCount {
    pub reason_code: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncLog {
    pub events: Vec<SpotifySyncEvent>,
    #[serde(default)]
    pub coverage_gap: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupReport {
    pub status: Status,
    pub bookmarklet: String,
    pub extension_path: String,
    pub extension_token: String,
    pub extension_token_fingerprint: String,
    pub example_queries: Vec<String>,
}
