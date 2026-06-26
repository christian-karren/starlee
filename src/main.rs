mod bundle;
mod capture;
mod chunking;
mod config;
mod documents;
mod embedding;
mod engine;
mod http;
mod identity;
mod index;
mod mcp;
mod model;
mod public_fetch;
mod sensor_assets;
mod spotify;
mod sync;
mod topics;
mod vault;
mod vault_backend;
mod youtube;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use engine::Engine;
use model::{Access, CaptureInput, SearchScope, SourceType};

#[derive(Parser)]
#[command(name = "starlee", version, about)]
struct Cli {
    #[arg(long, env = "STARLEE_HOME")]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Setup,
    CaptureText {
        #[arg(long)]
        title: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        author: Option<String>,
        #[arg(long)]
        site: Option<String>,
        #[arg(long, value_enum, default_value = "note")]
        r#type: TypeArg,
        #[arg(long, value_enum, default_value = "restricted")]
        access: AccessArg,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long = "topic")]
        topic: Vec<String>,
    },
    CaptureUrl {
        url: String,
    },
    Search {
        query: String,
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
        #[arg(long, value_enum, default_value = "both")]
        scope: ScopeArg,
    },
    Query {
        question: String,
        #[arg(long)]
        context: Option<String>,
        #[arg(long, default_value_t = 8)]
        max_chunks: usize,
    },
    CorpusOverview,
    Recent {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    List {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    Get {
        id: String,
    },
    /// Permanently delete a capture from the vault and index by id.
    Delete {
        id: String,
    },
    /// List user topics across the corpus with assignment counts.
    Topics,
    /// Replace the topic set on a record (topics persist in frontmatter).
    SetTopics {
        id: String,
        #[arg(long = "topic")]
        topic: Vec<String>,
    },
    /// Rename a topic across every record that carries it.
    RenameTopic {
        from: String,
        to: String,
    },
    /// Remove a topic from every record; the records themselves are kept.
    DeleteTopic {
        name: String,
    },
    Status,
    Doctor,
    Diagnostics {
        #[arg(short, long, default_value_t = 30)]
        limit: usize,
        #[arg(long)]
        last_capture: bool,
    },
    Reindex {
        #[arg(long)]
        stale_embeddings_only: bool,
    },
    Migrate,
    Bookmarklet,
    ConfigureYoutube {
        #[arg(long)]
        api_key: String,
    },
    ConfigureSpotify {
        #[arg(long, env = "SPOTIFY_CLIENT_ID")]
        client_id: Option<String>,
    },
    SyncSpotify,
    SyncStatus,
    SyncLog {
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        show_skips: bool,
        #[arg(long)]
        since: Option<String>,
    },
    Export {
        path: PathBuf,
        #[arg(long)]
        include_public_bodies: bool,
    },
    Ingest {
        path: PathBuf,
    },
    /// Import local documents (PDF, DOCX, TXT, MD) into the vault.
    Import {
        paths: Vec<PathBuf>,
        #[arg(long = "topic")]
        topic: Vec<String>,
    },
    Serve,
    Mcp,
}

#[derive(Clone, ValueEnum)]
enum TypeArg {
    Article,
    Youtube,
    Note,
}
#[derive(Clone, ValueEnum)]
enum AccessArg {
    Public,
    Restricted,
}
#[derive(Clone, ValueEnum)]
enum ScopeArg {
    Own,
    Borrowed,
    Both,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let engine = Arc::new(Engine::new(cli.home.unwrap_or_else(default_home)));
    let value = match cli.command {
        Command::Setup => serde_json::to_value(engine.onboarding()?)?,
        Command::CaptureText {
            title,
            text,
            url,
            author,
            site,
            r#type,
            access,
            tag,
            topic,
        } => {
            let source_type = match r#type {
                TypeArg::Article => SourceType::Article,
                TypeArg::Youtube => SourceType::Youtube,
                TypeArg::Note => SourceType::Note,
            };
            let access = match access {
                AccessArg::Public => Access::Public,
                AccessArg::Restricted => Access::Restricted,
            };
            let mut input = CaptureInput::new(title, text, source_type, access);
            input.author = author;
            input.site = site;
            input.url = url;
            input.tags = tag;
            input.topics = topic;
            serde_json::to_value(engine.capture(input)?)?
        }
        Command::CaptureUrl { url } => serde_json::to_value(engine.capture_public_url(&url)?)?,
        Command::Search {
            query,
            limit,
            scope,
        } => {
            let scope = match scope {
                ScopeArg::Own => SearchScope::Own,
                ScopeArg::Borrowed => SearchScope::Borrowed,
                ScopeArg::Both => SearchScope::Both,
            };
            serde_json::to_value(engine.search_scoped(&query, limit, scope)?)?
        }
        Command::Query {
            question,
            context,
            max_chunks,
        } => serde_json::to_value(engine.query(&question, context.as_deref(), max_chunks)?)?,
        Command::CorpusOverview => serde_json::to_value(engine.corpus_overview()?)?,
        Command::Recent { limit } => serde_json::to_value(engine.recent(limit)?)?,
        Command::List { limit } => serde_json::to_value(engine.recent(limit)?)?,
        Command::Get { id } => serde_json::to_value(engine.get_any(&id)?)?,
        Command::Delete { id } => {
            let deleted = engine.delete(&id)?;
            serde_json::json!({"deleted": deleted, "id": id})
        }
        Command::Topics => serde_json::to_value(engine.list_topics()?)?,
        Command::SetTopics { id, topic } => {
            serde_json::to_value(engine.set_record_topics(&id, topic)?)?
        }
        Command::RenameTopic { from, to } => {
            let changed = engine.rename_topic(&from, &to)?;
            serde_json::json!({"changed": changed})
        }
        Command::DeleteTopic { name } => {
            let changed = engine.delete_topic(&name)?;
            serde_json::json!({"changed": changed})
        }
        Command::Status => serde_json::to_value(engine.status()?)?,
        Command::Doctor => serde_json::to_value(engine.doctor()?)?,
        Command::Diagnostics {
            limit,
            last_capture,
        } => {
            if last_capture {
                serde_json::to_value(engine.last_capture_trace()?)?
            } else {
                serde_json::to_value(engine.capture_diagnostics(limit)?)?
            }
        }
        Command::Reindex {
            stale_embeddings_only,
        } => serde_json::to_value(engine.reindex(stale_embeddings_only)?)?,
        Command::Migrate => {
            let report = engine.migrate()?;
            if report.applied.is_empty() {
                eprintln!("Schema is up to date (version {}).", report.schema_version);
            } else {
                for migration in &report.applied {
                    eprintln!(
                        "Applied migration {}: {}",
                        migration.version, migration.description
                    );
                }
            }
            serde_json::to_value(report)?
        }
        Command::Bookmarklet => serde_json::to_value(config::bookmarklet(&engine.local_config()?))?,
        Command::ConfigureYoutube { api_key } => {
            engine.configure_youtube_api_key(api_key)?;
            serde_json::json!({"configured":true})
        }
        Command::ConfigureSpotify { client_id } => {
            let report = engine.configure_spotify(client_id)?;
            println!("Spotify connected successfully");
            serde_json::to_value(report)?
        }
        Command::SyncSpotify => serde_json::to_value(engine.sync_spotify()?)?,
        Command::SyncStatus => serde_json::to_value(engine.spotify_sync_status()?)?,
        Command::SyncLog {
            limit,
            show_skips,
            since,
        } => {
            let since = since.as_deref().map(parse_since).transpose()?;
            let log = engine.spotify_sync_log(limit, show_skips, since)?;
            if let Some(gap) = log.coverage_gap.as_deref() {
                println!("coverage_gap: {gap}");
            }
            for event in log.events {
                println!(
                    "{} [{}] {} {} episode={} show={} - {}",
                    event.timestamp,
                    event.outcome,
                    event.stage_reached,
                    event.reason_code,
                    event.episode_title.as_deref().unwrap_or("-"),
                    event.show_name.as_deref().unwrap_or("-"),
                    event.explanation
                );
                if let Some(error) = event.underlying_error.as_deref() {
                    println!("  underlying_error: {error}");
                }
            }
            return Ok(());
        }
        Command::Export {
            path,
            include_public_bodies,
        } => serde_json::to_value(engine.export_bundle(&path, include_public_bodies)?)?,
        Command::Ingest { path } => serde_json::to_value(engine.ingest_bundle(&path)?)?,
        Command::Import { paths, topic } => {
            serde_json::to_value(engine.import_documents(&paths, topic)?)?
        }
        Command::Serve => {
            engine.setup()?;
            let server = http::spawn(engine.clone(), engine.local_config()?)?;
            eprintln!(
                "Starlee capture endpoint listening on http://{}",
                server.address
            );
            return server.wait();
        }
        Command::Mcp => {
            engine.setup()?;
            let _capture_server = match http::spawn(engine.clone(), engine.local_config()?) {
                Ok(server) => Some(server),
                Err(error) => {
                    eprintln!(
                        "Starlee MCP continuing without owning the capture endpoint: {error:#}"
                    );
                    None
                }
            };
            return mcp::serve(engine.as_ref());
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn default_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Starlee")
}

fn parse_since(value: &str) -> Result<DateTime<Utc>> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(value.with_timezone(&Utc));
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    Ok(date.and_hms_opt(0, 0, 0).expect("valid midnight").and_utc())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::{ConfigStore, SpotifyOAuthConfig};
    use crate::embedding::{EMBEDDING_DIMENSION, Embedder};
    use anyhow::Context;
    use chrono::{TimeDelta, Utc};
    use std::sync::Arc;

    struct TestEmbedder;

    impl Embedder for TestEmbedder {
        fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|text| test_vector(text)).collect())
        }
        fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
            Ok(test_vector(text))
        }
        fn name(&self) -> &'static str {
            "deterministic-test-embedder"
        }
    }

    fn test_vector(text: &str) -> Vec<f32> {
        let mut vector = vec![0.0; EMBEDDING_DIMENSION];
        let lower = text.to_lowercase();
        if lower.contains("search") || lower.contains("recall") || lower.contains("forgotten") {
            vector[0] = 1.0;
        }
        if lower.contains("cooking") {
            vector[1] = 1.0;
        }
        vector
    }

    #[test]
    fn capture_search_and_reindex_round_trip() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let mut input = CaptureInput::new(
            "Knowledge compounds",
            "A durable digital brain makes forgotten ideas searchable again.",
            SourceType::Note,
            Access::Restricted,
        );
        input.tags = vec!["memory".into()];
        input.consumed_at = Some("2026-06-22T08:30:00Z".into());
        let captured = engine.capture(input)?;
        assert_eq!(
            captured.metadata.consumed_at.as_deref(),
            Some("2026-06-22T08:30:00Z")
        );
        assert!(PathBuf::from(&captured.file_path).exists());
        assert_eq!(
            engine
                .search_scoped("forgotten searchable", 5, SearchScope::Own)?
                .len(),
            1
        );
        assert_eq!(
            engine.search_scoped("recall", 5, SearchScope::Own)?[0].title,
            "Knowledge compounds"
        );
        let before = engine.status()?;
        let after = engine.reindex(false)?;
        assert_eq!(before.capture_count, after.capture_count);
        assert_eq!(after.chunks_stale, 0);
        assert_eq!(
            engine.get(&captured.metadata.id)?.unwrap().body,
            captured.body
        );
        Ok(())
    }

    fn capture_note(engine: &Engine, title: &str, text: &str) -> Result<crate::model::Record> {
        engine.capture(CaptureInput::new(
            title,
            text,
            SourceType::Note,
            Access::Restricted,
        ))
    }

    #[test]
    fn topics_persist_in_frontmatter_and_survive_reindex() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let mut input = CaptureInput::new(
            "Mitochondria",
            "The powerhouse of the cell is a recurring exam topic.",
            SourceType::Note,
            Access::Restricted,
        );
        // Messy input: whitespace, duplicate (case-insensitive), and an empty entry.
        input.topics = vec![
            "  Biology 101 ".into(),
            "biology 101".into(),
            "".into(),
            "Exams".into(),
        ];
        let record = engine.capture(input)?;
        assert_eq!(record.metadata.topics, vec!["Biology 101", "Exams"]);

        // The index mirror reflects the sanitized, de-duplicated topics.
        let topics = engine.list_topics()?;
        assert_eq!(topics.len(), 2);
        assert!(
            topics
                .iter()
                .any(|t| t.topic == "Biology 101" && t.count == 1)
        );

        // Topics are canonical in the Markdown, so a full reindex (which rebuilds
        // the index purely from the vault) preserves them with zero loss.
        engine.reindex(false)?;
        let reread = engine.get(&record.metadata.id)?.unwrap();
        assert_eq!(reread.metadata.topics, vec!["Biology 101", "Exams"]);
        assert_eq!(engine.list_topics()?.len(), 2);
        Ok(())
    }

    #[test]
    fn set_rename_and_delete_topic_propagate_through_vault_and_index() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let first = capture_note(&engine, "Cells", "Cell biology basics.")?;
        let second = capture_note(&engine, "Genes", "Genetics basics.")?;

        engine.set_record_topics(&first.metadata.id, vec!["Biology".into()])?;
        engine.set_record_topics(&second.metadata.id, vec!["Biology".into()])?;
        let topics = engine.list_topics()?;
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "Biology");
        assert_eq!(topics[0].count, 2);

        // Rename everywhere.
        assert_eq!(engine.rename_topic("biology", "Life Sciences")?, 2);
        let renamed = engine.list_topics()?;
        assert_eq!(renamed.len(), 1);
        assert_eq!(renamed[0].topic, "Life Sciences");
        assert_eq!(renamed[0].count, 2);

        // Deleting a topic strips it from records but keeps the records.
        assert_eq!(engine.delete_topic("life sciences")?, 2);
        assert!(engine.list_topics()?.is_empty());
        assert!(engine.get(&first.metadata.id)?.is_some());
        assert!(engine.get(&second.metadata.id)?.is_some());
        assert!(
            engine
                .get(&first.metadata.id)?
                .unwrap()
                .metadata
                .topics
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn delete_removes_record_from_vault_and_index_and_stays_consistent() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let keep = capture_note(&engine, "Keep me", "A searchable note about recall.")?;
        let drop = capture_note(&engine, "Drop me", "Another searchable note about recall.")?;
        assert_eq!(engine.status()?.capture_count, 2);

        assert!(engine.delete(&drop.metadata.id)?);

        // Gone from the vault file, the index, search, and the corpus count.
        assert!(!PathBuf::from(&drop.file_path).exists());
        assert!(engine.get(&drop.metadata.id)?.is_none());
        assert!(engine.get(&keep.metadata.id)?.is_some());
        let status = engine.status()?;
        assert_eq!(status.capture_count, 1);
        let hits = engine.search_scoped("recall", 10, SearchScope::Own)?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, keep.metadata.id);

        // Index matches a fresh rebuild from the vault: no orphaned chunks/vectors.
        let (chunks_before, _) = (status.chunk_count, ());
        let after = engine.reindex(false)?;
        assert_eq!(after.capture_count, 1);
        assert_eq!(after.chunk_count, chunks_before);

        // Deleting an unknown id is a no-op, not an error.
        assert!(!engine.delete("does-not-exist")?);
        Ok(())
    }

    #[test]
    fn recent_includes_author_and_topics_for_library_filtering() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let mut input = CaptureInput::new(
            "Photosynthesis",
            "Plants convert light into chemical energy.",
            SourceType::Article,
            Access::Public,
        );
        input.author = Some("Dr. Green".into());
        input.topics = vec!["Biology".into(), "Plants".into()];
        let record = engine.capture(input)?;

        let hits = engine.recent(10)?;
        let hit = hits
            .iter()
            .find(|hit| hit.id == record.metadata.id)
            .expect("captured record appears in recent");
        assert_eq!(hit.author.as_deref(), Some("Dr. Green"));
        let mut topics = hit.topics.clone();
        topics.sort();
        assert_eq!(topics, vec!["Biology".to_owned(), "Plants".to_owned()]);
        Ok(())
    }

    #[test]
    fn import_documents_ingests_dedupes_and_tags() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let lecture = temp.path().join("lecture1.txt");
        std::fs::write(
            &lecture,
            "Photosynthesis converts light into chemical energy.",
        )?;
        let unsupported = temp.path().join("image.png");
        std::fs::write(&unsupported, "not a real image")?;

        let report =
            engine.import_documents(&[lecture.clone(), unsupported], vec!["BIO 101".into()])?;
        assert_eq!(report.imported.len(), 1);
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("unsupported"));

        // Imported text is searchable and carries the batch topic.
        assert_eq!(
            engine
                .search_scoped("photosynthesis", 5, SearchScope::Own)?
                .len(),
            1
        );
        assert!(engine.list_topics()?.iter().any(|t| t.topic == "BIO 101"));

        // Re-importing identical content updates rather than duplicating.
        let again = engine.import_documents(&[lecture], vec!["BIO 101".into()])?;
        assert_eq!(again.imported.len(), 1);
        assert_eq!(engine.status()?.capture_count, 1);
        Ok(())
    }

    #[test]
    fn onboarding_report_does_not_expose_capture_token() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let report = engine.onboarding()?;
        let config = ConfigStore::new(temp.path()).load()?;
        let serialized = serde_json::to_string(&report)?;

        assert!(!serialized.contains(&config.capture_token));
        assert_eq!(report.extension_token, "redacted");
        assert_eq!(report.extension_token_fingerprint.len(), 12);
        assert!(report.bookmarklet.starts_with("redacted:"));
        assert!(
            PathBuf::from(&report.extension_path)
                .join("assets/icon-128.png")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn share_bundle_strips_restricted_bodies_and_searches_read_only() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let owner = Engine::with_embedder(temp.path().join("owner"), Arc::new(TestEmbedder));
        let mut private_memory = CaptureInput::new(
            "Private memory",
            "A restricted searchable insight about memory systems.",
            SourceType::Article,
            Access::Restricted,
        );
        private_memory.site = Some("paid.example".into());
        private_memory.url = Some("https://paid.example/story".into());
        private_memory.summary = Some("An insight about memory systems.".into());
        owner.capture(private_memory)?;
        let bundle_path = temp.path().join("shared.starlee");
        let audit = owner.export_bundle(&bundle_path, true)?;
        assert!(audit.valid);
        assert_eq!(audit.restricted_body_count, 0);

        let borrower = Engine::with_embedder(temp.path().join("borrower"), Arc::new(TestEmbedder));
        borrower.ingest_bundle(&bundle_path)?;
        let hits = borrower.search_scoped("recall", 5, SearchScope::Borrowed)?;
        assert_eq!(hits[0].title, "Private memory");
        assert!(hits[0].source.starts_with("borrowed:"));
        match borrower.get_any(&hits[0].id)?.unwrap() {
            crate::model::GetResult::Borrowed { record } => {
                assert_eq!(record.summary, "An insight about memory systems.");
            }
            crate::model::GetResult::Own { .. } => panic!("borrowed record reported as own"),
        }

        let connection = rusqlite::Connection::open(&bundle_path)?;
        connection.execute(
            "UPDATE chunks SET text='forbidden leak' WHERE access='restricted'",
            [],
        )?;
        drop(connection);
        assert!(crate::bundle::validate(&bundle_path).is_err());
        Ok(())
    }

    #[test]
    fn recapturing_a_url_updates_the_existing_markdown_record() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let input = |text: &str| {
            let mut input =
                CaptureInput::new("Same story", text, SourceType::Article, Access::Public);
            input.site = Some("example.com".into());
            input.url = Some("https://example.com/same-story".into());
            input
        };
        let first = engine.capture(input("The first version of the article."))?;
        let second = engine.capture(input(
            "The updated version contains better searchable recall.",
        ))?;
        assert_eq!(first.metadata.id, second.metadata.id);
        assert_eq!(engine.status()?.capture_count, 1);
        assert!(
            engine
                .get(&first.metadata.id)?
                .unwrap()
                .body
                .contains("updated version")
        );
        Ok(())
    }

    #[test]
    fn query_returns_citation_ready_chunks_and_overview() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let mut memory_systems = CaptureInput::new(
            "Agentic memory systems",
            "A durable digital brain helps people recall forgotten agent design patterns and connect them to new work.",
            SourceType::Article,
            Access::Restricted,
        );
        memory_systems.author = Some("Casey Researcher".into());
        memory_systems.site = Some("example.com".into());
        memory_systems.url = Some("https://example.com/agents".into());
        memory_systems.summary =
            Some("Agent memory systems connect forgotten design patterns.".into());
        memory_systems.tags = vec!["agents".into()];
        engine.capture(memory_systems)?;

        let query = engine.query("forgotten agent design", None, 8)?;
        assert!(!query.chunks.is_empty());
        assert_eq!(query.chunks[0].index, 1);
        assert_eq!(query.chunks[0].title, "Agentic memory systems");
        assert_eq!(query.chunks[0].domain.as_deref(), Some("example.com"));
        assert_eq!(query.chunks[0].chunk_index, 0);
        assert!(query.chunks[0].similarity >= 0.35);
        assert_eq!(query.chunks[0].consumed_at, None);

        let overview = engine.corpus_overview()?;
        assert_eq!(overview.total_captures, 1);
        assert_eq!(overview.source_breakdown.get("article"), Some(&1.0));
        assert_eq!(overview.top_domains, vec!["example.com"]);
        assert_eq!(overview.top_authors, vec!["Casey Researcher"]);
        assert!(overview.top_topics.iter().any(|topic| topic == "agent"));
        Ok(())
    }

    #[test]
    fn query_reports_gap_when_floor_excludes_results() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        engine.capture(CaptureInput::new(
            "Sparse note",
            "A durable digital brain makes forgotten ideas searchable again.",
            SourceType::Note,
            Access::Restricted,
        ))?;
        let store = ConfigStore::new(temp.path());
        let mut config = store.load()?;
        config.query_relevance_floor = 1.1;
        store.save(&config)?;

        let query = engine.query("forgotten searchable", None, 8)?;
        assert!(query.relevance_floor_hit);
        assert!(query.chunks.is_empty());
        assert!(query.total_retrieved >= 1);
        Ok(())
    }

    #[test]
    fn corpus_overview_handles_empty_vault() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        engine.setup()?;
        let overview = engine.corpus_overview()?;
        assert_eq!(overview.total_captures, 0);
        assert_eq!(overview.earliest_capture, None);
        assert_eq!(overview.latest_capture, None);
        assert!(overview.source_breakdown.is_empty());
        assert!(overview.top_topics.is_empty());
        Ok(())
    }

    #[test]
    fn doctor_rejects_missing_empty_expired_and_malformed_spotify_tokens() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        engine.setup()?;
        let store = ConfigStore::new(temp.path());

        let spotify_check = || -> Result<crate::model::DoctorCheck> {
            engine
                .doctor()?
                .checks
                .into_iter()
                .find(|check| check.name == "spotify_oauth")
                .context("doctor should include spotify_oauth check")
        };

        assert!(!spotify_check()?.ok);

        let mut config = store.load()?;
        config.spotify_oauth = Some(test_oauth("", Utc::now() + TimeDelta::minutes(30)));
        store.save(&config)?;
        assert!(!spotify_check()?.ok);

        config.spotify_oauth = Some(test_oauth("token", Utc::now() - TimeDelta::minutes(1)));
        store.save(&config)?;
        assert!(!spotify_check()?.ok);

        config.spotify_oauth = Some(test_oauth("token", Utc::now() + TimeDelta::minutes(30)));
        config.spotify_oauth.as_mut().unwrap().expires_at = "not-a-date".into();
        store.save(&config)?;
        assert!(!spotify_check()?.ok);

        config.spotify_oauth = Some(test_oauth("token", Utc::now() + TimeDelta::minutes(30)));
        store.save(&config)?;
        let check = spotify_check()?;
        assert!(check.ok);
        assert_eq!(check.detail, "Spotify Test User");
        assert!(engine.status()?.spotify_oauth_configured);
        Ok(())
    }

    #[test]
    fn stale_embedding_reindex_updates_only_stale_sources() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        engine.capture(CaptureInput::new(
            "Stale note",
            "A durable digital brain makes forgotten ideas searchable again.",
            SourceType::Note,
            Access::Restricted,
        ))?;
        let db = temp.path().join("index.db");
        let connection = rusqlite::Connection::open(db)?;
        connection.execute("UPDATE chunks SET embedding_model='older-model'", [])?;
        drop(connection);

        assert!(engine.status()?.chunks_stale > 0);
        let status = engine.reindex(true)?;
        assert_eq!(status.chunks_stale, 0);
        assert_eq!(
            status.embedding_model_current,
            "deterministic-test-embedder"
        );
        Ok(())
    }

    #[test]
    fn spotify_episode_capture_writes_required_frontmatter() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let mut input = CaptureInput::new(
            "A synced episode",
            "[Transcript unavailable]",
            SourceType::SpotifyEpisode,
            Access::Restricted,
        );
        input.source = Some("spotify_sync".into());
        input.site = Some("Spotify".into());
        input.url = Some("https://open.spotify.com/episode/ep-required".into());
        input.spotify_episode_id = Some("ep-required".into());
        input.show = Some("Required Show".into());
        input.transcript_status = Some("missing".into());

        let record = engine.capture(input)?;
        let file = std::fs::read_to_string(&record.file_path)?;
        assert!(file.contains("title: A synced episode"));
        assert!(file.contains("type: spotify_episode"));
        assert!(file.contains("source: spotify_sync"));
        assert!(file.contains("spotify_episode_id: ep-required"));
        assert!(file.contains("show: Required Show"));
        assert!(file.contains("transcript_status: missing"));

        let reread = engine.get("spotify:episode:ep-required")?.unwrap();
        assert_eq!(reread.metadata.source.as_deref(), Some("spotify_sync"));
        assert_eq!(reread.metadata.show.as_deref(), Some("Required Show"));
        assert_eq!(
            reread.metadata.transcript_status.as_deref(),
            Some("missing")
        );
        Ok(())
    }

    #[test]
    fn sync_log_records_serve_not_running_coverage_gap() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        engine.setup()?;
        let store = ConfigStore::new(temp.path());
        let mut config = store.load()?;
        config.capture_port = 9;
        store.save(&config)?;

        let log = engine.spotify_sync_log(10, true, None)?;
        assert!(
            log.coverage_gap
                .as_deref()
                .is_some_and(|gap| { gap.contains("sync service is not running") })
        );
        assert!(log.events.iter().any(|event| {
            event.reason_code == "serve_not_running" && event.explanation.contains("not running")
        }));
        Ok(())
    }

    fn test_oauth(access_token: &str, expires_at: chrono::DateTime<Utc>) -> SpotifyOAuthConfig {
        SpotifyOAuthConfig {
            client_id: "client123".into(),
            display_name: Some("Spotify Test User".into()),
            user_id: Some("spotify-user".into()),
            access_token: access_token.into(),
            refresh_token: "refresh".into(),
            expires_at: expires_at.to_rfc3339(),
        }
    }
}
