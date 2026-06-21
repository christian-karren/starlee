use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Access, CaptureInput, SourceType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturePayload {
    #[serde(default = "payload_version")]
    pub version: u32,
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub url: String,
    pub dom_extract: DomExtract,
    #[serde(default)]
    pub transcript: Vec<TranscriptSegment>,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomExtract {
    pub title: String,
    #[serde(default)]
    pub byline: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub html_meta: Value,
    #[serde(default)]
    pub selected_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub t: f64,
    pub text: String,
}

impl CapturePayload {
    pub fn into_input(self) -> Result<CaptureInput> {
        if self.version != 1 {
            bail!("unsupported capture payload version: {}", self.version);
        }
        if !matches!(self.source_type, SourceType::Article | SourceType::Youtube) {
            bail!("browser capture type must be article or youtube");
        }
        let text = match self.source_type {
            SourceType::Youtube if !self.transcript.is_empty() => self
                .transcript
                .iter()
                .map(|segment| format!("[{}] {}", timestamp(segment.t), segment.text.trim()))
                .collect::<Vec<_>>()
                .join("\n"),
            SourceType::Youtube if self.dom_extract.text.trim().is_empty() => {
                "[Transcript unavailable]".into()
            }
            _ => {
                let body = self.dom_extract.text.trim();
                if body.is_empty() {
                    self.dom_extract
                        .selected_text
                        .as_deref()
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                } else {
                    body.to_owned()
                }
            }
        };
        if text.is_empty() {
            bail!("captured article text cannot be empty");
        }
        let video_id = youtube_video_id(&self.url);
        Ok(CaptureInput {
            title: self.dom_extract.title,
            text,
            source_type: self.source_type,
            access: self.access,
            author: self.dom_extract.byline,
            site: self.dom_extract.site,
            url: Some(self.url),
            published_at: self.dom_extract.published_at,
            duration: None,
            video_id,
            summary: self.dom_extract.summary,
            tags: self.tags,
            spotify_episode_id: None,
            spotify_show_id: None,
            show: None,
            listen_duration_s: None,
            listen_progress_pct: None,
            transcript_status: None,
            transcript_source: None,
            matched_youtube_id: None,
            linked_youtube_id: None,
            description: None,
        })
    }
}

fn youtube_video_id(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if url.host_str()?.ends_with("youtu.be") {
        return url.path_segments()?.next().map(str::to_owned);
    }
    url.query_pairs()
        .find(|(key, _)| key == "v")
        .map(|(_, value)| value.into_owned())
}

fn payload_version() -> u32 {
    1
}

fn timestamp(seconds: f64) -> String {
    let seconds = seconds.max(0.0).floor() as u64;
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_timestamped_youtube_transcript() -> Result<()> {
        let payload = CapturePayload {
            version: 1,
            source_type: SourceType::Youtube,
            url: "https://youtu.be/test".into(),
            dom_extract: DomExtract {
                title: "Test".into(),
                byline: None,
                site: Some("youtube.com".into()),
                text: String::new(),
                published_at: None,
                summary: None,
                html_meta: Value::Null,
                selected_text: None,
            },
            transcript: vec![TranscriptSegment {
                t: 62.5,
                text: "A useful idea".into(),
            }],
            access: Access::Restricted,
            tags: Vec::new(),
        };
        assert_eq!(payload.into_input()?.text, "[01:02] A useful idea");
        Ok(())
    }

    #[test]
    fn accepts_selected_text_when_article_body_is_empty() -> Result<()> {
        let payload = CapturePayload {
            version: 1,
            source_type: SourceType::Article,
            url: "https://example.com/story".into(),
            dom_extract: DomExtract {
                title: "Selected passage".into(),
                byline: None,
                site: Some("example.com".into()),
                text: String::new(),
                published_at: None,
                summary: None,
                html_meta: Value::Null,
                selected_text: Some("A selected passage from the rendered page.".into()),
            },
            transcript: Vec::new(),
            access: Access::Restricted,
            tags: Vec::new(),
        };
        assert_eq!(
            payload.into_input()?.text,
            "A selected passage from the rendered page."
        );
        Ok(())
    }
}
