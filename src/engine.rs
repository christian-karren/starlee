use std::{path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    bundle::{self, BundleAudit},
    config::{ConfigStore, LocalConfig},
    embedding::{Embedder, FastEmbedder},
    index::Index,
    model::{CaptureInput, GetResult, Record, SearchHit, SearchScope, SetupReport, Status},
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
        let extension_path = sensor_assets::install(&self.home)?;
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
