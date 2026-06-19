const RESTRICTED_DOMAINS = new Set([
  "nytimes.com", "wsj.com", "ft.com", "economist.com", "bloomberg.com",
  "newyorker.com", "theatlantic.com", "washingtonpost.com", "latimes.com",
  "businessinsider.com", "substack.com", "medium.com"
]);

const PAYWALL_SELECTORS = [
  '[class*="paywall" i]', '[id*="paywall" i]', '[class*="subscribe" i]',
  '[data-testid*="paywall" i]', '[aria-label*="subscribe" i]'
];

export function classifyAccess(document, hostname = document.location?.hostname ?? "") {
  const schemaSignal = schemaAccessibility(document);
  if (schemaSignal === false) return result("restricted", "schema:isAccessibleForFree=false", "high");
  if (schemaSignal === true) return result("public", "schema:isAccessibleForFree=true", "high");

  const metaSignal = document.querySelector('[itemprop="isAccessibleForFree"]')?.getAttribute("content");
  if (isFalse(metaSignal)) return result("restricted", "microdata:isAccessibleForFree=false", "high");
  if (isTrue(metaSignal)) return result("public", "microdata:isAccessibleForFree=true", "high");

  const domain = hostname.toLowerCase().replace(/^www\./, "");
  if ([...RESTRICTED_DOMAINS].some((known) => domain === known || domain.endsWith(`.${known}`))) {
    return result("restricted", "known-metered-domain", "medium");
  }
  if (PAYWALL_SELECTORS.some((selector) => document.querySelector(selector))) {
    return result("restricted", "paywall-marker", "medium");
  }
  const bodyText = (document.body?.innerText || document.body?.textContent || "").slice(0, 10000).toLowerCase();
  if (/subscribe to (continue|read)|already a subscriber|sign in to continue|unlock this article/.test(bodyText)) {
    return result("restricted", "paywall-copy", "medium");
  }
  return result("restricted", "ambiguous-fail-closed", "low");
}

function schemaAccessibility(document) {
  for (const script of document.querySelectorAll('script[type="application/ld+json"]')) {
    try {
      const found = findAccessibility(JSON.parse(script.textContent || "null"));
      if (found !== undefined) return found;
    } catch { /* malformed publisher metadata is not authoritative */ }
  }
  return undefined;
}

function findAccessibility(value) {
  if (!value || typeof value !== "object") return undefined;
  if (Object.hasOwn(value, "isAccessibleForFree")) {
    const signal = value.isAccessibleForFree;
    if (isTrue(signal)) return true;
    if (isFalse(signal)) return false;
  }
  for (const child of Object.values(value)) {
    const found = findAccessibility(child);
    if (found !== undefined) return found;
  }
  return undefined;
}

const isTrue = (value) => value === true || String(value).toLowerCase() === "true";
const isFalse = (value) => value === false || String(value).toLowerCase() === "false";
const result = (access, reason, confidence) => ({ access, reason, confidence });

