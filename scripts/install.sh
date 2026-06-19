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
printf 'Installed Codex plugin source at %s\n' "$PLUGIN_HOME/starlee"
printf 'Registered personal plugin marketplace at %s\n' "$MARKETPLACE_PATH"
printf 'Browser extension folder: %s\n' "$HOME/Starlee/sensor-extension"
printf 'Load that folder in chrome://extensions once; then use the Save to Starlee page button.\n'
