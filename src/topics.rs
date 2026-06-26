//! User-managed topic taxonomy.
//!
//! Topics are a first-class, user-defined categorization that is distinct from
//! the free-form `tags` field. They are canonical in Markdown frontmatter
//! (`topics:`) and mirrored into the index (`source_topics`) for filtering, so a
//! full `reindex` rebuilds them from the vault without loss.
//!
//! Every topic that enters the vault or the index passes through
//! [`sanitize_topic`]/[`sanitize_topics`] first, so frontmatter and SQL never see
//! control characters, oversized strings, or duplicates.

/// Maximum stored length of a single topic, in Unicode scalar values.
pub const MAX_TOPIC_LEN: usize = 64;

/// Normalize a single topic name, returning `None` if nothing usable remains.
///
/// Rules: trim surrounding whitespace, collapse internal whitespace runs to a
/// single space, drop control characters, and cap the result at
/// [`MAX_TOPIC_LEN`] scalar values (without splitting a character).
pub fn sanitize_topic(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in raw.trim().chars() {
        if ch.is_control() {
            // Treat control characters (incl. newlines/tabs) as separators so a
            // multi-line paste cannot smuggle YAML or SQL structure into a topic.
            pending_space = !out.is_empty();
            continue;
        }
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    let trimmed: String = out.chars().take(MAX_TOPIC_LEN).collect();
    let trimmed = trimmed.trim_end().to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Sanitize a list of topics: normalize each, drop empties, and de-duplicate
/// case-insensitively while preserving first-seen order and the original casing.
pub fn sanitize_topics<I, S>(raw: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        if let Some(topic) = sanitize_topic(item.as_ref()) {
            let key = topic.to_lowercase();
            if seen.insert(key) {
                out.push(topic);
            }
        }
    }
    out
}

/// Whether two topic names refer to the same topic (case-insensitive match on
/// their sanitized forms).
pub fn topics_match(a: &str, b: &str) -> bool {
    match (sanitize_topic(a), sanitize_topic(b)) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_collapses_whitespace() {
        assert_eq!(sanitize_topic("  CS  101 "), Some("CS 101".to_owned()));
    }

    #[test]
    fn rejects_empty_and_whitespace_only() {
        assert_eq!(sanitize_topic("   "), None);
        assert_eq!(sanitize_topic(""), None);
    }

    #[test]
    fn strips_control_characters_that_could_break_yaml_or_sql() {
        // Newlines, tabs, and a NUL collapse to separators, never structure.
        assert_eq!(
            sanitize_topic("History\n: DROP\tTABLE\u{0}sources"),
            Some("History : DROP TABLE sources".to_owned())
        );
    }

    #[test]
    fn caps_length_at_64_scalar_values() {
        let long = "a".repeat(200);
        let cleaned = sanitize_topic(&long).unwrap();
        assert_eq!(cleaned.chars().count(), MAX_TOPIC_LEN);
    }

    #[test]
    fn dedupes_case_insensitively_preserving_first_casing_and_order() {
        let cleaned = sanitize_topics(["Biology", "biology", "Chemistry", " BIOLOGY "]);
        assert_eq!(cleaned, vec!["Biology".to_owned(), "Chemistry".to_owned()]);
    }

    #[test]
    fn drops_empty_entries_from_a_list() {
        let cleaned = sanitize_topics(["", "  ", "Valid"]);
        assert_eq!(cleaned, vec!["Valid".to_owned()]);
    }

    #[test]
    fn topics_match_is_case_insensitive_on_sanitized_forms() {
        assert!(topics_match(" cs 101 ", "CS 101"));
        assert!(!topics_match("CS 101", "CS 102"));
    }
}
