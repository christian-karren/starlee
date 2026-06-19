import { Readability, isProbablyReaderable } from "@mozilla/readability";
import { classifyAccess } from "./access.js";
import { htmlMeta, pageMetadata } from "./metadata.js";

export function isArticle(document) {
  return isProbablyReaderable(document, { minContentLength: 120, minScore: 18 });
}

export function extractArticle(document) {
  const metadata = pageMetadata(document);
  const article = new Readability(document.cloneNode(true), { charThreshold: 120 }).parse();
  if (!article?.textContent?.trim()) throw new Error("Starlee could not find readable article text");
  const classification = classifyAccess(document);
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
      text: article.textContent.trim(),
      summary: article.excerpt || undefined,
      html_meta: { ...htmlMeta(document), "starlee:access_reason": classification.reason }
    },
    tags: []
  };
}

