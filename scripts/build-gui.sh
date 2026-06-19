#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/target/release/Starlee.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
swiftc -parse-as-library -O -framework AppKit "$ROOT/gui/StarleeMenuBar.swift" -o "$APP/Contents/MacOS/StarleeMenuBar"
cp "$ROOT/gui/Info.plist" "$APP/Contents/Info.plist"
cp "$ROOT/target/release/starlee" "$APP/Contents/Resources/starlee"
chmod 755 "$APP/Contents/MacOS/StarleeMenuBar" "$APP/Contents/Resources/starlee"
printf '%s\n' "$APP"
