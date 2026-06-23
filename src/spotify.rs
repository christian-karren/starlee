use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    process::Command,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveTime, TimeDelta, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{
    config::{LocalConfig, SpotifyOAuthConfig},
    model::{Access, CaptureInput, SourceType, SpotifySyncEvent},
};

pub const REQUIRED_SCOPES: &[&str] = &["user-read-recently-played", "user-library-read"];

pub const SPOTIFY_SYNC_DETAIL: &str = "Spotify recently-played is polled without a type parameter; podcast episodes are filtered client-side.";

const AUTH_URL: &str = "https://accounts.spotify.com/authorize";
const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const RECENTLY_PLAYED_URL: &str = "https://api.spotify.com/v1/me/player/recently-played?limit=50";
const MIN_LISTEN_SECONDS: u64 = 10 * 60;
const CALLBACK_PORT: u16 = 8888;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConfigureReport {
    pub configured: bool,
    pub client_id_stored: bool,
    pub oauth_ready: bool,
    pub required_scopes: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncStatus {
    pub oauth_configured: bool,
    pub account: Option<String>,
    pub last_synced_at: Option<String>,
    pub next_sync_at: Option<String>,
    pub last_result_status: Option<String>,
    pub hourly_window: String,
    pub api_limitation: String,
    pub viable_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifySyncReport {
    pub ok: bool,
    pub checked_at: String,
    pub added: usize,
    pub skipped: usize,
    pub status: String,
    pub api_limitation: String,
    pub next_action: String,
}

#[derive(Debug, Clone)]
pub struct SpotifySyncEpisode {
    pub input: CaptureInput,
}

#[derive(Debug, Clone)]
pub struct SpotifySyncPlan {
    pub episodes: Vec<SpotifySyncEpisode>,
    pub events: Vec<SpotifySyncEvent>,
}

pub fn status(config: &LocalConfig) -> SpotifySyncStatus {
    SpotifySyncStatus {
        oauth_configured: oauth_is_valid(config),
        account: config
            .spotify_oauth
            .as_ref()
            .and_then(|oauth| oauth.display_name.clone().or_else(|| oauth.user_id.clone())),
        last_synced_at: config.spotify_sync.last_synced_at.clone(),
        next_sync_at: config
            .spotify_sync
            .next_sync_at
            .clone()
            .or_else(|| Some(next_sync_at(Local::now()).to_rfc3339())),
        last_result_status: config
            .spotify_sync
            .last_result
            .as_ref()
            .map(|result| result.status.clone()),
        hourly_window: "06:00-23:00 local time".into(),
        api_limitation: SPOTIFY_SYNC_DETAIL.into(),
        viable_strategy: "Poll Spotify recently-played, then keep podcast episodes from the mixed playback history.".into(),
    }
}

pub fn oauth_is_valid(config: &LocalConfig) -> bool {
    let Some(oauth) = config.spotify_oauth.as_ref() else {
        return false;
    };
    if oauth.access_token.trim().is_empty() {
        return false;
    }
    DateTime::parse_from_rfc3339(&oauth.expires_at)
        .map(|expires_at| expires_at.with_timezone(&Utc) > Utc::now())
        .unwrap_or(false)
}

pub fn next_sync_at(now: DateTime<Local>) -> DateTime<Local> {
    let start = NaiveTime::from_hms_opt(6, 0, 0).expect("valid time");
    let end = NaiveTime::from_hms_opt(23, 0, 0).expect("valid time");
    let local_time = now.time();
    let today = now.date_naive();
    let candidate = if local_time < start {
        today.and_time(start)
    } else if local_time >= end {
        (today + TimeDelta::days(1)).and_time(start)
    } else {
        let next_hour = now + TimeDelta::hours(1);
        next_hour
            .date_naive()
            .and_hms_opt(next_hour.hour(), 0, 0)
            .expect("valid hour")
    };
    candidate
        .and_local_timezone(now.timezone())
        .single()
        .unwrap_or(now + TimeDelta::hours(1))
}

pub fn configure_oauth(config: &LocalConfig) -> Result<SpotifyOAuthConfig> {
    let client_id = config
        .spotify_client_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("spotify_client_id is missing in ~/Starlee/config.json")?;
    let redirect_uri = config
        .spotify_redirect_uri
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("http://127.0.0.1:8888/callback");
    let verifier = random_urlsafe(64)?;
    let challenge = code_challenge(&verifier);
    let state = random_urlsafe(32)?;
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .with_context(|| format!("listen for Spotify callback on port {CALLBACK_PORT}"))?;

    let authorize_url = authorize_url(client_id, redirect_uri, &challenge, &state);
    open_browser(&authorize_url)?;
    let code = wait_for_code(listener, &state)?;
    let token = exchange_code(client_id, redirect_uri, &verifier, &code)?;
    let profile = spotify_profile(&token.access_token).ok();
    Ok(SpotifyOAuthConfig {
        client_id: client_id.to_owned(),
        display_name: profile
            .as_ref()
            .and_then(|profile| profile.display_name.clone()),
        user_id: profile.and_then(|profile| profile.id),
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .context("Spotify did not return a refresh token")?,
        expires_at: (Utc::now() + TimeDelta::seconds(token.expires_in)).to_rfc3339(),
    })
}

pub fn refresh_oauth(oauth: &SpotifyOAuthConfig) -> Result<SpotifyOAuthConfig> {
    let form = [
        ("grant_type", "refresh_token".to_owned()),
        ("refresh_token", oauth.refresh_token.clone()),
        ("client_id", oauth.client_id.clone()),
    ];
    let mut response = ureq::post(TOKEN_URL)
        .send_form(form.iter().map(|(key, value)| (*key, value.as_str())))
        .context("refresh Spotify access token")?;
    let token: TokenResponse = response
        .body_mut()
        .read_json()
        .context("parse Spotify refresh token response")?;
    Ok(SpotifyOAuthConfig {
        client_id: oauth.client_id.clone(),
        display_name: oauth.display_name.clone(),
        user_id: oauth.user_id.clone(),
        access_token: token.access_token,
        refresh_token: token
            .refresh_token
            .unwrap_or_else(|| oauth.refresh_token.clone()),
        expires_at: (Utc::now() + TimeDelta::seconds(token.expires_in)).to_rfc3339(),
    })
}

pub fn recently_played_sync_plan(access_token: &str) -> Result<SpotifySyncPlan> {
    let mut response = ureq::get(RECENTLY_PLAYED_URL)
        .header("Authorization", format!("Bearer {access_token}"))
        .call()
        .context("request Spotify recently-played history")?;
    let played: RecentlyPlayedResponse = response
        .body_mut()
        .read_json()
        .context("parse Spotify recently-played history")?;
    Ok(plan_from_recently_played(&played))
}

fn plan_from_recently_played(played: &RecentlyPlayedResponse) -> SpotifySyncPlan {
    let listen_durations = inferred_listen_durations(&played.items);
    let mut episodes = Vec::new();
    let mut events = Vec::new();
    let timestamp = Utc::now().to_rfc3339();
    for (index, played_item) in played.items.iter().enumerate() {
        let Some(item) = played_item.item.as_ref() else {
            continue;
        };
        if item.item_type != "episode" {
            continue;
        }
        let base_event = |outcome: &str, reason_code: &str, explanation: String| {
            let mut event = sync_event(
                &timestamp,
                Some(item),
                "detected",
                outcome,
                reason_code,
                explanation,
            );
            event.threshold_s = Some(MIN_LISTEN_SECONDS);
            event
        };
        if item.show.is_none() {
            events.push(base_event(
                "skipped",
                "episode_not_matched_in_feed",
                format!(
                    "Not captured: Spotify episode '{}' did not include show metadata.",
                    item.name
                ),
            ));
            continue;
        }
        let Some(listen_duration_s) = listen_durations[index] else {
            events.push(base_event(
                "skipped",
                "insufficient_listen_time",
                format!(
                    "Not captured: Spotify did not provide enough duration information to infer listen time; threshold is {MIN_LISTEN_SECONDS}s."
                ),
            ));
            continue;
        };
        if listen_duration_s < MIN_LISTEN_SECONDS {
            let mut event = base_event(
                "skipped",
                "insufficient_listen_time",
                format!(
                    "Not captured: only {listen_duration_s}s of listen time; threshold is {MIN_LISTEN_SECONDS}s."
                ),
            );
            event.listen_duration_s = Some(listen_duration_s);
            events.push(event);
            continue;
        }
        let mut event = base_event(
            "ok",
            "detected_ok",
            format!(
                "Detected Spotify podcast episode '{}' after an inferred {listen_duration_s}s listen.",
                item.name
            ),
        );
        event.listen_duration_s = Some(listen_duration_s);
        events.push(event);
        episodes.push(SpotifySyncEpisode {
            input: capture_input_for_episode(item, listen_duration_s),
        });
    }
    if events.is_empty() {
        let explanation = if played.items.is_empty() {
            "Spotify reported no recently played items during this poll."
        } else {
            "Spotify playback history contained no podcast episodes during this poll."
        };
        events.push(sync_event(
            &timestamp,
            None,
            "detected",
            "skipped",
            "nothing_playing",
            explanation,
        ));
    }
    SpotifySyncPlan { episodes, events }
}

fn sync_event(
    timestamp: &str,
    item: Option<&SpotifyItem>,
    stage_reached: &str,
    outcome: &str,
    reason_code: &str,
    explanation: impl Into<String>,
) -> SpotifySyncEvent {
    SpotifySyncEvent {
        id: 0,
        timestamp: timestamp.into(),
        episode_id: item.map(|item| item.id.clone()),
        episode_title: item.map(|item| item.name.clone()),
        show_name: item.and_then(|item| item.show.as_ref().map(|show| show.name.clone())),
        stage_reached: stage_reached.into(),
        outcome: outcome.into(),
        reason_code: reason_code.into(),
        explanation: explanation.into(),
        underlying_error: None,
        listen_duration_s: None,
        threshold_s: None,
    }
}

pub fn configure_report(client_id_stored: bool) -> SpotifyConfigureReport {
    SpotifyConfigureReport {
        configured: true,
        client_id_stored,
        oauth_ready: true,
        required_scopes: REQUIRED_SCOPES
            .iter()
            .map(|scope| (*scope).into())
            .collect(),
        next_action: "Spotify connected successfully".into(),
    }
}

fn authorize_url(client_id: &str, redirect_uri: &str, challenge: &str, state: &str) -> String {
    let mut url = Url::parse(AUTH_URL).expect("valid Spotify authorization URL");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", client_id)
        .append_pair("scope", &REQUIRED_SCOPES.join(" "))
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("state", state)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge);
    url.to_string()
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "start", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xdg-open", vec![url]);

    Command::new(command.0)
        .args(command.1)
        .status()
        .context("open Spotify consent page in browser")?;
    Ok(())
}

fn wait_for_code(listener: TcpListener, expected_state: &str) -> Result<String> {
    let (mut stream, _) = listener
        .accept()
        .context("wait for Spotify authorization callback")?;
    let mut request = [0_u8; 4096];
    let bytes = stream.read(&mut request)?;
    let request = String::from_utf8_lossy(&request[..bytes]);
    let first_line = request.lines().next().context("empty Spotify callback")?;
    let path = first_line
        .split_whitespace()
        .nth(1)
        .context("malformed Spotify callback")?;
    let url = Url::parse(&format!("http://127.0.0.1{path}"))?;
    let params = url.query_pairs().collect::<HashMap<_, _>>();
    let body = if params.get("state").map(|value| value.as_ref()) != Some(expected_state) {
        "Spotify connection failed: invalid state."
    } else if params.contains_key("error") {
        "Spotify connection was denied."
    } else {
        "Spotify connected successfully. You can return to Codex."
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    if params.get("state").map(|value| value.as_ref()) != Some(expected_state) {
        bail!("Spotify callback state did not match");
    }
    if let Some(error) = params.get("error") {
        bail!("Spotify authorization failed: {error}");
    }
    params
        .get("code")
        .map(|value| value.to_string())
        .context("Spotify callback did not include an authorization code")
}

fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    verifier: &str,
    code: &str,
) -> Result<TokenResponse> {
    let form = [
        ("grant_type", "authorization_code".to_owned()),
        ("code", code.to_owned()),
        ("redirect_uri", redirect_uri.to_owned()),
        ("client_id", client_id.to_owned()),
        ("code_verifier", verifier.to_owned()),
    ];
    let mut response = ureq::post(TOKEN_URL)
        .send_form(form.iter().map(|(key, value)| (*key, value.as_str())))
        .context("exchange Spotify authorization code for tokens")?;
    response
        .body_mut()
        .read_json()
        .context("parse Spotify token response")
}

fn spotify_profile(access_token: &str) -> Result<SpotifyProfile> {
    let mut response = ureq::get("https://api.spotify.com/v1/me")
        .header("Authorization", format!("Bearer {access_token}"))
        .call()
        .context("request Spotify profile")?;
    response
        .body_mut()
        .read_json()
        .context("parse Spotify profile")
}

fn capture_input_for_episode(episode: &SpotifyItem, listen_duration_s: u64) -> CaptureInput {
    let show = episode.show.as_ref();
    let title = episode.name.clone();
    let text = "[Transcript unavailable]";
    let mut input = CaptureInput::new(
        title.clone(),
        text,
        SourceType::SpotifyEpisode,
        Access::Restricted,
    );
    input.site = Some("Spotify".into());
    input.source = Some("spotify_sync".into());
    input.url = episode
        .external_urls
        .get("spotify")
        .cloned()
        .or_else(|| Some(format!("https://open.spotify.com/episode/{}", episode.id)));
    input.duration = episode.duration_ms.map(|duration| duration / 1000);
    input.spotify_episode_id = Some(episode.id.clone());
    input.spotify_show_id = show.map(|show| show.id.clone());
    input.show = show.map(|show| show.name.clone());
    input.listen_duration_s = Some(listen_duration_s);
    input.listen_progress_pct = input.duration.and_then(|duration| {
        listen_duration_s
            .saturating_mul(100)
            .checked_div(duration)
            .map(|progress| progress.min(100) as u8)
    });
    input.transcript_status = Some("missing".into());
    input.summary = show.map(|show| format!("Spotify episode from {}", show.name));
    input.description = episode.description.clone();
    input
}

fn inferred_listen_durations(items: &[RecentlyPlayedItem]) -> Vec<Option<u64>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let duration_s = item
                .item
                .as_ref()
                .and_then(|item| item.duration_ms)
                .map(|duration| duration / 1000)?;
            let gap_s = previous_item_gap_s(items, index).unwrap_or(duration_s);
            Some(duration_s.min(gap_s))
        })
        .collect()
}

fn previous_item_gap_s(items: &[RecentlyPlayedItem], index: usize) -> Option<u64> {
    if index == 0 {
        return None;
    }
    let newer = items[index - 1].played_at;
    let current = items[index].played_at;
    newer
        .signed_duration_since(current)
        .to_std()
        .ok()
        .map(|duration| duration.as_secs())
}

fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64_urlsafe(&digest)
}

fn random_urlsafe(bytes_len: usize) -> Result<String> {
    let mut bytes = vec![0_u8; bytes_len];
    getrandom::fill(&mut bytes).context("generate Spotify OAuth random value")?;
    Ok(base64_urlsafe(&bytes))
}

fn base64_urlsafe(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let chunk = ((bytes[index] as u32) << 16)
            | ((bytes[index + 1] as u32) << 8)
            | bytes[index + 2] as u32;
        out.push(ALPHABET[((chunk >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 6) & 63) as usize] as char);
        out.push(ALPHABET[(chunk & 63) as usize] as char);
        index += 3;
    }
    let remaining = bytes.len() - index;
    if remaining == 1 {
        let chunk = (bytes[index] as u32) << 16;
        out.push(ALPHABET[((chunk >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 63) as usize] as char);
    } else if remaining == 2 {
        let chunk = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(ALPHABET[((chunk >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 12) & 63) as usize] as char);
        out.push(ALPHABET[((chunk >> 6) & 63) as usize] as char);
    }
    out
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct SpotifyProfile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RecentlyPlayedResponse {
    #[serde(default)]
    items: Vec<RecentlyPlayedItem>,
}

#[derive(Debug, Deserialize)]
struct RecentlyPlayedItem {
    #[serde(default, alias = "track")]
    item: Option<SpotifyItem>,
    played_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct SpotifyItem {
    id: String,
    name: String,
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    external_urls: HashMap<String, String>,
    #[serde(default)]
    show: Option<SpotifyShow>,
}

#[derive(Debug, Deserialize)]
struct SpotifyShow {
    id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::{
        io::Read,
        net::{TcpListener, TcpStream},
        thread,
    };

    #[test]
    fn schedules_next_sync_inside_hourly_window() {
        let now = Local.with_ymd_and_hms(2026, 6, 21, 5, 30, 0).unwrap();
        assert_eq!(
            next_sync_at(now).time(),
            NaiveTime::from_hms_opt(6, 0, 0).unwrap()
        );

        let now = Local.with_ymd_and_hms(2026, 6, 21, 14, 15, 0).unwrap();
        assert_eq!(
            next_sync_at(now).time(),
            NaiveTime::from_hms_opt(15, 0, 0).unwrap()
        );

        let now = Local.with_ymd_and_hms(2026, 6, 21, 23, 30, 0).unwrap();
        assert_eq!(
            next_sync_at(now).time(),
            NaiveTime::from_hms_opt(6, 0, 0).unwrap()
        );
    }

    #[test]
    fn infers_duration_from_gap_to_newer_playback() {
        let items = vec![
            played_item("newer", "episode", 3600, "2026-06-21T12:20:00Z"),
            played_item("older", "episode", 3600, "2026-06-21T12:00:00Z"),
        ];
        assert_eq!(
            inferred_listen_durations(&items),
            vec![Some(3600), Some(1200)]
        );
    }

    #[test]
    fn builds_authorization_url_with_pkce_and_requested_scopes() {
        let url = authorize_url(
            "client123",
            "http://127.0.0.1:8888/callback",
            "challenge123",
            "state123",
        );
        let parsed = Url::parse(&url).unwrap();
        let query = parsed
            .query_pairs()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<HashMap<_, _>>();

        assert_eq!(parsed.as_str().split('?').next(), Some(AUTH_URL));
        assert_eq!(query.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            query.get("client_id").map(String::as_str),
            Some("client123")
        );
        assert_eq!(
            query.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:8888/callback")
        );
        assert_eq!(
            query.get("scope").map(String::as_str),
            Some("user-read-recently-played user-library-read")
        );
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some("challenge123")
        );
        assert_eq!(query.get("state").map(String::as_str), Some("state123"));
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn oauth_validity_requires_nonempty_future_access_token() {
        let mut config = config_with_oauth("token", Utc::now() + TimeDelta::minutes(30));
        assert!(oauth_is_valid(&config));

        config.spotify_oauth.as_mut().unwrap().access_token = "   ".into();
        assert!(!oauth_is_valid(&config));

        config.spotify_oauth.as_mut().unwrap().access_token = "token".into();
        config.spotify_oauth.as_mut().unwrap().expires_at =
            (Utc::now() - TimeDelta::minutes(1)).to_rfc3339();
        assert!(!oauth_is_valid(&config));

        config.spotify_oauth.as_mut().unwrap().expires_at = "not-a-date".into();
        assert!(!oauth_is_valid(&config));
    }

    #[test]
    fn callback_listener_accepts_valid_code_and_rejects_bad_state() -> Result<()> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            write!(
                stream,
                "GET /callback?code=abc123&state=good-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.contains("Spotify connected successfully"));
        });
        assert_eq!(wait_for_code(listener, "good-state")?, "abc123");
        sender.join().unwrap();

        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let sender = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).unwrap();
            write!(
                stream,
                "GET /callback?code=abc123&state=wrong-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.contains("invalid state"));
        });
        assert!(wait_for_code(listener, "good-state").is_err());
        sender.join().unwrap();
        Ok(())
    }

    #[test]
    fn parses_spotify_recently_played_track_alias() -> Result<()> {
        let played: RecentlyPlayedResponse = serde_json::from_str(
            r#"{
              "items": [{
                "played_at": "2026-06-22T08:00:00Z",
                "track": {
                  "id": "song1",
                  "name": "A Song",
                  "type": "track",
                  "duration_ms": 180000,
                  "external_urls": {"spotify": "https://open.spotify.com/track/song1"}
                }
              }]
            }"#,
        )?;

        assert_eq!(played.items.len(), 1);
        assert_eq!(
            played.items[0].item.as_ref().unwrap().item_type.as_str(),
            "track"
        );
        let plan = plan_from_recently_played(&played);
        assert!(plan.episodes.is_empty());
        assert_eq!(plan.events.len(), 1);
        assert_eq!(plan.events[0].reason_code, "nothing_playing");
        assert!(plan.events[0].explanation.contains("no podcast episodes"));
        Ok(())
    }

    #[test]
    fn filters_recently_played_to_qualifying_episodes() {
        let items = vec![
            played_item("latest-episode", "episode", 1800, "2026-06-22T08:30:00Z"),
            played_item("track", "track", 180, "2026-06-22T08:20:00Z"),
            played_item("short-episode", "episode", 3600, "2026-06-22T08:15:00Z"),
            played_item("long-episode", "episode", 3600, "2026-06-22T08:00:00Z"),
            played_item_without_show("episode-no-show", "2026-06-22T07:00:00Z"),
        ];
        let played = RecentlyPlayedResponse { items };
        let plan = plan_from_recently_played(&played);

        assert_eq!(plan.episodes.len(), 2);
        assert_eq!(
            plan.events
                .iter()
                .filter(|event| event.outcome == "skipped")
                .count(),
            2
        );
        assert_eq!(
            plan.episodes[0].input.spotify_episode_id.as_deref(),
            Some("latest-episode")
        );
        assert_eq!(
            plan.episodes[1].input.spotify_episode_id.as_deref(),
            Some("long-episode")
        );
        assert_eq!(plan.episodes[1].input.listen_duration_s, Some(900));
        assert_eq!(
            plan.episodes[1].input.source.as_deref(),
            Some("spotify_sync")
        );
        assert_eq!(
            plan.episodes[1].input.transcript_status.as_deref(),
            Some("missing")
        );
        assert!(plan.events.iter().any(|event| {
            event.reason_code == "insufficient_listen_time"
                && event.explanation.contains("only 300s")
        }));
        assert!(plan.events.iter().any(|event| {
            event.reason_code == "episode_not_matched_in_feed"
                && event.episode_id.as_deref() == Some("episode-no-show")
        }));
    }

    #[test]
    fn episode_capture_falls_back_to_open_spotify_url_and_caps_progress() {
        let episode = SpotifyItem {
            id: "ep123".into(),
            name: "Deep Episode".into(),
            item_type: "episode".into(),
            duration_ms: Some(1200 * 1000),
            description: Some("Episode description".into()),
            external_urls: HashMap::new(),
            show: Some(SpotifyShow {
                id: "show123".into(),
                name: "Deep Show".into(),
            }),
        };

        let input = capture_input_for_episode(&episode, 3600);
        assert_eq!(
            input.url.as_deref(),
            Some("https://open.spotify.com/episode/ep123")
        );
        assert_eq!(input.listen_progress_pct, Some(100));
        assert_eq!(input.show.as_deref(), Some("Deep Show"));
        assert_eq!(input.spotify_show_id.as_deref(), Some("show123"));
        assert_eq!(input.description.as_deref(), Some("Episode description"));
    }

    fn played_item(
        id: &str,
        item_type: &str,
        duration_s: u64,
        played_at: &str,
    ) -> RecentlyPlayedItem {
        RecentlyPlayedItem {
            item: Some(SpotifyItem {
                id: id.into(),
                name: id.into(),
                item_type: item_type.into(),
                duration_ms: Some(duration_s * 1000),
                description: None,
                external_urls: HashMap::new(),
                show: Some(SpotifyShow {
                    id: "show".into(),
                    name: "Show".into(),
                }),
            }),
            played_at: DateTime::parse_from_rfc3339(played_at)
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    fn played_item_without_show(id: &str, played_at: &str) -> RecentlyPlayedItem {
        RecentlyPlayedItem {
            item: Some(SpotifyItem {
                id: id.into(),
                name: id.into(),
                item_type: "episode".into(),
                duration_ms: Some(3600 * 1000),
                description: None,
                external_urls: HashMap::new(),
                show: None,
            }),
            played_at: DateTime::parse_from_rfc3339(played_at)
                .unwrap()
                .with_timezone(&Utc),
        }
    }

    fn config_with_oauth(access_token: &str, expires_at: DateTime<Utc>) -> LocalConfig {
        LocalConfig {
            version: 1,
            capture_port: crate::config::DEFAULT_CAPTURE_PORT,
            capture_token: "a".repeat(64),
            query_relevance_floor: 0.35,
            extension: crate::config::ExtensionState::default(),
            pending_capture_request: None,
            capture_request_status: None,
            capture_diagnostics: Vec::new(),
            youtube_api_key: None,
            spotify_client_id: Some("client123".into()),
            spotify_redirect_uri: Some("http://127.0.0.1:8888/callback".into()),
            spotify_oauth: Some(SpotifyOAuthConfig {
                client_id: "client123".into(),
                display_name: Some("User".into()),
                user_id: Some("user123".into()),
                access_token: access_token.into(),
                refresh_token: "refresh".into(),
                expires_at: expires_at.to_rfc3339(),
            }),
            spotify_sync: crate::config::SpotifySyncConfig::default(),
            borrowed_bundles: Vec::new(),
        }
    }
}
