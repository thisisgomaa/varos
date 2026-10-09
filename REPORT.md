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
