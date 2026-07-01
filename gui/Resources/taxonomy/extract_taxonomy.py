#!/usr/bin/env python3
import json
import re
import sys


TOPIC_RULES = [
    ("Tech / AI", [
        " ai ", "artificial intelligence", "machine learning", "llm", "large language model",
        "model", "openai", "anthropic", "claude", "chatgpt", "inference", "training",
    ]),
    ("Tech / AI Infrastructure", [
        "ai infrastructure", "data center", "datacenter", "compute cluster", "gpu cluster",
        "inference infrastructure", "training cluster", "cloud gpu", "accelerator",
    ]),
    ("Tech / Enterprise SaaS", [
        "figma", "salesforce", "saas", "enterprise software", "enterprise", "b2b software",
        "productivity software", "collaboration software", "workflow", "design tool",
    ]),
    ("Tech / Semiconductors", [
        "semiconductor", "semiconductors", "chip", "chips", "gpu", "nvidia", "tsmc",
        "amd", "foundry", "wafer", "lithography", "memory chip",
    ]),
    ("Tech / Fintech", [
        "fintech", "stripe", "payments", "banking", "neobank", "lending", "credit card",
        "wallet", "crypto exchange", "stablecoin",
    ]),
    ("Tech / Consumer Hardware", [
        "iphone", "ipad", "apple watch", "headset", "vision pro", "wearable",
        "consumer hardware", "device", "devices",
    ]),
    ("Tech / Robotics", [
        "robot", "robots", "robotics", "humanoid", "autonomous", "drone", "warehouse automation",
    ]),
    ("Tech / Digital Advertising", [
        "advertising", "adtech", "ads", "digital ads", "google ads", "meta ads",
        "performance marketing", "targeting",
    ]),
    ("Tech / E-commerce", [
        "e-commerce", "ecommerce", "shopify", "marketplace", "retail marketplace",
        "online shopping", "merchant",
    ]),
    ("Tech / Cybersecurity", [
        "cybersecurity", "security", "ransomware", "malware", "phishing", "zero trust",
        "breach", "vulnerability",
    ]),
    ("Politics / Presidency", [
        "president", "presidency", "white house", "executive order", "administration",
        "trump", "biden",
    ]),
    ("Politics / House", [
        "house of representatives", "speaker of the house", "congressman", "congresswoman",
        "house republican", "house democrat",
    ]),
    ("Politics / Senate", [
        "senate", "senator", "filibuster", "majority leader", "minority leader",
    ]),
    ("Politics / Elections", [
        "election", "campaign", "primary", "polling", "ballot", "voter", "electoral",
    ]),
    ("Politics / Middle East", [
        "middle east", "israel", "iran", "gaza", "hamas", "hezbollah", "netanyahu",
        "saudi arabia",
    ]),
    ("Business / Markets", [
        "stock market", "markets", "earnings", "revenue", "profit", "ipo", "valuation",
        "acquisition", "merger",
    ]),
    ("Business / Oil & Gas", [
        "oil", "gas", "opec", "crude", "lng", "shale", "refinery", "energy market",
    ]),
    ("Business / Retail", [
        "retail", "consumer spending", "store", "stores", "brand", "supply chain",
    ]),
    ("News / General", [
        "breaking", "report", "reported", "according to", "new york times", "washington post",
        "associated press", "reuters", "bloomberg",
    ]),
]


KNOWN_COMPANIES = [
    "AMD", "Adobe", "Airbnb", "Amazon", "Anthropic", "Apple", "Cursor", "Databricks",
    "Figma", "Google", "Intel", "Meta", "Microsoft", "Netflix", "NVIDIA", "OpenAI",
    "Palantir", "Salesforce", "Shopify", "SpaceX", "Stripe", "Tesla", "TSMC",
]


def normalize_text(item):
    parts = [
        item.get("title", ""),
        item.get("source", ""),
        item.get("site", ""),
        item.get("author", ""),
        item.get("snippet", ""),
        " ".join(item.get("topics", []) or []),
    ]
    return " " + " ".join(str(part) for part in parts if part) + " "


def contains(text, needle):
    if needle.strip() in {"ai", "ads"}:
        return re.search(rf"\b{re.escape(needle.strip())}\b", text) is not None
    return needle in text


def extract_topics(text):
    lower = text.lower()
    topics = []
    for topic, needles in TOPIC_RULES:
        if any(contains(lower, needle) for needle in needles):
            topics.append(topic)
    return topics or ["News / General"]


def extract_companies(text):
    companies = []
    for company in KNOWN_COMPANIES:
        if re.search(rf"(?<![A-Za-z0-9]){re.escape(company)}(?![A-Za-z0-9])", text, re.IGNORECASE):
            companies.append(company)
    return sorted(set(companies), key=lambda value: value.lower())


def main():
    try:
        items = json.load(sys.stdin)
    except json.JSONDecodeError:
        print(json.dumps({"items": []}))
        return
    output = []
    for item in items if isinstance(items, list) else []:
        text = normalize_text(item)
        output.append({
            "id": item.get("id", ""),
            "topics": extract_topics(text),
            "companies": extract_companies(text),
        })
    print(json.dumps({"items": output}, separators=(",", ":")))


if __name__ == "__main__":
    main()
