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
