# SVG import (slice 7.1, 2026-10-09)

Owner lifted the Figma gate for tonight. Provisional File menu/native picker patterns reuse the
existing shell; owner design review and GUI acceptance are pending. No new shortcut is assigned.

`varos-import` is the foreign-format firewall. It depends internally on core only; core's native
format reader has no import dependency. Locally available usvg 0.45.1 normalizes SVG; no prior-art
code was borrowed. Font discovery and external resource resolution are disabled.

Open accepts SVG/SVGZ and creates a dirty Untitled document with one transparent, viewBox-sized
artboard. With a viewBox, declared viewport dimensions are normalized to viewBox units and its
origin maps to [0,0]. Without a viewBox, usvg's resolved viewport is used. Place SVG uses an embedded
vector group, remaps all ids, preserves nested groups, and creates one undo entry.

`varos-cli import-svg input.svg --out output.vrs` writes an editable document and returns a report.
Bridge `import_svg` requires explicit `api:"1.2"`, board, expected_rev, request_id and absolute path;
it places into the specified board and returns an observed mutation receipt (rev, undo_steps,
selection and changes) with the loss report synchronously. Retried requests retain that receipt. Existing files-scope
containment applies; symlinks, hardlinks, protected roots and network volumes are refused. Non-Unix
Bridge source reads currently fail closed; Open/Place/CLI are platform-neutral. MCP discovery opts
in with `tools/list` params `{"api":"1.2"}`; default tool listings and 1.0/1.1 replies are unchanged.

Limits: 16 MiB compressed/decompressed input, 100000 XML nodes, 128 ancestor levels, 1000 contours per
path, plus the existing core document limits. Gzip decoding is bounded before XML parsing.

Unsupported gradients/patterns, text, images, filters, clipping/masks and animation are reported.
Stroke dashes and caps/joins differing from Varos’s undashed round caps/joins, nonuniform stroke transforms, compositing isolation, reversed paint order
and nonzero compound-fill conversion are reported when encountered. Group opacity is flattened
onto paths and reported because overlapping children can differ. Even-odd holes remain editable;
disjoint contours and nested islands become separate paths. Containment uses flattened curves;
crossing/touching compound boundaries are explicitly refused before whole-contour containment.
Self-intersections within a single contour and curve flattening remain fidelity limitations.

Integration seams: the SVG-only row is `file.place.svg` / `PlaceSvg` (label "Place SVG…"), without
shortcuts or settings keys, so raster Place can remain additive. Import uses the format-neutral
`PlaceArtwork` command, which refuses an active transaction before allocating IDs or beginning
its own undo step. Keep sibling crash-safety guards around this command when integrating.
Bridge discovery adds only `import_svg` under the 1.2 opt-in; combine sibling allowlists/tool
metadata without changing legacy lists/fixtures. A future StrokeStyle integration should map
SVG caps/joins/dashes there and remove these loss notes only after renderer/export coverage.
