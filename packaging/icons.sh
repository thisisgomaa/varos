#!/usr/bin/env bash
# Render from the original Varos ICNS, preserving its own macOS canvas/inset.
# Requires Python 3 + Pillow. Optional argument: owner-supplied ICNS path.
# Outputs are checked in; builds and packaging never need the source app installed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/crates/ui-egui/src/varos/generate_icons.py" "${1:-$ROOT/assets/app-icon/vectorcraft.icns}"
