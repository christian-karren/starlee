use crate::model::SourceType;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ChunkOptions {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            max_chars: 1800,
            overlap_chars: 270,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Chunk {
    pub char_start: usize,
    pub char_end: usize,
    pub t_start: Option<f64>,
    pub t_end: Option<f64>,
    pub text: String,
}

#[derive(Debug, Clone)]
struct Unit {
    start: usize,
    end: usize,
    t_start: Option<f64>,
    t_end: Option<f64>,
    text: String,
}

pub(crate) fn chunk_text(
    text: &str,
    source_type: &SourceType,
    options: ChunkOptions,
) -> Vec<Chunk> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    match source_type {
        SourceType::Youtube | SourceType::SpotifyEpisode => {
            let units = transcript_units(text, options.max_chars, options.overlap_chars);
            if units.iter().any(|unit| unit.t_start.is_some()) {
                pack_units(units, options.max_chars)
            } else {
                article_chunks(text, options)
            }
        }
        SourceType::Article | SourceType::Note => article_chunks(text, options),
    }
}

fn article_chunks(text: &str, options: ChunkOptions) -> Vec<Chunk> {
    let units = paragraph_units(text, options.max_chars, options.overlap_chars);
    if units.is_empty() {
        fixed_window_chunks(text, options.max_chars, options.overlap_chars)
    } else {
        pack_units(units, options.max_chars)
    }
}

fn paragraph_units(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut cursor = 0;
    for paragraph in text.split_inclusive("\n\n") {
        let paragraph_start = cursor;
        cursor += paragraph.len();
        let trimmed = paragraph.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = paragraph.len() - paragraph.trim_start().len();
        let trailing = paragraph.len() - paragraph.trim_end().len();
        let start = paragraph_start + leading;
        let end = paragraph_start + paragraph.len() - trailing;
        if end - start <= max_chars {
            units.push(Unit {
                start,
                end,
                t_start: None,
                t_end: None,
                text: text[start..end].to_owned(),
            });
        } else {
            units.extend(sentence_units(text, start, end, max_chars, overlap_chars));
        }
    }
    units
}

fn sentence_units(
    text: &str,
    start: usize,
    end: usize,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut sentence_start = start;
    let mut index = start;
    for ch in text[start..end].chars() {
        index += ch.len_utf8();
        if matches!(ch, '.' | '!' | '?') {
            let sentence_end = consume_following_space(text, index, end);
            push_text_unit(
                &mut units,
                text,
                sentence_start,
                sentence_end,
                max_chars,
                overlap_chars,
            );
            sentence_start = sentence_end;
        }
    }
    push_text_unit(
        &mut units,
        text,
        sentence_start,
        end,
        max_chars,
        overlap_chars,
    );
    units
}

fn push_text_unit(
    units: &mut Vec<Unit>,
    text: &str,
    start: usize,
    end: usize,
    max_chars: usize,
    overlap_chars: usize,
) {
    let trimmed = text[start..end].trim();
    if trimmed.is_empty() {
        return;
    }
    let leading = text[start..end].len() - text[start..end].trim_start().len();
    let trailing = text[start..end].len() - text[start..end].trim_end().len();
    let start = start + leading;
    let end = end - trailing;
    if end - start <= max_chars {
        units.push(Unit {
            start,
            end,
            t_start: None,
            t_end: None,
            text: text[start..end].to_owned(),
        });
        return;
    }
    units.extend(
        fixed_window_chunks(&text[start..end], max_chars, overlap_chars)
            .into_iter()
            .map(|chunk| Unit {
                start: start + chunk.char_start,
                end: start + chunk.char_end,
                t_start: None,
                t_end: None,
                text: chunk.text,
            }),
    );
}

fn consume_following_space(text: &str, mut index: usize, end: usize) -> usize {
    while index < end {
        let Some(ch) = text[index..end].chars().next() else {
            break;
        };
        if !ch.is_whitespace() || ch == '\n' {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn transcript_units(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<Unit> {
    let mut units = Vec::new();
    let mut cursor = 0;
    for line in text.split_inclusive('\n') {
        let line_start = cursor;
        cursor += line.len();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let leading = line.len() - line.trim_start().len();
        let trailing = line.len() - line.trim_end().len();
        let start = line_start + leading;
        let end = line_start + line.len() - trailing;
        let timestamp = parse_timestamp_prefix(trimmed);
        if end - start <= max_chars {
            units.push(Unit {
                start,
                end,
                t_start: timestamp,
                t_end: timestamp,
                text: text[start..end].to_owned(),
            });
        } else {
            let mut split = fixed_window_chunks(&text[start..end], max_chars, overlap_chars)
                .into_iter()
                .map(|chunk| Unit {
                    start: start + chunk.char_start,
                    end: start + chunk.char_end,
                    t_start: timestamp,
                    t_end: timestamp,
                    text: chunk.text,
                })
                .collect::<Vec<_>>();
            units.append(&mut split);
        }
    }
    units
}

fn pack_units(units: Vec<Unit>, max_chars: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current: Vec<Unit> = Vec::new();
    let mut current_len = 0;
    for unit in units {
        let separator = if current.is_empty() { 0 } else { 2 };
        if !current.is_empty() && current_len + separator + unit.text.len() > max_chars {
            chunks.push(chunk_from_units(&current));
            current.clear();
            current_len = 0;
        }
        current_len += if current.is_empty() {
            unit.text.len()
        } else {
            separator + unit.text.len()
        };
        current.push(unit);
    }
    if !current.is_empty() {
        chunks.push(chunk_from_units(&current));
    }
    chunks
}

fn chunk_from_units(units: &[Unit]) -> Chunk {
    let first = units.first().expect("chunk has units");
    let last = units.last().expect("chunk has units");
    Chunk {
        char_start: first.start,
        char_end: last.end,
        t_start: units.iter().find_map(|unit| unit.t_start),
        t_end: units.iter().rev().find_map(|unit| unit.t_end),
        text: units
            .iter()
            .map(|unit| unit.text.trim())
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

pub(crate) fn fixed_window_chunks(
    text: &str,
    max_chars: usize,
    overlap_chars: usize,
) -> Vec<Chunk> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        while !text.is_char_boundary(start) {
            start += 1;
        }
        let mut end = (start + max_chars).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.len()
            && let Some(boundary) = text[start..end].rfind(char::is_whitespace)
            && boundary > max_chars / 2
        {
            end = start + boundary;
        }
        chunks.push(Chunk {
            char_start: start,
            char_end: end,
            t_start: None,
            t_end: None,
            text: text[start..end].trim().to_owned(),
        });
        if end == text.len() {
            break;
        }
        start = end.saturating_sub(overlap_chars);
    }
    chunks
}

fn parse_timestamp_prefix(line: &str) -> Option<f64> {
    let rest = line.strip_prefix('[')?;
    let (value, _) = rest.split_once(']')?;
    let mut total = 0.0;
    let parts = value.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    for part in parts {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(max_chars: usize) -> ChunkOptions {
        ChunkOptions {
            max_chars,
            overlap_chars: max_chars / 10,
        }
    }

    #[test]
    fn article_chunks_preserve_paragraph_boundaries() {
        let text = "Memory systems need durable capture.\n\nRetrieval depends on coherent chunks.\n\nUnrelated cooking notes belong elsewhere.";

        let chunks = chunk_text(text, &SourceType::Article, opts(80));

        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.contains("Memory systems"));
        assert!(chunks[0].text.contains("Retrieval depends"));
        assert!(!chunks[0].text.contains("cooking"));
        assert_eq!(chunks[1].text, "Unrelated cooking notes belong elsewhere.");
    }

    #[test]
    fn long_paragraph_prefers_sentence_splits() {
        let text = "First idea has enough detail to stand alone. Second idea also has enough detail to become its own sentence chunk. Third idea closes the paragraph cleanly.";

        let chunks = chunk_text(text, &SourceType::Article, opts(74));

        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| chunk.text.len() <= 74));
        assert!(chunks[0].text.ends_with('.'));
        assert!(!chunks[0].text.contains("Third idea"));
    }

    #[test]
    fn fallback_fixed_window_handles_plain_long_words() {
        let text = "x".repeat(220);

        let chunks = fixed_window_chunks(&text, 75, 10);

        assert!(chunks.len() > 2);
        assert!(chunks.iter().all(|chunk| chunk.text.len() <= 75));
    }

    #[test]
    fn transcript_chunks_preserve_timestamp_ranges() {
        let text = "[00:01] Opening thought\n[00:04] More context\n[00:09] A new section";

        let chunks = chunk_text(text, &SourceType::Youtube, opts(60));

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].t_start, Some(1.0));
        assert_eq!(chunks[0].t_end, Some(4.0));
        assert_eq!(chunks[1].t_start, Some(9.0));
        assert_eq!(chunks[1].t_end, Some(9.0));
    }

    #[test]
    fn timestampless_transcript_falls_back_to_text_chunking() {
        let text = "Opening paragraph without timing.\n\nSecond paragraph without timing.";

        let chunks = chunk_text(text, &SourceType::Youtube, opts(120));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].t_start, None);
        assert!(chunks[0].text.contains("Second paragraph"));
    }

    #[test]
    fn empty_and_tiny_documents_are_stable() {
        assert!(chunk_text("", &SourceType::Note, opts(40)).is_empty());

        let chunks = chunk_text("Tiny note.", &SourceType::Note, opts(40));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Tiny note.");
    }

    #[test]
    fn very_long_documents_stay_bounded() {
        let paragraph = "A focused paragraph keeps one idea together for retrieval. ".repeat(12);
        let text = (0..80)
            .map(|_| paragraph.trim().to_owned())
            .collect::<Vec<_>>()
            .join("\n\n");

        let chunks = chunk_text(&text, &SourceType::Article, opts(900));

        assert!(chunks.len() > 10);
        assert!(chunks.iter().all(|chunk| chunk.text.len() <= 900));
        assert!(chunks.iter().all(|chunk| !chunk.text.trim().is_empty()));
    }
}
