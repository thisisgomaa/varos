## feat/p2-stroke
- `REPORT.md`: retained both lane reports, including every finding, gate, limitation and historical scope note.
- `docs/PLAN.md`: retained all geometry/fitter/shapes and StrokeStyle progress rows; no PLAN or GATE_LOG lines removed.
- `varos/NOTICE`: retained all VectorCraft attribution and the kurbo 0.13.1 attribution.
- `varos/crates/varos-core/Cargo.toml`: unified kurbo into one exact 0.13.1 dependency with only std enabled, retaining the geometry comment.
- `varos/crates/varos-core/src/geom.rs`: retained all geometry exports and stroke compatibility exports; one canonical painted_extent includes stroke bounds and remains the source for painted_padding.
- Adapter unification: canonical `geom/kurbo.rs` exposes its existing contour conversion; `geom/stroke_adapter.rs` re-exports contour and aliases to_bez_path as path, preserving all callers without a second implementation. These two merge-only support edits were required for unification.
- Other shared shortcuts, commands, settings, stores and Bridge verbs remain intact; no duplicate introduced. All five conflict files are marker-free; both sides' report/plan/NOTICE lines checked verbatim.
- Validation: `cd varos && cargo fmt --all && cargo build --offline -j 3 --workspace --all-targets` PASS; `git diff --check` PASS.
- Tests: all use `cargo test --offline -j 3 -p <crate>`; core `--test geometry ""` 14, `geom::` 5, `--test stroke_style ""` 20, `stroke` 18; PDF `--test stroke_v5 ""` 5; Bridge `stroke` 4; raster `stroke` 3; render-wgpu `stroke` 11; app `stroke` 10. Every invocation PASS, zero failures.
- Evidence: `/tmp/g2-stroke-build.log`, `/tmp/g2-stroke-test-results.json`, `/tmp/g2-{core-geometry,core-geom-unit,core-stroke-style,core-stroke,pdf-stroke-v5,bridge-stroke,raster-stroke,gpu-stroke,app-stroke}.log`.
- Deferred integration work: none. No Git write command used; moderator owns staging and commit.

## feat/p4-planar
- `REPORT.md`: retained every geometry, stroke and planar report line, finding, gate and limitation from both sides.
- `docs/PLAN.md`: retained all geometry, stroke, planar and cutting progress rows; no PLAN/GATE_LOG line removed.
- `varos/NOTICE`: retained all geometry/kurbo and planar/construction attribution rows.
- `varos/crates/varos-app/src/ui/panels/mod.rs`: retained both stroke and Construction panel exports.
- `varos/crates/varos-bridge/src/economy.rs`: combined API/economy/construction arguments; retained stroke gating, `op` aliases, repeat limits and all six construction verbs.
- `varos/crates/varos-bridge/src/service.rs`: retained broad stroke API 1.2 support, v5/schema fields and per-tool keys; advertised repeat, stroke and construction verbs once; removed Pathfinder from 1.2 unsupported list.
- `varos/crates/varos-core/src/bridge.rs`: retained stroke validation/budgets plus all construction/cutting preconditions and refusals.
- `varos/crates/varos-core/src/scene.rs`: retained stroke reports/errors and Shape Builder highlights/construction drag overlays.
- Merge-only repair in `varos/crates/varos-bridge/src/mcp.rs`: unified `tools_for`/`tools_for_api`, shared equivalent schema constraints, kept standalone capability schemas; 23,953 bytes <= 24,000; 172 references resolve. `command.rs` changes are cargo-fmt only.
- Canonical `painted_extent` and `geom/kurbo.rs` unchanged; no shortcut, command, setting, store, adapter, Bridge verb or test removed/duplicated.
- Validation: `cargo fmt --all` and `cargo build --offline -j 3 --workspace --all-targets` PASS; eight conflict files marker-free; both-side report/PLAN/NOTICE lines verified; `git diff --check` PASS.
- Tests: 15 targeted invocations using `cargo test --offline -j 3 -p <crate> <filter>` (including full integration-test targets): core geometry/stroke/construction/planar/cache; Bridge artboards/contracts; app construction/pathfinder/stroke; CLI planar; PDF/raster/GPU stroke — 198 passed, 0 failed, 4 existing ignored.
- Evidence: `/tmp/g2-planar-build.log`, `/tmp/g2-planar-test-results.json`, `/tmp/g2-planar-*.log`, `/tmp/g2-schema.json`.
- Deferred merge work: none. No Git write commands used; moderator owns staging and commit.

## feat/p4-tools
- `REPORT.md`: retained every geometry, stroke, planar and tools report line, finding, gate and limitation from both sides.
- `docs/PLAN.md`: retained all prior lane rows and added the 4A progress row; no PLAN/GATE_LOG line removed.
- `varos/NOTICE`: retained geometry/kurbo, planar/construction and select/transform attribution with shared license rows intact.
- `varos/crates/varos-app/src/main.rs`: combined cursors/tool names and kept both shortcut test modules; Shift+E now expects the merged Eraser while plain E remains Free Transform.
- `varos/crates/varos-app/src/shell/tokens.rs`: retained both Stroke and namespaced 4A token families without duplicate constants.
- `varos/crates/varos-bridge/src/dto.rs`: combined every construction and 4A operation plus their target-ID routing in one enum/match.
- `varos/crates/varos-bridge/src/economy.rs`: unified equivalent API checks; preserved stroke/construction gates and enabled all six 4A verbs only with explicit API 1.2.
- `varos/crates/varos-bridge/src/service.rs`: retained broad 1.2 support, v5/stroke schemas, construction and 4A capability verbs once; standalone schema expansion preserves referenced constraints and siblings.
- `varos/crates/varos-cli/tests/commands.rs`: retained both complete headless CLI tests as separate test functions.
- `varos/crates/varos-core/src/bridge.rs`: retained stroke evaluation/budgets, construction refusals and all transform/options/isolation/layer validation arms.
- `varos/crates/varos-core/src/editor.rs`: kept every ToolKind and both document-replacement resets; preserved both lanes' merged gesture routing.
- Merge-only repair in `varos/crates/varos-bridge/src/mcp.rs`: shared equivalent discriminator/type/repeat/property schemas and duplicate definitions, retaining definition aliases and every verb/constraint; compacted opt-in descriptions. API 1.2 tools/list is 23,979/24,000 bytes; all 218 references resolve. Canonical painted_extent and geom/kurbo.rs unchanged; no duplicate shortcut/command/setting/store/adapter introduced.
- Validation: fmt and offline -j 3 workspace/all-targets build PASS; 16 targeted core/Bridge/app/CLI invocations cover both lanes plus geometry/stroke regressions: 201 passed, 0 failed, 4 existing ignored; markers absent, both-side report/PLAN/NOTICE lines retained, git diff --check PASS. Evidence: /tmp/g2-tools-build.log, /tmp/g2-tools-test-results.json, /tmp/g2-tools-*.log and /tmp/g2-tools-schema.json.
- Deferred merge work: none. No Git write commands used; moderator owns staging and commit.

## feat/p3-trace
- `REPORT.md`: retained both complete lane reports, all findings, gates, limitations and historical notes.
- `docs/PLAN.md`: retained every existing progress row and added Image Trace; no PLAN/GATE_LOG line dropped.
- `varos/NOTICE`: retained all geometry/kurbo, stroke, planar/construction, transform and trace attribution rows.
- `varos/crates/varos-bridge/src/design.rs`: retained transform/local resolution and target budgets; added checked trace insertion and affected-ID reporting to the same atomic batch path.
- `varos/crates/varos-bridge/src/dto.rs`: one Operation enum retains construction/transform/stroke and TraceRgba; unified target-free routing for ToolOptions and TraceRgba.
- `varos/crates/varos-bridge/src/economy.rs`: shared edit_enabled gate preserves 1.1/1.2 normalization; one explicit 1.2 verb gate includes stroke, construction, transform and trace; retained op aliases and repeat limits.
- `varos/crates/varos-bridge/src/service.rs`: retained broad 1.2 support, v5/stroke schemas and construction/transform capabilities; advertised trace/repeat once, retained trace metadata, economy defaults, preflight and naming.
- `varos/crates/varos-bridge/tests/contracts.rs`: reconstructed interleaved functions; every original test retained exactly once, including stroke and trace atomicity/rollback/economy regressions.
- `varos/crates/varos-bridge/tests/export_reports.rs`: unified equivalent API advertisement tests into one superset assertion including select, edit, capabilities and export_pdf.
- `varos/crates/varos-core/src/bridge.rs`: retained every stroke/construction/transform validation arm and added trace check_insert preflight.
- `varos/crates/varos-core/src/command.rs`: retained every transform command and execution arm; added one InsertTracedPaths command with checked allocation/remapping and one undoable insertion.
- `varos/crates/varos-core/src/lib.rs`: exported both select_transform and trace once. Canonical painted_extent and geom/kurbo.rs unchanged; no duplicate shortcut, command, setting, store or adapter introduced.
- Merge-only support repair `varos/crates/varos-bridge/src/mcp.rs`: moved trace schema into the explicit 1.2 projection; retained every verb/constraint/public operation definition, removed a duplicate capability API assignment, shortened private shared references and descriptions; default and 1.2 size ratchets pass.
- Validation: fmt + offline -j 3 workspace/all-targets build PASS; 15 targeted core/Bridge/CLI/app invocations PASS (240 passed, 0 failed, 4 existing ignored); markers absent, all report/PLAN/NOTICE rows and contract tests retained, git diff --check PASS. Evidence: /tmp/g2-trace-build.log, /tmp/g2-trace-test-results.json, /tmp/g2-trace-*.log. Deferred merge work: none. No Git write commands used; moderator owns staging/commit.

## feat/p7-svgimport
- `REPORT.md`: retained both complete lane reports, every finding, gate, limitation and historical note.
- `docs/PLAN.md`: retained all existing progress rows and added SVG import; both sides checked verbatim.
- `varos/crates/varos-bridge/src/mcp.rs`: unified tools/list through the existing API projection; import_svg appears once for explicit 1.2, remains callable, and preserves all sibling tools/verbs/schema constraints. Shortened only opt-in descriptions: 23,988/24,000 bytes, all 237 references resolve.
- `varos/crates/varos-bridge/src/service.rs`: retained broad API 1.2 support, v5/stroke/construction/transform/trace capabilities; added import_svg once with its 1.2-only gate and tool API mapping. Merge-only receipt repair tracks originating API so request_status preserves API 1.2 stroke fields and byte-identical import receipts; legacy filtering remains intact.
- `varos/crates/varos-cli/src/main.rs`: one verb table and dispatch retain trace plus import-svg, including source/output guards and loss reports.
- `varos/crates/varos-cli/tests/commands.rs`: reconstructed separate test functions; every test from both stages retained exactly once.
- `varos/crates/varos-core/src/bridge.rs`: retained all stroke/construction/transform/trace validation and added checked PlaceArtwork preflight.
- `varos/crates/varos-core/src/command.rs`: retained every existing command/execution arm and added one PlaceArtwork variant/arm using checked, ID-remapped, single-undo placement.
- `varos/crates/varos-core/src/lib.rs`: exported select_transform, trace and placement exactly once.
- Unifications: one CLI verb table, MCP projection and capability response; no duplicate shortcut/command/setting/store/adapter or Bridge verb. Canonical painted_extent, geom/kurbo.rs, NOTICE and GATE_LOG byte-identical to HEAD; all nine conflict files marker-free.
- Validation: cargo fmt --all and cargo build --offline -j 3 --workspace --all-targets PASS; git diff --check PASS. Thirteen cargo test --offline -j 3 -p <crate> <filter> invocations cover import/placement/app menus, trace core/Bridge/CLI, full CLI commands/Bridge contracts/artboards, geometry/stroke/construction/select_transform: 248 passed, 0 failed, 4 existing ignored.
- Evidence: /tmp/g2-svg-build.log, /tmp/g2-svg-test-results.json, /tmp/g2-svg-*.log and /tmp/g2-svg-schema.json. Deferred merge work: none. No Git write command used; moderator owns staging/commit.


## feat/text-programme
- `REPORT.md`: retained both complete reports, every prior lane entry and all Text programme findings, tests, gates and historical limitations; removed only conflict markers and added a separating blank line.
- `varos/NOTICE`: retained all existing geometry/kurbo/construction/transform/trace attribution and added composer, BStudio kashida/caret and bundled-font attribution with every copyright/license row preserved.
- Unifications: additive document union; no duplicate implementation, shortcut, command, setting, store, adapter or Bridge verb introduced. No source repair required; canonical painted_extent and geom/kurbo.rs, PLAN and GATE_LOG byte-identical to integration HEAD.
- Validation: `cd varos && cargo fmt --all && cargo build --offline -j 3 --workspace --all-targets` PASS; both-side lines checked verbatim, both conflict files marker-free; `git diff --check` PASS.
- Lane tests: `cargo test --offline -j 3 -p <crate> <filter>` PASS: varos-text empty filter 62; varos-import empty filter 14; varos-bridge `--test import_svg` empty filter 3; varos-cli import_svg 1; varos-core placement 1; varos-app svg 4 — 85 passed, zero failed/ignored.
- Vendor reconstruction: COSMIC PASS including both patches and ledger hashes; egui_tiles verification SKIP because pristine archive is uncached (existing lane limitation; no egui_tiles changes in this merge).
- Evidence: `/tmp/g2-text-build.log`, `/tmp/g2-text-test-results.json`, `/tmp/g2-text-{text,svg,bridge-svg-full,cli-import,core-placement,app-svg,vendor}.log`.
- Deferred merge work: none. No Git write command used; moderator owns staging and commit.
