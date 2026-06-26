//! Browser capture bridge state, diagnostics, and health helpers.

use std::{collections::BTreeMap, net::TcpStream, process::Command, time::Duration};

use chrono::{DateTime, Utc};

use crate::config::{
    CaptureDiagnosticEvent, CaptureRequestPageMetadata, ExtensionState, LocalConfig,
};

// Generous enough to let the browser open the YouTube transcript panel and wait
// out a slow transcript load (which can take several seconds) before the request
// is considered stale.
pub const CAPTURE_REQUEST_TTL: Duration = Duration::from_secs(25);
pub const EXTENSION_HEARTBEAT_FRESHNESS: Duration = Duration::from_secs(5 * 60);
const CAPTURE_DIAGNOSTIC_LIMIT: usize = 120;

pub const CAPTURE_STATUS_QUEUED: &str = "queued";
pub const CAPTURE_STATUS_PICKED_UP: &str = "picked_up";
pub const CAPTURE_STATUS_EXTRACTING: &str = "extracting";
pub const CAPTURE_STATUS_POSTED: &str = "posted";
pub const CAPTURE_STATUS_SAVED: &str = "capture_saved";
pub const CAPTURE_STATUS_FAILED: &str = "capture_failed";
pub const CAPTURE_STATUS_PERMISSION_DENIED: &str = "permission_denied";
pub const CAPTURE_STATUS_UNSUPPORTED_PAGE: &str = "unsupported_page";
pub const CAPTURE_STATUS_EXTENSION_UNAVAILABLE: &str = "extension_unavailable";
pub const CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE: &str = "content_script_unreachable";
pub const CAPTURE_STATUS_TIMED_OUT: &str = "timed_out";

pub(crate) fn capture_service_reachable(port: u16) -> bool {
    TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(250),
    )
    .is_ok()
}

pub(crate) fn process_running(pattern: &str) -> bool {
    Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub(crate) fn extension_is_fresh(extension: &ExtensionState, max_age: Duration) -> bool {
    extension.can_capture_active_tab && extension_heartbeat_is_fresh(extension, max_age)
}

pub(crate) fn extension_heartbeat_is_fresh(extension: &ExtensionState, max_age: Duration) -> bool {
    let Some(last_handshake_at) = extension.last_handshake_at.as_deref() else {
        return false;
    };
    let Some(last_handshake_at) = parse_rfc3339_utc(last_handshake_at) else {
        return false;
    };
    let Ok(max_age) = chrono::TimeDelta::from_std(max_age) else {
        return false;
    };
    Utc::now().signed_duration_since(last_handshake_at) <= max_age
}

pub(crate) fn expire_stale_capture_request(config: &mut LocalConfig) -> bool {
    let Some(status) = config.capture_request_status.as_mut() else {
        return clear_pending_capture_request(config);
    };
    if capture_status_is_terminal(&status.status) {
        return clear_pending_capture_request(config);
    }
    let Some(requested_at) = parse_rfc3339_utc(&status.requested_at) else {
        return false;
    };
    let Ok(ttl) = chrono::TimeDelta::from_std(CAPTURE_REQUEST_TTL) else {
        return false;
    };
    if Utc::now().signed_duration_since(requested_at) <= ttl {
        return false;
    }
    let event = {
        status.status = CAPTURE_STATUS_TIMED_OUT.into();
        status.completed_at = Some(Utc::now().to_rfc3339());
        status.message = Some("The browser did not pick up the request in time.".into());
        diagnostic_event(DiagnosticEventInput {
            component: "engine",
            event: "capture_request_timed_out",
            request_id: Some(&status.id),
            status: Some(&status.status),
            source: Some(&status.source),
            browser: status.browser.as_deref(),
            message: status.message.as_deref(),
            page: status.page.clone(),
        })
    };
    append_capture_diagnostic(config, event);
    config.pending_capture_request = None;
    true
}

fn clear_pending_capture_request(config: &mut LocalConfig) -> bool {
    if config.pending_capture_request.is_some() {
        config.pending_capture_request = None;
        return true;
    }
    false
}

pub(crate) fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

pub(crate) fn normalize_capture_request_status(status: &str) -> String {
    match status {
        CAPTURE_STATUS_QUEUED
        | CAPTURE_STATUS_PICKED_UP
        | CAPTURE_STATUS_EXTRACTING
        | CAPTURE_STATUS_POSTED
        | CAPTURE_STATUS_SAVED
        | CAPTURE_STATUS_FAILED
        | CAPTURE_STATUS_PERMISSION_DENIED
        | CAPTURE_STATUS_UNSUPPORTED_PAGE
        | CAPTURE_STATUS_EXTENSION_UNAVAILABLE
        | CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE
        | CAPTURE_STATUS_TIMED_OUT => status.to_owned(),
        "service_down" | "token_missing" | "token_invalid" | "payload_too_large"
        | "empty_extract" | "no_active_tab" => CAPTURE_STATUS_FAILED.into(),
        _ => CAPTURE_STATUS_FAILED.into(),
    }
}

pub(crate) fn capture_status_is_terminal(status: &str) -> bool {
    matches!(
        status,
        CAPTURE_STATUS_SAVED
            | CAPTURE_STATUS_FAILED
            | CAPTURE_STATUS_PERMISSION_DENIED
            | CAPTURE_STATUS_UNSUPPORTED_PAGE
            | CAPTURE_STATUS_EXTENSION_UNAVAILABLE
            | CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE
            | CAPTURE_STATUS_TIMED_OUT
    )
}

pub(crate) fn default_capture_status_message(status: &str) -> Option<String> {
    let message = match status {
        CAPTURE_STATUS_EXTRACTING => "Browser extension is extracting the active tab.",
        CAPTURE_STATUS_POSTED => "Browser extension posted the capture to Starlee.",
        CAPTURE_STATUS_SAVED => "Saved to Starlee.",
        CAPTURE_STATUS_FAILED => "Starlee capture failed.",
        CAPTURE_STATUS_PERMISSION_DENIED => {
            "Grant Starlee site access in the browser, or reload the page and try again."
        }
        CAPTURE_STATUS_UNSUPPORTED_PAGE => {
            "The active page is not an article or YouTube watch page Starlee can capture."
        }
        CAPTURE_STATUS_EXTENSION_UNAVAILABLE => {
            "Load or reload the Starlee browser extension, then try again."
        }
        CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE => {
            "The Starlee content script was not running in the page."
        }
        CAPTURE_STATUS_TIMED_OUT => "The browser did not pick up the request in time.",
        _ => return None,
    };
    Some(message.into())
}

pub(crate) fn safe_bridge_failure_message(
    status: &str,
    stored_message: Option<&str>,
) -> Option<String> {
    match status {
        CAPTURE_STATUS_PERMISSION_DENIED
        | CAPTURE_STATUS_UNSUPPORTED_PAGE
        | CAPTURE_STATUS_EXTENSION_UNAVAILABLE
        | CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE
        | CAPTURE_STATUS_TIMED_OUT
        | CAPTURE_STATUS_FAILED => default_capture_status_message(status),
        _ => stored_message
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(240).collect()),
    }
}

pub(crate) fn bridge_next_action(
    extension_setup_present: bool,
    extension_config_present: bool,
    checked_in_recently: bool,
    can_capture_active_tab: bool,
    last_request_status: Option<&str>,
) -> String {
    if !extension_setup_present || !extension_config_present {
        return "Run `starlee setup`, then load or reload ~/Starlee/sensor-extension in your browser."
            .into();
    }
    if !checked_in_recently {
        return "Load or reload the Starlee browser extension, then try again.".into();
    }
    if !can_capture_active_tab {
        return "Grant Starlee site access in the browser, or reload the page and try again."
            .into();
    }
    match last_request_status {
        Some(CAPTURE_STATUS_PERMISSION_DENIED) => {
            "Grant Starlee site access in the browser, or reload the page and try again.".into()
        }
        Some(CAPTURE_STATUS_UNSUPPORTED_PAGE) => {
            "Open an article or YouTube watch page, then try capture again.".into()
        }
        Some(CAPTURE_STATUS_TIMED_OUT) => {
            "Make the target browser window active, reload the extension, then try again.".into()
        }
        Some(CAPTURE_STATUS_EXTENSION_UNAVAILABLE) => {
            "Load or reload the Starlee browser extension, then try again.".into()
        }
        Some(CAPTURE_STATUS_CONTENT_SCRIPT_UNREACHABLE) => {
            "Reload the page so the Starlee content script loads (it does not inject into tabs opened before the extension was enabled), confirm the extension is enabled and allowed on this site, then try capture again.".into()
        }
        Some(CAPTURE_STATUS_FAILED) => {
            "Retry capture from the active tab; run `starlee doctor` if it fails again.".into()
        }
        _ => "Bridge is ready. Open an article or YouTube watch page and capture again.".into(),
    }
}

/// Derives a transcript-specific next action from a request's diagnostic trace.
///
/// `bridge_next_action` only knows about install/heartbeat/permission state, so a
/// YouTube capture that reached the page but came back without a transcript would
/// otherwise be told "Bridge is ready." This inspects the YouTube extractor events
/// and maps the recorded reason code to the precise recovery step. Returns `None`
/// when the transcript was captured, when extraction never ran, or when the trace
/// has no YouTube extractor events (a non-YouTube capture).
pub(crate) fn youtube_transcript_next_action(events: &[CaptureDiagnosticEvent]) -> Option<String> {
    let segments = events
        .iter()
        .rev()
        .find(|event| event.event == "youtube_segments_extracted");
    if let Some(event) = segments {
        if event.status.as_deref() == Some("ok") {
            return None;
        }
        let reason = event
            .safe_metadata
            .get("transcript_reason")
            .map(String::as_str)
            .unwrap_or("");
        return Some(youtube_reason_action(reason).into());
    }
    // Metadata failed before transcript discovery ever started.
    let metadata_failed = events.iter().rev().any(|event| {
        event.event == "youtube_metadata_extracted" && event.status.as_deref() == Some("failed")
    });
    if metadata_failed {
        return Some(youtube_reason_action("youtube_metadata_unavailable").into());
    }
    None
}

/// Maps a YouTube transcript reason code to a single, safe recovery instruction.
/// Unknown reasons fall through to a generic transcript-retry action.
pub(crate) fn youtube_reason_action(reason: &str) -> &'static str {
    match reason {
        "rendered_transcript_segments_found" => {
            "Transcript captured. Open an article or YouTube watch page and capture again."
        }
        "transcript_disabled_by_video" => {
            "This video does not provide a transcript (captions are disabled). Starlee saved the video metadata only."
        }
        "transcript_language_unavailable" => {
            "No transcript is available in a supported language for this video. Starlee saved the video metadata only."
        }
        "transcript_rows_empty" => {
            "The transcript panel opened but rendered no lines. Reload the YouTube tab, open the transcript once, then capture again."
        }
        "transcript_panel_still_loading" => {
            "The transcript was still loading when capture finished. Reload the tab, wait for the video to settle, then capture again."
        }
        "transcript_panel_not_opened" => {
            "Starlee found the transcript control but the panel did not open. Open the transcript once manually, then capture again."
        }
        "transcript_button_not_found" => {
            "No \"Show transcript\" control was found. Expand the video description to reveal it, or open the transcript once manually, then capture again."
        }
        "transcript_discovery_timed_out" => {
            "Transcript discovery timed out before lines rendered. Reload the YouTube tab and capture again once the page has fully loaded."
        }
        "transcript_panel_not_rendered" => {
            "The transcript had not rendered when capture ran. Reload the YouTube tab so the content script runs after the page loads, then capture again."
        }
        "youtube_metadata_unavailable" => {
            "The YouTube watch page had not finished loading (video metadata was unavailable). Reload the tab and capture again."
        }
        "extractor_failure" => {
            "YouTube extraction failed before reading the page. Reload the YouTube tab and capture again."
        }
        _ => {
            "Starlee could not capture this video's transcript. Reload the YouTube tab, open the transcript once, then capture again."
        }
    }
}

pub(crate) fn sanitize_page_metadata(
    page: CaptureRequestPageMetadata,
) -> CaptureRequestPageMetadata {
    CaptureRequestPageMetadata {
        title: page
            .title
            .and_then(|value| sanitize_metadata_string(&value, 240)),
        url: page
            .url
            .and_then(|value| sanitize_metadata_string(&value, 2048)),
        domain: page
            .domain
            .and_then(|value| sanitize_metadata_string(&value, 255)),
    }
}

pub(crate) fn recent_diagnostics(
    config: &LocalConfig,
    limit: usize,
) -> Vec<CaptureDiagnosticEvent> {
    config
        .capture_diagnostics
        .iter()
        .rev()
        .take(limit)
        .map(|event| {
            let mut event = event.clone();
            event.request_id = None;
            event.page = None;
            event
        })
        .collect()
}

pub(crate) fn append_capture_diagnostic(config: &mut LocalConfig, event: CaptureDiagnosticEvent) {
    config.capture_diagnostics.push(event);
    let excess = config
        .capture_diagnostics
        .len()
        .saturating_sub(CAPTURE_DIAGNOSTIC_LIMIT);
    if excess > 0 {
        config.capture_diagnostics.drain(0..excess);
    }
}

pub(crate) fn sanitize_capture_diagnostic_event(
    event: CaptureDiagnosticEvent,
) -> CaptureDiagnosticEvent {
    let mut safe_metadata = BTreeMap::new();
    for (key, value) in event.safe_metadata {
        if let (Some(key), Some(value)) = (
            sanitize_metadata_key(&key),
            sanitize_metadata_string(&value, 240).filter(|value| !contains_sensitive_token(value)),
        ) {
            safe_metadata.insert(key, value);
        }
    }
    CaptureDiagnosticEvent {
        timestamp: parse_rfc3339_utc(&event.timestamp)
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| Utc::now().to_rfc3339()),
        component: sanitize_metadata_string(&event.component, 64)
            .unwrap_or_else(|| "unknown".into()),
        event: sanitize_metadata_string(&event.event, 96).unwrap_or_else(|| "unknown".into()),
        request_id: event
            .request_id
            .and_then(|value| sanitize_metadata_string(&value, 64)),
        status: event
            .status
            .and_then(|value| sanitize_metadata_string(&value, 64)),
        source: event
            .source
            .and_then(|value| sanitize_metadata_string(&value, 64)),
        browser: event
            .browser
            .and_then(|value| sanitize_metadata_string(&value, 80)),
        message: event
            .message
            .and_then(|value| sanitize_metadata_string(&value, 240))
            .map(|value| {
                if contains_sensitive_token(&value) {
                    "[redacted]".into()
                } else {
                    value
                }
            }),
        page: event.page.map(sanitize_page_metadata),
        safe_metadata,
    }
}

// Defense-in-depth: even though the extension never puts secrets into diagnostic
// values or messages, drop/redact anything that looks like a credential so a future
// regression cannot leak one. Catches SAPISIDHASH/Bearer headers and long hex runs
// (40-char SHA-1, 64-char capture token).
fn contains_sensitive_token(value: &str) -> bool {
    if value.contains("SAPISIDHASH") || value.contains("Bearer ") {
        return true;
    }
    let mut hex_run = 0usize;
    for character in value.chars() {
        if character.is_ascii_hexdigit() {
            hex_run += 1;
            if hex_run >= 40 {
                return true;
            }
        } else {
            hex_run = 0;
        }
    }
    false
}

fn sanitize_metadata_key(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 80 {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    if [
        "token",
        "cookie",
        "html",
        "body",
        "selected_text",
        "transcript_text",
        "embedding",
        "oauth",
        "sapisid",
        "bearer",
        "secret",
        "credential",
        "password",
    ]
    .iter()
    .any(|forbidden| lower.contains(forbidden))
    {
        return None;
    }
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        Some(value.to_owned())
    } else {
        None
    }
}

pub(crate) struct DiagnosticEventInput<'a> {
    pub(crate) component: &'a str,
    pub(crate) event: &'a str,
    pub(crate) request_id: Option<&'a str>,
    pub(crate) status: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
    pub(crate) browser: Option<&'a str>,
    pub(crate) message: Option<&'a str>,
    pub(crate) page: Option<CaptureRequestPageMetadata>,
}

pub(crate) fn diagnostic_event(input: DiagnosticEventInput<'_>) -> CaptureDiagnosticEvent {
    CaptureDiagnosticEvent {
        timestamp: Utc::now().to_rfc3339(),
        component: input.component.into(),
        event: input.event.into(),
        request_id: input
            .request_id
            .and_then(|value| sanitize_metadata_string(value, 64)),
        status: input
            .status
            .and_then(|value| sanitize_metadata_string(value, 64)),
        source: input
            .source
            .and_then(|value| sanitize_metadata_string(value, 64)),
        browser: input
            .browser
            .and_then(|value| sanitize_metadata_string(value, 80)),
        message: input
            .message
            .and_then(|value| sanitize_metadata_string(value, 240)),
        page: input.page.map(sanitize_page_metadata),
        safe_metadata: BTreeMap::new(),
    }
}

fn sanitize_metadata_string(value: &str, max_chars: usize) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.chars().take(max_chars).collect())
}
