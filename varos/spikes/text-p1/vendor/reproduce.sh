#!/bin/zsh
# Rebuild vendor/cosmic-text (gitignored, ~6 MB) from the pristine registry copy + the P1b patch.
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
SRC="$(ls -d ~/.cargo/registry/src/*/cosmic-text-0.19.0 | head -1)"
[[ -d "$SRC" ]] || { echo "cosmic-text 0.19.0 is not in the local registry: run 'cargo fetch' in spikes/text-p1 online once" >&2; exit 1; }
rm -rf "$HERE/cosmic-text" && cp -R "$SRC" "$HERE/cosmic-text"
( cd "$HERE/cosmic-text" && patch -p1 --silent < "$HERE/cosmic-text-0.19.0-p1b.patch" )
echo "vendor/cosmic-text rebuilt from $SRC + cosmic-text-0.19.0-p1b.patch"
