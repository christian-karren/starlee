import { extractArticle, isArticle } from "./article.js";
import { extractYouTube, isYouTubeWatch } from "./youtube.js";

export function detectedType(document) {
  if (isYouTubeWatch(document)) return "youtube";
  if (isArticle(document)) return "article";
  return null;
}

export function capturePayload(document) {
  const type = detectedType(document);
  if (type === "youtube") return withConsumedAt(extractYouTube(document));
  if (type === "article") return withConsumedAt(extractArticle(document));
  throw new Error("This page does not look like an article or YouTube video");
}

function withConsumedAt(payload) {
  return { ...payload, consumed_at: new Date().toISOString() };
}
