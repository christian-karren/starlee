#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

cd "$ROOT/sensor"
npm install
npm run build

cd "$ROOT"
cargo test bridge_smoke_saves_menu_bar_capture_and_records_sanitized_terminal_status
