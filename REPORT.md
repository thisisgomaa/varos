# w2-export-paths — Lane C completion
Branch feat/w2-export-paths; original base b3d39ee; resumed WIP 67b2f9b, completion uncommitted.
Status: implemented (provisional UI, owner design review pending).
1.8: persisted Advanced toggle/state; All/Range, bleed/colour/whole-board scope, card × row expansion.
Exports: Scale/Suffix/Format/remove/add; iOS/Android/Web; prefix, folder reveal, scale/format sub-folders; single/per-artboard PDF.
SVG: row-specific styling/decimals/IDs/minify popovers; incumbent PDF options hook retained.
1.4: ⌘N New Document categories, size/units, count/grid/row/column/spacing, bleed/ppi; quick presets reachable.
Unit switching preserves physical dimensions; real headless kit Create → document test passes.
2.3: Outline uses coverage rings; Offset delta/join/miter; Expand bakes fill/stroke/corners/transforms; scale-strokes panel toggle.
Fixed grouped rotation, stroke sibling placement/paint order/clip exemption, and unstroked Outline no-op; mask-source conversion refuses.
4F: Direct-selection corner widgets, radius drag/typed field, round/inverted/chamfer; authored anchors stay live until bake.
Fixed tab-scoped corner IDs, locked/hidden targets, one-undo corner gesture, independent SVG row control IDs.
Fixed persisted grid-relative ranges, invalid ranges, Selection/whole-board availability, and app/Bridge PDF bleed parity/double expansion.
Bridge sub-folders authorize/canonicalize the root, use no-follow directory descriptors, retain fresh publication/collision refusal.
API 1.2 verbs/schema/list_verbs + capabilities; CLI new-document/export-screens; path/corner operations via CLI apply/EditCommand.
Writer change: next version, locally 6; key doc.paths[].corners with radius/kind; integrator assigns final number.
Named pure migration migrate_v5_to_live_corners; earlier-key/future-version refusals; frozen live fixture and v5 header replay.
Attribution: VectorCraft adaptations and NOTICE present; no missing offline crates or new uncached dependencies.
Gates: fmt / dependency directions / git diff --check PASS; native + Windows x86_64-pc-windows-msvc all-target Clippy -D warnings PASS.
Workspace gate: cargo test --offline --workspace -j 3 --no-fail-fast PASS (exit 0): 1,913 passed / 0 failed / 15 existing ignored; 110 suites.
Ratchets 3/3 PASS; unchanged ratchet source, ui.rs 816 ≤ 843; Bridge contracts 88 passed / 4 ignored.
Frozen audit: all 247 original fixture files (231 core + 16 Bridge) byte-identical to b3d39ee.
Evidence: /tmp/w2-final-*.log; /tmp/w2-ratchets.log; /tmp/w2-bridge-fixtures.log; /tmp/w2-immutability-audit.json; /tmp/w2-final-test-totals.json.
Merge: retain later-main changes; union Bridge 1.2 discovery and commands; renumber corner constant/migration/lane fixtures with sibling writers.
No commit/push/merge, GUI launch or installation; independent review, owner design/native acceptance and Windows runtime remain pending.
