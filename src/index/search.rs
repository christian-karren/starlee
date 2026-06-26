//! Hybrid FTS and vector retrieval helpers used by the Index facade.

use std::collections::HashMap;

use anyhow::Result;
use bytemuck::cast_slice;
use rusqlite::{Connection, params};

use crate::{
    embedding::Embedder,
    model::{Access, QueryChunk, SearchHit},
};

pub(crate) fn search(
    connection: &Connection,
    query: &str,
    limit: usize,
    embedder: &dyn Embedder,
) -> Result<Vec<SearchHit>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let candidate_limit = limit.saturating_mul(8).max(limit);
    let mut candidates: HashMap<String, (SearchHit, f64)> = HashMap::new();
    collect_fts(connection, query, candidate_limit, &mut candidates)?;
    let query_embedding = embedder.embed_query(query)?;
    collect_vectors(
        connection,
        &query_embedding,
        candidate_limit,
        &mut candidates,
    )?;
    let mut hits = candidates
        .into_values()
        .map(|(mut hit, score)| {
            hit.score = score;
            hit
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.captured_at.cmp(&a.captured_at))
    });
    hits.truncate(limit);
    Ok(hits)
}

pub(crate) fn query_chunks(
    connection: &Connection,
    query: &str,
    limit: usize,
    embedder: &dyn Embedder,
) -> Result<Vec<QueryChunk>> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let candidate_limit = limit.saturating_mul(8).max(limit);
    let mut candidates: HashMap<i64, (QueryChunk, f64)> = HashMap::new();
    collect_query_chunk_fts(connection, query, candidate_limit, &mut candidates)?;
    let query_embedding = embedder.embed_query(query)?;
    collect_query_chunk_vectors(
        connection,
        &query_embedding,
        candidate_limit,
        &mut candidates,
    )?;
    let mut chunks = candidates
        .into_values()
        .map(|(mut chunk, score)| {
            chunk.similarity = chunk.similarity.max(score.min(1.0) as f32);
            chunk
        })
        .collect::<Vec<_>>();
    chunks.sort_by(|a, b| {
        b.similarity
            .total_cmp(&a.similarity)
            .then_with(|| b.captured_at.cmp(&a.captured_at))
    });
    chunks.truncate(limit);
    for (index, chunk) in chunks.iter_mut().enumerate() {
        chunk.index = index + 1;
    }
    Ok(chunks)
}

fn collect_query_chunk_fts(
    connection: &Connection,
    query: &str,
    limit: usize,
    candidates: &mut HashMap<i64, (QueryChunk, f64)>,
) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT c.rowid,s.title,s.url,s.site,s.captured_at,c.text,s.file_path,c.ord,bm25(chunk_fts),s.consumed_at
             FROM chunk_fts JOIN chunks c ON c.rowid=chunk_fts.rowid
             JOIN sources s ON s.id=c.source_id WHERE chunk_fts MATCH ?1
             ORDER BY bm25(chunk_fts),s.captured_at DESC LIMIT ?2",
    )?;
    let fts_query = query
        .split_whitespace()
        .map(escape_fts)
        .collect::<Vec<_>>()
        .join(" OR ");
    let rows = statement.query_map(params![fts_query, limit], map_query_chunk_fts)?;
    for (rank, row) in rows.enumerate() {
        let (rowid, mut chunk) = row?;
        let score = 0.45 / (60.0 + rank as f64 + 1.0);
        chunk.similarity = 0.9_f32 - (rank as f32 * 0.01).min(0.2);
        candidates
            .entry(rowid)
            .and_modify(|entry| {
                entry.1 += score;
                entry.0.similarity = entry.0.similarity.max(chunk.similarity);
            })
            .or_insert((chunk, score));
    }
    Ok(())
}

fn collect_query_chunk_vectors(
    connection: &Connection,
    query_embedding: &[f32],
    limit: usize,
    candidates: &mut HashMap<i64, (QueryChunk, f64)>,
) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT c.rowid,s.title,s.url,s.site,s.captured_at,c.text,s.file_path,c.ord,v.distance,s.consumed_at
             FROM chunk_vectors v JOIN chunks c ON c.rowid=v.rowid
             JOIN sources s ON s.id=c.source_id
             WHERE v.embedding MATCH ?1 AND k = ?2 ORDER BY v.distance",
    )?;
    let rows = statement.query_map(
        params![cast_slice::<f32, u8>(query_embedding), limit],
        |row| {
            let rowid: i64 = row.get(0)?;
            let distance: f32 = row.get(8)?;
            Ok((
                rowid,
                map_query_chunk(row, distance_to_similarity(distance))?,
            ))
        },
    )?;
    for (rank, row) in rows.enumerate() {
        let (rowid, chunk) = row?;
        let score = 0.55 / (60.0 + rank as f64 + 1.0);
        candidates
            .entry(rowid)
            .and_modify(|entry| {
                entry.1 += score;
                entry.0.similarity = entry.0.similarity.max(chunk.similarity);
            })
            .or_insert((chunk, score));
    }
    Ok(())
}

fn collect_fts(
    connection: &Connection,
    query: &str,
    limit: usize,
    candidates: &mut HashMap<String, (SearchHit, f64)>,
) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT s.id,s.title,s.type,s.site,s.url,s.captured_at,s.access,
                    snippet(chunk_fts,0,'[',']',' … ',24),s.file_path,bm25(chunk_fts),s.consumed_at
             FROM chunk_fts JOIN chunks c ON c.rowid=chunk_fts.rowid
             JOIN sources s ON s.id=c.source_id WHERE chunk_fts MATCH ?1
             ORDER BY bm25(chunk_fts),s.captured_at DESC LIMIT ?2",
    )?;
    let fts_query = query
        .split_whitespace()
        .map(escape_fts)
        .collect::<Vec<_>>()
        .join(" OR ");
    let rows = statement.query_map(params![fts_query, limit], map_search_hit)?;
    for (rank, row) in rows.enumerate() {
        let hit = row?;
        let score = 0.45 / (60.0 + rank as f64 + 1.0);
        candidates
            .entry(hit.id.clone())
            .and_modify(|entry| entry.1 += score)
            .or_insert((hit, score));
    }
    Ok(())
}

fn collect_vectors(
    connection: &Connection,
    query_embedding: &[f32],
    limit: usize,
    candidates: &mut HashMap<String, (SearchHit, f64)>,
) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT s.id,s.title,s.type,s.site,s.url,s.captured_at,s.access,c.text,s.file_path,v.distance,s.consumed_at
             FROM chunk_vectors v JOIN chunks c ON c.rowid=v.rowid
             JOIN sources s ON s.id=c.source_id
             WHERE v.embedding MATCH ?1 AND k = ?2 ORDER BY v.distance",
    )?;
    let rows = statement.query_map(
        params![cast_slice::<f32, u8>(query_embedding), limit],
        map_search_hit,
    )?;
    for (rank, row) in rows.enumerate() {
        let hit = row?;
        let score = 0.55 / (60.0 + rank as f64 + 1.0);
        candidates
            .entry(hit.id.clone())
            .and_modify(|entry| entry.1 += score)
            .or_insert((hit, score));
    }
    Ok(())
}

fn escape_fts(word: &str) -> String {
    format!("\"{}\"", word.replace('"', "\"\""))
}

fn map_search_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchHit> {
    let source_type: String = row.get(2)?;
    let access: String = row.get(6)?;
    Ok(SearchHit {
        id: row.get(0)?,
        title: row.get(1)?,
        source_type: serde_json::from_value(serde_json::Value::String(source_type))
            .unwrap_or_default(),
        site: row.get(3)?,
        author: None,
        url: row.get(4)?,
        captured_at: row.get(5)?,
        consumed_at: row.get(10)?,
        access: if access == "public" {
            Access::Public
        } else {
            Access::Restricted
        },
        topics: Vec::new(),
        snippet: row.get(7)?,
        file_path: row.get(8)?,
        score: 0.0,
        source: "own".into(),
    })
}

fn map_query_chunk_fts(row: &rusqlite::Row<'_>) -> rusqlite::Result<(i64, QueryChunk)> {
    let rowid: i64 = row.get(0)?;
    Ok((rowid, map_query_chunk(row, 0.0)?))
}

fn map_query_chunk(row: &rusqlite::Row<'_>, similarity: f32) -> rusqlite::Result<QueryChunk> {
    let url: Option<String> = row.get(2)?;
    let site: Option<String> = row.get(3)?;
    Ok(QueryChunk {
        index: 0,
        title: row.get(1)?,
        domain: domain_from(url.as_deref()).or(site),
        url,
        captured_at: row.get(4)?,
        consumed_at: row.get(9)?,
        vault_path: row.get(6)?,
        chunk_index: row.get::<_, i64>(7)? as usize,
        chunk_text: row.get(5)?,
        similarity,
    })
}

fn distance_to_similarity(distance: f32) -> f32 {
    1.0 / (1.0 + distance.max(0.0))
}

fn domain_from(value: Option<&str>) -> Option<String> {
    let url = url::Url::parse(value?).ok()?;
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_owned())
}
