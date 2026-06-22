mod bundle;
mod capture;
mod config;
mod embedding;
mod engine;
mod http;
mod index;
mod mcp;
mod model;
mod public_fetch;
mod sensor_assets;
mod spotify;
mod vault;
mod youtube;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
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
    Get {
        id: String,
    },
    Status,
    Doctor,
    Reindex,
    Bookmarklet,
    ConfigureYoutube {
        #[arg(long)]
        api_key: String,
    },
    ConfigureSpotify {
        #[arg(long, env = "SPOTIFY_CLIENT_ID")]
        client_id: String,
    },
    SyncSpotify,
    SyncStatus,
    Export {
        path: PathBuf,
        #[arg(long)]
        include_public_bodies: bool,
    },
    Ingest {
        path: PathBuf,
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
        Command::Get { id } => serde_json::to_value(engine.get_any(&id)?)?,
        Command::Status => serde_json::to_value(engine.status()?)?,
        Command::Doctor => serde_json::to_value(engine.doctor()?)?,
        Command::Reindex => serde_json::to_value(engine.reindex()?)?,
        Command::Bookmarklet => serde_json::to_value(config::bookmarklet(&engine.local_config()?))?,
        Command::ConfigureYoutube { api_key } => {
            engine.configure_youtube_api_key(api_key)?;
            serde_json::json!({"configured":true})
        }
        Command::ConfigureSpotify { client_id } => {
            serde_json::to_value(engine.configure_spotify_placeholder(client_id)?)?
        }
        Command::SyncSpotify => serde_json::to_value(engine.sync_spotify()?)?,
        Command::SyncStatus => serde_json::to_value(engine.spotify_sync_status()?)?,
        Command::Export {
            path,
            include_public_bodies,
        } => serde_json::to_value(engine.export_bundle(&path, include_public_bodies)?)?,
        Command::Ingest { path } => serde_json::to_value(engine.ingest_bundle(&path)?)?,
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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::config::ConfigStore;
    use crate::embedding::{EMBEDDING_DIMENSION, Embedder};
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
        let captured = engine.capture(input)?;
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
        let after = engine.reindex()?;
        assert_eq!(before.capture_count, after.capture_count);
        assert_eq!(
            engine.get(&captured.metadata.id)?.unwrap().body,
            captured.body
        );
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
}
