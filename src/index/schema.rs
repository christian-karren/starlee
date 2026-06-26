//! SQLite connection setup and durable schema migrations for the local index.

use std::{path::Path, sync::Once};

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, ffi::sqlite3_auto_extension, params};
use sqlite_vec::sqlite3_vec_init;

static REGISTER_VEC: Once = Once::new();

pub const CURRENT_SCHEMA_VERSION: i64 = 5;
const SCHEMA_VERSION_KEY: &str = "version";

#[derive(Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) description: &'static str,
    pub(crate) sql: &'static str,
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
    Migration {
        version: 5,
        description: "mirror user topics for filtering",
        sql: "CREATE TABLE IF NOT EXISTS source_topics (
                  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
                  topic TEXT NOT NULL,
                  topic_key TEXT NOT NULL,
                  PRIMARY KEY (source_id, topic_key)
              );
              CREATE INDEX IF NOT EXISTS source_topics_key_idx ON source_topics(topic_key);",
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

pub(crate) fn connection(path: &Path) -> Result<Connection> {
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
    let connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
    Ok(connection)
}

pub(crate) fn create_base_schema(connection: &Connection) -> Result<()> {
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
        CREATE TABLE IF NOT EXISTS source_topics (
            source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
            topic TEXT NOT NULL,
            topic_key TEXT NOT NULL,
            PRIMARY KEY (source_id, topic_key)
        );
        CREATE INDEX IF NOT EXISTS source_topics_key_idx ON source_topics(topic_key);
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

pub(crate) fn run_migrations(connection: &mut Connection) -> Result<MigrationReport> {
    run_migrations_with(connection, MIGRATIONS)
}

pub(crate) fn run_migrations_with(
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

pub(crate) fn schema_version(connection: &Connection) -> Result<i64> {
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
