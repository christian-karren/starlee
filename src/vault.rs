use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Utc};
use sha2::{Digest, Sha256};

use crate::model::{CaptureInput, Frontmatter, Record, SourceType};

pub struct Vault {
    root: PathBuf,
}

impl Vault {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create vault {}", self.root.display()))
    }

    pub fn write(&self, input: CaptureInput) -> Result<Record> {
        if input.title.trim().is_empty() {
            bail!("title cannot be empty")
        }
        if input.text.trim().is_empty() {
            bail!("text cannot be empty")
        }

        self.init()?;
        let now = Utc::now();
        let year = self.root.join(now.year().to_string());
        fs::create_dir_all(&year)?;
        let (id, file_name) = if matches!(input.source_type, SourceType::SpotifyEpisode) {
            let episode_id = input
                .spotify_episode_id
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .context("spotify episode captures require spotify_episode_id")?;
            (
                format!("spotify:episode:{episode_id}"),
                format!("{}-spotify-{episode_id}.md", now.format("%Y-%m-%d")),
            )
        } else {
            let mut hasher = Sha256::new();
            hasher.update(input.url.as_deref().unwrap_or(&input.title));
            hasher.update(now.timestamp_nanos_opt().unwrap_or_default().to_le_bytes());
            let digest = format!("{:x}", hasher.finalize());
            let id = format!(
                "{}-{}-{}",
                now.format("%Y-%m%d"),
                &digest[..6],
                &digest[6..12]
            );
            (id.clone(), format!("{}-{}.md", id, slugify(&input.title)))
        };
        let path = year.join(file_name);
        self.write_record(path, id, input, now)
    }

    pub fn replace(&self, existing: &Record, input: CaptureInput) -> Result<Record> {
        self.write_record(
            PathBuf::from(&existing.file_path),
            existing.metadata.id.clone(),
            input,
            Utc::now(),
        )
    }

    fn write_record(
        &self,
        path: PathBuf,
        id: String,
        input: CaptureInput,
        captured_at: chrono::DateTime<Utc>,
    ) -> Result<Record> {
        if input.title.trim().is_empty() {
            bail!("title cannot be empty")
        }
        if input.text.trim().is_empty() {
            bail!("text cannot be empty")
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let summary = input
            .summary
            .unwrap_or_else(|| extractive_summary(&input.text));
        let metadata = Frontmatter {
            id: id.clone(),
            source_type: input.source_type,
            title: input.title.clone(),
            author: input.author,
            site: input.site,
            url: input.url,
            captured_at,
            published_at: input.published_at,
            duration: input.duration,
            video_id: input.video_id,
            word_count: Some(input.text.split_whitespace().count()),
            access: input.access,
            summary,
            tags: input.tags,
            spotify_episode_id: input.spotify_episode_id,
            spotify_show_id: input.spotify_show_id,
            show: input.show,
            listen_duration_s: input.listen_duration_s,
            listen_progress_pct: input.listen_progress_pct,
            transcript_status: input.transcript_status,
            transcript_source: input.transcript_source,
            matched_youtube_id: input.matched_youtube_id,
            linked_youtube_id: input.linked_youtube_id,
        };
        let yaml = serde_yaml::to_string(&metadata)?;
        let description = input
            .description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                format!(
                    "<!-- spotify_description: {} -->\n\n",
                    sanitize_comment(value)
                )
            })
            .unwrap_or_default();
        let document = format!("---\n{}---\n\n{}{}\n", yaml, description, input.text.trim());
        let temporary = path.with_extension("md.tmp");
        fs::write(&temporary, document)?;
        fs::rename(&temporary, &path)?;
        Ok(Record {
            metadata,
            body: input.text.trim().to_owned(),
            file_path: path.display().to_string(),
        })
    }

    pub fn read(&self, path: &Path) -> Result<Record> {
        let text = fs::read_to_string(path)?;
        let rest = text
            .strip_prefix("---\n")
            .context("missing YAML frontmatter")?;
        let (yaml, body) = rest
            .split_once("\n---\n")
            .context("unterminated YAML frontmatter")?;
        let metadata = serde_yaml::from_str(yaml)?;
        Ok(Record {
            metadata,
            body: strip_nonqueryable_description(body).trim().to_owned(),
            file_path: path.display().to_string(),
        })
    }

    pub fn records(&self) -> Result<Vec<Record>> {
        let mut paths = Vec::new();
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        for year in fs::read_dir(&self.root)? {
            let year = year?.path();
            if !year.is_dir() {
                continue;
            }
            for entry in fs::read_dir(year)? {
                let path = entry?.path();
                if path.extension().and_then(|v| v.to_str()) == Some("md") {
                    paths.push(path);
                }
            }
        }
        paths.sort();
        paths.into_iter().map(|path| self.read(&path)).collect()
    }
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in value.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-')
        .chars()
        .take(64)
        .collect::<String>()
        .trim_matches('-')
        .to_owned()
}

fn extractive_summary(text: &str) -> String {
    let mut end = text.len().min(480);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let candidate = &text[..end];
    if let Some(sentence) = candidate.rfind(['.', '!', '?']) {
        candidate[..=sentence].trim().to_owned()
    } else {
        candidate.trim().to_owned()
    }
}

fn sanitize_comment(value: &str) -> String {
    value.replace("-->", "--&gt;").replace('\n', " ")
}

fn strip_nonqueryable_description(body: &str) -> &str {
    let trimmed = body.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<!-- spotify_description:")
        && let Some((_, body)) = rest.split_once("-->")
    {
        return body;
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_portable() {
        assert_eq!(slugify("Meta & AI: What Now?"), "meta-ai-what-now");
    }

    #[test]
    fn spotify_episode_uses_stable_identity_and_hides_description_from_body() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let vault = Vault::new(temp.path().join("vault"));
        let record = vault.write(CaptureInput {
            title: "A podcast episode".into(),
            text: "[Transcript unavailable]".into(),
            source_type: SourceType::SpotifyEpisode,
            access: crate::model::Access::Restricted,
            author: None,
            site: Some("Spotify".into()),
            url: Some("https://open.spotify.com/episode/ep123".into()),
            published_at: None,
            duration: Some(3600),
            video_id: None,
            summary: None,
            tags: Vec::new(),
            spotify_episode_id: Some("ep123".into()),
            spotify_show_id: Some("show456".into()),
            show: Some("Great Show".into()),
            listen_duration_s: Some(2400),
            listen_progress_pct: Some(67),
            transcript_status: Some("missing".into()),
            transcript_source: None,
            matched_youtube_id: None,
            linked_youtube_id: None,
            description: Some("This description should not be queryable.".into()),
        })?;

        assert_eq!(record.metadata.id, "spotify:episode:ep123");
        assert!(record.file_path.ends_with("-spotify-ep123.md"));
        let reread = vault.read(Path::new(&record.file_path))?;
        assert_eq!(reread.body, "[Transcript unavailable]");
        assert_eq!(reread.metadata.show.as_deref(), Some("Great Show"));
        Ok(())
    }
}
