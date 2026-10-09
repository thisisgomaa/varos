# Lane B — Phase 5: PARTIAL hand-back
Branch: feat/w2-gradients; worktree-only, uncommitted.
5.0: additive Appearance read-view, BaseSlot/StackItem/EntryOpts/Look, authoritative borrowed paints.
Paint readers routed through the view: scene/hash, hit tests, PDF/SVG, validation, thumbnails,
selection comparisons, duplicate/construction, eyedropper, inspector and Bridge elements.
45 frozen v4/v5 model fixtures round-trip byte-identically (only existing v4→v5 stamp changes).
5.1 groundwork ONLY: pure linear/radial/focal/ellipse placement, midpoint/opacity, Pad/Reflect/Repeat,
reverse, 1024-entry LUT, strict validation and standalone-definition round-trips.
No Gradient/SwatchRef Paint variants, swatch table, GPU/CPU paint pipeline, PDF/SVG gradients,
next-format migration/refusal fixtures, gradient Bridge verbs or CLI integration shipped.
5.2–5.5 NOT implemented: picker/tool, Swatches, Colour Guide/Recolor, stroke modes.
Gradient tab stays disabled; no provisional UI was delivered or visually verified.
Format keys added: NONE. Writer remains v5; moderator has no bump to renumber in this hand-back.
Attribution: VectorCraft gradient maths and appearance shape headers + NOTICE rows; licences present.
No new dependencies; no missing local-registry crate encountered.
Gates (all cargo commands from varos/, offline -j 3; fmt uses its standard flags):
fmt --all --check: PASS; dependency directions: PASS; git diff --check: PASS.
workspace --no-fail-fast: exit 0; 1888 primary passed / 0 failed / 15 ignored (108 summaries).
Additionally 3 filtered subprocess pass reports; total pass reports 1891.
New tests: 10 passed (2 Appearance, 8 gradient maths); final targeted rerun passed.
Native + x86_64-pc-windows-msvc workspace/all-targets clippy -D warnings: PASS / 0 warnings.
Ratchets: 3 shell + 6 Bridge passed; source/caps unchanged; ui.rs unchanged, 811/843 lines.
Bridge: 1.0/1.1 tools/list byte fixtures PASS; all 15 fixture files unchanged.
Evidence: varos/target/lane-b-gates/{workspace,clippy-native,clippy-windows,slices}.log.
No commit/push/merge, GUI launch or install; independent review/owner acceptance pending.
NOT a completed Phase 5 or a gradient feature ready for merge/owner testing.
