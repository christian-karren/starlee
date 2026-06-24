#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/target/release/Starlee.app"
ICON_SOURCE="$ROOT/assets/brand/starlee_desktop_application_icon.png"
ICONSET="$ROOT/target/release/StarleeDesktopIcon.iconset"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
swiftc -parse-as-library -O -framework AppKit -framework WebKit -framework UserNotifications "$ROOT"/gui/*.swift -o "$APP/Contents/MacOS/StarleeMenuBar"
cp "$ROOT/gui/Info.plist" "$APP/Contents/Info.plist"
cp "$ROOT/target/release/starlee" "$APP/Contents/Resources/starlee"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
sips -z 16 16 "$ICON_SOURCE" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$ICON_SOURCE" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$ICON_SOURCE" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$ICON_SOURCE" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$ICON_SOURCE" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$ICON_SOURCE" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$ICON_SOURCE" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$ICON_SOURCE" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$ICON_SOURCE" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$ICON_SOURCE" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/StarleeDesktopIcon.icns"
rm -rf "$ICONSET"
if [ -d "$ROOT/gui/Resources" ]; then
  cp -R "$ROOT/gui/Resources/." "$APP/Contents/Resources/"
fi
chmod 755 "$APP/Contents/MacOS/StarleeMenuBar" "$APP/Contents/Resources/starlee"
if command -v codesign >/dev/null 2>&1; then
  codesign --force --deep --sign - "$APP" >/dev/null
fi
"$ROOT/scripts/verify-gui-bundle.sh" "$APP"
printf '%s\n' "$APP"
