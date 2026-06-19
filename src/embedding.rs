use std::{path::PathBuf, sync::Mutex};

use anyhow::{Context, Result, bail};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

pub const EMBEDDING_DIMENSION: usize = 384;

pub trait Embedder: Send + Sync {
    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn embed_query(&self, text: &str) -> Result<Vec<f32>>;
    fn name(&self) -> &'static str;
}

pub struct FastEmbedder {
    cache_dir: PathBuf,
    model: Mutex<Option<TextEmbedding>>,
}

impl FastEmbedder {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            model: Mutex::new(None),
        }
    }

    fn with_model<T>(&self, operation: impl FnOnce(&mut TextEmbedding) -> Result<T>) -> Result<T> {
        let mut guard = self
            .model
            .lock()
            .map_err(|_| anyhow::anyhow!("embedding model lock poisoned"))?;
        if guard.is_none() {
            std::fs::create_dir_all(&self.cache_dir)?;
            let options = TextInitOptions::new(EmbeddingModel::BGESmallENV15Q)
                .with_cache_dir(self.cache_dir.clone())
                .with_show_download_progress(true);
            *guard = Some(
                TextEmbedding::try_new(options)
                    .context("initialize local BGE-small embedding model")?,
            );
        }
        operation(guard.as_mut().expect("model initialized"))
    }

    fn validate(embeddings: &[Vec<f32>]) -> Result<()> {
        if let Some(vector) = embeddings
            .iter()
            .find(|vector| vector.len() != EMBEDDING_DIMENSION)
        {
            bail!(
                "embedding dimension mismatch: expected {EMBEDDING_DIMENSION}, got {}",
                vector.len()
            );
        }
        Ok(())
    }
}

impl Embedder for FastEmbedder {
    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let documents = texts
            .iter()
            .map(|text| format!("passage: {text}"))
            .collect::<Vec<_>>();
        let embeddings = self.with_model(|model| model.embed(documents, None))?;
        Self::validate(&embeddings)?;
        Ok(embeddings)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
        let mut embeddings =
            self.with_model(|model| model.embed(vec![format!("query: {text}")], None))?;
        Self::validate(&embeddings)?;
        embeddings
            .pop()
            .context("embedding model returned no query vector")
    }

    fn name(&self) -> &'static str {
        "BAAI/bge-small-en-v1.5-quantized"
    }
}
