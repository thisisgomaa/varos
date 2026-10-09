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
