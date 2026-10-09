# p2-geom — core geometry lane

- Scope: kurbo 0.13.1 adapter, shape constructors, fitter and pure path edits; attribution in source + NOTICE.
- Files: core `src/geom/{kurbo,shapes,fit,edit}.rs`, `src/geom.rs`, `tests/geometry.rs`, Cargo.toml/lock, NOTICE, PLAN.
- Snapshot discrepancy confirmed: HEAD=f2066f1; pre-review geometry is uncommitted; REPORT.md was absent. No commit created, per instruction.

## Fix round

- P2 fit.rs: certify sample distance against converted f32 curves; exact sample polyline fallback on failed certification.
- Test: reviewer’s (100000000,0)/(100000008,8)/(100000024,0) example and transposed case both satisfy 0.01.
- P2 kurbo.rs: retain the current point after ClosePath; subsequent segments create a separate contour instead of disappearing.
- Test: post-close LineTo/QuadTo/CurveTo, repeated closes, following LineTo and explicit MoveTo; compound open extras reject.
- P2 edit.rs: cache nearest pair candidates in a heap with stable slots/generation invalidation; recompute only affected pairs.
- Test: 1,000 paths retain deterministic ordering/2,000 anchors; exactly 3,992,004 distances vs 666,666,000 before.
- Complexity: O(n²) distance evaluations/storage, O(n² log n) heap work; existing reversal/handle/endpoint tests pass.
- Merge risks: document common coordinate space, zero-ID allocation and one-undo caller contract on Join; command reconciliation remains deferred by core-only brief.
- No shared editor/app/Bridge/storage changes or new shortcuts/settings/commands; stroke consumers can use the public kurbo adapter.
- Targeted: geometry 14 passed/0 failed; join complexity regression 1 passed/0 failed.
- Gates: fmt PASS; dependency directions PASS; workspace 1,527 passed / 0 failed / 15 ignored.
- Native + Windows-target Clippy (`--workspace --all-targets -D warnings`) PASS, 0 warnings; final test-only lint correction rerun 1/1 PASS.
- Logs: `/tmp/p2-geom-fix-{tests,clippy-native,clippy-windows,join-final}.log`.
- Ratchets: 3/3 PASS, source unchanged; Bridge contracts 70 passed / 0 failed / 4 ignored; all 14 fixtures byte-identical to HEAD.
- No findings disputed; no commit/push/GUI/install; independent re-review and owner acceptance remain pending.

# Lane p2-stroke — 2.1 StrokeStyle / format v5
Checkout HEAD/base 7b48f2c; original implementation and fix round remain uncommitted here.
Scope: authored style + core coverage, GPU/CPU/PDF/SVG, v5 migration/corpus, opt-in Bridge 1.2, provisional Stroke UI.
## Fix round
P1 PDF: original rings now emitted only for native stroke; baked knockout paints coverage alone. Stream regression + independent PDFium test (3 PDFs / 12 pixels) pass.
P1 raster: Scene.errors yield diagnostics and no pixels; encoding/sampling refuse failed rasters; checked page API and Bridge preserve errors. Rotated locally-valid dashed-path raster + board/page Bridge refusal tests pass.
P2 scrub: numeric gesture accumulator publishes one checked field batch on release; typed arrow steps retain kit behaviour. Real multi-frame kit drag, single undo, and arrow-step tests pass.
P2 no-op: identical SetStrokeStyle exits before begin; changed-target batches omit unchanged styles. Revision/history/redo preservation regression passes.
P2 dash: explicit Dash(index) operation and selective same-length differences preserve each target's other entries. Mixed [6,3]/[10,8] scrub and core difference tests pass.
P2 idle: stroke inspection cached by revision + object/anchor/direct selection, bypassed during transactions/dirty edits; has_length uses active controls without arc integration. Cache invalidation + handle-degeneracy tests pass.
All six findings accepted and fixed; no findings skipped or disputed.
Corpus: refreshed only 33 incorrect baked v5 PDF files and hashes; 43 JSON/PDF/SVG triples and 131 hash entries; historical fixtures untouched.
Merge risk: new numeric field IDs use p2-stroke-number namespace; no new shortcuts/settings/window keys; Bridge remains additive and gated to 1.2.
Shared-file fixes use local blocks; raster refusal reaches thumbnail worker, picker, and Bridge; Reverse Path head/scale swap regression added.
Integrator still owns sibling kurbo adapter unification, crash-rollback reconciliation, worker/Prim consumers, and first-merged-writer version ownership.
Gates: fmt / dependency directions / whitespace PASS.
Gates: cargo test --offline --workspace -j 2 --no-fail-fast: 1556 passed / 0 failed / 15 ignored (89 suites), exit 0.
Gates: native + x86_64-pc-windows-msvc clippy --all-targets -D warnings PASS (exit 0 each).
Gates: ratchets 3/3; Bridge contracts 73 passed / 4 ignored; all 14 Bridge + 39 v1-v4 fixture files byte-identical to HEAD; ratchet source/ui.rs unchanged.
Logs: /tmp/p2-stroke-fix-{workspace-final2,clippy-native-final,clippy-windows-final,ratchets,bridge-contracts}.log.
Rendered regression: python3 tools/check_stroke_pdf_render.py (local pypdfium2 + Pillow); no writer-dependent pixel goldens.
No commit/push/merge, GUI launch, installation, Windows runtime test, native acceptance, or .bad quarantine test performed.

Lane K — 4E planar construction + 4D cutting; feat/p4-planar, HEAD 7b48f2c.
Status: implemented (provisional UI, owner design/native acceptance pending); changes remain uncommitted.
Scope: planar/Pathfinder/Shape Builder/cutting core, kit panel/icons, additive Bridge 1.2 verbs, CLI, tests and attribution.
## Fix round — 2026-10-09
P1 mask replacement: reject the entire replacement before begin/bake/allocation if any removed path is a mask source.
Test: construction_replacements_reject_mask_sources_atomically covers Pathfinder, Builder, Knife, Eraser, Divide Below; tree/content/revision unchanged.
P1 compound Scissors: explicitly refuse compound or mask-source cuts; checked compound calls return an error, interactive calls retain geometry.
Test: scissors_refuses_compound_cut_without_changing_hole_coverage_or_undo proves centre stays transparent and content/revision unchanged; cubic/open cuts still pass.
P1 untouched Builder sources: replace only hit owner components; bake only removed sources, retaining unrelated IDs/handles/selection.
Test: builder_retains_distant_cubic_path_and_selection_verbatim checks the distant selected ellipse exactly.
P2 no-op Builder: compare resulting source coverage before mutation; unchanged merge retains geometry/IDs and creates no undo entry.
Test: builder_isolated_merge_is_noop_including_ids_revision_and_undo checks IDs, revision, content and undo goes directly to AddShape.
P2 hover/drag cost: transient revision/selection arrangement cache; incremental new-segment hit checks, reset per gesture.
Test: arrangement_reuses_hover_and_drag_and_invalidates_revision_selection_undo checks one hover build, linear drag work, selection/edit/undo/redo/replace-doc invalidation.
Disagreements: none; all five reviewer findings addressed. Existing compound-anchor test replaced with a hole-coverage regression.
Merge: additive command/Bridge names and Construction drag; no settings/schema/storage edits; shared-file additions remain small, self-contained blocks.
Merge test: construction_shortcuts preserves Ctrl+C, plain M and sibling Shift+C/plain E slots; Alt variants do not select lane tools.
Integration notes: union sibling API 1.2 capabilities within existing size gate; central crash guard is absent here and must cover execute/creation/batches; retain source.clone() for stroke fields and reconcile flattening policy.
Gates: fmt exit 0; dependency-direction check exit 0 PASS; git diff --check exit 0.
Workspace gate: cargo test --offline --workspace -j 2 --no-fail-fast exit 0; 1,534 passed / 0 failed / 15 existing ignored; 85 suite/doc summaries (3 child-fixture summaries excluded).
Clippy final: native exit 0, Windows x86_64-pc-windows-msvc exit 0; both --workspace --all-targets --offline -j 2 -D warnings, 0 warnings.
Ratchets: 3/3 pass; ratchet source/tokens/ui.rs byte-identical to HEAD; ui.rs remains 843/843 lines.
Bridge: 14/14 fixtures byte-identical to HEAD; 15 artboards/construction + 70 contract tests pass, including legacy/default and 1.2 opt-in size gate.
Not done: owner native/design/heat acceptance and sibling merge/crash-guard integration; no GUI/install/commit/push as instructed.
Evidence: /tmp/varos-planar-fix-20261009/{workspace-final,clippy-native-final,clippy-windows-final,fmt,deps}.log and compat.json.
# Lane p4-tools — 4A select/transform + Layers
Branch feat/p4-tools; HEAD 7b48f2c. Lane implementation and fixes are uncommitted; no pre-review commit exists locally.
Scope: core select_transform/tools modules; provisional kit sheets/isolation; additive command/menu/Bridge/CLI wiring; docs/TOOLS_4A.md + NOTICE.
## Fix round (2026-10-09)
- P1 Release Build: preflight peak paths, anchors/holes, copied group/leaf/layer nodes and stable-ID allocations before mutation; reject duplicate/overlapping targets.
  Tests: release_build_1000_paths_refused_before_mutation; release_build_preflights_anchors_holes_groups_nodes_and_ids (tiny budgets + duplicate targets).
- P1 Flatten: carry removed ancestor hidden/locked flags onto surviving children; same helper used by Merge, leaving unrelated nodes untouched.
  Test: flatten_preserves_nested_hidden_locked_state_and_undo (nested layers, group, visible sibling, undo).
- P2 no-op history: skip unchanged sampling and repeated Hide/Lock Others before opening transactions.
  Test: noop_sampling_and_hide_lock_preserve_document_revision_and_redo (document, revision, undo + redo snapshots).
- P2 Transform Each Reflect: toggle changes refresh the live preview before Apply.
  Test: transform_each_reflect_toggle_enable_disable_then_apply (headless real kit pointer clicks, enable/disable, Apply + undo).
- P2 options: dispatch Wand/Eyedropper commands only when values differ; idle frames retain selection without executing prune.
  Test: options_dispatch_only_on_change_and_preserve_selection_during_idle_frames (real kit clicks + idle-frame sentinel).
- Merge risk: SLICE4A_TOOLS_* tokens, Slice4a routing and 4a UI IDs are additive/namespaced; fixes stay in lane modules + one small bridge.rs validation block.
  Test: slice4a_shortcuts_preserve_shift_o_and_leave_shift_e_for_eraser_lane; no settings/schema-format changes or ratchet increases.
- Integrator: reconcile sibling isolation/selection, autosave/crash settlement, new image/geometry units and lane 0.8 Layers; coordinated MCP compaction remains necessary.
Gates (offline, -j 2, from varos/): fmt PASS (0); dependency directions PASS (0, zero violations); workspace PASS (0): 1553 passed / 0 failed / 15 ignored, 89 summaries.
Clippy --workspace --all-targets -- -D warnings: native PASS (0); x86_64-pc-windows-msvc PASS (0); zero warnings on both.
Ratchets PASS 3/3 (0); Bridge contracts 70 passed / 4 ignored, select_transform 6/6; MCP tools/list 23,989/24,000 bytes (0); git diff --check PASS (0).
Frozen contracts: 14/14 Bridge fixtures byte-identical to HEAD; ratchet file byte-identical; ui.rs 843/843 lines; new module production unwrap calls 0.
Evidence: /tmp/varos-p4-tools-fix-20261009-1410/ (final gate logs + status.txt).
No findings disputed. No commit/push/merge/GUI/install; independent re-review + owner design/native acceptance pending; distort/perspective and lane 0.8 Align deferred.
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

# Lane T — Text programme
Pure varos-text implementation: paragraph composition, kashida, bidi coverage, metrics, outlines; no core/app/Bridge/schema changes.
Original plan, provenance, corpus measurements and prior gates: docs/foundation/work_orders/text-programme-evidence/.
## Fix round — 2026-10-09
P2 quadratic scans FIXED: one grapheme/join/word index, cached safe slots and word counts; Off bypasses candidate discovery.
Space distribution FIXED: sorted visual prefix counts replace per-glyph space scans; reshape cap remains 64.
Tests: 20k-boundary/index and 20k-space prefix oracles, bidi/ties, long Arabic and space-heavy composition; 576 Arabic configurations retained.
P2 hyphenation hook FIXED: additive compose_with_hyphenation accepts an optional provider for Greedy and EveryLine.
Selected breaks measure/render virtual hyphens; original source/copy retained, glyph maps to preceding grapheme, diagnostic records source byte.
Tests: exact glyph/advance oracle, paragraph offsets, no-provider/wide fallback, overflow includes hyphen width, budget refusal, invalid grapheme rejection, narrow final-letter natural-break regression.
P2 non-test unwrap FIXED in composer/kashida/paths with checked selection/closure and indexed joining metadata; static scan: zero in all three.
Tests: both composer modes and empty paragraphs, candidate oracle, cubic-close parity plus new singleton/duplicate/repeated-close cases.
Merge risks: shared vendor/NOTICE/checker edits remain additive; no sibling core/app/Bridge hotspots, shortcuts, commands or settings touched.
MERGE.md and lane-files.json identify atomic convergence replacement, untracked files/fonts, vendor patch/checker coupling and per-glyph NonZero contract.
Tests: 7 new regressions; varos-text 62 passed / 0 failed; final workspace 1,533 passed / 0 failed / 15 existing ignored (406.26 s).
Gates PASS: fmt, dependency directions, native + Windows all-target clippy -D warnings, wasm32 library check, git diff --check.
Ratchets: 3 passed, no limits raised. Dedicated Bridge contracts/fixtures: 70 passed / 0 failed / 4 existing ignored.
Vendor: COSMIC full reconstruction PASS; egui_tiles SKIP (pristine archive uncached), permitted by brief.
Commands, exit codes, timings, logs and manifest: docs/foundation/work_orders/text-programme-evidence/fix-round/verification.json.
No finding disputed. Reviewer requests committed snapshot, but explicit user no-commit rule takes precedence; HEAD remains 7b48f2c.
No commit/push/GUI/install performed; independent fix re-review, owner visual acceptance and WASM runtime parity remain unverified.
