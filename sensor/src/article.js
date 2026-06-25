import { Readability, isProbablyReaderable } from "@mozilla/readability";
import { classifyAccess } from "./access.js";
import { htmlMeta, pageMetadata } from "./metadata.js";

export function isArticle(document) {
  return isProbablyReaderable(document, { minContentLength: 120, minScore: 18 });
}

export function extractArticle(document, options = {}) {
  const metadata = pageMetadata(document);
  emitArticleDiagnostic(options, "article_extraction_started", {
    status: "started",
    safe_metadata: {
      host: safeHost(document),
      readerable: String(isArticle(document)),
      canonical_present: String(Boolean(metadata.canonical))
    }
  });
  try {
    const article = new Readability(document.cloneNode(true), { charThreshold: 120 }).parse();
    const text = article?.textContent?.trim() || "";
    if (!text) {
      emitArticleDiagnostic(options, "article_extraction_empty", {
        status: "empty_article",
        message: "Mozilla Readability did not return article text.",
        safe_metadata: {
          title_present: String(Boolean(article?.title || metadata.title)),
          host: safeHost(document)
        }
      });
      throw new Error("Starlee could not find readable article text");
    }
    const classification = classifyAccess(document);
    emitArticleDiagnostic(options, "article_extraction_succeeded", {
      status: "ok",
      safe_metadata: {
        access: classification.access,
        access_reason: classification.reason,
        text_char_count: String(text.length),
        word_count: String(wordCount(text)),
        title_present: String(Boolean(article.title || metadata.title)),
        byline_present: String(Boolean(article.byline || metadata.byline)),
        published_at_present: String(Boolean(metadata.published_at))
      }
    });
    return {
      version: 1,
      type: "article",
      url: metadata.canonical,
      access: classification.access,
      dom_extract: {
        title: article.title || metadata.title,
        byline: article.byline || metadata.byline,
        site: article.siteName || metadata.site,
        published_at: metadata.published_at,
        text,
        summary: article.excerpt || undefined,
        html_meta: { ...htmlMeta(document), "starlee:access_reason": classification.reason }
      },
      tags: []
    };
  } catch (error) {
    if (error?.message !== "Starlee could not find readable article text") {
      emitArticleDiagnostic(options, "article_extraction_failed", {
        status: "capture_failed",
        message: "Article extraction failed before a payload could be built.",
        safe_metadata: {
          error_kind: "readability_exception",
          error_message: redactedErrorMessage(error?.message || error)
        }
      });
    }
    throw error;
  }
}

function emitArticleDiagnostic(options, event, detail = {}) {
  if (typeof options.onDiagnostic !== "function") return;
  options.onDiagnostic({
    component: "article_extractor",
    event,
    status: detail.status,
    message: detail.message,
    safe_metadata: detail.safe_metadata || {}
  });
}

function wordCount(value = "") {
  return value.trim().split(/\s+/).filter(Boolean).length;
}

function safeHost(document) {
  try {
    return document.location?.hostname?.replace(/^www\./, "") || "";
  } catch {
    return "";
  }
}

function redactedErrorMessage(message = "") {
  return String(message)
    .replace(/https?:\/\/\S+/g, "[url]")
    .replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "[email]")
    .slice(0, 160);
}
