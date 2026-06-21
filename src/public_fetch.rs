use std::io::Read;

use anyhow::{Context, Result, bail};
use scraper::{Html, Selector};
use serde_json::Value;

use crate::model::{Access, CaptureInput, SourceType};

const MAX_PUBLIC_PAGE_BYTES: u64 = 16 * 1024 * 1024;
const RESTRICTED_DOMAINS: &[&str] = &[
    "nytimes.com",
    "wsj.com",
    "ft.com",
    "economist.com",
    "bloomberg.com",
    "newyorker.com",
    "theatlantic.com",
    "washingtonpost.com",
    "latimes.com",
    "businessinsider.com",
    "substack.com",
    "medium.com",
];

pub fn fetch_explicitly_public(url: &str) -> Result<CaptureInput> {
    let parsed = url::Url::parse(url).context("parse capture URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("only HTTP(S) URLs can be captured");
    }
    let hostname = parsed
        .host_str()
        .context("capture URL has no hostname")?
        .trim_start_matches("www.");
    if RESTRICTED_DOMAINS
        .iter()
        .any(|domain| hostname == *domain || hostname.ends_with(&format!(".{domain}")))
    {
        bail!("restricted or metered publisher: use the in-browser sensor");
    }

    let mut response = ureq::get(url).call().context("fetch public page")?;
    let mut html = String::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_PUBLIC_PAGE_BYTES + 1)
        .read_to_string(&mut html)?;
    if html.len() as u64 > MAX_PUBLIC_PAGE_BYTES {
        bail!("public page exceeds capture size limit");
    }
    let document = Html::parse_document(&html);
    if accessibility(&document) != Some(true) {
        bail!("page is not explicitly marked isAccessibleForFree=true; use the in-browser sensor");
    }

    let title = meta(&document, "meta[property='og:title']", "content")
        .or_else(|| text(&document, "title"))
        .unwrap_or_else(|| hostname.to_owned());
    let body = ["article", "main", "body"]
        .iter()
        .find_map(|selector| text(&document, selector))
        .context("public page did not contain readable text")?;
    if body.split_whitespace().count() < 20 {
        bail!("public page did not contain enough readable text");
    }
    Ok(CaptureInput {
        title,
        text: body,
        source_type: SourceType::Article,
        access: Access::Public,
        author: meta(&document, "meta[name='author']", "content"),
        site: meta(&document, "meta[property='og:site_name']", "content")
            .or_else(|| Some(hostname.to_owned())),
        url: Some(url.to_owned()),
        published_at: meta(
            &document,
            "meta[property='article:published_time']",
            "content",
        ),
        duration: None,
        video_id: None,
        summary: meta(&document, "meta[name='description']", "content"),
        tags: Vec::new(),
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

fn accessibility(document: &Html) -> Option<bool> {
    let selector = Selector::parse("script[type='application/ld+json']").expect("static selector");
    document.select(&selector).find_map(|script| {
        serde_json::from_str::<Value>(&script.text().collect::<String>())
            .ok()
            .and_then(|value| find_access(&value))
    })
}

fn find_access(value: &Value) -> Option<bool> {
    match value {
        Value::Object(values) => {
            if let Some(signal) = values.get("isAccessibleForFree") {
                return match signal {
                    Value::Bool(value) => Some(*value),
                    Value::String(value) => value.parse().ok(),
                    _ => None,
                };
            }
            values.values().find_map(find_access)
        }
        Value::Array(values) => values.iter().find_map(find_access),
        _ => None,
    }
}

fn text(document: &Html, selector: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    let value = document
        .select(&selector)
        .next()?
        .text()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!normalized.is_empty()).then_some(normalized)
}

fn meta(document: &Html, selector: &str, attribute: &str) -> Option<String> {
    let selector = Selector::parse(selector).ok()?;
    document
        .select(&selector)
        .next()?
        .value()
        .attr(attribute)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_public_schema_signal() {
        let document = Html::parse_document(
            r#"<script type="application/ld+json">{"@graph":[{"@type":"Article","isAccessibleForFree":true}]}</script>"#,
        );
        assert_eq!(accessibility(&document), Some(true));
    }
}
