# Lane E — Phase 11 — feat/w3-live
Implemented (provisional UI, owner design review pending); worktree only, uncommitted.
Blend W: two-click Make, cubic/colour interpolation, unequal-contour subdivision, Replace Spine + Direct spine editing/orientation.
Repeat: radial/grid/mirror; Envelope: Arc/Flag/Bulge warp or four-point 2×2 mesh.
Authored children remain editable; isolation, Options, Release, Expand, source-edit rollback and undo; repeated rejection remains fallible.
One bounded pure evaluator with exact per-node and document caches; idle reuse covered headlessly.
Canvas/hit bounds and PDF/SVG consume derived geometry; native files preserve editable sources.
API 1.2 verbs: live_make/options/release/expand/isolate/spine; full progressive schemas, no inline additions.
CLI existing apply path exercises API guard, Make, Options, native reopen and Expand in a real-process test.
FORMAT v13: named pure migrate_v12_to_v13; frozen JSON/PDF/refusal fixtures + hashes.
INTEGRATOR: replace lane-local v9→v10→v11→v12 identity rows with sibling migrations, then append v14.
New modules under varos/crates: varos-core/src/live/*, varos-bridge/src/live.rs, varos-app/src/ui/live.rs.
Shared seams marked Lane E; prior-art headers + NOTICE attribution to VectorCraft a469568.
330 protected tracked fixtures/tokens/kit/ratchet files audited unchanged; ui.rs 809/843 lines.
Existing format tests now compare document bytes/parsed PDF objects across one-/two-digit stamps.
Test-only lopdf reuse added to raster; no new runtime crates; no missing offline dependency.
Limits: vector path sources only, matching contour topology; ≤4096 outputs / 200000 anchors.
Envelope samples cubics at 64 intervals; nonlinear gradient envelopes explicitly refused.
Source coordinates ≤1e6 pt; spine ≤64 anchors; grouped/image/text/mask sources refused.
Gates: fmt/dependency directions PASS; native and Windows all-target clippy -D warnings PASS.
New tests: 22 PASS (14 core / 3 Bridge / 4 PDF / 1 real CLI); UI ratchets: 3 PASS.
Bridge contracts: 92 PASS / 4 ignored; API 1.0/1.1 frozen, tools/list 1.2 23887/24000 B, list_verbs 15007 B.
Full workspace: 2264 PASS / 0 FAIL / 15 ignored; cargo test --offline --workspace -j 3 --no-fail-fast.
No commit/push, GUI launch or installation; worktree changes only.
Pending: owner native/design acceptance and mixed-lane integration review; no hands-on UI claim.
Docs: docs/reference/LIVE_NODES_V13.md; docs/adr/ADR-0008-live-nodes-v13.md; PLAN progress.
Evidence: /tmp/w3-live-workspace-release.log, /tmp/w3-live-clippy-{native,windows}-release.log, /tmp/w3-live-{fmt,deps,bridge-cap,ratchets}.log.
Wasm text gate: not applicable (Lane E; no text implementation changes).
