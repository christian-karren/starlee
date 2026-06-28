export function attachSelectedText(payload, selectedText) {
  const text = String(selectedText || "").trim();
  if (!text || payload?.type !== "article" || !payload?.dom_extract) return payload;
  return {
    ...payload,
    dom_extract: {
      ...payload.dom_extract,
      selected_text: text
    }
  };
}
