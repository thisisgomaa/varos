#!/bin/zsh
# DEPRECATED (ADR-0011 C1, 2026-10-07): kept only so the old name keeps working.
# It no longer writes a token-bearing project .mcp.json. It runs the one-time, token-free,
# user-scope registration instead:   varos-cli bridge register claude [--replace] [--dry-run]
# Prefer running that command directly (from Varos.app/Contents/MacOS/ once installed).
# A project .mcp.json written by the old version of this script still holds an expired
# per-launch token: delete it; it is no longer needed.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
echo "Local agents are trusted; no pairing needed. Register once, then say use Varos. bridge-connect.sh is deprecated; running: varos-cli bridge register claude $*" >&2
for installed in "/Applications/Varos.app/Contents/MacOS/varos-cli" "$HOME/Applications/Varos.app/Contents/MacOS/varos-cli"; do
  if [[ -x "$installed" && -x "${installed:h}/varos-bridge" ]]; then
    exec "$installed" bridge register claude "$@"
  fi
done
cd "$ROOT/varos"
cargo build -q -p varos-cli -p varos-bridge
exec ./target/debug/varos-cli bridge register claude "$@"
