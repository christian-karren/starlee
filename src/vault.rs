use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, Utc};
use sha2::{Digest, Sha256};

use crate::model::{CaptureInput, Frontmatter, Record};

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
        let year = self.root.join(now.year().to_string());
        fs::create_dir_all(&year)?;
        let path = year.join(format!("{}-{}.md", id, slugify(&input.title)));
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
        };
        let yaml = serde_yaml::to_string(&metadata)?;
        let document = format!("---\n{}---\n\n{}\n", yaml, input.text.trim());
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
            body: body.trim().to_owned(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_portable() {
        assert_eq!(slugify("Meta & AI: What Now?"), "meta-ai-what-now");
    }
}
