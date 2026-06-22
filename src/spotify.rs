use chrono::{DateTime, Local, NaiveTime, TimeDelta, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::config::LocalConfig;

pub const REQUIRED_SCOPES: &[&str] = &[
    "user-read-currently-playing",
    "user-read-playback-state",
    "user-read-recently-played",
    "user-library-read",
];

pub const EPISODE_HISTORY_LIMITATION: &str =
    "Spotify Web API /me/player/recently-played currently does not support podcast episodes.";

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

pub fn status(config: &LocalConfig) -> SpotifySyncStatus {
    SpotifySyncStatus {
        oauth_configured: config.spotify_oauth.is_some(),
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
        hourly_window: "06:00–23:00 local time".into(),
        api_limitation: EPISODE_HISTORY_LIMITATION.into(),
        viable_strategy: "Use Spotify current-playback polling for episodes in progress, or add a mobile/desktop companion that records local playback events. True passive episode history is not available from Spotify's recently-played endpoint.".into(),
    }
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

pub fn unsupported_sync_report() -> SpotifySyncReport {
    SpotifySyncReport {
        ok: false,
        checked_at: Utc::now().to_rfc3339(),
        added: 0,
        skipped: 0,
        status: "blocked_by_spotify_api".into(),
        api_limitation: EPISODE_HISTORY_LIMITATION.into(),
        next_action: "Decide whether Starlee should pivot Spotify passive sync to current-playback sampling, a mobile companion, or user-triggered Spotify share-sheet capture.".into(),
    }
}

pub fn configure_report(client_id_stored: bool) -> SpotifyConfigureReport {
    SpotifyConfigureReport {
        configured: false,
        client_id_stored,
        oauth_ready: false,
        required_scopes: REQUIRED_SCOPES.iter().map(|scope| (*scope).into()).collect(),
        next_action: "Spotify client id is stored, but the OAuth callback/token exchange is intentionally not completed until the passive episode-history strategy is resolved.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn documents_spotify_episode_history_limitation() {
        let report = unsupported_sync_report();
        assert!(!report.ok);
        assert_eq!(report.status, "blocked_by_spotify_api");
        assert!(report.api_limitation.contains("podcast episodes"));
    }

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
}
