use anyhow::{Context, Result};
use serde::Deserialize;

use crate::model::CaptureInput;

pub fn enrich_youtube(input: &mut CaptureInput, api_key: &str) -> Result<()> {
    let Some(video_id) = input.video_id.as_deref() else {
        return Ok(());
    };
    let url = format!(
        "https://www.googleapis.com/youtube/v3/videos?part=snippet%2CcontentDetails&id={video_id}&key={api_key}"
    );
    let response: VideoList = ureq::get(&url)
        .call()
        .context("request YouTube Data API metadata")?
        .body_mut()
        .read_json()
        .context("parse YouTube Data API metadata")?;
    let Some(video) = response.items.into_iter().next() else {
        return Ok(());
    };
    input.title = video.snippet.title;
    input.author = Some(video.snippet.channel_title);
    input.published_at = Some(video.snippet.published_at);
    input.duration = parse_iso8601_duration(&video.content_details.duration);
    Ok(())
}

#[derive(Deserialize)]
struct VideoList {
    #[serde(default)]
    items: Vec<Video>,
}

#[derive(Deserialize)]
struct Video {
    snippet: Snippet,
    #[serde(rename = "contentDetails")]
    content_details: ContentDetails,
}

#[derive(Deserialize)]
struct Snippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    #[serde(rename = "publishedAt")]
    published_at: String,
}

#[derive(Deserialize)]
struct ContentDetails {
    duration: String,
}

fn parse_iso8601_duration(value: &str) -> Option<u64> {
    let value = value.strip_prefix("PT")?;
    let mut number = String::new();
    let mut seconds = 0_u64;
    for character in value.chars() {
        if character.is_ascii_digit() {
            number.push(character);
            continue;
        }
        let amount = number.parse::<u64>().ok()?;
        number.clear();
        seconds += match character {
            'H' => amount * 3600,
            'M' => amount * 60,
            'S' => amount,
            _ => return None,
        };
    }
    if number.is_empty() {
        Some(seconds)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_youtube_duration() {
        assert_eq!(parse_iso8601_duration("PT1H2M3S"), Some(3723));
        assert_eq!(parse_iso8601_duration("PT14M8S"), Some(848));
    }
}
