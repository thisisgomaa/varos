# Lane E — Phase 11 — feat/w3-live
Blend, Repeat and Envelope implemented (provisional UI, owner design review pending); FORMAT v13.
Authored sources remain editable; bounded cached evaluation serves canvas, hit bounds, SVG/PDF and Expand.
API 1.2 live verbs use progressive schemas; CLI and frozen native/refusal fixtures are covered.
Sibling appearance/effects, CMYK, render, Arabic/a11y and text/v14 reconciliation remains integration work.

## Fix round
P1 spine replacement: released root-level spine returns to roots; structural precheck precedes publication.
P1 mesh placement: points follow baked/live geometry transforms and offset Paste; duplicate maps only its copy.
P2 swatches: Make preserves authored fill/stroke references; evaluation resolves paints without destroying links.
Four new regressions cover root save/reopen/undo, recolour/Release, Move/copy/Paste, multi-source previews/undo/redo.
Targeted live tests: 18 PASS; fmt, dependency directions, native + Windows all-target clippy -D warnings PASS.
Full offline workspace gate: 2271 PASS / 0 FAIL / 15 ignored (--workspace -j 3 --no-fail-fast).
UI ratchets: 3 PASS; Bridge ratchets/frozen 1.0/1.1 fixtures: 6 PASS; 1.2 tools/list 23887/24000 B.
Ratchets, protected fixtures/tokens/kit and UI source unchanged; ui.rs remains 809/843 lines.
Disagreements: none with the three reproduced defects; merge-risk items require sibling integration review.
Evidence: /tmp/w3-live-fix-{targeted,fmt,deps,workspace,clippy-native,clippy-windows,ratchets,bridge-fixtures}.log.
Fixes are uncommitted worktree edits; no git writes, merge, GUI launch, bundle rebuild, push or installation.
Pending: fresh independent fix review, cross-lane integration, and owner native/design acceptance.
