#!/usr/bin/env python3
import json
import re
import sys


TOPIC_RULES = [
    ("Tech / AI", [
        " ai ", "artificial intelligence", "machine learning", "llm", "large language model",
        "model", "openai", "anthropic", "claude", "chatgpt", "inference", "training",
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
    return topics or ["General"]


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
