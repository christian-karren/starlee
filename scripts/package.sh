#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCH=$(uname -m)
NAME="starlee-${VERSION}-macos-${ARCH}"
OUT="$ROOT/release/$NAME"
DMG_NAME="Starlee-${VERSION}-macos-${ARCH}"
DMG_STAGING="$ROOT/release/$DMG_NAME-staging"
DMG_BACKGROUND="$ROOT/assets/brand/starlee_dmg_background.png"
RW_DMG="$ROOT/release/$DMG_NAME-rw.dmg"
DMG="$ROOT/release/$DMG_NAME.dmg"
VOLUME_NAME="Starlee Installer"
DMG_MOUNT="/Volumes/$VOLUME_NAME"

mkdir -p "$OUT"
cp "$ROOT/target/release/starlee" "$OUT/starlee"
cp "$ROOT/README.md" "$OUT/README.md"
cp "$ROOT/LICENSE" "$OUT/LICENSE"
cp -R "$ROOT/docs" "$OUT/docs"
cp -R "$ROOT/target/release/Starlee.app" "$OUT/Starlee.app"
chmod 755 "$OUT/starlee"
LC_ALL=C tar -C "$ROOT/release" -czf "$ROOT/release/$NAME.tar.gz" "$NAME"
rm -rf "$DMG_STAGING" "$RW_DMG" "$DMG"
mkdir -p "$DMG_STAGING/.background"
cp -R "$ROOT/target/release/Starlee.app" "$DMG_STAGING/Starlee.app"
ln -s /Applications "$DMG_STAGING/Applications"
cp "$DMG_BACKGROUND" "$DMG_STAGING/.background/starlee_dmg_background.png"
hdiutil create \
  -volname "$VOLUME_NAME" \
  -srcfolder "$DMG_STAGING" \
  -fs HFS+ \
  -format UDRW \
  -ov \
  "$RW_DMG" >/dev/null
hdiutil detach "$DMG_MOUNT" >/dev/null 2>&1 || true
hdiutil attach "$RW_DMG" \
  -mountpoint "$DMG_MOUNT" \
  -readwrite \
  -noverify >/dev/null
cleanup_dmg_mount() {
  if [ -d "$DMG_MOUNT" ]; then
    hdiutil detach "$DMG_MOUNT" >/dev/null 2>&1 || true
  fi
}
trap cleanup_dmg_mount EXIT INT TERM
chflags hidden "$DMG_MOUNT/.background" 2>/dev/null || true
osascript <<APPLESCRIPT
tell application "Finder"
  tell disk "$VOLUME_NAME"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {100, 100, 1300, 860}
    set viewOptions to the icon view options of container window
    set arrangement of viewOptions to not arranged
    set icon size of viewOptions to 160
    set background picture of viewOptions to file ".background:starlee_dmg_background.png"
    set position of item "Starlee.app" of container window to {300, 380}
    set position of item "Applications" of container window to {900, 380}
    close
    open
    update without registering applications
    delay 5
  end tell
end tell
APPLESCRIPT
sync
hdiutil detach "$DMG_MOUNT" >/dev/null
trap - EXIT INT TERM
hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$DMG" >/dev/null
rm -rf "$DMG_STAGING" "$RW_DMG"
printf '%s\n' "$ROOT/release/$NAME.tar.gz"
printf '%s\n' "$DMG"
