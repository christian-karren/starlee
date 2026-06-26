use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};

use crate::model::{Access, BorrowedRecord, SearchHit};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAudit {
    pub path: String,
    pub source_count: u64,
    pub chunk_count: u64,
    pub restricted_body_count: u64,
    pub public_body_count: u64,
    pub valid: bool,
}

pub fn audit(path: &Path) -> Result<BundleAudit> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    let source_count = count(&connection, "SELECT count(*) FROM sources")?;
    let chunk_count = count(&connection, "SELECT count(*) FROM chunks")?;
    let restricted_body_count = count(
        &connection,
        "SELECT count(*) FROM chunks WHERE access='restricted' AND text IS NOT NULL",
    )?;
    let public_body_count = count(
        &connection,
        "SELECT count(*) FROM chunks WHERE access='public' AND text IS NOT NULL",
    )?;
    Ok(BundleAudit {
        path: path.display().to_string(),
        source_count,
        chunk_count,
        restricted_body_count,
        public_body_count,
        valid: integrity == "ok" && restricted_body_count == 0,
    })
}

pub fn validate(path: &Path) -> Result<BundleAudit> {
    let audit = audit(path).with_context(|| format!("audit bundle {}", path.display()))?;
    if !audit.valid {
        bail!(
            "bundle audit failed: {} restricted body chunks present",
            audit.restricted_body_count
        );
    }
    Ok(audit)
}

pub fn search(
    paths: &[PathBuf],
    query: &str,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let terms = query
        .split_whitespace()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    let mut candidates: HashMap<String, Candidate> = HashMap::new();
    for path in paths {
        validate(path)?;
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut statement = connection.prepare(
            "SELECT s.id,s.title,s.type,s.site,s.url,s.captured_at,s.access,s.summary,c.embedding
             FROM sources s JOIN chunks c ON c.source_id=s.id",
        )?;
        let bundle_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("borrowed")
            .to_owned();
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Vec<u8>>(8)?,
            ))
        })?;
        for row in rows {
            let (id, title, source_type, site, url, captured_at, access, summary, bytes) = row?;
            let vector = decode_vector(&bytes)?;
            let similarity = cosine(query_embedding, &vector);
            let lexical_text = format!("{title} {summary}").to_lowercase();
            let lexical = terms
                .iter()
                .filter(|term| lexical_text.contains(term.as_str()))
                .count();
            let key = format!("{}:{id}", path.display());
            let candidate = candidates.entry(key).or_insert_with(|| Candidate {
                hit: SearchHit {
                    id,
                    title,
                    source_type: serde_json::from_value(serde_json::Value::String(source_type))
                        .unwrap_or_default(),
                    site,
                    author: None,
                    url,
                    captured_at,
                    consumed_at: None,
                    access: if access == "public" {
                        Access::Public
                    } else {
                        Access::Restricted
                    },
                    topics: Vec::new(),
                    snippet: summary,
                    file_path: format!("{}#source", path.display()),
                    score: 0.0,
                    source: format!("borrowed:{bundle_name}"),
                },
                semantic: f32::NEG_INFINITY,
                lexical,
            });
            candidate.semantic = candidate.semantic.max(similarity);
            candidate.lexical = candidate.lexical.max(lexical);
        }
    }

    let mut semantic_order = candidates.keys().cloned().collect::<Vec<_>>();
    semantic_order.sort_by(|a, b| candidates[b].semantic.total_cmp(&candidates[a].semantic));
    for (rank, key) in semantic_order.into_iter().enumerate() {
        candidates
            .get_mut(&key)
            .expect("candidate exists")
            .hit
            .score += 0.55 / (61.0 + rank as f64);
    }
    let mut lexical_order = candidates
        .iter()
        .filter(|(_, candidate)| candidate.lexical > 0)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    lexical_order.sort_by(|a, b| candidates[b].lexical.cmp(&candidates[a].lexical));
    for (rank, key) in lexical_order.into_iter().enumerate() {
        candidates
            .get_mut(&key)
            .expect("candidate exists")
            .hit
            .score += 0.45 / (61.0 + rank as f64);
    }
    let mut hits = candidates
        .into_values()
        .map(|candidate| candidate.hit)
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    Ok(hits)
}

pub fn get(paths: &[PathBuf], id: &str) -> Result<Option<BorrowedRecord>> {
    for path in paths {
        validate(path)?;
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut statement = connection
            .prepare("SELECT id,title,url,captured_at,access,summary FROM sources WHERE id=?1")?;
        let mut rows = statement.query([id])?;
        if let Some(row) = rows.next()? {
            let access: String = row.get(4)?;
            return Ok(Some(BorrowedRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                captured_at: row.get(3)?,
                consumed_at: None,
                access: if access == "public" {
                    Access::Public
                } else {
                    Access::Restricted
                },
                summary: row.get(5)?,
                bundle_path: path.display().to_string(),
            }));
        }
    }
    Ok(None)
}

struct Candidate {
    hit: SearchHit,
    semantic: f32,
    lexical: usize,
}

fn count(connection: &Connection, sql: &str) -> Result<u64> {
    connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(Into::into)
}

fn decode_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        bail!("invalid vector byte length")
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn cosine(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return -1.0;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        -1.0
    } else {
        dot / (left_norm * right_norm)
    }
}
