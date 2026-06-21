use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Article,
    Youtube,
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
    pub url: Option<String>,
    pub captured_at: DateTime<Utc>,
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
    pub url: Option<String>,
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
    pub access: Access,
    pub summary: String,
    pub bundle_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum GetResult {
    Own { record: Record },
    Borrowed { record: BorrowedRecord },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub captured_at: String,
    pub access: Access,
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
    pub vault_path: String,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub similarity: f32,
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
pub struct SetupReport {
    pub status: Status,
    pub bookmarklet: String,
    pub extension_path: String,
    pub extension_token: String,
    pub example_queries: Vec<String>,
}
