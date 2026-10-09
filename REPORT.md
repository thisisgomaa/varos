Lane p3-trace — slice 3.8 Image Trace engine — Fix round, 2026-10-09
Delivery is modified/untracked at HEAD 7b48f2c; no commit, push, GUI launch or installation.
Core: trace/{mod,quantize,contour,fit,tests}.rs; InsertTracedPaths + small command/bridge/lib integration.
Bridge: additive trace_rgba DTO/dispatch/schema and API 1.2 capabilities; legacy verb table preserved.
CLI: isolated trace.rs + tiny main.rs routing, PNG → editable VRS; image dependency and NOTICE attribution.
Limits: 16 Mi pixels, 100000 output anchors; alpha <128 omitted; grayscale up to 8 levels; engine only.

## Fix round
P2 inherited protection: check complete destination ancestor chain before history; locked/hidden parent tests verify document, revision, allocator, selection and history unchanged.
P2 API 1.2 economy: shared edit_enabled gate for normalization, service preflight and naming; capabilities advertise repeat, schemas/descriptions agree; mixed trace/defaults/tuples/repeat/IDs receipt test covers locals, retry, undo/redo and rollback.
P2 preflight regression: 99 layer-target operations exceed 1000 expanded paths; API 1.2 refuses before allocation/history.
P2 stroke validation: require Paint::None even at zero width; invalid and valid Solid strokes both rejected without mutation.
Cheap hardening: cyclic/missing ancestors refused; visible unlocked sublayer succeeds; dedicated test. All three reviewer findings fixed; no disagreements.
Trace-specific coverage: 27 tests (22 core, 3 Bridge, 2 CLI); six new regression tests pass in the full workspace run.
Merge locality: no editor.rs/model.rs/app/settings/shortcuts edits; shared integrations stay in small trace-specific blocks; API 1.0/1.1 capability branches preserved.
Merge follow-up: geom/fit absent locally, so unification TODO retained; integrator must combine sibling API 1.2 verbs/capabilities and recheck future Path/stroke fields.
Crash-safety seam: existing prepare/publish design batch retained; mixed-batch rollback test passes; sibling panic/transaction implementation absent locally.

Gate cargo fmt --all --check: PASS, exit 0.
Gate python3 ../tools/check_dep_directions.py: PASS, exit 0, 0 violations.
Gate cargo test --offline --workspace -j 2 --no-fail-fast: PASS, exit 0; 1539 passed, 0 failed, 15 ignored, 87 result blocks.
Gates cargo clippy --offline --workspace --all-targets -j 2 [native / --target x86_64-pc-windows-msvc] -- -D warnings: both PASS, exit 0, 0 warnings.
Ratchets: 3 tests pass; 1 file byte-identical to HEAD. Bridge fixtures: all 14 API 1.0/1.1 files byte-identical; contracts 73 passed, 0 failed, 4 ignored.
Gate git diff --check: PASS; logs /private/tmp/p3-trace-fix-*.log. UI/presets/image-object integration and renewed independent/owner review remain outside this fix round.
