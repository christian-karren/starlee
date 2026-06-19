#!/bin/sh
set -eu

BIN="${STARLEE_BIN:-$HOME/.local/bin/starlee}"
LABEL="com.starlee.capture"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
LOG_DIR="$HOME/Starlee/logs"

if [ ! -x "$BIN" ]; then
  printf '%s\n' "Starlee binary not found at $BIN" >&2
  exit 1
fi

mkdir -p "$HOME/Library/LaunchAgents" "$LOG_DIR"

cat > "$PLIST.tmp" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>$LABEL</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string>
    <string>serve</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$LOG_DIR/serve.log</string>
  <key>StandardErrorPath</key>
  <string>$LOG_DIR/serve.err.log</string>
</dict>
</plist>
EOF
mv "$PLIST.tmp" "$PLIST"

launchctl bootout "gui/$(id -u)" "$PLIST" >/dev/null 2>&1 || true
launchctl bootstrap "gui/$(id -u)" "$PLIST"
launchctl kickstart -k "gui/$(id -u)/$LABEL"
printf 'Installed and started Starlee capture service: %s\n' "$LABEL"
