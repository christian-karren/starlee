//! Storage backend abstraction for the vault (PRD REQ-002).
//!
//! The vault's high-level logic (identity, frontmatter assembly, summaries)
//! is separated from *where the bytes live* by the [`VaultBackend`] trait. All
//! storage is addressed by a [`VaultPath`] — a vault-root-relative logical path
//! — so the same logical record maps cleanly onto a local filesystem today and,
//! in a later phase, onto an encrypted synced blob store, without the rest of
//! the engine knowing the difference.
//!
//! Today the only implementation is [`LocalFsBackend`], which reproduces the
//! original direct-`fs` behavior (atomic writes, year-partitioned layout).

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

/// A vault-root-relative logical path, e.g. `2026/2026-ab12cd-ef34gh-title.md`.
///
/// Always relative and free of `.`/`..` segments, so a `VaultPath` can never
/// escape the vault root. Segments are separated by `/` regardless of platform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct VaultPath(String);

impl VaultPath {
    /// Parse a relative path into a `VaultPath`, rejecting absolute paths and
    /// any `.`/`..`/empty segment so traversal outside the vault is impossible.
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = input.replace('\\', "/");
        let trimmed = normalized.trim_matches('/');
        if trimmed.is_empty() {
            bail!("vault path cannot be empty");
        }
        if normalized.starts_with('/') {
            bail!("vault path must be relative: {input}");
        }
        for segment in trimmed.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                bail!("vault path may not contain '.' or '..' segments: {input}");
            }
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The relative path as a [`PathBuf`] for joining against a root.
    pub fn as_path(&self) -> PathBuf {
        PathBuf::from(&self.0)
    }
}

/// Where vault record bytes are stored. Implementations address records by
/// [`VaultPath`] and need not be a filesystem.
pub trait VaultBackend: Send + Sync {
    /// Ensure the backend is ready to accept writes (e.g. create the root).
    fn init(&self) -> Result<()>;

    /// Atomically write `bytes` to `rel`, creating any parent structure.
    fn write(&self, rel: &VaultPath, bytes: &[u8]) -> Result<()>;

    /// Read the bytes stored at `rel`.
    fn read(&self, rel: &VaultPath) -> Result<Vec<u8>>;

    /// Remove `rel`. Returns `true` if something was removed, `false` if it was
    /// already absent.
    fn remove(&self, rel: &VaultPath) -> Result<bool>;

    /// List every stored record path.
    fn list(&self) -> Result<Vec<VaultPath>>;

    /// The local root under which records are materialized. Used to derive the
    /// absolute, user-facing path for a record (e.g. "Reveal in Finder") and to
    /// map an absolute path back to a logical [`VaultPath`].
    fn root(&self) -> &Path;
}

/// Filesystem-backed vault storage: records live at `{root}/{year}/{name}.md`.
pub struct LocalFsBackend {
    root: PathBuf,
}

impl LocalFsBackend {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn absolute(&self, rel: &VaultPath) -> PathBuf {
        self.root.join(rel.as_path())
    }
}

impl VaultBackend for LocalFsBackend {
    fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create vault {}", self.root.display()))
    }

    fn write(&self, rel: &VaultPath, bytes: &[u8]) -> Result<()> {
        let path = self.absolute(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Write to a temp sibling then rename so a reader never observes a
        // half-written record.
        let temporary = path.with_extension("md.tmp");
        fs::write(&temporary, bytes).with_context(|| format!("write {}", temporary.display()))?;
        fs::rename(&temporary, &path).with_context(|| format!("commit {}", path.display()))?;
        Ok(())
    }

    fn read(&self, rel: &VaultPath) -> Result<Vec<u8>> {
        let path = self.absolute(rel);
        fs::read(&path).with_context(|| format!("read {}", path.display()))
    }

    fn remove(&self, rel: &VaultPath) -> Result<bool> {
        let path = self.absolute(rel);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).with_context(|| format!("delete {}", path.display())),
        }
    }

    fn list(&self) -> Result<Vec<VaultPath>> {
        let mut paths = Vec::new();
        if !self.root.exists() {
            return Ok(paths);
        }
        for year in fs::read_dir(&self.root)? {
            let year = year?.path();
            if !year.is_dir() {
                continue;
            }
            let Some(year_name) = year.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            for entry in fs::read_dir(&year)? {
                let path = entry?.path();
                if path.extension().and_then(|v| v.to_str()) != Some("md") {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
                    paths.push(VaultPath::parse(&format!("{year_name}/{name}"))?);
                }
            }
        }
        paths.sort();
        Ok(paths)
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_path_rejects_traversal_and_absolute() {
        assert!(VaultPath::parse("2026/note.md").is_ok());
        assert!(VaultPath::parse("../secret").is_err());
        assert!(VaultPath::parse("2026/../../secret").is_err());
        assert!(VaultPath::parse("/etc/passwd").is_err());
        assert!(VaultPath::parse("").is_err());
        assert!(VaultPath::parse("a/./b").is_err());
    }

    #[test]
    fn write_read_remove_round_trip() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backend = LocalFsBackend::new(temp.path().join("vault"));
        backend.init()?;
        let rel = VaultPath::parse("2026/hello.md")?;
        backend.write(&rel, b"# hello")?;
        assert_eq!(backend.read(&rel)?, b"# hello");
        assert!(backend.remove(&rel)?);
        // Idempotent: removing again reports nothing was removed.
        assert!(!backend.remove(&rel)?);
        Ok(())
    }

    #[test]
    fn list_finds_year_partitioned_records_only() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let backend = LocalFsBackend::new(temp.path().join("vault"));
        backend.init()?;
        backend.write(&VaultPath::parse("2025/a.md")?, b"a")?;
        backend.write(&VaultPath::parse("2026/b.md")?, b"b")?;
        // A non-markdown file and a stray top-level file must be ignored.
        backend.write(&VaultPath::parse("2026/notes.txt")?, b"x")?;
        let listed = backend.list()?;
        assert_eq!(
            listed,
            vec![
                VaultPath::parse("2025/a.md")?,
                VaultPath::parse("2026/b.md")?
            ]
        );
        Ok(())
    }
}
