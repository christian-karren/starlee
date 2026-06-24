#!/bin/sh
set -eu

APP="${1:-target/release/Starlee.app}"
PLIST="$APP/Contents/Info.plist"

if [ ! -f "$PLIST" ]; then
  printf 'missing app Info.plist: %s\n' "$PLIST" >&2
  exit 1
fi

if [ ! -x "$APP/Contents/MacOS/StarleeMenuBar" ]; then
  printf 'missing app executable: %s\n' "$APP/Contents/MacOS/StarleeMenuBar" >&2
  exit 1
fi

if [ ! -x "$APP/Contents/Resources/starlee" ]; then
  printf 'missing bundled starlee CLI: %s\n' "$APP/Contents/Resources/starlee" >&2
  exit 1
fi

if [ ! -f "$APP/Contents/Resources/StarleeDesktopIcon.icns" ]; then
  printf 'missing desktop app icon: %s\n' "$APP/Contents/Resources/StarleeDesktopIcon.icns" >&2
  exit 1
fi

if [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$PLIST")" != "StarleeMenuBar" ]; then
  printf 'unexpected CFBundleExecutable in %s\n' "$PLIST" >&2
  exit 1
fi

if [ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIconFile' "$PLIST")" != "StarleeDesktopIcon" ]; then
  printf 'unexpected CFBundleIconFile in %s\n' "$PLIST" >&2
  exit 1
fi

if /usr/libexec/PlistBuddy -c 'Print :LSUIElement' "$PLIST" >/dev/null 2>&1; then
  printf 'Starlee.app must be Dock-visible; remove LSUIElement from %s\n' "$PLIST" >&2
  exit 1
fi
