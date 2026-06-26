//! Stable, content-addressed record identity (PRD REQ-001).
//!
//! Record IDs must be derived deterministically from a record's *canonical
//! source identity* rather than from wall-clock time, so the same source
//! captured on two machines produces the same ID and converges on sync instead
//! of duplicating. Identity is computed from a normalized canonical key:
//!
//! - a normalized URL when the capture has one (the common case),
//! - the stable `spotify:episode:{id}` form for Spotify episodes,
//! - otherwise a hash of the title + body (for pasted notes with no URL).
//!
//! The ID embeds no timestamp and no per-device state, so it is identical on
//! every device for the same content.

use sha2::{Digest, Sha256};
use url::Url;

/// Domain-separation prefix mixed into every ID hash. Bumping this string would
/// change all generated IDs, so it is versioned deliberately.
const ID_DOMAIN: &str = "starlee-id-v1\n";

/// Query parameters stripped during URL normalization because they carry
/// tracking/session state rather than identity. Matched case-insensitively.
/// Any parameter whose name begins with `utm_` is also stripped.
const TRACKING_PARAMS: &[&str] = &[
    "gclid",
    "dclid",
    "gclsrc",
    "gbraid",
    "wbraid",
    "fbclid",
    "msclkid",
    "yclid",
    "twclid",
    "igshid",
    "mc_cid",
    "mc_eid",
    "_hsenc",
    "_hsmi",
    "vero_id",
    "oly_anon_id",
    "oly_enc_id",
    "_openstat",
    "ref",
    "ref_src",
    "ref_url",
    "referrer",
    "spm",
    "scm",
];

/// Compute the stable record ID for a capture.
///
/// `url` is the captured source URL when present. `title` and `body` are used
/// only as the fallback identity for captures that have no URL.
pub fn record_id(url: Option<&str>, title: &str, body: &str) -> String {
    id_from_key(&canonical_key(url, title, body))
}

/// Build the canonical identity key for a capture. Exposed for the migration
/// path (re-keying existing records) and for tests.
pub fn canonical_key(url: Option<&str>, title: &str, body: &str) -> String {
    match url.map(str::trim).filter(|value| !value.is_empty()) {
        Some(url) => format!("url:{}", normalize_url(url)),
        None => {
            let mut hasher = Sha256::new();
            hasher.update(title.trim().as_bytes());
            hasher.update(b"\n");
            hasher.update(body.trim().as_bytes());
            format!("content:{:x}", hasher.finalize())
        }
    }
}

/// Hash a canonical key into the short, file-name-safe ID form `xxxxxxxx-xxxxxxxx`
/// (64 bits of SHA-256, lowercase hex). 64 bits keeps collision probability
/// negligible at realistic vault sizes while staying readable in a filename.
fn id_from_key(canonical_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ID_DOMAIN.as_bytes());
    hasher.update(canonical_key.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{}-{}", &digest[..8], &digest[8..16])
}

/// Normalize a URL so that variants pointing at the same resource collapse to
/// one canonical string: lowercase scheme/host, drop default ports, drop the
/// fragment, strip tracking query params, sort the remaining params, and remove
/// a trailing slash from non-root paths.
///
/// Input that does not parse as an absolute URL falls back to a trimmed,
/// lowercased form so identity is still deterministic.
pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    let Ok(mut url) = Url::parse(trimmed) else {
        return trimmed.to_ascii_lowercase();
    };

    // Scheme and host are lowercased by the `url` crate already; normalize the
    // remaining identity-bearing components.
    url.set_fragment(None);

    // Drop the port when it is the scheme default (e.g. :443 for https), but
    // keep a non-default port like :8080 since it is part of the identity.
    let default_port = match url.scheme() {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    };
    if url.port().is_some() && url.port() == default_port {
        let _ = url.set_port(None);
    }

    // Filter tracking params and sort the survivors for a stable ordering.
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(name, _)| !is_tracking_param(name))
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    if kept.is_empty() {
        url.set_query(None);
    } else {
        let mut sorted = kept;
        sorted.sort();
        let mut serializer = url.query_pairs_mut();
        serializer.clear();
        for (name, value) in &sorted {
            serializer.append_pair(name, value);
        }
        drop(serializer);
    }

    // Trim a single trailing slash from non-root paths so `/a/b` and `/a/b/`
    // share an identity; leave the root path "/" intact.
    if url.path().len() > 1
        && url.path().ends_with('/')
        && let Ok(mut segments) = url.path_segments_mut()
    {
        segments.pop_if_empty();
    }

    url.as_str().to_owned()
}

fn is_tracking_param(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("utm_") || TRACKING_PARAMS.contains(&lower.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_deterministic_for_the_same_url() {
        let a = record_id(Some("https://example.com/post"), "Title A", "body one");
        let b = record_id(
            Some("https://example.com/post"),
            "Different title",
            "body two",
        );
        // URL-keyed identity ignores title/body: same URL -> same ID.
        assert_eq!(a, b);
    }

    #[test]
    fn id_has_the_expected_shape() {
        let id = record_id(Some("https://example.com/x"), "t", "b");
        let (left, right) = id.split_once('-').expect("hyphenated id");
        assert_eq!(left.len(), 8);
        assert_eq!(right.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn id_carries_no_timestamp_so_it_is_stable_across_captures() {
        // Two "captures" of the same content must yield byte-identical IDs.
        let first = record_id(Some("https://example.com/a"), "T", "B");
        let second = record_id(Some("https://example.com/a"), "T", "B");
        assert_eq!(first, second);
    }

    #[test]
    fn tracking_params_do_not_change_identity() {
        let clean = record_id(Some("https://example.com/a?x=1"), "t", "b");
        let tracked = record_id(
            Some("https://example.com/a?utm_source=twitter&x=1&fbclid=abc"),
            "t",
            "b",
        );
        assert_eq!(clean, tracked);
    }

    #[test]
    fn fragment_and_trailing_slash_do_not_change_identity() {
        let canonical = normalize_url("https://example.com/a/b");
        assert_eq!(normalize_url("https://example.com/a/b/"), canonical);
        assert_eq!(normalize_url("https://example.com/a/b#section"), canonical);
    }

    #[test]
    fn host_and_scheme_case_are_normalized() {
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path"),
            "https://example.com/Path"
        );
    }

    #[test]
    fn default_port_is_dropped_but_path_case_is_preserved() {
        assert_eq!(
            normalize_url("https://example.com:443/Path"),
            "https://example.com/Path"
        );
        assert_eq!(
            normalize_url("http://example.com:8080/Path"),
            "http://example.com:8080/Path"
        );
    }

    #[test]
    fn query_param_order_is_canonicalized() {
        assert_eq!(
            normalize_url("https://example.com/a?b=2&a=1"),
            normalize_url("https://example.com/a?a=1&b=2")
        );
    }

    #[test]
    fn root_path_slash_is_preserved() {
        assert_eq!(
            normalize_url("https://example.com/"),
            "https://example.com/"
        );
    }

    #[test]
    fn non_url_input_falls_back_deterministically() {
        assert_eq!(normalize_url("  Not A URL  "), "not a url");
        let a = record_id(Some("not a url"), "t", "b");
        let b = record_id(Some("NOT A URL"), "t", "b");
        assert_eq!(a, b);
    }

    #[test]
    fn note_without_url_is_content_addressed() {
        let a = record_id(None, "My Note", "the body");
        let b = record_id(None, "My Note", "the body");
        let c = record_id(None, "My Note", "a different body");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn empty_url_falls_back_to_content_identity() {
        // A blank/whitespace URL must not be treated as a URL key.
        let blank = record_id(Some("   "), "My Note", "the body");
        let none = record_id(None, "My Note", "the body");
        assert_eq!(blank, none);
    }
}
