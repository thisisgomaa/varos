#!/bin/zsh
# Connect Claude Code to the Varos that is running right now (Bridge slice 1, ADR-0009).
# Writes a project-scoped `.mcp.json` next to this repo (gitignored: it holds this launch's token).
# Usage: tools/mac/bridge-connect.sh            # Varos must already be open
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/varos"
cargo build -q -p varos-cli -p varos-bridge
ENDPOINT="$(./target/debug/varos-cli bridge-endpoint | while read -r p; do [[ -S "$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["socket"])' "$p" 2>/dev/null)" ]] && echo "$p"; done | tail -1)"
if [[ -z "$ENDPOINT" ]]; then
  echo "bridge-connect: no running Varos with a live Bridge endpoint found — open Varos first (the installed /Applications/Varos.app is fine)." >&2
  exit 1
fi
SOCKET="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["socket"])' "$ENDPOINT")"
TOKEN="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["token"])' "$ENDPOINT")"
python3 - "$ROOT/.mcp.json" "$ROOT/varos/target/debug/varos-bridge" "$SOCKET" "$TOKEN" <<'PY'
import json, sys
out, bin_, sock, tok = sys.argv[1:]
json.dump({"mcpServers": {"varos": {"type": "stdio", "command": bin_, "args": ["mcp", "--attach", sock, "--token", tok]}}}, open(out, "w"), indent=2)
PY
echo "bridge-connect: wrote $ROOT/.mcp.json for the Varos launch at $SOCKET"
echo "Now open Claude Code in $ROOT (new session), approve the project MCP server 'varos' when asked, and check /mcp."
