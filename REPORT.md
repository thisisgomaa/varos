# Lane B — Phase 10 live vector effects (2026-10-10)
State: implemented (provisional UI, owner design review pending); uncommitted worktree only.
Typed Offset / Zig Zag / Transform / 15 Warp styles on Path.effects; seven width presets + custom points.
Shared bounded evaluator/cache feeds canvas, CPU raster, PDF/SVG and Expand; copies paint independently.
Width tool ⇧W, on-canvas width points; symmetric drag edits one undo step, asymmetric profiles via API/CLI.
Effect menu + sheets; Apply Last ⇧⌘E / Last ⌥⇧⌘E; preview Cancel restores and OK commits once.
Bridge 1.2 progressive verbs: live_effects, width_profile, expand_live, width_tool; typed CLI batch supported.
v11 writer + pure migrate_v10_to_v11; temporary v9→v10 identity must be replaced by Lane A at integration.
ADR-0016 proposed; PLAN and format reference updated; source attributions and NOTICE rows added.
Appearance-stack integration deferred to integrator; effects currently live on Path.
Transform copies on clipping-mask sources refuse until integration defines mask ownership.
No GUI, installation, commit or push; visual acceptance remains unverified by instruction.
No missing crates; test-only lopdf reuses the already locked dependency for PDF golden normalization.
Tokens/kit/shell ratchets/historical fixtures untouched; ui.rs 821 / 843 lines; new fixture SHA256 6 / 6.
Validation logs + results.json: /tmp/varos-lane-b-effects-20261009/; final gates below.
fmt: PASS; dependency directions: PASS; git diff --check: PASS.
cargo test --offline --workspace -j 3 --no-fail-fast: PASS 2,274 passed / 0 failed / 15 ignored (152 suites).
Native clippy --offline --workspace --all-targets -j 3 -- -D warnings: PASS.
Windows x86_64-pc-windows-msvc clippy (same flags): PASS.
Bridge ratchets: PASS 6/6; 1.0/1.1 fixtures byte-identical (23,993 B each).
API 1.2 tools/list: PASS 23,937 / 24,000 B (63 B headroom); progressive schemas only.
App ratchets: PASS 3/3; targeted regression: 133 passed / 0 failed / 1 ignored across 13 suites.
Regression coverage (29/29 dedicated tests PASS): evaluator goldens, invalid parameters, cache invalidation, idle scene signature,
Expand/undo, open/overlapping copies, hit testing, mixed-stack previews, PDF/SVG/CPU parity,
width-profile geometry/dash fractions/drag, v11 migration/refusals and API discovery/atomic batches.
