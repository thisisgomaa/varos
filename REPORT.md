# Lane A — Phase 6 / FORMAT v10
State: implemented (provisional UI, owner design review pending); uncommitted `feat/w3-appearance`.
Scope: ordered base markers + extra fills/strokes; per-entry paint/opacity/visibility/Normal blend; group/layer look; Expand Appearance.
Masks: one stored form, GroupRole Clip/MaskAlpha + mask_child; row wrapping, empty drawing target, mode/release, nested masks, world geometry preserved.
UI: kit-only Properties Appearance; add/reorder/delete/swatch/opacity/visibility; Layers fx jump, Add mask, thumbnail drop, ⌘7 chip.
Rendering: recursive GPU draw lists/pooled layers; CPU isolated alpha; PDF transparency Forms + Alpha SMask; SVG masks + ordered paint elements.
GPU limits: depth 6, pool 512 MiB including colour MSAA/resolved surfaces; over-budget/depth composition flattens with Renderer.appearance_notes.
Fallback loss: deeper isolation/opacity/masks are omitted on the canvas; document and CPU/PDF/SVG output retain the full tree.
CPU guard: 128 MiB intermediate-surface budget, depth/surface cap 12; export refuses before allocating rather than silently flattening.
Heat guard: scene cache reuses composition for 25 pans in a headless test; cached geometry capped at 262,144 points; first rebuild/real thermals unmeasured.
Format: named pure v9→v10 migration, default-key omission, plain v9 JSON stamp-only identity, 4 frozen JSON/PDF/SVG states + 4 refusals and hashes.
Integration: v10 is pre-assigned; moderator rechains v10→v11→v12→v13→v14; shared-file hooks marked Lane A; no effects/contents slot added.
Bridge: appearance/mask edit verbs + opt-in appearance describe; progressive discovery only; tools/list 1.2 = 23,912 / 24,000 B (88 B headroom).
CLI: `appearance INPUT.vrs EDIT.json OUTPUT.vrs` / `mask ...`; bounded typed edits, resource preservation, create-new output.
Focused behaviours: 18 passed / 0 failed (core 9, Bridge 3, CLI 1, PDF 3, raster 2); strengthened GPU planner/shader/budget tests 3 passed; kit/CPU guards pass in workspace.
Gates: cargo fmt --all --check; python3 ../tools/check_dep_directions.py; native + Windows all-target clippy -D warnings PASS; varos-text WASM check PASS; Cargo compile/test gates --offline -j 3.
Workspace: cargo test --offline --workspace -j 3 --no-fail-fast PASS; 2,266 top-level passed / 0 failed / 15 ignored, plus 3 passing subprocess tests (2,269 total executions).
Ratchets: unchanged; ui.rs 812 / 843 lines; 3 UI source ratchets and 6 Bridge ratchet tests PASS, including frozen 1.0/1.1 fixtures and 1.2 cap.
Evidence: /tmp/varos-lane-a-appearance/{tests9,focused-final,gpu-final,clippy-native-final,clippy-windows-final,fmt-final,deps-final,wasm-final}.log.
No new dependencies; no missing offline crate; prior-art appearance attribution retained and NOTICE updated.
Unverified: native GPU pixels/device validation, PDF viewer pixels, real heat, owner interaction/design acceptance; headless shader/draw-list and CPU/SVG pixels verified.
No commit, push, GUI run or install performed; PLAN Progress records the provisional UI decision.
