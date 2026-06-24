import { extractArticle, isArticle } from "./article.js";
import { extractYouTube, isYouTubeWatch } from "./youtube.js";

export function detectedType(document) {
  if (isYouTubeWatch(document)) return "youtube";
  if (isArticle(document)) return "article";
  return null;
}

export async function capturePayload(document, options = {}) {
  const type = detectedType(document);
  options.onDiagnostic?.({
    component: "payload_builder",
    event: "payload_page_type_detected",
    status: type || "unsupported",
    safe_metadata: { page_type: type || "unsupported" }
  });
  if (type === "youtube") return withConsumedAt(await extractYouTube(document, {
    discoverTranscript: options.discoverYouTubeTranscript ?? false,
    transcriptDiscoveryTimeoutMs: options.transcriptDiscoveryTimeoutMs,
    onDiagnostic: options.onDiagnostic
  }));
  if (type === "article") return withConsumedAt(extractArticle(document));
  throw new Error("This page does not look like an article or YouTube video");
}

function withConsumedAt(payload) {
  return { ...payload, consumed_at: new Date().toISOString() };
}
