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
  if (type === "youtube") {
    const payload = withConsumedAt(await extractYouTube(document, {
      discoverTranscript: options.discoverYouTubeTranscript ?? false,
      transcriptDiscoveryTimeoutMs: options.transcriptDiscoveryTimeoutMs,
      onDiagnostic: options.onDiagnostic
    }));
    emitPayloadBuilt(options, payload);
    return payload;
  }
  if (type === "article") {
    const payload = withConsumedAt(extractArticle(document, options));
    emitPayloadBuilt(options, payload);
    return payload;
  }
  throw new Error("This page does not look like an article or YouTube video");
}

function withConsumedAt(payload) {
  return { ...payload, consumed_at: new Date().toISOString() };
}

function emitPayloadBuilt(options, payload) {
  options.onDiagnostic?.({
    component: "payload_builder",
    event: "payload_built",
    status: "ok",
    safe_metadata: {
      payload_type: payload?.type || "unknown",
      access: payload?.access || "unknown",
      text_char_count: String(payload?.dom_extract?.text?.length || 0),
      transcript_segment_count: String(payload?.transcript?.length || 0),
      transcript_status: payload?.transcript_status || "",
      transcript_source: payload?.transcript_source || "",
      transcript_reason: payload?.transcript_reason || ""
    }
  });
}
