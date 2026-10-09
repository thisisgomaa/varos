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
