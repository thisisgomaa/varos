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
