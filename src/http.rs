use std::{
    io::Read,
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use anyhow::{Context, Result};
use serde_json::json;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::{capture::CapturePayload, config::LocalConfig, engine::Engine};

const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

pub struct RunningServer {
    pub address: String,
    server: Arc<Server>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<()>>>,
}

impl RunningServer {
    pub fn wait(mut self) -> Result<()> {
        self.handle
            .take()
            .expect("server thread present")
            .join()
            .map_err(|_| anyhow::anyhow!("capture server thread panicked"))?
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.server.unblock();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn spawn(engine: Arc<Engine>, config: LocalConfig) -> Result<RunningServer> {
    let listener = TcpListener::bind(("127.0.0.1", config.capture_port))
        .with_context(|| format!("bind capture endpoint on 127.0.0.1:{}", config.capture_port))?;
    let address = listener.local_addr()?.to_string();
    let server = Arc::new(
        Server::from_listener(listener, None)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?,
    );
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_server = server.clone();
    let thread_shutdown = shutdown.clone();
    let handle = thread::spawn(move || run(thread_server, thread_shutdown, engine, config));
    Ok(RunningServer {
        address,
        server,
        shutdown,
        handle: Some(handle),
    })
}

fn run(
    server: Arc<Server>,
    shutdown: Arc<AtomicBool>,
    engine: Arc<Engine>,
    config: LocalConfig,
) -> Result<()> {
    while !shutdown.load(Ordering::Relaxed) {
        if let Some(request) = server.recv_timeout(Duration::from_millis(250))? {
            handle(request, &engine, &config)?;
        }
    }
    Ok(())
}

fn handle(mut request: Request, engine: &Engine, config: &LocalConfig) -> Result<()> {
    if request.method() == &Method::Options {
        return respond(request, StatusCode(204), json!({}));
    }
    match (request.method(), request.url()) {
        (&Method::Get, "/health") => respond(
            request,
            StatusCode(200),
            json!({
                "status":"ready", "service":"starlee-capture", "payload_version":1
            }),
        ),
        (&Method::Post, "/extension/hello") => {
            if !authorized(&request, &config.capture_token) {
                return respond(request, StatusCode(401), json!({"error":"unauthorized"}));
            }
            let body = read_body(&mut request)?;
            let value = serde_json::from_str::<serde_json::Value>(&body)?;
            let state = engine.record_extension_hello(
                value
                    .get("browser")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                value
                    .get("extension_version")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned),
                value
                    .get("can_capture_active_tab")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
            )?;
            respond(request, StatusCode(200), serde_json::to_value(state)?)
        }
        (&Method::Post, "/capture-request") => {
            if !authorized(&request, &config.capture_token) {
                return respond(request, StatusCode(401), json!({"error":"unauthorized"}));
            }
            let body = read_body(&mut request)?;
            let value =
                serde_json::from_str::<serde_json::Value>(&body).unwrap_or_else(|_| json!({}));
            let source = value
                .get("source")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("menu-bar");
            let capture_request = engine.create_capture_request(source)?;
            respond(
                request,
                StatusCode(202),
                serde_json::to_value(capture_request)?,
            )
        }
        (&Method::Get, "/capture-request") => {
            if !authorized(&request, &config.capture_token) {
                return respond(request, StatusCode(401), json!({"error":"unauthorized"}));
            }
            respond(
                request,
                StatusCode(200),
                json!({"request": engine.take_capture_request()?}),
            )
        }
        (&Method::Post, "/capture") => {
            if !authorized(&request, &config.capture_token) {
                return respond(request, StatusCode(401), json!({"error":"unauthorized"}));
            }
            if request.body_length().unwrap_or(0) > MAX_CAPTURE_BYTES {
                return respond(
                    request,
                    StatusCode(413),
                    json!({"error":"capture payload too large"}),
                );
            }
            let body = read_body(&mut request)?;
            let result = serde_json::from_str::<CapturePayload>(&body)
                .map_err(Into::into)
                .and_then(CapturePayload::into_input)
                .and_then(|input| engine.capture(input));
            match result {
                Ok(record) => respond(request, StatusCode(201), serde_json::to_value(record)?),
                Err(error) => respond(request, StatusCode(400), json!({"error":error.to_string()})),
            }
        }
        _ => respond(request, StatusCode(404), json!({"error":"not found"})),
    }
}

fn read_body(request: &mut Request) -> Result<String> {
    let mut body = String::new();
    request
        .as_reader()
        .take((MAX_CAPTURE_BYTES + 1) as u64)
        .read_to_string(&mut body)?;
    if body.len() > MAX_CAPTURE_BYTES {
        anyhow::bail!("capture payload too large");
    }
    Ok(body)
}

fn authorized(request: &Request, token: &str) -> bool {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .map(|header| header.value.as_str() == format!("Bearer {token}"))
        .unwrap_or(false)
}

fn respond(request: Request, status: StatusCode, body: serde_json::Value) -> Result<()> {
    let response = Response::from_string(serde_json::to_string(&body)?)
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header(
            "Access-Control-Allow-Headers",
            "Authorization, Content-Type",
        ))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"));
    request.respond(response)?;
    Ok(())
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid static header")
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpStream,
        sync::Arc,
    };

    use super::*;
    use crate::{
        embedding::{EMBEDDING_DIMENSION, Embedder},
        engine::Engine,
    };

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
            "http-test"
        }
    }

    #[test]
    fn requires_token_and_captures_authenticated_payload() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Arc::new(Engine::with_embedder(
            temp.path().to_owned(),
            Arc::new(TestEmbedder),
        ));
        let config = LocalConfig {
            version: 1,
            capture_port: 0,
            capture_token: "secret-token".into(),
            query_relevance_floor: 0.35,
            extension: Default::default(),
            pending_capture_request: None,
            youtube_api_key: None,
            spotify_client_id: None,
            spotify_oauth: None,
            spotify_sync: Default::default(),
            borrowed_bundles: Vec::new(),
        };
        let server = spawn(engine.clone(), config)?;
        assert!(server.address.starts_with("127.0.0.1:"));

        let payload = serde_json::to_string(&json!({
            "version":1, "type":"article", "url":"https://example.com/brain",
            "access":"public", "dom_extract":{
                "title":"A captured idea", "text":"Local knowledge remains searchable."
            }
        }))?;
        let unauthorized = post(&server.address, &payload, None)?;
        assert!(unauthorized.starts_with("HTTP/1.1 401"));
        let authorized = post(&server.address, &payload, Some("secret-token"))?;
        assert!(authorized.starts_with("HTTP/1.1 201"));
        assert_eq!(
            engine
                .search_scoped("knowledge", 5, crate::model::SearchScope::Own)?
                .len(),
            1
        );
        drop(server);
        Ok(())
    }

    #[test]
    fn records_extension_handshake_and_serves_capture_request() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let engine = Arc::new(Engine::with_embedder(
            temp.path().to_owned(),
            Arc::new(TestEmbedder),
        ));
        let config = LocalConfig {
            version: 1,
            capture_port: 0,
            capture_token: "secret-token".into(),
            query_relevance_floor: 0.35,
            extension: Default::default(),
            pending_capture_request: None,
            youtube_api_key: None,
            spotify_client_id: None,
            spotify_oauth: None,
            spotify_sync: Default::default(),
            borrowed_bundles: Vec::new(),
        };
        let server = spawn(engine.clone(), config)?;
        let hello = post_path(
            &server.address,
            "/extension/hello",
            r#"{"browser":"Chrome","extension_version":"0.1.0","can_capture_active_tab":true}"#,
            Some("secret-token"),
        )?;
        assert!(hello.starts_with("HTTP/1.1 200"));
        assert!(engine.local_config()?.extension.last_handshake_at.is_some());

        let created = post_path(
            &server.address,
            "/capture-request",
            r#"{"source":"test"}"#,
            Some("secret-token"),
        )?;
        assert!(created.starts_with("HTTP/1.1 202"));

        let first = get_path(&server.address, "/capture-request", Some("secret-token"))?;
        assert!(first.contains("\"request\""));
        assert!(first.contains("\"id\""));
        let second = get_path(&server.address, "/capture-request", Some("secret-token"))?;
        assert!(second.contains("\"request\":null"));
        drop(server);
        Ok(())
    }

    fn post(address: &str, body: &str, token: Option<&str>) -> Result<String> {
        post_path(address, "/capture", body, token)
    }

    fn post_path(address: &str, path: &str, body: &str, token: Option<&str>) -> Result<String> {
        let mut stream = TcpStream::connect(address)?;
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{authorization}Connection: close\r\n\r\n{body}",
            body.len()
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    }

    fn get_path(address: &str, path: &str, token: Option<&str>) -> Result<String> {
        let mut stream = TcpStream::connect(address)?;
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {address}\r\n{authorization}Connection: close\r\n\r\n"
        )?;
        let mut response = String::new();
        stream.read_to_string(&mut response)?;
        Ok(response)
    }
}
