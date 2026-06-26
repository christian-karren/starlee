use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Utc};

use crate::model::{CaptureInput, Frontmatter, Record, SourceType};
use crate::vault_backend::{LocalFsBackend, VaultBackend, VaultPath};

pub struct Vault {
    backend: Box<dyn VaultBackend>,
}

impl Vault {
    /// Construct a vault backed by the local filesystem rooted at `root`.
    pub fn new(root: PathBuf) -> Self {
        Self {
            backend: Box::new(LocalFsBackend::new(root)),
        }
    }

    pub fn init(&self) -> Result<()> {
        self.backend.init()
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
        let year = now.year().to_string();
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
            // Stable, content-addressed identity (PRD REQ-001): derived from the
            // canonical URL (or title+body for note-only captures), never from
            // wall-clock time, so the same source captured on any device shares
            // one ID and converges on sync instead of duplicating.
            let id = crate::identity::record_id(input.url.as_deref(), &input.title, &input.text);
            (id.clone(), format!("{}-{}.md", id, slugify(&input.title)))
        };
        let rel = VaultPath::parse(&format!("{year}/{file_name}"))?;
        self.write_record(rel, id, input, now)
    }

    pub fn replace(&self, existing: &Record, input: CaptureInput) -> Result<Record> {
        let rel = self.to_logical(Path::new(&existing.file_path))?;
        self.write_record(rel, existing.metadata.id.clone(), input, Utc::now())
    }

    fn write_record(
        &self,
        rel: VaultPath,
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
        let summary = input
            .summary
            .unwrap_or_else(|| extractive_summary(&input.text));
        let metadata = Frontmatter {
            id: id.clone(),
            source_type: input.source_type,
            title: input.title.clone(),
            author: input.author,
            site: input.site,
            source: input.source,
            url: input.url,
            captured_at,
            consumed_at: input.consumed_at,
            published_at: input.published_at,
            duration: input.duration,
            video_id: input.video_id,
            word_count: Some(input.text.split_whitespace().count()),
            access: input.access,
            summary,
            tags: input.tags,
            topics: crate::topics::sanitize_topics(input.topics),
            spotify_episode_id: input.spotify_episode_id,
            spotify_show_id: input.spotify_show_id,
            show: input.show,
            listen_duration_s: input.listen_duration_s,
            listen_progress_pct: input.listen_progress_pct,
            transcript_status: input.transcript_status,
            transcript_source: input.transcript_source,
            transcript_reason: input.transcript_reason,
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
        self.backend.write(&rel, document.as_bytes())?;
        Ok(Record {
            metadata,
            body: input.text.trim().to_owned(),
            file_path: self.display_path(&rel),
        })
    }

    pub fn read(&self, path: &Path) -> Result<Record> {
        let rel = self.to_logical(path)?;
        self.read_logical(&rel)
    }

    fn read_logical(&self, rel: &VaultPath) -> Result<Record> {
        let bytes = self.backend.read(rel)?;
        let text = String::from_utf8(bytes).context("vault record is not valid UTF-8")?;
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
            file_path: self.display_path(rel),
        })
    }

    pub fn records(&self) -> Result<Vec<Record>> {
        self.backend
            .list()?
            .into_iter()
            .map(|rel| self.read_logical(&rel))
            .collect()
    }

    /// Permanently delete a record's Markdown file from the vault.
    ///
    /// Returns `true` if a file was removed, `false` if it was already gone.
    /// The resolved path is validated to live inside the vault root, so a record
    /// whose `file_path` was tampered with to point elsewhere (e.g. via `..`
    /// traversal) is refused rather than deleting an arbitrary file.
    pub fn delete(&self, record: &Record) -> Result<bool> {
        let rel = self.to_logical_secure(Path::new(&record.file_path))?;
        self.backend.remove(&rel)
    }

    /// The absolute, user-facing path for a record (used as `Record.file_path`
    /// so consumers like the desktop "Reveal in Finder" action can open it).
    fn display_path(&self, rel: &VaultPath) -> String {
        self.backend
            .root()
            .join(rel.as_path())
            .display()
            .to_string()
    }

    /// Map a path that may be absolute (as stored in `Record.file_path` / the
    /// index) back to a logical [`VaultPath`]. Lenient: used on read/write paths
    /// that originate from the vault itself.
    fn to_logical(&self, path: &Path) -> Result<VaultPath> {
        let root = self.backend.root();
        let relative = match path.strip_prefix(root) {
            Ok(stripped) => stripped.to_path_buf(),
            Err(_) => path.to_path_buf(),
        };
        VaultPath::parse(&relative.to_string_lossy())
    }

    /// Like [`Self::to_logical`] but fails closed if the path resolves outside
    /// the vault root, collapsing any `..` segments first. Used for deletion so
    /// a tampered `file_path` can never remove an arbitrary file.
    fn to_logical_secure(&self, path: &Path) -> Result<VaultPath> {
        let root = self
            .backend
            .root()
            .canonicalize()
            .with_context(|| format!("resolve vault root {}", self.backend.root().display()))?;
        let resolved = if path.exists() {
            path.canonicalize()
                .with_context(|| format!("resolve {}", path.display()))?
        } else {
            // The file may already be gone; still resolve the parent so that any
            // `..` segments are collapsed before the containment check.
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let parent = parent
                .canonicalize()
                .with_context(|| format!("resolve {}", parent.display()))?;
            match path.file_name() {
                Some(name) => parent.join(name),
                None => parent,
            }
        };
        if !resolved.starts_with(&root) {
            bail!(
                "refusing to delete path outside the vault: {}",
                resolved.display()
            );
        }
        let relative = resolved
            .strip_prefix(&root)
            .with_context(|| format!("relativize {}", resolved.display()))?;
        VaultPath::parse(&relative.to_string_lossy())
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
    use std::fs;

    use super::*;

    #[test]
    fn slugs_are_portable() {
        assert_eq!(slugify("Meta & AI: What Now?"), "meta-ai-what-now");
    }

    #[test]
    fn delete_removes_a_record_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let vault = Vault::new(temp.path().join("vault"));
        let record = vault.write(CaptureInput::new(
            "Note",
            "Body text",
            SourceType::Note,
            crate::model::Access::Restricted,
        ))?;
        assert!(Path::new(&record.file_path).exists());
        assert!(vault.delete(&record)?);
        assert!(!Path::new(&record.file_path).exists());
        // Idempotent: deleting again reports nothing was removed.
        assert!(!vault.delete(&record)?);
        Ok(())
    }

    #[test]
    fn delete_refuses_paths_outside_the_vault() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let vault = Vault::new(temp.path().join("vault"));
        let record = vault.write(CaptureInput::new(
            "Note",
            "Body text",
            SourceType::Note,
            crate::model::Access::Restricted,
        ))?;
        // A file that lives outside the vault root must never be deletable, even
        // if a record's file_path is tampered to point at it directly...
        let outside = temp.path().join("secret.txt");
        fs::write(&outside, "do not delete")?;
        let mut absolute = record.clone();
        absolute.file_path = outside.display().to_string();
        assert!(vault.delete(&absolute).is_err());
        assert!(outside.exists());

        // ...or via a `..` traversal segment that escapes the root.
        let mut traversal = record.clone();
        traversal.file_path = temp
            .path()
            .join("vault")
            .join("..")
            .join("secret.txt")
            .display()
            .to_string();
        assert!(vault.delete(&traversal).is_err());
        assert!(outside.exists());
        Ok(())
    }

    #[test]
    fn spotify_episode_uses_stable_identity_and_hides_description_from_body() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let vault = Vault::new(temp.path().join("vault"));
        let mut input = CaptureInput::new(
            "A podcast episode",
            "[Transcript unavailable]",
            SourceType::SpotifyEpisode,
            crate::model::Access::Restricted,
        );
        input.site = Some("Spotify".into());
        input.source = Some("spotify_sync".into());
        input.url = Some("https://open.spotify.com/episode/ep123".into());
        input.duration = Some(3600);
        input.spotify_episode_id = Some("ep123".into());
        input.spotify_show_id = Some("show456".into());
        input.show = Some("Great Show".into());
        input.listen_duration_s = Some(2400);
        input.listen_progress_pct = Some(67);
        input.transcript_status = Some("missing".into());
        input.description = Some("This description should not be queryable.".into());
        let record = vault.write(input)?;

        assert_eq!(record.metadata.id, "spotify:episode:ep123");
        assert!(record.file_path.ends_with("-spotify-ep123.md"));
        let reread = vault.read(Path::new(&record.file_path))?;
        assert_eq!(reread.body, "[Transcript unavailable]");
        assert_eq!(reread.metadata.show.as_deref(), Some("Great Show"));
        Ok(())
    }
}
