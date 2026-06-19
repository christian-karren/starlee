export function pageMetadata(document) {
  const canonical = document.querySelector('link[rel="canonical"]')?.href || document.location?.href || "";
  return {
    title: meta(document, "property", "og:title") || document.title || "Untitled",
    byline: meta(document, "name", "author") || meta(document, "property", "article:author") || undefined,
    site: meta(document, "property", "og:site_name") || document.location?.hostname || undefined,
    published_at: meta(document, "property", "article:published_time") || meta(document, "name", "date") || undefined,
    canonical
  };
}

export function htmlMeta(document) {
  const values = {};
  for (const element of document.querySelectorAll("meta[name], meta[property]")) {
    const key = element.getAttribute("property") || element.getAttribute("name");
    const content = element.getAttribute("content");
    if (key && content && values[key] === undefined) values[key] = content;
  }
  return values;
}

function meta(document, attribute, value) {
  return document.querySelector(`meta[${attribute}="${value}"]`)?.getAttribute("content")?.trim();
}

