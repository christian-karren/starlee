#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

if rg -n 'captions\.download|youtube\.com/api/timedtext' src sensor/src; then
  echo "Forbidden owner-only or server-side transcript API reference found" >&2
  exit 1
fi

if rg -n 'api\.openai\.com|api\.anthropic\.com|generativelanguage\.googleapis\.com' src sensor/src; then
  echo "External inference provider reference found" >&2
  exit 1
fi

cargo test share_bundle_strips_restricted_bodies_and_searches_read_only
cargo test requires_token_and_captures_authenticated_payload
echo "Starlee legal/privacy invariants passed"
