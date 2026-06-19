#!/bin/sh
set -eu

if [ -x "$HOME/.local/bin/starlee" ]; then
  exec "$HOME/.local/bin/starlee" mcp
fi

if [ -x "./target/release/starlee" ]; then
  exec "./target/release/starlee" mcp
fi

printf '%s\n' "Starlee is not installed. Run ./scripts/install.sh from the Starlee repository first." >&2
exit 127
