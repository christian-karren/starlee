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
    pub transcript_status: Option<String>,
    #[serde(default)]
    pub transcript_source: Option<String>,
    #[serde(default)]
    pub transcript_reason: Option<String>,
    #[serde(default)]
    pub extractor_version: Option<String>,
    #[serde(default)]
    pub access: Access,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub consumed_at: Option<String>,
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
    pub fn into_input(mut self) -> Result<CaptureInput> {
        if self.version != 1 {
            bail!("unsupported capture payload version: {}", self.version);
        }
        if !matches!(self.source_type, SourceType::Article | SourceType::Youtube) {
            bail!("browser capture type must be article or youtube");
        }
        let video_id = if matches!(self.source_type, SourceType::Youtube) {
            let video_id = youtube_video_id(&self.url).or_else(|| {
                self.dom_extract
                    .html_meta
                    .get("starlee:youtube_video_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            });
            let Some(video_id) = video_id.filter(|value| !value.trim().is_empty()) else {
                bail!("youtube captures require a video id");
            };
            if self.dom_extract.title.trim().is_empty() {
                bail!("youtube captures require a title");
            }
            self.url = canonical_youtube_url(&video_id);
            Some(video_id)
        } else {
            None
        };
        let transcript = clean_transcript(self.transcript);
        let text = match self.source_type {
            SourceType::Youtube if !transcript.is_empty() => transcript
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
        let mut input =
            CaptureInput::new(self.dom_extract.title, text, self.source_type, self.access);
        input.author = self.dom_extract.byline;
        input.site = self.dom_extract.site;
        if matches!(input.source_type, SourceType::Youtube) {
            input.source = Some("manual_capture".into());
            input.transcript_status = Some(
                self.transcript_status
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        if transcript.is_empty() {
                            "unavailable".into()
                        } else {
                            "full".into()
                        }
                    }),
            );
            input.transcript_source = Some(
                self.transcript_source
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        if transcript.is_empty() {
                            "unavailable".into()
                        } else {
                            "rendered_dom".into()
                        }
                    }),
            );
            input.transcript_reason = Some(
                self.transcript_reason
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        if transcript.is_empty() {
                            "transcript_unavailable".into()
                        } else {
                            "rendered_transcript_segments_found".into()
                        }
                    }),
            );
        }
        input.url = Some(self.url);
        input.published_at = self.dom_extract.published_at;
        input.video_id = video_id;
        input.summary = self.dom_extract.summary;
        input.tags = self.tags;
        input.consumed_at = self.consumed_at;
        Ok(input)
    }
}

fn youtube_video_id(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    if url.host_str()?.ends_with("youtu.be") {
        return url.path_segments()?.next().and_then(clean_youtube_video_id);
    }
    url.query_pairs()
        .find(|(key, _)| key == "v")
        .and_then(|(_, value)| clean_youtube_video_id(&value))
}

fn clean_youtube_video_id(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() >= 3
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        Some(value.to_owned())
    } else {
        None
    }
}

fn canonical_youtube_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

fn clean_transcript(transcript: Vec<TranscriptSegment>) -> Vec<TranscriptSegment> {
    let mut seen = std::collections::HashSet::new();
    transcript
        .into_iter()
        .filter_map(|segment| {
            let text = segment.text.trim();
            if !segment.t.is_finite() || text.is_empty() {
                return None;
            }
            let key = (segment.t.max(0.0).floor() as u64, text.to_owned());
            if !seen.insert(key) {
                return None;
            }
            Some(TranscriptSegment {
                t: segment.t.max(0.0),
                text: text.to_owned(),
            })
        })
        .collect()
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
            transcript_status: Some("full".into()),
            transcript_source: Some("rendered_dom".into()),
            transcript_reason: Some("rendered_transcript_segments_found".into()),
            extractor_version: Some("youtube-dom-v1".into()),
            access: Access::Restricted,
            tags: Vec::new(),
            consumed_at: None,
        };
        let input = payload.into_input()?;
        assert_eq!(input.text, "[01:02] A useful idea");
        assert_eq!(
            input.url.as_deref(),
            Some("https://www.youtube.com/watch?v=test")
        );
        assert_eq!(input.source.as_deref(), Some("manual_capture"));
        assert_eq!(input.transcript_status.as_deref(), Some("full"));
        assert_eq!(input.transcript_source.as_deref(), Some("rendered_dom"));
        assert_eq!(
            input.transcript_reason.as_deref(),
            Some("rendered_transcript_segments_found")
        );
        Ok(())
    }

    #[test]
    fn youtube_fallback_sets_explicit_transcript_metadata() -> Result<()> {
        let payload = CapturePayload {
            version: 1,
            source_type: SourceType::Youtube,
            url: "https://www.youtube.com/watch?v=abc123".into(),
            dom_extract: DomExtract {
                title: "Test video".into(),
                byline: Some("Channel".into()),
                site: Some("youtube.com".into()),
                text: String::new(),
                published_at: None,
                summary: None,
                html_meta: Value::Null,
                selected_text: None,
            },
            transcript: vec![
                TranscriptSegment {
                    t: f64::NAN,
                    text: "bad".into(),
                },
                TranscriptSegment {
                    t: 2.0,
                    text: "   ".into(),
                },
            ],
            transcript_status: None,
            transcript_source: None,
            transcript_reason: Some("transcript_panel_not_rendered".into()),
            extractor_version: None,
            access: Access::Restricted,
            tags: Vec::new(),
            consumed_at: Some("2026-06-23T05:00:00Z".into()),
        };
        let input = payload.into_input()?;

        assert_eq!(input.text, "[Transcript unavailable]");
        assert_eq!(input.video_id.as_deref(), Some("abc123"));
        assert_eq!(input.transcript_status.as_deref(), Some("unavailable"));
        assert_eq!(input.transcript_source.as_deref(), Some("unavailable"));
        assert_eq!(
            input.transcript_reason.as_deref(),
            Some("transcript_panel_not_rendered")
        );
        assert_eq!(input.access, Access::Restricted);
        Ok(())
    }

    #[test]
    fn youtube_requires_title_and_video_id() {
        let mut payload = CapturePayload {
            version: 1,
            source_type: SourceType::Youtube,
            url: "https://www.youtube.com/watch".into(),
            dom_extract: DomExtract {
                title: "Test video".into(),
                byline: None,
                site: Some("youtube.com".into()),
                text: String::new(),
                published_at: None,
                summary: None,
                html_meta: Value::Null,
                selected_text: None,
            },
            transcript: Vec::new(),
            transcript_status: None,
            transcript_source: None,
            transcript_reason: None,
            extractor_version: None,
            access: Access::Restricted,
            tags: Vec::new(),
            consumed_at: None,
        };
        assert!(payload.clone().into_input().is_err());
        payload.url = "https://www.youtube.com/watch?v=abc123".into();
        payload.dom_extract.title = " ".into();
        assert!(payload.into_input().is_err());
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
            transcript_status: None,
            transcript_source: None,
            transcript_reason: None,
            extractor_version: None,
            access: Access::Restricted,
            tags: Vec::new(),
            consumed_at: Some("2026-06-22T12:00:00Z".into()),
        };
        let input = payload.into_input()?;
        assert_eq!(input.text, "A selected passage from the rendered page.");
        assert_eq!(input.consumed_at.as_deref(), Some("2026-06-22T12:00:00Z"));
        Ok(())
    }
}
