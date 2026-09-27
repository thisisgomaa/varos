# Bundled UI fonts

Unmodified, complete Regular (400) TTF files from the official IBM Plex and Noto repositories.
`manifest.json` pins the upstream commits, package versions where available, URLs, byte lengths
and SHA-256 hashes. No system-font lookup, downloaded-at-startup font, subset or renamed font.

- IBM Plex Sans: proportional UI labels and names.
- IBM Plex Mono: numeric/data roles.
- Noto Sans Symbols 2: missing Mac Command/Option/Shift symbols; Noto Sans Symbols: Control ⌃.
- IBM Plex Sans Arabic: included as the named `IBM Plex Sans Arabic` family for diagnostics;
  **not** in the production proportional/mono fallback chains. egui 0.35's RTL cluster/cursor
  mapping failed the U0-A probe. Do not enable it in editable UI until the text-layout seam is fixed.

Existing egui default fonts remain as final fallbacks; no claim of full Unicode coverage or
Arabic/bidi editing support. The regular UI sizes and colors are unchanged in this slice.

IBM's original copyright/OFL notices are the three `plex-*-LICENSE.txt` files. Both Noto faces
use `noto-symbols-LICENSE.txt` (OFL 1.1). `tools/mac/bundle.sh` copies these notices and the
manifest to `Varos.app/Contents/Resources/Licenses/Fonts`. Distributing a bare executable also
requires accompanying these notices. Fonts are never modified under their Reserved Font Names.

For an asset update, compare every file with its recorded upstream revision and verify the
manifest's byte count and SHA-256. Run `cargo test --locked -p varos-app --test fonts` and the
explicit ignored diagnostic, then inspect the real native window before promoting a new family.
Coverage tests parse cmap directly using the already-locked ttf-parser (test dependency):
epaint 0.35 `has_glyph` compares font-face identity with the replacement face and reports false
negatives for valid glyphs in that face, so it is unsuitable as the coverage oracle here.
