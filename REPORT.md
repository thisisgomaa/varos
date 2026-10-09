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
