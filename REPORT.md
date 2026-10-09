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
p0-crash — feat/p0-crash, pre-review HEAD 96e9db1; fix changes are uncommitted.
REPORT.md was absent at handoff; reviewer final.txt, lane brief, CLAUDE.md and PLAN principles were read.
Slices 0.2/0.9 remain implemented, pending independent review and owner acceptance.
Fix round
P1 fixed — core editor.rs stores undo/redo/pending as immutable Arc<Document>; rollback copies live state and bounded history handles.
P1 test — 200-history checkpoint proves pointer sharing, pending/redo preservation and forced-panic restoration; existing crash tests retained.
P1 fixed — Bridge service.rs checkpoints mutations only; ten board-describe reads prove no additional checkpoint access.
P2 fixed — core clipping.rs caches menu enablement per editor revision/selection, bypasses active transactions, invalidates on command/begin.
P2 fixed — indexed node/leaf/z lookup and deduplicated selection units eliminate per-descendant subtree collection.
P2 tests — 512 selected paths, 100 mirror reads/one computation; partial selection, transaction invalidation/cancel, release/undo/replacement covered (7 new tests total).
Coverage note fixed — command Make/Undo/Redo matches content streams and page bounds of frozen native_rich.pdf; fixture unchanged.
Merge risks — burger key varos.object.clipping.enablement is namespaced; existing obj.clip/obj.release_clip IDs and Illustrator keys retained.
Shared changes confined to editor history/cache, command invalidation, Bridge checkpoint predicate and native/burger consumers; no schema/settings changes.
Integrator: retain fallible execute/execute_ui distinction, combine sibling API 1.2 verbs/schema/capability gates, preserve GPU submission guards.
Disagreements: none. Sibling branches were not modified; GPU loss/native interaction remains unverified under the no-GUI rule.
Gates — fmt PASS; dependency directions PASS; git diff --check PASS.
Workspace — cargo test --offline --workspace -j 2 --no-fail-fast: 1537 passed, 0 failed, 15 ignored (90 test/doc-test binaries).
Clippy — native and x86_64-pc-windows-msvc, workspace/all-targets, offline -j 2 -D warnings: both exit 0.
Ratchets — core 37/37 + Bridge 106/106 (4 tests passed); shell 3 tests passed; ui.rs 843/843; caps/source unchanged.
Bridge contracts — 72 passed, 0 failed, 4 ignored; API fixtures byte-unchanged against base f2066f1; PDF exports 36 passed.
Storage coverage — 114 storage tests passed, including recents/settings corruption quarantine and preserved earlier bad files.
Logs — /tmp/varos-p0-crash-fix-20261009-unique/{workspace-final,panic-ratchets,ui-ratchets,clippy-native,clippy-windows,deps,fmt}.log
No commit/push/merge/GUI/install; HEAD remains 96e9db1. Requested fixes complete locally; independent re-review/native acceptance remain.

Fix round — p0-commands (2026-10-09)
Review read first; REPORT.md was absent at pre-review HEAD, so this report is newly created. No findings disputed.
P1 compound: model.rs bounds include every contour; fill hits use XOR even-odd parity for disjoint/nested rings.
Test: compound_islands_nested_parity_and_bounds_agree checks interior selection, gaps, nested parity, bbox and Fit Artboard.
P2 Alt handle: tools/direct.rs checks handles before Option group clicks; removed handle-branch unwraps.
Test: option_handle_drag_breaks_only_the_grabbed_handle checks opposite-handle preservation and no duplicate.
P2 Join: command_wave.rs honors exactly two direct-selected endpoints; invalid/interior selections are no-ops; whole paths retain nearest matching.
Test: join_honors_far_explicit_endpoints_and_rejects_interior_points; existing whole-path/single-path Join tests pass.
P2 history: commit_wave compares content before publishing; distribution/spacing/key alignment suppress float-ulp translation noise.
Test: repeated_alignment_distribution_spacing_and_average_preserve_redo checks unchanged content/revision and retained redo.
P2 guide snapping: editor.rs explicitly adds visible converted guide world contours independently of paint targets.
Test: converted_guides_snap_moves_and_resize_with_other_targets_disabled checks move, resize and hidden-guide exclusion.
P2 idle work: recovery_host.rs dispatches paste preference only when a tab's value differs; newly created tabs inherit it.
Test: unchanged_paste_preference_does_not_execute_or_prune_selection; persisted preference/new-session tests pass.
Merge risk: settings writer merges owned fields into existing JSON, preserving sibling keys; refuses incompatible version writes.
Test: settings_writer_preserves_sibling_keys_and_refuses_changed_version; no new commands, shortcuts or settings keys in this round.
Shared edits are local blocks; commit_wave is lane-named. No global history rewrite; Bridge/menu/lifecycle files untouched.
Integrator: reconcile 4A key-object ownership, contour consumers and API 1.2 gating across sibling lanes; schema is 23,993/24,000 bytes and needs shared deduplication for additions, never a raised cap.
Gates: fmt --all --check, dependency directions and git diff --check PASS; native + x86_64-pc-windows-msvc all-target clippy exit 0, zero warnings.
Workspace offline -j 2 --no-fail-fast: exit 0; 86 unfiltered summaries, 1,565 passed / 0 failed / 15 ignored; 3 child passes (1,568 total).
Regression additions: 7 named tests (5 core, 2 app); command-wave 31/31, view quick wins 6/6, Bridge contracts 78 passed / 4 ignored.
Ratchet filter: 3 passed; ratchet source unchanged; ui.rs 826/843. All 14 Bridge fixtures byte-identical; schema-size test 1 passed.
Logs: /tmp/p0-{workspace-final,clippy-native,clippy-windows,fmt,deps,ratchet,schema}.log. No commit/push/merge/GUI/install; independent re-review and owner hand test remain pending.
# p0-export — 0.4/0.5 fix round

Baseline: pre-review commit `1d27270`; original REPORT.md was absent; prior handoff read.

## Fix round

- P1 durability: retain directory-sync warnings in ExportWrite/ExportResult, Done notes and multi-artboard Bridge receipts (`durable=false`). Tests: injected SyncDir failure, Done aggregation, first/last-page receipt warnings.
- P2 identity: selection cards cache `selection:<object/group ID>`; names are display-only. Test: duplicate names, uncheck one, rename/reorder, reopen preserves the export set.
- P2 legacy DTO: version-aware decoding rejects all five raster fields for default/1.0/1.1, including null, while preserving duplicate/unknown-field rejection. Tests cover save/save_as/export_pdf and explicit 1.2 raster acceptance.
- P2 thumbnails: replace per-card polling threads with a sheet-owned bounded decoder (16 pending), ThumbService completions, terminal failures and cancellation on drop. Tests: unwritable cache completes, bounded admission, drop signals shutdown.
- P2 repaint heat: cache card IDs; persist only after control/checklist changes; skip offscreen preview work. Tests: unchanged-model and actual headless sheet repaints leave preferences untouched.
- Merge risk: partial batch cancellation retains written count and prior durability warnings; regression test passes. Shared changes stay in export-specific blocks; no new shortcut/command/settings key in this round.
- Coverage gap: independent known-rectangle SVG/PNG/WebP/TIFF tests check fill, transparency, scaling and selection crop; crop bounds use floating-point tolerance.
- API 1.2 admission/discovery remains additive; default discovery and legacy fixtures retained. Sibling-lane registry reconciliation remains integrator work.
- Disagreements: none; all ranked findings addressed; fixes pending independent re-review.
- Gates: fmt PASS; dependency directions PASS; offline workspace PASS (1534 passed, 0 failed, 15 ignored).
- Clippy: native and x86_64-pc-windows-msvc all-targets PASS, -D warnings; offline -j 2.
- Explicit gates: ratchets 3 passed; Bridge contracts/fixtures 70 passed, 4 ignored; targeted rendering 1 passed, legacy decoder 2 passed.
- Ratchet ceilings and Bridge fixture files unchanged against f2066f1; git diff --check passes.
- No commit, push, GUI or installation; native visual interactions and Windows runtime remain unverified.
# p1-docsetup — Fix round (2026-10-09)
Base: feat/p1-docsetup HEAD 06b3cfd; fix changes remain uncommitted; no push/GUI/install.
P1 units-only Bridge publication: compare display units alongside content; one undo, no-op suppressed; phase1::units_only_publishes_once_and_noop_has_no_history.
P1 setup scrubs: reuse one open transaction, settle on release/lifecycle, discard net-zero; core scrub_is_one_undo_and_net_zero_preserves_history + app setup_live_ops_share_history_and_settle_before_lifecycle (PPI/bleed).
P2 Document Info: cache by editor revision; hash colour keys, preserve first-seen order and signed-zero equality; info_refreshes_only_on_revision_changes + colours_deduplicate_in_first_seen_order_and_counts_are_independent.
P2 Bridge templates: dedicated template_jobs module on existing file worker; owned revision snapshot/read, accepted ticket + receipt, cancellable exclusive publication; worker_pins_snapshot_cancels_and_opens_dirty_untitled + template_host_only_queues_revision_snapshot_and_cancel_flag.
Template completion: opening settles edits/switches UI; save/error completions stay quiet; worker regression asserts both completion routes.
Queued cancellation: IPC retains flags owned by workers after accepted reply; accepted_worker_flag_remains_addressable_until_work_finishes.
P2 translucent pages: grid under zero/partial-alpha page fills, then explicit colour composites; checkerboard_is_bounded_canvas_furniture checks opaque/zero/partial alpha, order, bounds.
P2 token law: DOC_CHECKERBOARD only in shell/tokens.rs; pure SceneStyle input, additive styled canvas entry point; same scene test verifies supplied palette; exporters retain unstyled APIs.
Cheap coverage: schemas_are_opt_in now checks setup references/required fields, template 1.2 requirements and absence from legacy schema; independent literal Info counts added.
Review metadata correction: report was absent and is now supplied; current committed target is 06b3cfd, not the review snapshot f2066f1. No defect findings disputed.
Merge risk: new numeric ids use p1-docsetup + SessionId; no new shortcuts/settings keys; shared edits are small additive blocks, worker logic lives in a new module.
Integrator notes in PHASE1_DOCUMENT_BASICS.md: PDF must consume document_setup::bleed edges; sibling setup/grid/window/data-root/API 1.2 additions must reuse these seams. Sibling diffs unavailable locally; reconciliation remains at integration.
Gates: cargo fmt --all --check PASS (exit 0); python3 ../tools/check_dep_directions.py PASS (exit 0); git diff --check PASS.
Workspace: cargo test --offline --workspace -j 2 --no-fail-fast PASS: 1531 passed, 0 failed, 15 ignored (86 top-level suites; existing ignores retained).
Clippy: --offline --workspace --all-targets -j 2 -- -D warnings PASS native and x86_64-pc-windows-msvc (both exit 0, 0 warnings).
Ratchets: 3 passed, 0 failed; caps unchanged; ui.rs 836/843 lines. Bridge contracts/frozen fixtures: 70 passed, 0 failed, 4 ignored (native socket sandbox restrictions); fixture files unchanged.
Evidence: /tmp/p1-final-{workspace,ratchets,bridge-fixtures}.log; /tmp/p1-clippy-{native,windows}.log; /tmp/p1-final-status.json.
Owner visual review/installed-app acceptance remain pending by instruction; no GPU/EventLoop test added, no non-test unwrap added, ratchet limits unchanged.
# p1-pdfprint — Phase 1.2 / 1.3 / 1.9

Existing lane: PDF options + kit hook, CLI/API 1.2, Preview print fallback, detached Varos/PDF/SVG/2× PNG clipboard.

## Fix round — 2026-10-09

- P2 selection: core `capture_selection_clipboard(cut)` shares Copy/Cut source rules; direct path selection works; anchors cannot publish stale data. Test: direct Copy/Cut + stale-anchor rejection.
- P2 Cut availability: publish lossless Varos data even without painted bounds; omit only PNG for oversized selections; report actual types/reasons through desktop notices and Bridge. Tests: unpainted/opacity-zero Cut + oversized Cut + undo.
- P2 locked groups: capture lock-filtered Cut sources before publication, then execute core Cut unchanged. Test: all public bytes match filtered payload; OS/internal payloads agree; locked art remains; one Undo restores.
- P2 coincident boards: resolve source bleed in PDF `PageSpec` at planning, never by rectangle lookup. Test: coincident boards with 3/9 pt bleed in all/active exports; parsed boxes checked.
- Merge diagnostics: options writer retains `export_pdf_bytes_with_report` notes and appends ppi note. Test: default/customized writer report preservation.
- Merge seams: sheet/job fields are additive `pdf_options`; dedicated `ui/export/pdf_options.rs` hook; `pdf-*` UI IDs, permitted Cmd+P, no persisted settings keys. Core editor addition is one five-line helper.
- API 1.2 stays opt-in/additive in existing dispatch/discovery; legacy fixtures retained. Integrator must reconcile sibling sheet/job hooks and union of 1.2 tools; no sibling branch merged.
- No finding disputed or skipped; 4 P2 findings fixed, plus local merge risks.
- Focused gates: clipboard 6 passed; PDF options 5 passed (6 new regressions across both suites).
- Full workspace: `cargo test --offline --workspace -j 2 --no-fail-fast` PASS: 1,527 passed / 0 failed / 15 ignored, 88 suite summaries; long Arabic look-ahead regression passed.
- Final desktop after additive field rename/Copy command wiring: 749 passed / 0 failed / 1 ignored; clipboard 6/6 and all 3 ratchets pass.
- fmt PASS; dependency directions PASS, 0 violations; native + Windows x86_64-pc-windows-msvc workspace/all-targets clippy `-D warnings` PASS, 0 warnings each.
- Bridge contracts 71 passed / 0 failed / 4 ignored; 14 tracked legacy fixtures byte-identical. Ratchet source/tokens/historic PDF writer unchanged; `ui.rs` 843/843 lines.
- Evidence logs: `/private/tmp/p1-pdfprint-fix-{tests,app-final,clippy-native,clippy-windows}.log`; whitespace check PASS.
- Snapshot: actual HEAD remains base `f2066f1`; incoming implementation was uncommitted and `REPORT.md` absent. Changes preserved; this report created. No commit/push/merge/GUI/install performed.
- Limits: provisional UI awaits owner review; native print/paste unverified; Preview fallback requires File > Print, not a direct AppKit print panel.
# Lane p1-autosave

## Fix round
- Reviewed Sol's `final.txt` and the cycle2 lane contract; agree with all findings.
- Checkout discrepancy: no pre-review commit or REPORT.md was present; HEAD is `7b48f2c`, with the implementation uncommitted/untracked.
- P1 save race: manual/inline/Bridge saves carry the pre-publication temporary-file fingerprint through completion; RecentStore forwards it.
- P1 unsafe reads: regular-file descriptors, Unix nonblocking open, 64 KiB streaming hash, native-reader 256 MiB ceiling; pinned reads share the bound.
- P2 retry loop: lock contention is distinct from invalidation; both defer an expired capture by 30 seconds without warning on contention.
- P3 Recent test: lifecycle completion uses a configured real RecentStore and compares its actual entries and persisted file bytes.
- Fix files: `storage/durable.rs`, `file_ports.rs`, `file_jobs.rs`, `lifecycle.rs`, `recent_files.rs`, `bridge_fs.rs`, `autosave_io.rs`, `autosave_host.rs`.
- Regression tests: external replacement after manual/Bridge publication, fresh Bridge baseline, FIFO/directory/oversize refusal, fake-clock contention/invalidation backoff, real Recent invariance.
- PASS: `cargo fmt --all --check`; `python3 ../tools/check_dep_directions.py`; `git diff --check`.
- PASS: `cargo test --offline --workspace -j 2 --no-fail-fast`: 1,531 passed, 0 failed, 15 ignored (85 suites).
- PASS: native and `--target x86_64-pc-windows-msvc` `cargo clippy --offline --workspace --all-targets -j 2 -- -D warnings`.
- PASS: unchanged ratchets and frozen Bridge fixture checks within the workspace run; fixture files unchanged.
- PASS: final publication follow-up tests (2) and file-job tests (4), including fresh Bridge identity, after the last publication adjustment.
- Evidence: `/tmp/autosave-workspace.log`, `/tmp/autosave-clippy-native.log`, `/tmp/autosave-clippy-windows.log`, `/tmp/fix-round-{published,file-jobs}.log`.
- No commit, push, GUI run, or installation performed; provisional UI/owner hand testing and merged-lane integration review remain pending.
