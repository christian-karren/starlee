#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST="${STARLEE_INSTALL_DIR:-$HOME/.local/bin}"
PLUGIN_HOME="${STARLEE_PLUGIN_HOME:-$HOME/plugins}"
MARKETPLACE_PATH="${STARLEE_MARKETPLACE_PATH:-$HOME/.agents/plugins/marketplace.json}"

cd "$ROOT/sensor"
npm install
npm run build
cd "$ROOT"
cargo build --release --locked
mkdir -p "$DEST"
install -m 755 "$ROOT/target/release/starlee" "$DEST/starlee"
chmod +x "$ROOT/scripts/starlee-mcp.sh" "$ROOT/scripts/install-service.sh"
"$DEST/starlee" setup >/dev/null

if [ "$(uname -s)" = "Darwin" ] && [ "${STARLEE_INSTALL_SERVICE:-1}" != "0" ]; then
  STARLEE_BIN="$DEST/starlee" "$ROOT/scripts/install-service.sh"
fi

if [ "$(uname -s)" = "Darwin" ] && [ "${STARLEE_INSTALL_APP:-1}" != "0" ]; then
  APP_PATH=$("$ROOT/scripts/build-gui.sh")
  APP_DEST="${STARLEE_APP_DIR:-$HOME/Applications}"
  mkdir -p "$APP_DEST"
  pkill -f "$APP_DEST/Starlee.app/Contents/MacOS/StarleeMenuBar" >/dev/null 2>&1 || true
  rm -rf "$APP_DEST/Starlee.app"
  cp -R "$APP_PATH" "$APP_DEST/Starlee.app"
  MENUBAR_PLIST="$HOME/Library/LaunchAgents/com.starlee.menubar.plist"
  if [ -f "$MENUBAR_PLIST" ]; then
    launchctl bootout "gui/$(id -u)" "$MENUBAR_PLIST" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$MENUBAR_PLIST"
    launchctl kickstart -k "gui/$(id -u)/com.starlee.menubar"
  else
    open "$APP_DEST/Starlee.app"
  fi
fi

if [ "$(uname -s)" = "Darwin" ] && [ "${STARLEE_INSTALL_SAFARI:-1}" != "0" ]; then
  if ! "$ROOT/scripts/install-safari-extension.sh"; then
    printf 'Warning: Safari extension install did not complete. Run: ./scripts/install-safari-extension.sh\n' >&2
  fi
fi

mkdir -p "$PLUGIN_HOME" "$(dirname "$MARKETPLACE_PATH")"
ln -sfn "$ROOT" "$PLUGIN_HOME/starlee"

python3 - "$MARKETPLACE_PATH" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1]).expanduser()
if path.exists():
    data = json.loads(path.read_text())
else:
    data = {"name": "personal", "interface": {"displayName": "Personal"}, "plugins": []}

data.setdefault("name", "personal")
data.setdefault("interface", {}).setdefault("displayName", "Personal")
plugins = data.setdefault("plugins", [])
entry = {
    "name": "starlee",
    "source": {"source": "local", "path": "./plugins/starlee"},
    "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
    "category": "Productivity",
}
for index, plugin in enumerate(plugins):
    if plugin.get("name") == "starlee":
        plugins[index] = entry
        break
else:
    plugins.append(entry)

path.write_text(json.dumps(data, indent=2) + "\n")
PY

if command -v codex >/dev/null 2>&1; then
  if ! codex plugin add starlee@personal >/dev/null; then
    printf 'Warning: Codex plugin install did not complete. Run: codex plugin add starlee@personal\n' >&2
  fi
  codex mcp remove starlee >/dev/null 2>&1 || true
fi

printf 'Installed Starlee to %s\n' "$DEST/starlee"
printf 'Initialized local vault at %s\n' "$HOME/Starlee"
if [ "$(uname -s)" = "Darwin" ] && [ "${STARLEE_INSTALL_APP:-1}" != "0" ]; then
  printf 'Installed Starlee app to %s\n' "${STARLEE_APP_DIR:-$HOME/Applications}/Starlee.app"
fi
if [ "$(uname -s)" = "Darwin" ] && [ "${STARLEE_INSTALL_SAFARI:-1}" != "0" ]; then
  printf 'Installed Starlee Safari app to %s\n' "${STARLEE_APP_DIR:-$HOME/Applications}/Starlee Safari.app"
fi
printf 'Installed Codex plugin source at %s\n' "$PLUGIN_HOME/starlee"
printf 'Registered personal plugin marketplace at %s\n' "$MARKETPLACE_PATH"
printf 'Browser extension folder: %s\n' "$HOME/Starlee/sensor-extension"
printf 'Load that folder in chrome://extensions once; then use the Save to Starlee page button.\n'
printf '\nRedacted Starlee doctor report:\n'
"$DEST/starlee" doctor
