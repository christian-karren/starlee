//! Local text extraction for user-uploaded documents (REQ-006).
//!
//! Everything runs on-device: plain text and Markdown are read directly, PDFs
//! go through `pdf-extract`, and `.docx` files are unzipped and their
//! `word/document.xml` reduced to text. No content ever leaves the machine.

use std::{io::Read, path::Path};

use anyhow::{Context, Result, bail};

/// Largest single upload accepted, in bytes (50 MB). Bounds memory on hostile or
/// accidentally huge files before any parsing happens.
pub const MAX_DOCUMENT_BYTES: u64 = 50 * 1024 * 1024;

pub struct ExtractedDocument {
    pub title: String,
    pub text: String,
}

/// Extract a document's title (from its file name) and plain-text body.
///
/// Fails on unsupported extensions, files over [`MAX_DOCUMENT_BYTES`], and
/// documents with no extractable text (e.g. a scanned, image-only PDF).
pub fn extract_text(path: &Path) -> Result<ExtractedDocument> {
    let metadata = std::fs::metadata(path).with_context(|| format!("read {}", path.display()))?;
    if metadata.len() > MAX_DOCUMENT_BYTES {
        bail!(
            "file is {} MB; the upload limit is {} MB",
            metadata.len() / (1024 * 1024),
            MAX_DOCUMENT_BYTES / (1024 * 1024)
        );
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Untitled document")
        .to_owned();

    let text = match extension.as_str() {
        "txt" | "text" | "md" | "markdown" => {
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?
        }
        "pdf" => extract_pdf(path)?,
        "docx" => extract_docx(path)?,
        "" => bail!("file has no extension; supported types are pdf, docx, txt, md"),
        other => bail!("unsupported document type: .{other} (supported: pdf, docx, txt, md)"),
    };

    let text = text.trim().to_owned();
    if text.is_empty() {
        bail!(
            "no extractable text in {} (a scanned/image-only document has no text layer)",
            path.display()
        );
    }
    Ok(ExtractedDocument { title, text })
}

fn extract_pdf(path: &Path) -> Result<String> {
    pdf_extract::extract_text(path).with_context(|| format!("extract text from {}", path.display()))
}

fn extract_docx(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("read .docx {}", path.display()))?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .context("not a valid .docx (missing word/document.xml)")?
        .read_to_string(&mut xml)?;
    Ok(docx_xml_to_text(&xml))
}

/// Reduce WordprocessingML to readable text: paragraph/line/tab markers become
/// whitespace, all other tags are dropped, and the basic XML entities are
/// decoded.
fn docx_xml_to_text(xml: &str) -> String {
    let normalized = xml
        .replace("</w:p>", "\n")
        .replace("<w:br/>", "\n")
        .replace("<w:br />", "\n")
        .replace("<w:tab/>", "\t")
        .replace("<w:tab />", "\t");

    let mut out = String::with_capacity(normalized.len());
    let mut in_tag = false;
    for ch in normalized.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(value: &str) -> String {
    // &amp; is decoded last so an escaped "&lt;" in the source stays "&lt;".
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_plain_text_and_markdown() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, "  Hello world  ")?;
        let doc = extract_text(&txt)?;
        assert_eq!(doc.title, "notes");
        assert_eq!(doc.text, "Hello world");

        let md = dir.path().join("syllabus.md");
        std::fs::write(&md, "# Week 1\n\nReadings")?;
        assert_eq!(extract_text(&md)?.text, "# Week 1\n\nReadings");
        Ok(())
    }

    #[test]
    fn rejects_unsupported_and_empty() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let bad = dir.path().join("photo.png");
        std::fs::write(&bad, "not text")?;
        assert!(extract_text(&bad).is_err());

        let empty = dir.path().join("blank.txt");
        std::fs::write(&empty, "   \n  ")?;
        assert!(extract_text(&empty).is_err());
        Ok(())
    }

    #[test]
    fn extracts_text_from_a_minimal_docx() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("essay.docx");
        let document_xml = "<?xml version=\"1.0\"?><w:document><w:body>\
            <w:p><w:r><w:t>First &amp; foremost</w:t></w:r></w:p>\
            <w:p><w:r><w:t>Second line</w:t></w:r></w:p>\
            </w:body></w:document>";
        let file = std::fs::File::create(&path)?;
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )?;
        zip.write_all(document_xml.as_bytes())?;
        zip.finish()?;

        let doc = extract_text(&path)?;
        assert_eq!(doc.title, "essay");
        assert_eq!(doc.text, "First & foremost\nSecond line");
        Ok(())
    }

    #[test]
    fn docx_xml_decodes_entities_and_breaks() {
        let text = docx_xml_to_text("<w:p><w:r><w:t>a&lt;b</w:t><w:br/><w:t>c</w:t></w:r></w:p>");
        assert_eq!(text.trim(), "a<b\nc");
    }
}
