use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Once,
};

use anyhow::{Result, bail};
use bytemuck::cast_slice;
use rusqlite::{Connection, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;

use crate::{
    bundle::{BundleAudit, audit},
    embedding::{EMBEDDING_DIMENSION, Embedder},
    model::{Access, Record, SearchHit},
};

static REGISTER_VEC: Once = Once::new();

pub struct Index {
    path: PathBuf,
}

impl Index {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn connection(&self) -> Result<Connection> {
        REGISTER_VEC.call_once(|| unsafe {
            sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut std::ffi::c_char,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> std::ffi::c_int,
            >(sqlite3_vec_init as *const ())));
        });
        let connection = Connection::open(&self.path)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
        Ok(connection)
    }

    pub fn init(&self) -> Result<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS sources (
                id TEXT PRIMARY KEY, type TEXT NOT NULL, title TEXT NOT NULL,
                author TEXT, site TEXT, url TEXT, captured_at TEXT NOT NULL,
                published_at TEXT, access TEXT NOT NULL, summary TEXT NOT NULL,
                file_path TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                rowid INTEGER PRIMARY KEY, id TEXT UNIQUE NOT NULL,
                source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                ord INTEGER NOT NULL, char_start INTEGER, char_end INTEGER,
                t_start REAL, t_end REAL, access TEXT NOT NULL, text TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS chunk_fts USING fts5(
                text, content='chunks', content_rowid='rowid'
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS chunk_vectors USING vec0(
                embedding FLOAT[384]
            );
            DROP TRIGGER IF EXISTS chunks_ai;
            DROP TRIGGER IF EXISTS chunks_ad;
            CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunk_fts(rowid,text) VALUES(new.rowid,new.text);
            END;
            CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunk_fts(chunk_fts,rowid,text) VALUES('delete',old.rowid,old.text);
                DELETE FROM chunk_vectors WHERE rowid=old.rowid;
            END;",
        )?;
        Ok(())
    }

    pub fn upsert(&self, record: &Record, embedder: &dyn Embedder) -> Result<()> {
        self.init()?;
        let chunks = chunk_text(&record.body, 1800, 270);
        let texts = chunks
            .iter()
            .map(|chunk| chunk.2.clone())
            .collect::<Vec<_>>();
        let embeddings = embedder.embed_documents(&texts)?;
        if embeddings.len() != chunks.len() {
            bail!(
                "embedding count mismatch: expected {}, got {}",
                chunks.len(),
                embeddings.len()
            );
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction()?;
        tx.execute("DELETE FROM sources WHERE id=?1", [&record.metadata.id])?;
        tx.execute(
            "INSERT INTO sources(id,type,title,author,site,url,captured_at,published_at,access,summary,file_path)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![record.metadata.id, serde_json::to_value(&record.metadata.source_type)?.as_str(),
                record.metadata.title, record.metadata.author, record.metadata.site, record.metadata.url,
                record.metadata.captured_at.to_rfc3339(), record.metadata.published_at,
                serde_json::to_value(&record.metadata.access)?.as_str(), record.metadata.summary, record.file_path]
        )?;
        for (ord, (chunk, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            if embedding.len() != EMBEDDING_DIMENSION {
                bail!("embedding dimension mismatch for chunk {ord}");
            }
            tx.execute(
                "INSERT INTO chunks(id,source_id,ord,char_start,char_end,access,text) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![format!("{}:{ord}", record.metadata.id), record.metadata.id, ord, chunk.0, chunk.1,
                    serde_json::to_value(&record.metadata.access)?.as_str(), chunk.2]
            )?;
            let rowid = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO chunk_vectors(rowid,embedding) VALUES(?1,?2)",
                params![rowid, cast_slice::<f32, u8>(&embedding)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn rebuild(&self, records: &[Record], embedder: &dyn Embedder) -> Result<()> {
        for path in [
            self.path.clone(),
            PathBuf::from(format!("{}-wal", self.path.display())),
            PathBuf::from(format!("{}-shm", self.path.display())),
        ] {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        self.init()?;
        for record in records {
            self.upsert(record, embedder)?;
        }
        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
        embedder: &dyn Embedder,
    ) -> Result<Vec<SearchHit>> {
        self.init()?;
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.connection()?;
        let candidate_limit = limit.saturating_mul(8).max(limit);
        let mut candidates: HashMap<String, (SearchHit, f64)> = HashMap::new();
        self.collect_fts(&connection, query, candidate_limit, &mut candidates)?;
        let query_embedding = embedder.embed_query(query)?;
        self.collect_vectors(
            &connection,
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

    fn collect_fts(
        &self,
        connection: &Connection,
        query: &str,
        limit: usize,
        candidates: &mut HashMap<String, (SearchHit, f64)>,
    ) -> Result<()> {
        let mut statement = connection.prepare(
            "SELECT s.id,s.title,s.url,s.captured_at,s.access,
                    snippet(chunk_fts,0,'[',']',' … ',24),s.file_path,bm25(chunk_fts)
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
        &self,
        connection: &Connection,
        query_embedding: &[f32],
        limit: usize,
        candidates: &mut HashMap<String, (SearchHit, f64)>,
    ) -> Result<()> {
        let mut statement = connection.prepare(
            "SELECT s.id,s.title,s.url,s.captured_at,s.access,c.text,s.file_path,v.distance
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

    pub fn get(&self, id: &str) -> Result<Option<PathBuf>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT file_path FROM sources WHERE id=?1")?;
        let mut rows = statement.query([id])?;
        Ok(rows
            .next()?
            .map(|row| row.get::<_, String>(0))
            .transpose()?
            .map(PathBuf::from))
    }

    pub fn get_by_url(&self, url: &str) -> Result<Option<PathBuf>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement =
            connection.prepare("SELECT file_path FROM sources WHERE url=?1 LIMIT 1")?;
        let mut rows = statement.query([url])?;
        Ok(rows
            .next()?
            .map(|row| row.get::<_, String>(0))
            .transpose()?
            .map(PathBuf::from))
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<SearchHit>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id,title,url,captured_at,access,summary,file_path FROM sources ORDER BY captured_at DESC LIMIT ?1")?;
        let rows = statement.query_map([limit], |row| {
            let access: String = row.get(4)?;
            Ok(SearchHit {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                captured_at: row.get(3)?,
                access: if access == "public" {
                    Access::Public
                } else {
                    Access::Restricted
                },
                snippet: row.get(5)?,
                file_path: row.get(6)?,
                score: 0.0,
                source: "own".into(),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn counts(&self) -> Result<(u64, u64)> {
        self.init()?;
        let connection = self.connection()?;
        Ok((
            connection.query_row("SELECT count(*) FROM sources", [], |r| r.get(0))?,
            connection.query_row("SELECT count(*) FROM chunks", [], |r| r.get(0))?,
        ))
    }

    pub fn export_bundle(&self, path: &Path, include_public_bodies: bool) -> Result<BundleAudit> {
        self.init()?;
        if path.exists() {
            bail!("refusing to overwrite existing bundle: {}", path.display());
        }
        let temporary = path.with_extension("starlee.tmp");
        if temporary.exists() {
            std::fs::remove_file(&temporary)?;
        }
        let connection = self.connection()?;
        connection.execute(
            "ATTACH DATABASE ?1 AS bundle",
            [temporary.display().to_string()],
        )?;
        let result = (|| -> Result<()> {
            connection.execute_batch(
                "CREATE TABLE bundle.bundle_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                 INSERT INTO bundle.bundle_meta VALUES('format','starlee-share-v1');
                 CREATE TABLE bundle.sources(
                    id TEXT PRIMARY KEY,type TEXT NOT NULL,title TEXT NOT NULL,author TEXT,site TEXT,url TEXT,
                    captured_at TEXT NOT NULL,published_at TEXT,access TEXT NOT NULL,summary TEXT NOT NULL
                 );
                 CREATE TABLE bundle.chunks(
                    id TEXT PRIMARY KEY,source_id TEXT NOT NULL,ord INTEGER NOT NULL,access TEXT NOT NULL,
                    text TEXT,embedding BLOB NOT NULL
                 );
                 INSERT INTO bundle.sources
                    SELECT id,type,title,author,site,url,captured_at,published_at,access,summary FROM main.sources;"
            )?;
            connection.execute(
                "INSERT INTO bundle.chunks(id,source_id,ord,access,text,embedding)
                 SELECT c.id,c.source_id,c.ord,c.access,
                    CASE WHEN c.access='public' AND ?1 THEN c.text ELSE NULL END,
                    v.embedding
                 FROM main.chunks c JOIN main.chunk_vectors v ON v.rowid=c.rowid",
                [include_public_bodies],
            )?;
            Ok(())
        })();
        connection.execute_batch("DETACH DATABASE bundle")?;
        result?;
        let preflight = audit(&temporary)?;
        if !preflight.valid {
            bail!("bundle audit blocked export: restricted body detected");
        }
        std::fs::rename(&temporary, path)?;
        audit(path)
    }
}

fn escape_fts(word: &str) -> String {
    format!("\"{}\"", word.replace('"', "\"\""))
}

fn map_search_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<SearchHit> {
    let access: String = row.get(4)?;
    Ok(SearchHit {
        id: row.get(0)?,
        title: row.get(1)?,
        url: row.get(2)?,
        captured_at: row.get(3)?,
        access: if access == "public" {
            Access::Public
        } else {
            Access::Restricted
        },
        snippet: row.get(5)?,
        file_path: row.get(6)?,
        score: 0.0,
        source: "own".into(),
    })
}

fn chunk_text(text: &str, max_chars: usize, overlap: usize) -> Vec<(usize, usize, String)> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        while !text.is_char_boundary(start) {
            start += 1;
        }
        let mut end = (start + max_chars).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.len()
            && let Some(boundary) = text[start..end].rfind(char::is_whitespace)
            && boundary > max_chars / 2
        {
            end = start + boundary;
        }
        chunks.push((start, end, text[start..end].trim().to_owned()));
        if end == text.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }
    chunks
}

#[allow(dead_code)]
fn _path_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_long_text_with_overlap() {
        let text = "word ".repeat(1000);
        let chunks = chunk_text(&text, 1000, 150);
        assert!(chunks.len() > 4);
        assert_eq!(chunks[0].0, 0);
    }
}
