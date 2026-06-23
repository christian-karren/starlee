use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Once,
};

use anyhow::{Result, bail};
use bytemuck::cast_slice;
use rusqlite::{Connection, OptionalExtension, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;

use chrono::{DateTime, Utc};

use crate::{
    bundle::{BundleAudit, audit},
    chunking::{ChunkOptions, chunk_text},
    embedding::{EMBEDDING_DIMENSION, Embedder},
    model::{Access, QueryChunk, Record, SearchHit, SpotifyReasonCount, SpotifySyncEvent},
};

static REGISTER_VEC: Once = Once::new();

pub struct Index {
    path: PathBuf,
}

pub const CURRENT_SCHEMA_VERSION: i64 = 4;
const SCHEMA_VERSION_KEY: &str = "version";

#[derive(Clone, Copy)]
struct Migration {
    version: i64,
    description: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "initialize schema metadata",
        sql: "SELECT 1;",
    },
    Migration {
        version: 2,
        description: "add consumed_at to sources",
        sql: "ALTER TABLE sources ADD COLUMN consumed_at TEXT;",
    },
    Migration {
        version: 3,
        description: "add embedding_model to chunks",
        sql: "ALTER TABLE chunks ADD COLUMN embedding_model TEXT NOT NULL DEFAULT '';",
    },
    Migration {
        version: 4,
        description: "add sync readiness placeholders",
        sql: "ALTER TABLE sources ADD COLUMN device_id TEXT;
              CREATE TABLE IF NOT EXISTS sync_state(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    },
];

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct MigrationReport {
    pub schema_version: i64,
    pub applied: Vec<AppliedMigration>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: i64,
    pub description: String,
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
        let mut connection = self.connection()?;
        create_base_schema(&connection)?;
        run_migrations(&mut connection)?;
        Ok(())
    }

    pub fn migrate(&self) -> Result<MigrationReport> {
        let mut connection = self.connection()?;
        create_base_schema(&connection)?;
        run_migrations(&mut connection)
    }

    pub fn schema_version(&self) -> Result<i64> {
        self.init()?;
        let connection = self.connection()?;
        schema_version(&connection)
    }

    pub fn upsert(&self, record: &Record, embedder: &dyn Embedder) -> Result<()> {
        self.init()?;
        let chunks = chunk_text(
            &record.body,
            &record.metadata.source_type,
            ChunkOptions::default(),
        );
        let texts = chunks
            .iter()
            .map(|chunk| chunk.text.clone())
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
            "INSERT INTO sources(id,type,title,author,site,url,captured_at,published_at,access,summary,file_path,consumed_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![record.metadata.id, serde_json::to_value(&record.metadata.source_type)?.as_str(),
                record.metadata.title, record.metadata.author, record.metadata.site, record.metadata.url,
                record.metadata.captured_at.to_rfc3339(), record.metadata.published_at,
                serde_json::to_value(&record.metadata.access)?.as_str(), record.metadata.summary, record.file_path,
                record.metadata.consumed_at]
        )?;
        for (ord, (chunk, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            if embedding.len() != EMBEDDING_DIMENSION {
                bail!("embedding dimension mismatch for chunk {ord}");
            }
            tx.execute(
                "INSERT INTO chunks(id,source_id,ord,char_start,char_end,t_start,t_end,access,text,embedding_model)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![format!("{}:{ord}", record.metadata.id), record.metadata.id, ord, chunk.char_start,
                    chunk.char_end, chunk.t_start, chunk.t_end, serde_json::to_value(&record.metadata.access)?.as_str(),
                    chunk.text, embedder.name()]
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

    pub fn reembed_stale(&self, records: &[Record], embedder: &dyn Embedder) -> Result<usize> {
        self.init()?;
        let stale_ids = self.stale_source_ids(embedder.name())?;
        if stale_ids.is_empty() {
            return Ok(0);
        }
        let stale = stale_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut updated = 0;
        for record in records {
            if stale.contains(&record.metadata.id) {
                self.upsert(record, embedder)?;
                updated += 1;
            }
        }
        Ok(updated)
    }

    fn stale_source_ids(&self, model: &str) -> Result<Vec<String>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT source_id FROM chunks
             WHERE embedding_model IS NULL OR embedding_model != ?1
             ORDER BY source_id",
        )?;
        let rows = statement.query_map([model], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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

    pub fn query_chunks(
        &self,
        query: &str,
        limit: usize,
        embedder: &dyn Embedder,
    ) -> Result<Vec<QueryChunk>> {
        self.init()?;
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let connection = self.connection()?;
        let query_embedding = embedder.embed_query(query)?;
        let mut statement = connection.prepare(
            "SELECT s.title,s.url,s.site,s.captured_at,c.text,s.file_path,c.ord,v.distance,s.consumed_at
             FROM chunk_vectors v JOIN chunks c ON c.rowid=v.rowid
             JOIN sources s ON s.id=c.source_id
             WHERE v.embedding MATCH ?1 AND k = ?2 ORDER BY v.distance",
        )?;
        let rows = statement.query_map(
            params![cast_slice::<f32, u8>(&query_embedding), limit],
            |row| {
                let url: Option<String> = row.get(1)?;
                let site: Option<String> = row.get(2)?;
                let distance: f32 = row.get(7)?;
                Ok(QueryChunk {
                    index: 0,
                    title: row.get(0)?,
                    domain: domain_from(url.as_deref()).or(site),
                    url,
                    captured_at: row.get(3)?,
                    consumed_at: row.get(8)?,
                    vault_path: row.get(5)?,
                    chunk_index: row.get::<_, i64>(6)? as usize,
                    chunk_text: row.get(4)?,
                    similarity: distance_to_similarity(distance),
                })
            },
        )?;
        rows.enumerate()
            .map(|(index, row)| {
                let mut chunk = row?;
                chunk.index = index + 1;
                Ok::<QueryChunk, rusqlite::Error>(chunk)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
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
        &self,
        connection: &Connection,
        query_embedding: &[f32],
        limit: usize,
        candidates: &mut HashMap<String, (SearchHit, f64)>,
    ) -> Result<()> {
        let mut statement = connection.prepare(
            "SELECT s.id,s.title,s.url,s.captured_at,s.access,c.text,s.file_path,v.distance,s.consumed_at
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
            "SELECT id,title,url,captured_at,access,summary,file_path,consumed_at FROM sources ORDER BY COALESCE(consumed_at,captured_at) DESC LIMIT ?1")?;
        let rows = statement.query_map([limit], |row| {
            let access: String = row.get(4)?;
            Ok(SearchHit {
                id: row.get(0)?,
                title: row.get(1)?,
                url: row.get(2)?,
                captured_at: row.get(3)?,
                consumed_at: row.get(7)?,
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

    pub fn insert_spotify_sync_event(&self, event: &SpotifySyncEvent) -> Result<()> {
        self.init()?;
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO spotify_sync_events(
                timestamp,episode_id,episode_title,show_name,stage_reached,outcome,reason_code,
                explanation,underlying_error,listen_duration_s,threshold_s
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                event.timestamp,
                event.episode_id,
                event.episode_title,
                event.show_name,
                event.stage_reached,
                event.outcome,
                event.reason_code,
                event.explanation,
                event.underlying_error,
                event.listen_duration_s,
                event.threshold_s
            ],
        )?;
        Ok(())
    }

    pub fn spotify_sync_events(
        &self,
        limit: usize,
        show_skips: bool,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<SpotifySyncEvent>> {
        self.init()?;
        let connection = self.connection()?;
        let since = since
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "0000-01-01T00:00:00Z".into());
        let outcome_filter = if show_skips {
            "1=1"
        } else {
            "outcome != 'skipped'"
        };
        let sql = format!(
            "SELECT id,timestamp,episode_id,episode_title,show_name,stage_reached,outcome,
                    reason_code,explanation,underlying_error,listen_duration_s,threshold_s
             FROM spotify_sync_events
             WHERE timestamp >= ?1 AND {outcome_filter}
             ORDER BY timestamp DESC, id DESC LIMIT ?2"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params![since, limit], map_spotify_sync_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn spotify_last_successful_poll_at(&self) -> Result<Option<String>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT timestamp FROM spotify_sync_events
             WHERE reason_code IN ('nothing_playing','detected_ok','captured_ok','no_feed_transcript','duplicate_already_captured','insufficient_listen_time')
             ORDER BY timestamp DESC LIMIT 1",
        )?;
        let mut rows = statement.query([])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn spotify_last_capture_at(&self) -> Result<Option<String>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT timestamp FROM spotify_sync_events
             WHERE reason_code IN ('captured_ok','no_feed_transcript') ORDER BY timestamp DESC LIMIT 1",
        )?;
        let mut rows = statement.query([])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn spotify_recent_reason_counts(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<SpotifyReasonCount>> {
        self.init()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT reason_code,count(*) FROM spotify_sync_events
             WHERE timestamp >= ?1 AND outcome IN ('skipped','failed')
             GROUP BY reason_code ORDER BY count(*) DESC, reason_code",
        )?;
        let rows = statement.query_map([since.to_rfc3339()], |row| {
            Ok(SpotifyReasonCount {
                reason_code: row.get(0)?,
                count: row.get::<_, i64>(1)? as usize,
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

    pub fn stale_chunk_count(&self, model: &str) -> Result<u64> {
        self.init()?;
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT count(*) FROM chunks WHERE embedding_model IS NULL OR embedding_model != ?1",
                [model],
                |r| r.get(0),
            )
            .map_err(Into::into)
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

fn map_spotify_sync_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<SpotifySyncEvent> {
    Ok(SpotifySyncEvent {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        episode_id: row.get(2)?,
        episode_title: row.get(3)?,
        show_name: row.get(4)?,
        stage_reached: row.get(5)?,
        outcome: row.get(6)?,
        reason_code: row.get(7)?,
        explanation: row.get(8)?,
        underlying_error: row.get(9)?,
        listen_duration_s: row.get::<_, Option<i64>>(10)?.map(|value| value as u64),
        threshold_s: row.get::<_, Option<i64>>(11)?.map(|value| value as u64),
    })
}

fn create_base_schema(connection: &Connection) -> Result<()> {
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
        CREATE TABLE IF NOT EXISTS spotify_sync_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            episode_id TEXT,
            episode_title TEXT,
            show_name TEXT,
            stage_reached TEXT NOT NULL,
            outcome TEXT NOT NULL,
            reason_code TEXT NOT NULL,
            explanation TEXT NOT NULL,
            underlying_error TEXT,
            listen_duration_s INTEGER,
            threshold_s INTEGER
        );
        CREATE INDEX IF NOT EXISTS spotify_sync_events_timestamp_idx
            ON spotify_sync_events(timestamp DESC);
        CREATE INDEX IF NOT EXISTS spotify_sync_events_reason_idx
            ON spotify_sync_events(reason_code, timestamp DESC);
        CREATE INDEX IF NOT EXISTS spotify_sync_events_episode_idx
            ON spotify_sync_events(episode_id, timestamp DESC);
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

fn run_migrations(connection: &mut Connection) -> Result<MigrationReport> {
    run_migrations_with(connection, MIGRATIONS)
}

fn run_migrations_with(
    connection: &mut Connection,
    migrations: &[Migration],
) -> Result<MigrationReport> {
    validate_migrations(migrations)?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    let mut current_version = schema_version(connection)?;
    let mut applied = Vec::new();
    for migration in migrations {
        if migration.version <= current_version {
            continue;
        }
        eprintln!(
            "Applying migration {}: {}",
            migration.version, migration.description
        );
        let tx = connection.unchecked_transaction()?;
        if let Err(error) = tx.execute_batch(migration.sql) {
            tx.rollback()?;
            return Err(anyhow::anyhow!(
                "Migration {} failed: {error}. Database unchanged.",
                migration.version
            ));
        }
        set_schema_version(&tx, migration.version)?;
        tx.commit()?;
        current_version = migration.version;
        applied.push(AppliedMigration {
            version: migration.version,
            description: migration.description.to_owned(),
        });
    }
    Ok(MigrationReport {
        schema_version: current_version,
        applied,
    })
}

fn validate_migrations(migrations: &[Migration]) -> Result<()> {
    for (index, migration) in migrations.iter().enumerate() {
        let expected = index as i64 + 1;
        if migration.version != expected {
            bail!(
                "invalid migration sequence: expected version {expected}, got {}",
                migration.version
            );
        }
    }
    Ok(())
}

fn schema_version(connection: &Connection) -> Result<i64> {
    Ok(connection
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM schema_meta WHERE key=?1",
            [SCHEMA_VERSION_KEY],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn set_schema_version(connection: &Connection, version: i64) -> Result<()> {
    connection.execute(
        "INSERT OR REPLACE INTO schema_meta(key,value) VALUES(?1,?2)",
        params![SCHEMA_VERSION_KEY, version.to_string()],
    )?;
    Ok(())
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
        consumed_at: row.get(8)?,
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

fn distance_to_similarity(distance: f32) -> f32 {
    1.0 / (1.0 + distance.max(0.0))
}

fn domain_from(value: Option<&str>) -> Option<String> {
    let url = url::Url::parse(value?).ok()?;
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        chunking::fixed_window_chunks,
        embedding::{EMBEDDING_DIMENSION, Embedder},
        model::{Access, Frontmatter, SourceType},
    };
    use chrono::Utc;

    struct StaticEmbedder {
        name: &'static str,
    }

    impl Embedder for StaticEmbedder {
        fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|text| test_vector(text)).collect())
        }

        fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(test_vector(text))
        }

        fn name(&self) -> &'static str {
            self.name
        }
    }

    fn test_vector(text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; EMBEDDING_DIMENSION];
        vector[0] = 1.0;
        vector[1] = text.len() as f32;
        vector
    }

    #[test]
    fn chunks_long_text_with_overlap() {
        let text = "word ".repeat(1000);
        let chunks = fixed_window_chunks(&text, 1000, 150);
        assert!(chunks.len() > 4);
        assert_eq!(chunks[0].char_start, 0);
    }

    #[test]
    fn stores_and_queries_spotify_sync_events() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        index.insert_spotify_sync_event(&SpotifySyncEvent {
            id: 0,
            timestamp: "2026-06-22T08:00:00Z".into(),
            episode_id: Some("ep1".into()),
            episode_title: Some("Episode One".into()),
            show_name: Some("Show".into()),
            stage_reached: "detected".into(),
            outcome: "skipped".into(),
            reason_code: "insufficient_listen_time".into(),
            explanation: "Not captured: only 12s of listen time.".into(),
            underlying_error: None,
            listen_duration_s: Some(12),
            threshold_s: Some(600),
        })?;
        index.insert_spotify_sync_event(&SpotifySyncEvent {
            id: 0,
            timestamp: "2026-06-22T08:10:00Z".into(),
            episode_id: Some("ep2".into()),
            episode_title: Some("Episode Two".into()),
            show_name: Some("Show".into()),
            stage_reached: "captured".into(),
            outcome: "ok".into(),
            reason_code: "captured_ok".into(),
            explanation: "Captured Spotify episode.".into(),
            underlying_error: None,
            listen_duration_s: Some(1200),
            threshold_s: Some(600),
        })?;

        let without_skips = index.spotify_sync_events(10, false, None)?;
        assert_eq!(without_skips.len(), 1);
        assert_eq!(without_skips[0].reason_code, "captured_ok");

        let with_skips = index.spotify_sync_events(10, true, None)?;
        assert_eq!(with_skips.len(), 2);
        assert_eq!(with_skips[1].reason_code, "insufficient_listen_time");

        let since = chrono::DateTime::parse_from_rfc3339("2026-06-22T08:05:00Z")?
            .with_timezone(&chrono::Utc);
        assert_eq!(index.spotify_sync_events(10, true, Some(since))?.len(), 1);
        assert_eq!(
            index
                .spotify_recent_reason_counts(
                    chrono::DateTime::parse_from_rfc3339("2026-06-22T00:00:00Z")?
                        .with_timezone(&chrono::Utc)
                )?
                .first()
                .map(|count| (count.reason_code.as_str(), count.count)),
            Some(("insufficient_listen_time", 1))
        );
        assert_eq!(
            index.spotify_last_successful_poll_at()?.as_deref(),
            Some("2026-06-22T08:10:00Z")
        );
        assert_eq!(
            index.spotify_last_capture_at()?.as_deref(),
            Some("2026-06-22T08:10:00Z")
        );
        Ok(())
    }

    #[test]
    fn migrations_create_durable_schema_and_are_idempotent() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        index.init()?;

        let connection = index.connection()?;
        assert_eq!(schema_version(&connection)?, CURRENT_SCHEMA_VERSION);
        assert!(column_exists(&connection, "sources", "consumed_at")?);
        assert!(column_exists(&connection, "sources", "device_id")?);
        assert!(column_exists(&connection, "chunks", "embedding_model")?);
        assert!(table_exists(&connection, "sync_state")?);
        assert!(table_exists(&connection, "schema_meta")?);
        drop(connection);

        let report = index.migrate()?;
        assert_eq!(report.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(report.applied.is_empty());
        Ok(())
    }

    #[test]
    fn migrations_upgrade_legacy_rows_without_guessing_new_values() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut connection = Connection::open(temp.path().join("legacy.db"))?;
        create_base_schema(&connection)?;
        connection.execute(
            "INSERT INTO sources(id,type,title,captured_at,access,summary,file_path)
             VALUES('source-1','article','Legacy','2026-06-22T08:00:00Z','restricted','Summary','/tmp/source-1.md')",
            [],
        )?;
        connection.execute(
            "INSERT INTO chunks(id,source_id,ord,access,text)
             VALUES('source-1:0','source-1',0,'restricted','Legacy body')",
            [],
        )?;

        let report = run_migrations(&mut connection)?;

        assert_eq!(report.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            connection.query_row(
                "SELECT consumed_at,device_id FROM sources WHERE id='source-1'",
                [],
                |row| Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?
                ))
            )?,
            (None, None)
        );
        assert_eq!(
            connection.query_row(
                "SELECT embedding_model FROM chunks WHERE id='source-1:0'",
                [],
                |row| row.get::<_, String>(0)
            )?,
            ""
        );
        assert!(table_exists(&connection, "sync_state")?);
        Ok(())
    }

    #[test]
    fn migrations_apply_only_versions_newer_than_schema_meta() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut connection = Connection::open(temp.path().join("pending.db"))?;
        connection.execute_batch(
            "CREATE TABLE schema_meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_meta(key,value) VALUES('version','1');",
        )?;
        let migrations = [
            Migration {
                version: 1,
                description: "already applied",
                sql: "THIS WOULD FAIL IF RE-RUN;",
            },
            Migration {
                version: 2,
                description: "pending",
                sql: "CREATE TABLE pending_only(id TEXT PRIMARY KEY);",
            },
        ];

        let report = run_migrations_with(&mut connection, &migrations)?;

        assert_eq!(report.schema_version, 2);
        assert_eq!(report.applied.len(), 1);
        assert_eq!(report.applied[0].version, 2);
        assert!(table_exists(&connection, "pending_only")?);
        Ok(())
    }

    #[test]
    fn migration_failure_rolls_back_and_keeps_prior_version() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut connection = Connection::open(temp.path().join("rollback.db"))?;
        connection.execute_batch(
            "CREATE TABLE sources(id TEXT PRIMARY KEY);
             CREATE TABLE chunks(id TEXT PRIMARY KEY);",
        )?;
        let migrations = [
            Migration {
                version: 1,
                description: "ok",
                sql: "CREATE TABLE ok_table(id TEXT PRIMARY KEY);",
            },
            Migration {
                version: 2,
                description: "bad",
                sql: "CREATE TABLE half_applied(id TEXT PRIMARY KEY); THIS IS NOT SQL;",
            },
        ];

        let error = run_migrations_with(&mut connection, &migrations).unwrap_err();
        assert!(error.to_string().contains("Migration 2 failed"));
        assert_eq!(schema_version(&connection)?, 1);
        assert!(table_exists(&connection, "ok_table")?);
        assert!(!table_exists(&connection, "half_applied")?);
        Ok(())
    }

    #[test]
    fn migration_sequence_must_be_contiguous_from_one() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let mut connection = Connection::open(temp.path().join("invalid-sequence.db"))?;
        let migrations = [Migration {
            version: 2,
            description: "skips one",
            sql: "CREATE TABLE skipped(id TEXT PRIMARY KEY);",
        }];

        let error = run_migrations_with(&mut connection, &migrations).unwrap_err();

        assert!(error.to_string().contains("expected version 1, got 2"));
        assert!(!table_exists(&connection, "schema_meta")?);
        assert!(!table_exists(&connection, "skipped")?);
        Ok(())
    }

    #[test]
    fn upsert_persists_consumed_at_and_embedding_model() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        let record = test_record(
            "record-1",
            "Captured with engagement metadata",
            Some("2026-06-22T12:30:00Z".into()),
        );
        index.upsert(&record, &StaticEmbedder { name: "model-a" })?;

        let connection = index.connection()?;
        assert_eq!(
            connection.query_row(
                "SELECT consumed_at FROM sources WHERE id='record-1'",
                [],
                |row| row.get::<_, Option<String>>(0)
            )?,
            Some("2026-06-22T12:30:00Z".into())
        );
        assert_eq!(
            connection.query_row(
                "SELECT DISTINCT embedding_model FROM chunks WHERE source_id='record-1'",
                [],
                |row| row.get::<_, String>(0)
            )?,
            "model-a"
        );
        Ok(())
    }

    #[test]
    fn upsert_persists_transcript_timestamp_ranges() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        let mut record = test_record(
            "video-1",
            "[00:01] Opening thought\n[00:04] More context\n[00:09] A new section",
            None,
        );
        record.metadata.source_type = SourceType::Youtube;
        index.upsert(&record, &StaticEmbedder { name: "model-a" })?;

        let connection = index.connection()?;
        let ranges = {
            let mut statement = connection.prepare(
                "SELECT t_start,t_end FROM chunks WHERE source_id='video-1' ORDER BY ord",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<f64>>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        assert_eq!(ranges.first().copied(), Some((Some(1.0), Some(9.0))));
        Ok(())
    }

    #[test]
    fn reembed_stale_updates_only_sources_with_stale_chunks() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        let stale = test_record("stale", "This stale source should be refreshed", None);
        let fresh = test_record("fresh", "This fresh source should not be touched", None);
        index.upsert(&stale, &StaticEmbedder { name: "model-a" })?;
        index.upsert(&fresh, &StaticEmbedder { name: "model-a" })?;
        let connection = index.connection()?;
        connection.execute(
            "UPDATE chunks SET embedding_model='model-old' WHERE source_id='stale'",
            [],
        )?;
        drop(connection);

        let updated = index.reembed_stale(&[stale, fresh], &StaticEmbedder { name: "model-a" })?;

        assert_eq!(updated, 1);
        assert_eq!(index.stale_chunk_count("model-a")?, 0);
        let connection = index.connection()?;
        assert_eq!(
            connection.query_row(
                "SELECT embedding_model FROM chunks WHERE source_id='fresh'",
                [],
                |row| row.get::<_, String>(0)
            )?,
            "model-a"
        );
        Ok(())
    }

    #[test]
    fn missing_embedding_model_counts_as_stale() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let index = Index::new(temp.path().join("index.db"));
        let stale = test_record("missing-model", "This source lacks model provenance", None);
        index.upsert(&stale, &StaticEmbedder { name: "model-a" })?;
        let connection = index.connection()?;
        connection.execute(
            "UPDATE chunks SET embedding_model='' WHERE source_id='missing-model'",
            [],
        )?;
        drop(connection);

        assert_eq!(index.stale_chunk_count("model-a")?, 1);
        assert_eq!(
            index.reembed_stale(&[stale], &StaticEmbedder { name: "model-a" })?,
            1
        );
        assert_eq!(index.stale_chunk_count("model-a")?, 0);
        Ok(())
    }

    fn test_record(id: &str, body: &str, consumed_at: Option<String>) -> Record {
        Record {
            metadata: Frontmatter {
                id: id.into(),
                source_type: SourceType::Article,
                title: format!("{id} title"),
                author: None,
                site: Some("example.com".into()),
                source: None,
                url: Some(format!("https://example.com/{id}")),
                captured_at: Utc::now(),
                consumed_at,
                published_at: None,
                duration: None,
                video_id: None,
                word_count: None,
                access: Access::Restricted,
                summary: String::new(),
                tags: Vec::new(),
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
            },
            body: body.into(),
            file_path: format!("/tmp/{id}.md"),
        }
    }

    fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool> {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for candidate in columns {
            if candidate? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
        Ok(connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get::<_, bool>(0),
        )?)
    }
}
