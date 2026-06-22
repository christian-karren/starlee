use std::io::{self, BufRead, Write};

use anyhow::Result;
use serde_json::{Value, json};

use std::path::PathBuf;

use crate::{
    config::bookmarklet,
    engine::Engine,
    model::{CaptureInput, SearchScope},
};

pub fn serve(engine: &Engine) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line)?;
        if request.get("id").is_none() {
            continue;
        }
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let result = dispatch(engine, &request);
        let response = match result {
            Ok(value) => json!({"jsonrpc":"2.0","id":id,"result":value}),
            Err(error) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":error.to_string()}})
            }
        };
        serde_json::to_writer(&mut stdout, &response)?;
        writeln!(stdout)?;
        stdout.flush()?;
    }
    Ok(())
}

fn dispatch(engine: &Engine, request: &Value) -> Result<Value> {
    match request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "initialize" => {
            let requested = request
                .pointer("/params/protocolVersion")
                .and_then(Value::as_str);
            let protocol_version = match requested {
                Some("2025-03-26") => "2025-03-26",
                Some("2025-06-18") => "2025-06-18",
                _ => "2025-11-25",
            };
            Ok(json!({
                "protocolVersion":protocol_version,
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"starlee","version":env!("CARGO_PKG_VERSION")}
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tool_definitions()})),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let value = call_tool(engine, name, args)?;
            Ok(json!({"content":[{"type":"text","text":serde_json::to_string_pretty(&value)?}]}))
        }
        method => anyhow::bail!("unsupported MCP method: {method}"),
    }
}

fn call_tool(engine: &Engine, name: &str, args: Value) -> Result<Value> {
    Ok(match name {
        "setup" => serde_json::to_value(engine.onboarding()?)?,
        "capture" => {
            if let Some(text) = args.get("text").and_then(Value::as_str) {
                let mut value = args.clone();
                if value.get("title").is_none() {
                    value["title"] = Value::String(text.chars().take(80).collect());
                }
                serde_json::to_value(
                    engine.capture(serde_json::from_value::<CaptureInput>(value)?)?,
                )?
            } else {
                serde_json::to_value(
                    engine.capture_public_url(args["url"].as_str().unwrap_or_default())?,
                )?
            }
        }
        "search" => {
            let scope = match args["scope"].as_str().unwrap_or("both") {
                "own" => SearchScope::Own,
                "borrowed" => SearchScope::Borrowed,
                _ => SearchScope::Both,
            };
            serde_json::to_value(engine.search_scoped(
                args["query"].as_str().unwrap_or_default(),
                args["k"].as_u64().unwrap_or(5) as usize,
                scope,
            )?)?
        }
        "starlee_query" => {
            let scope = args["scope"].as_str().unwrap_or("vault");
            if scope != "vault" {
                anyhow::bail!(
                    "starlee_query currently supports scope='vault'; borrowed/all query synthesis is not implemented yet"
                );
            }
            serde_json::to_value(engine.query(
                args["question"].as_str().unwrap_or_default(),
                args.get("context").and_then(Value::as_str),
                args["max_chunks"].as_u64().unwrap_or(8) as usize,
            )?)?
        }
        "starlee_corpus_overview" => serde_json::to_value(engine.corpus_overview()?)?,
        "starlee_spotify_sync_status" => serde_json::to_value(engine.spotify_sync_status()?)?,
        "starlee_spotify_sync_log" => serde_json::to_value(
            engine.spotify_sync_log(
                args["limit"].as_u64().unwrap_or(20) as usize,
                args["show_skips"].as_bool().unwrap_or(false),
                args.get("since")
                    .and_then(Value::as_str)
                    .map(parse_since)
                    .transpose()?,
            )?,
        )?,
        "recent" => {
            serde_json::to_value(engine.recent(args["k"].as_u64().unwrap_or(10) as usize)?)?
        }
        "get" => serde_json::to_value(engine.get_any(args["id"].as_str().unwrap_or_default())?)?,
        "status" => serde_json::to_value(engine.status()?)?,
        "doctor" => serde_json::to_value(engine.doctor()?)?,
        "reindex" => serde_json::to_value(engine.reindex()?)?,
        "bookmarklet" => serde_json::to_value(bookmarklet(&engine.local_config()?))?,
        "configure_youtube" => {
            engine.configure_youtube_api_key(
                args["api_key"].as_str().unwrap_or_default().to_owned(),
            )?;
            json!({"configured":true})
        }
        "export" => serde_json::to_value(engine.export_bundle(
            &PathBuf::from(args["path"].as_str().unwrap_or_default()),
            args["include_public_bodies"].as_bool().unwrap_or(false),
        )?)?,
        "ingest" => serde_json::to_value(
            engine.ingest_bundle(&PathBuf::from(args["path"].as_str().unwrap_or_default()))?,
        )?,
        _ => anyhow::bail!("unknown Starlee tool: {name}"),
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "setup",
            "Initialize the local Starlee vault and index",
            json!({"type":"object"}),
        ),
        tool(
            "capture",
            "Capture pasted text into the local vault",
            json!({"type":"object","properties":{"title":{"type":"string"},"text":{"type":"string"},"source_type":{"type":"string","enum":["article","youtube","note"]},"access":{"type":"string","enum":["public","restricted"]},"url":{"type":"string"},"author":{"type":"string"},"site":{"type":"string"},"summary":{"type":"string"},"tags":{"type":"array","items":{"type":"string"}}},"anyOf":[{"required":["text"]},{"required":["url"]}]}),
        ),
        tool(
            "search",
            "Search the local brain and return cited hits",
            json!({"type":"object","required":["query"],"properties":{"query":{"type":"string"},"k":{"type":"integer","minimum":1,"maximum":50},"scope":{"type":"string","enum":["own","borrowed","both"]}}}),
        ),
        tool(
            "starlee_query",
            "Retrieve citation-ready chunks from the user's Starlee corpus for Codex to synthesize a grounded answer",
            json!({"type":"object","required":["question"],"properties":{"question":{"type":"string"},"context":{"type":"string"},"scope":{"type":"string","enum":["vault","borrowed","all"],"default":"vault"},"max_chunks":{"type":"integer","minimum":1,"maximum":20,"default":8}}}),
        ),
        tool(
            "starlee_corpus_overview",
            "Return vault-wide Starlee corpus statistics for session orientation without a retrieval call",
            json!({"type":"object"}),
        ),
        tool(
            "starlee_spotify_sync_status",
            "Report Spotify sync configuration, scheduler state, and current API limitations",
            json!({"type":"object"}),
        ),
        tool(
            "starlee_spotify_sync_log",
            "Return structured Spotify sync traces, including skips and failures with reason codes",
            json!({"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":200,"default":20},"show_skips":{"type":"boolean","default":false},"since":{"type":"string","description":"RFC3339 timestamp or YYYY-MM-DD"}}}),
        ),
        tool(
            "recent",
            "List recent captures",
            json!({"type":"object","properties":{"k":{"type":"integer","minimum":1,"maximum":50}}}),
        ),
        tool(
            "get",
            "Get a complete local record by id",
            json!({"type":"object","required":["id"],"properties":{"id":{"type":"string"}}}),
        ),
        tool(
            "status",
            "Report Starlee health and counts",
            json!({"type":"object"}),
        ),
        tool(
            "doctor",
            "Run redacted Starlee setup diagnostics",
            json!({"type":"object"}),
        ),
        tool(
            "reindex",
            "Rebuild the disposable index from Markdown",
            json!({"type":"object"}),
        ),
        tool(
            "bookmarklet",
            "Generate a personalized zero-install browser capture bookmarklet",
            json!({"type":"object"}),
        ),
        tool(
            "configure_youtube",
            "Store an optional local YouTube Data API key for richer video metadata",
            json!({"type":"object","required":["api_key"],"properties":{"api_key":{"type":"string"}}}),
        ),
        tool(
            "export",
            "Export a privacy-audited share bundle; restricted bodies are always removed",
            json!({"type":"object","required":["path"],"properties":{"path":{"type":"string"},"include_public_bodies":{"type":"boolean"}}}),
        ),
        tool(
            "ingest",
            "Validate and mount a borrowed Starlee bundle read-only",
            json!({"type":"object","required":["path"],"properties":{"path":{"type":"string"}}}),
        ),
    ]
}

fn parse_since(value: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    if let Ok(value) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(value.with_timezone(&chrono::Utc));
    }
    let date = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")?;
    Ok(date.and_hms_opt(0, 0, 0).expect("valid midnight").and_utc())
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({"name":name,"description":description,"inputSchema":input_schema})
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::embedding::{EMBEDDING_DIMENSION, Embedder};

    struct TestEmbedder;
    impl Embedder for TestEmbedder {
        fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|_| vec![1.0; EMBEDDING_DIMENSION])
                .collect())
        }
        fn embed_query(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(vec![1.0; EMBEDDING_DIMENSION])
        }
        fn name(&self) -> &'static str {
            "mcp-test"
        }
    }

    #[test]
    fn negotiates_current_stable_protocol_and_lists_tools() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let initialized = dispatch(
            &engine,
            &json!({
                "method":"initialize","params":{"protocolVersion":"2025-11-25"}
            }),
        )?;
        assert_eq!(initialized["protocolVersion"], "2025-11-25");
        let tools = dispatch(&engine, &json!({"method":"tools/list"}))?;
        assert!(
            tools["tools"]
                .as_array()
                .is_some_and(|tools| tools.len() >= 10)
        );
        let tool_names = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(tool_names.contains(&"starlee_query"));
        assert!(tool_names.contains(&"starlee_corpus_overview"));
        assert!(tool_names.contains(&"starlee_spotify_sync_status"));
        assert!(tool_names.contains(&"starlee_spotify_sync_log"));
        Ok(())
    }

    #[test]
    fn starlee_query_rejects_unimplemented_borrowed_scope() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Engine::with_embedder(temp.path().to_owned(), Arc::new(TestEmbedder));
        let error = dispatch(
            &engine,
            &json!({
                "method":"tools/call",
                "params":{
                    "name":"starlee_query",
                    "arguments":{"question":"agents","scope":"all"}
                }
            }),
        )
        .unwrap_err();
        assert!(error.to_string().contains("scope='vault'"));
        Ok(())
    }

    #[test]
    fn starlee_skill_documents_query_workflow_and_gap_handling() {
        let skill = include_str!("../skills/starlee/SKILL.md");
        assert!(skill.contains("starlee_corpus_overview"));
        assert!(skill.contains("starlee_query"));
        assert!(skill.contains("relevance_floor_hit"));
        assert!(skill.contains("Do not synthesize from training"));
        assert!(skill.contains("Sources:"));
    }
}
