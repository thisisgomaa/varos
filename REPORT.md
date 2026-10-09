Lane p7-svgimport — slice 7.1 SVG import; working-tree changes retained, no commit/push.
Delivered: varos-import firewall; SVG/SVGZ Open/Place; CLI import-svg; Bridge 1.2 import_svg.
Core placement preserves groups/remaps IDs with one undo; native format/schema untouched.
Shared app/Bridge/core edits remain additive, localized; importer/placement live in new modules.

## Fix round
P1 topology — replaced first-point nesting with boundary intersection checks and whole-contour containment.
Crossing/touching compound contours explicitly fail import rather than silently deleting artwork.
Tests: reviewer repro + reordered starts + concave crossing + edge/vertex/coincident contact;
disjoint concave overlapping bounds, existing holes/islands and round-trip coverage retained.
P1 Bridge receipt — refresh document/selection observations; return mutation receipt with loss report.
Mutating host test verifies envelope/result revision, undo_steps=1, created IDs/selection,
byte-identical retry/status receipt, immediate second import using returned rev, and two atomic undos.
P2 strokes — report default butt/miter and square/bevel conversion to Varos round caps/joins.
Tests: defaults/explicit mismatches/dashes warned; round/round and export-import round-trip loss-free.
Merge risks — SVG-only PlaceSvg/file.place.svg now labels Place SVG…; no shortcut/settings keys.
Menu test checks namespaced row, label, command and absent accelerator; placement rejects open transactions.
Transaction test proves refusal preserves document/revision and the caller’s open transaction.
Sibling raster Place, StrokeStyle mapping, crash guards and API 1.2 allowlist union documented for integrator.
Gates: workspace PASS — 1532 passed / 0 failed / 15 ignored across 90 suites.
Fmt/dependency directions PASS; Python tool tests 16/16; importer 14/14; Bridge import 3/3.
Native and Windows x86_64-pc-windows-msvc all-target clippy -D warnings PASS, 0 warnings.
Ratchets 3/3; Bridge contracts 70 passed/4 ignored; legacy fixtures byte-unchanged.
Ratchet file/tokens/ui.rs (843)/native reader/core fixtures unchanged; logs /tmp/p7-fix-*.log.
All findings fixed; no disagreements. Owner GUI/design acceptance pending; no GUI/install/commit/push/merge.
