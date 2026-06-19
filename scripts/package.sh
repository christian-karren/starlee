#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCH=$(uname -m)
NAME="starlee-${VERSION}-macos-${ARCH}"
OUT="$ROOT/release/$NAME"

mkdir -p "$OUT"
cp "$ROOT/target/release/starlee" "$OUT/starlee"
cp "$ROOT/README.md" "$OUT/README.md"
cp "$ROOT/LICENSE" "$OUT/LICENSE"
cp -R "$ROOT/docs" "$OUT/docs"
cp -R "$ROOT/target/release/Starlee.app" "$OUT/Starlee.app"
chmod 755 "$OUT/starlee"
LC_ALL=C tar -C "$ROOT/release" -czf "$ROOT/release/$NAME.tar.gz" "$NAME"
printf '%s\n' "$ROOT/release/$NAME.tar.gz"
