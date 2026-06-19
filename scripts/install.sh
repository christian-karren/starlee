#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST="${STARLEE_INSTALL_DIR:-$HOME/.local/bin}"

cd "$ROOT/sensor"
npm install
npm run build
cd "$ROOT"
cargo build --release --locked
mkdir -p "$DEST"
install -m 755 "$ROOT/target/release/starlee" "$DEST/starlee"
printf 'Installed Starlee to %s\n' "$DEST/starlee"
printf 'Next: %s setup\n' "$DEST/starlee"
