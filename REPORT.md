# Lane D — w2-tools-ui — resume complete (2026-10-09)
Branch feat/w2-tools-ui; retained WIP e2ba362; resume fixes are uncommitted.
Status: implemented (provisional UI, owner design review pending).
4B: Rounded Rectangle, Polygon, Star, Line, Arc, Spiral, Rectangular/Polar Grid; numeric kit sheets, Shift/Option, live radius/count arrows.
Rail: new ui/rail_flyout.rs; right-click/450 ms hold, registry icons + Illustrator keys, last-used tool per document/group.
4C: Pencil N + fidelity/smoothness options, open endpoint continuation; Smooth, Path Eraser, Join, Curvature Shift+~; one undo per stroke.
Retained additive Drawing EditCommand, API 1.2 list_verbs/schema, and headless CLI apply wiring; legacy APIs remain frozen.
Resume fixes: radial Shift preserves radius; stationary strokes are inert and settle transactions; Pencil respects isolation and offsets the join handle.
Resume fixes: protected destinations/creation limits/ID exhaustion and mask replacement refuse before history; numeric sheets retain rejected values and own keyboard input.
Resume fixes: sheets use document-scoped IDs; Line sheet uses length/angle; flyout release resets hold state, suppresses release activation, and permits a second hold.
Focused coverage: 21 core drawing + 5 kit UI/flyout + 3 Bridge drawing + 1 CLI drawing tests pass; lifecycle/shortcut tests pass in workspace.
Gate cargo fmt --all --check: PASS, exit 0.
Gate python3 ../tools/check_dep_directions.py: PASS, exit 0, zero violations.
Gate cargo test --offline --workspace -j 3 --no-fail-fast: PASS, exit 0; 1909 passed / 0 failed / 15 ignored; 3 nested child summaries excluded.
Gate cargo clippy --offline --workspace --all-targets -j 3 -- -D warnings: PASS, exit 0, zero warnings.
Gate same Clippy command with --target x86_64-pc-windows-msvc: PASS, exit 0, zero warnings.
Ratchets: 3/3 PASS; ratchet source byte-identical to main; ui.rs 784/843 lines; no thresholds raised.
Bridge contracts: 88 passed / 0 failed / 4 ignored; all 16 fixture files byte-identical to main (including 1.0/1.1 tools/list).
Format: no new persisted keys or writer/version changes; shapes remain ordinary baked paths. No new dependencies or missing-registry workaround.
Attribution: reuses already-attributed core geometry; new SVG glyphs are Varos originals with embedded provenance; existing Lucide registry provenance retained.
Integration: union additive ToolKind/Drawing/Bridge routing with sibling lanes; preserve document-scoped UI IDs and curvature settlement before file/tab actions.
Evidence: /tmp/varos-w2-tools-ui-hfn_14j0/{workspace,clippy-native,clippy-windows,ratchets,bridge-fixtures,fmt,deps}.log; exit-codes.json, summary.json, compatibility.json.
Replaced unrelated root REPORT.md; git diff --check PASS. No commit, push, merge, GUI launch, installation, or Windows runtime test.
Pending: independent integration review and owner design/native/heat acceptance. Blob Brush remains deferred by scope.
