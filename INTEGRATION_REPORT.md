# Wave 2 — stage 1 integration (`integ/w2`, 2026-10-09)

Base: main `c852a55` (includes the stroke engine hotfix). Merged in order with `git merge --no-ff`, one commit per
branch: `63336e5` tools-ui · `ee94919` view · `149e988` text · `b27fcf0` import, plus one separate, revertable fix
`7391ea5` (pre-existing import-lane bug found during integration, see below). Nothing pushed; main untouched.
Main's `geom/kurbo.rs`, `painted_extent` and `stroke/canvas.rs` are untouched by every resolution (canonical).

## feat/w2-tools-ui (4B shapes + rail flyouts, 4C Pencil/Smooth/Path Eraser/Join/Curvature)
- No textual conflicts against main. fmt clean; build PASS; core/app/bridge/cli tests 1,673 passed, 0 failed.

## feat/w2-view (Outline, Pixel Preview + Snap to Pixel, Navigator, screen modes)
- `REPORT.md`, `docs/PLAN.md`: both lane reports and both Progress rows retained.
- `shell/tokens.rs`: Lane D drawing tokens and Lane E view tokens kept side by side (one token home).
- `core/editor.rs`: staged drawing options AND staged view-depth/pan/zoom/canvas requests are both applied;
  `pointer_move` returns in presentation mode first, then runs drawing movement (presentation = no editing).
- `core/scene.rs`: main's hotfix `canvas = true` stroke-cache path is passed through Lane E's
  `view_depth_scene::present` wrapper (`build_scene_impl(..., Some(style), true)`), so Outline/Pixel Preview use the
  hotfix canvas cache and export keeps `build_scene_for_export`.
- Tests: core/app/bridge/cli/render-wgpu 1,752 passed, 0 failed.

## feat/w2-text (TextBox model, Type tool, Properties Type, outline export — format change)
- 17 conflicted files, all resolved keeping both lanes:
  - Core `EditCommand`: `Drawing(_)` + `AddText`/`SetText`; batch API gate lists all three as API 1.2-only;
    `bridge::check` runs Drawing validation (early return) then text `check_change`; second dispatcher has both arms.
  - `editor.rs` undo/redo: drawing gesture cancel kept AND selection pruning keeps text identities.
  - Bridge: `Operation` carries drawing + text ops; `design.rs` applies drawing then text; `economy.rs` verb filter
    admits drawing verbs (construction) and `set_stroke_style`/`add_text`/`set_text` (1.2); `lib.rs` keeps
    `view_depth` + `text` modules; `mcp.rs` descriptions unioned; CLI verbs `view-depth`, `add-text`, `set-text`.
  - App tables unioned: tokens (Lane D/E/G), icons (Draw* + Type), shortcut parity (Lane E + `T` Type).
- Unification — tool rail: Lane D replaced the rail with kit flyout groups; Lane G had added a Type button to the
  old rail. ONE rail kept (Lane D's); Type is its own flyout group after the Pen group (Illustrator order), with
  `Icon::Type` and tool name "Type (T)".
- Unification — keyboard capture: one predicate = numeric drawing sheet OR live text session OR export field.
- `main.rs`: main's hotfix canvas hint (no canvas dialogs) kept on Lane G's text-preview scene.
- Merge-caused fixes: (1) `text_product` tests built `SceneStyle` without Lane E's `outline`/`canvas` fields;
  (2) the text preview renders from a per-frame `Editor` clone and `CanvasStrokeCache::clone` is empty by design,
  which silently defeated the hotfix's cross-frame stroke cache in any document with text — `TextProduct` now owns
  one cache and lends it to the preview (regression `text_preview_keeps_canvas_stroke_cache_across_frames`).
- Format: kept SYMBOLIC. `TEXT_FORMAT_VERSION` (provisional value 6) with `FORMAT_VERSION = TEXT_FORMAT_VERSION`
  and `migrate_to_text_boxes`. Stage 2 must renumber after images v6 → gradients v7 → text v8 (→ corners v9 if
  separate) and re-run every migration/refusal fixture.
- Tests: workspace 2,001 passed, 0 failed, 15 ignored.

## feat/w2-import (PDF/AI/DXF import, OS clipboard in, drag-drop)
- `REPORT.md`, `docs/PLAN.md`: retained. `varos/NOTICE`: Lane D Join row + Lane H DXF/VectorCraft and lopdf rows.
- `varos-cli/src/main.rs`: both `text` and `import` modules.
- Merge-caused fix: the combined API 1.2 tools/list reached 24,430 B (> 24,000 cap). Cap NOT raised; the
  progressive-disclosure projection now omits generic "Apply <verb>" lines from the inline extended-verb summary
  (enum and validation unchanged; `list_verbs` still lists every line, `schema` every parameter). 1.2 = 23,272 B.
- Cross-lane checks: Cmd+V paste deferral (Lane H) is gated by `wants_keyboard`, so a live text session still gets
  egui Paste; internal Varos copy (incl. text source) falls through to the in-app paste.
- Separate fix `7391ea5` (NOT merge-caused; present on `feat/w2-import` alone): `clipboard_in::capture` compared
  the published `org.varos.clipboard` flavour as `serde_json::Value`; any non-dyadic f32 (12.3, 0.1) round-trips
  to a different f64, so Cmd+V refused the app's own copy of ordinary artwork/text as "Unknown/malformed".
  Now compares exact serialised bytes first. Two regressions failed before, pass after. Revertable on its own.

## Shortcuts
New bindings, all Illustrator parity, no clashes, no losers: N Pencil · \ Line Segment · Shift+~ Curvature ·
Cmd+Y Outline · Opt+Cmd+Y Pixel Preview · Shift+F Presentation Mode · T Type · Shift+Cmd+P Place.
`bindings_are_unique_and_dispatch_requires_a_listed_chord` PASS.

## Final gates (after `7391ea5`)
- `cargo fmt --all --check` PASS · `python3 ../tools/check_dep_directions.py` PASS
- `cargo test --offline --workspace -j 3 --no-fail-fast`: 125 suites, 2,031 passed, 0 failed, 15 ignored
- clippy native and `--target x86_64-pc-windows-msvc`, `--workspace --all-targets -D warnings`: PASS, 0 warnings
- Ratchets: shell 3/3 PASS; `ui.rs` 831/843 (no glue move needed); Bridge ratchets PASS
- Bridge 1.0/1.1 tools/list byte-frozen fixtures PASS (23,993 B each, fixtures untouched); 1.2 = 23,272/24,000 B
- `cargo check -p varos-text --target wasm32-unknown-unknown` PASS
- `tools/check_vendor_patches.py`: cosmic-text PASS, egui_tiles SKIP (archive not cached)
- `docs/PLAN.md` Progress table: 5 rows appended (4B, 4C, 8.1–8.4, Text P2–P4, 7.2–7.5), no row removed.

## Deferred: none. Notes for stage 2
- Images (v6) / gradients (v7): Lane E image-box outlines, Lane D blob state + appearance routing, Lane G
  text/image/gradient export traversal, Lane H image-aware import all still need their stage-2 semantic merges.
- Renumber the symbolic text format (see above).
- Known consequence, unchanged: a text-engine failure during canvas preview lands in `Scene.errors`; main's hotfix
  policy (no canvas dialogs) means it is not surfaced on canvas, and the text is not drawn (fallback scene has no text outlines).

---

# Previous integration report (wave g2), kept for history

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
## feat/p0-commands
- `REPORT.md`: retained both complete lane reports and all evidence/gate lines.
- `docs/PLAN.md`: retained 0.2/0.9 crash/clipping and 0.8/0.7 command/view rows.
- `varos/NOTICE`: retained every crash, GPU, command, tool, icon and view attribution.
- `mac_menu.rs`: united all key-code adapters, including Digit7 and the command/view keys.
- `main.rs`: retained clipping, legacy Redo and command/view/tool dispatch; unified overlapping arms through crash-safe `execute_ui`; necessary extra merge fix in `shortcuts/parity.rs` adds clipping and legacy Redo to the single allowlist and updates its assertion.
- `menus/mod.rs`: united egui key adapters with one arm per key.
- `menus/object.rs`: retained separate Clipping Mask, Layers, Lock/Hide, Path and Compound Path groups without duplicated rows.
- `menus/tests.rs`: retained clipping and Select tests; united frozen-menu normalization and removed its unreachable clipping arm.
- `ui/ops.rs`: kept crash-safe dispatch and unified distribution under existing `DistributeMode`/`DistributeSpacing` callers; legacy core Distribute remains available.
- `economy.rs`: united clipping and command/view/anchor verb acceptance; legacy API refusal stays in service.
- `service.rs`: united API 1.2 capabilities/select/edit/export gates and legacy refusals; advertises each added verb once, preserves mutation-only rollback checkpoints, propagates paste-setting errors.
- `contracts.rs`: reconstructed both complete appended test sets; retained clipping/legacy discovery and command/view tests, added assertions for united capability verbs/select, checked Undo result.
- `editor.rs`: retained Arc history/crash tests and command/view modules, made SelectionState and Editor independently Clone, routed added interactive selection/lasso calls through `execute_ui`; canonical `painted_extent` and `geom/kurbo.rs` remain unchanged; PLAN/GATE_LOG history retained.
- Validation: `cargo fmt --all` and `cargo build --offline -j 3 --workspace --all-targets` PASS; `cargo test --offline -j 3 -p {varos-core,varos-bridge,varos-app} ''` run separately: 479/133/754 passed, zero failed, 2/6/1 ignored; final discovery 1 and menu 16 passed; marker scan and `git diff --check` PASS. Existing unused-Result warnings remain; deferred: none; no git writes.

## feat/p0-export
- `REPORT.md`: retained complete crash, command/view and export lane reports, including every evidence/gate line.
- `docs/PLAN.md`: retained all crash/clipping, command/view and SVG/raster progress rows.
- `varos/crates/varos-app/src/menus/tests.rs`: unified post-split exclusions once, preserving view additions, export/PDF normalization and both lanes' tests.
- `varos/crates/varos-app/src/shell/kit/icons.rs`: united command/view and export variants, SVG mappings and registry entries; one 70-icon registry.
- `varos/crates/varos-app/src/shell/tokens.rs`: retained all guide/zoom/command-menu and export-sheet tokens in the existing single token store.
- `varos/crates/varos-app/src/ui.rs`: uses the new sheet dispatcher for export jobs and retains the command lane's status-bar signature, typed zoom and operation queue; existing export callers remain wired.
- `varos/crates/varos-bridge/src/mcp.rs`: unified discovery through `tools_for_api`; opt-in 1.2 includes clipping/command/view plus SVG/raster once, legacy discovery stays unchanged, new tool calls accepted; added combined-discovery regression test.
- `varos/crates/varos-bridge/src/service.rs`: united all 1.2 admission/capability gates and advertised tools; retained legacy export refusals, command verbs, mutation-only rollback and report receipts.
- Necessary merge-only fix: `varos/crates/varos-app/src/shortcuts/parity.rs` adds the export lane's existing Alt+primary+E chord once to the command lane's dispatch allowlist; menu/key adapters and callers retained.
- Geometry/adapter implementations unchanged; no alternate `painted_extent` or kurbo adapter introduced. Existing NOTICE and GATE_LOG content unchanged.
- Validation: `cargo fmt --all`, `cargo build --offline -j 3 --workspace --all-targets`, marker scan and `git diff --check` PASS; unfiltered `cargo test --offline -j 3 -p <crate> ''`: core 479, Bridge 137, app 769, raster 17, CLI 28 passed; zero failures, 12 ignored. Existing unused-Result warnings remain.
- Logs: `/tmp/g1-p0-export-build-final.log` and `/tmp/g1-p0-export-<crate>-tests.log`; deferred: none; no Git write commands, commit, push, installation or GUI actions.

## feat/p1-docsetup
- `REPORT.md`: retained every crash, command/view, export and document lane report/evidence line; `docs/PLAN.md`: retained all progress rows; `varos/NOTICE`: retained every attribution.
- `varos/crates/varos-app/src/bridge_host.rs`: retained paste-settings dispatch, propagated fallible execution, added window-memory query and preserved queued template snapshot/cancellation workers.
- `varos/crates/varos-app/src/file_jobs.rs`: retained screen-export destination handling and both Bridge/template worker cases; `varos/crates/varos-app/src/host.rs`: united export and setup/info/template lifecycle settlement.
- `varos/crates/varos-app/src/menus/tests.rs`: united each post-split exclusion once; `varos/crates/varos-app/src/shell/tokens.rs`: retained command/export and document/checkerboard constants in the single token store.
- `varos/crates/varos-app/src/ui.rs`: retained export dispatcher, command status-bar signature, document guides/sheets and shared field visibility; `varos/crates/varos-app/src/ui/bar.rs`: retained document rows and command submenus.
- `varos/crates/varos-app/src/ui/ops.rs`: retained view/zoom and setup scrub transactions/finish, unified Units once, routed setup dispatch through crash-safe execute_ui.
- `varos/crates/varos-bridge/src/cli.rs`: united export/template/window verb allowlist; `varos/crates/varos-bridge/src/design.rs`: retained forced-panic regression hook and setup adapter validation/execution.
- `varos/crates/varos-bridge/src/dto.rs`: united request/operation variants and all API/board/receipt callers; `varos/crates/varos-bridge/src/economy.rs`: united document, clipping, command/view and anchor verb admission, retaining version restrictions.
- `varos/crates/varos-bridge/src/mcp.rs`: one combined API 1.2 discovery projection, tools_12 delegates to tools_for_api, all tools admitted once; preserved legacy schemas and strengthened combined-discovery regression.
- `varos/crates/varos-bridge/src/service.rs`: united admission/capabilities, paste/window host methods and template/export routing; advertises each tool/verb once and preserves legacy restrictions, receipts and mutation-only crash checkpoints.
- `varos/crates/varos-core/src/bridge.rs`: retained command/view checks and setup validation; `varos/crates/varos-core/src/scene.rs`: retained document grid_step alongside styled checkerboard, with unstyled exporter entry points.
- Necessary merge-only fixes: `varos/crates/varos-app/src/shortcuts/parity.rs` registers existing primary+Alt+P once; `varos/crates/varos-app/src/ui/tests.rs` enlarges burger fixture viewport for combined rows; `varos/crates/varos-bridge/tests/phase1.rs` counts retained export tools and verifies combined capabilities. No geometry/adapter edits; canonical geom.rs/painted_extent unchanged; GATE_LOG history untouched.
- Validation: cargo fmt --all and cargo build --offline -j 3 --workspace --all-targets PASS; cargo test --offline -j 3 -p <crate> '' PASS: core 484, Bridge 140, app 778, raster 18, CLI 29, PDF 97; zero failures, 15 existing ignores. All 19 marker scans, both report/PLAN/NOTICE retention checks and git diff --check PASS; existing unused-Result warnings remain.
- Logs: /tmp/g1-docsetup-build-final.log and /tmp/g1-docsetup-<crate>-tests.log; deferred: none; no Git write commands, commit, push, GUI or installation.

## feat/p1-pdfprint
- `REPORT.md`: retained all lane reports/evidence; `docs/PLAN.md`: retained every progress row, including PDF/Print/clipboard.
- `varos/crates/varos-app/src/app_command.rs`: one legacy ExportPdf plus ExportScreens, ExportPdfOptions and Print; `host.rs`: united Print/Document Setup chords and export/setup/template lifecycle barriers.
- `varos/crates/varos-app/src/bridge_host.rs`: retained raster fields plus PDF options in shared requests and all template/window/export/print/clipboard host effects; `lifecycle.rs`: preserved screen batches, PDF options/default callers and Print host ownership; filled a merged test job's options.
- `varos/crates/varos-app/src/export_ui.rs`: one sheet/dispatcher, exclusive Screens/PDF-page controls and one active export action; preserved scopes, cards, cancellation, identity/repaint tests, reports and remembered Print options; screen PDF jobs receive the same options.
- `varos/crates/varos-app/src/main.rs`: Copy/Cut use the shared OS publisher while retaining core clipboard/history; Print and export routing retained; existing WindowStore supersedes obsolete text-window helpers, with no second store.
- `varos/crates/varos-app/src/mac_menu.rs`: one Print/save/export document-row classification; `menus/mod.rs`: preserved NewTemplate and platform Print enablement; `menus/tests.rs`: retained both shortcut assertions and unified all post-split exclusions.
- `varos/crates/varos-app/src/ui.rs`: retained shared export dispatch, document sheets/guides and typed status-bar operations; PDF options flow through that dispatcher.
- `varos/crates/varos-bridge/src/cli.rs`: united verb admission; `dto.rs`: all request/tool/API/board/receipt variants retained once; options decoder preserves explicit null presence, legacy refusal and raster-field checks; `lib.rs`: retained storage/templates modules and opt-in effect list.
- `varos/crates/varos-bridge/src/mcp.rs`: canonical string-API projection plus PDF options/Print/Copy/Cut, all tools once, all callers retained; `service.rs`: one combined API 1.2 admission/capability map, all verbs/effects, legacy guards and mutation-only rollback retained.
- `varos/crates/varos-bridge/tests/contracts.rs`: retained complete test sets, unified discovery expectations and checked legacy options at the shared decoder boundary; `varos/crates/varos-cli/src/main.rs`: united verbs/flags, one fractional raster PPI parser with integer validation for PDF; `varos/crates/varos-cli/tests/commands.rs`: retained clipping, PDF/Print and clipboard tests.
- `varos/crates/varos-raster/src/lib.rs`: retained export module and clipboard PNG adapter. Canonical painted_extent/geom/kurbo adapter unchanged; NOTICE and GATE_LOG history untouched.
- Necessary merge-only fixes: `varos/crates/varos-app/src/shortcuts/parity.rs` registers Print once; `ui/export/model.rs` fills PDF job options and source-board bleed; `file_jobs.rs` applies options to screen PDF exports; `os_clipboard.rs` propagates merged fallible core execution and keeps crash-safe UI dispatch; `varos/crates/varos-bridge/tests/phase1.rs` counts all opt-in tools.
- Necessary cross-lane bleed reconciliation: `varos/crates/varos-pdf/src/export.rs` carries canonical document_setup edges alongside legacy scalar bleed; `src/options.rs` applies asymmetric Media/Bleed/Trim boxes and marks; `tests/export_pdf.rs` updates the page literal; `tests/options.rs` adds independent asymmetric geometry/mark assertions. Coincident-board and historic PDF fixtures retained.
- Validation: cargo fmt --all; cargo build --offline -j 3 --workspace --all-targets PASS; cargo test --offline -j 3 -p <crate> '' PASS: core 484, Bridge 142, app 785, raster 18, CLI 31, PDF 103 (1563 passed, zero failed, 15 existing ignores). All 21 marker scans, report/PLAN retention and git diff --check PASS; logs /tmp/g1-pdfprint-*.log; existing unused-Result warnings remain. Deferred: none; no Git write commands, commits, GUI or installation.

## feat/p1-autosave
- `REPORT.md`: preserved both complete reports and every evidence/gate line.
- `docs/PLAN.md`: preserved all existing progress rows and added slice 1.10.
- `docs/foundation/GATE_LOG.md`: retained complete command/view and autosave histories.
- `app_command.rs`: retained paste preference commands and all autosave commands, each once.
- `file_jobs.rs`: united Template/Autosaved completion variants, quiet policy and exhaustive Bridge handling; kept publication fingerprints/export jobs.
- `lifecycle.rs`: united host-owned commands, export/PDF/print dispatch and autosave confirmation/conflict paths; retained template and autosave completions/tests.
- `main.rs`: retained GPU notices and window geometry persistence alongside edit-publication admission/invalidation; releases the permit before idle autosave submission.
- `recovery_host.rs`: unified duplicate settings field, initialization and load assignment; one settings command/writer path handles recovery, paste and autosave, retaining all callers and scheduler resets.
- `storage/mod.rs`: retained templates/window plus autosave/publication modules.
- `storage/settings.rs`: one v2 settings schema/writer retains paste + recovery + autosave; migrates v1, validates intervals, preserves additive keys, refuses unsupported versions; retained both test sets and strengthened combined-field migration/round-trip coverage.
- `ui/bar.rs`: retained command-menu helper and autosave status module/controls; no shortcut duplication.
- Validation: `cargo fmt --all` and `cargo build --offline -j 3 --workspace --all-targets` PASS; `cargo test --offline -j 3 -p <crate> ''` PASS: app 806, core 484, Bridge 144; zero failures, 9 ignored; marker scan, documentation line-preservation check and `git diff --check` PASS. Logs: `/tmp/g1-p1-autosave-build-final.log`, `/tmp/g1-p1-autosave-{app,core,bridge}-tests.log`.
- Deferred: none. Existing unused-Result warnings remain; canonical `painted_extent`/`geom/kurbo.rs`, NOTICE, shortcuts and Bridge verbs preserved. No Git writes, commit, push, installation or GUI actions; moderator owns staging/commit.

## main-now
- `INTEGRATION_REPORT.md`: retained both complete integration histories and added this section; `REPORT.md`: retained both lane reports verbatim; `docs/PLAN.md`: retained every lane progress row; `docs/foundation/GATE_LOG.md`: retained both gate entries; `varos/NOTICE`: retained every attribution row.
- `varos/Cargo.lock`: unioned image/serde_json and kurbo/proc-macro2 dependencies; `varos/crates/varos-cli/Cargo.toml`: retained PNG decoding and JSON dependencies once.
- App `bridge_host.rs`: retained checked SVG placement and queued template jobs; `file_ports.rs`: retained both SVG and template pickers; `lifecycle.rs`: retained SVG placement, document/template commands, and autosave completion behavior.
- App `host.rs`: unioned all typed document/menu routes; `main.rs`: retained construction/transform and view/anchor tools, their test modules, styled scenes plus stroke diagnostics, and main's Shift+Command+Z redo behavior.
- App `menus/file.rs`: unioned SVG/template/setup rows; `menus/mod.rs`: unioned enablement and typed commands; `menus/tests.rs`: retained both suites and combined the frozen-row exclusions; `shell/kit/icons.rs`: one 78-icon catalogue; `shell/tokens.rs`: unioned all namespaced token families.
- App `thumbs/mod.rs`: retained checked stroke rasterization plus export-asset/artboard thumbnails; `ui/ops.rs`: one arm per shared operation, main's execute_ui panic reporting, plus all stroke/construction operations.
- Bridge `cli.rs`: one admission path accepts all effects; `lib.rs`: retained transform adapter and storage/template exports; `dto.rs`: unioned effects/operations and canonical ID/API/board/mutation routing, retaining legacy file decoding tests.
- Bridge `design.rs`: retained transform/trace staging, document setup, command-wave adapters, panic injection, target budgets, and both internal/limit error classifications; `economy.rs`: one economy predicate and the union of opt-in verb gates, aliases, defaults and repeat limits.
- Bridge `mcp.rs`: one tools_for/tools_for_api/tools_12 projection, unioned all tools/verbs, kept local schema references and both schema tests; shortened discovery annotations only, retaining validation constraints. `service.rs`: broad 1.2 admission with effect-specific gates, unioned capabilities/receipts, preserved checkpoint rollback and internal/limit errors.
- Bridge `tests/contracts.rs`: reconstructed both complete suites and updated the added-tool count to include import_svg; `tests/export_reports.rs`: retained both export suites and unified the API-advertisement assertion. CLI `src/main.rs`: one verb table includes trace/import and all main exports; `tests/commands.rs`: reconstructed every original test separately.
- Core `bridge.rs`: unioned allocation checks and all preflight arms; `command.rs`: unioned variants and execution arms; `editor.rs`: combined transient state/resets, isolation/guide/stroke hits and gesture release routes; `lib.rs`: unioned module exports; `scene.rs`: combined styled grid/checkerboard with stroke diagnostics and construction overlays; `tools/mod.rs`: one router retains all tools.
- Merge-only support: `varos-core/src/construction.rs` and `select_transform.rs` gained Clone for main's editor rollback; construction/eyedropper/transform tool callers and app `ui/select_transform.rs` use execute_ui; app `shortcuts/parity.rs` admits every incoming tool chord once; Bridge `tests/phase1.rs` accounts for the additional import tool and shared op/verb discriminator. Canonical painted_extent and geom/kurbo.rs are unchanged.
- Verification: cargo fmt --all and cargo build --offline -j 3 --workspace --all-targets PASS; all original named tests and report/PLAN/GATE_LOG/NOTICE lines retained; markers absent; git diff --check PASS. Full crate tests with cargo test --offline -j 3 -p <crate> '' --no-fail-fast: 1,873 passed, 3 failed, 15 ignored; core/app/CLI/PDF/raster/render-wgpu/import/text PASS. Evidence: /tmp/g2-main-build.log, /tmp/g2-main-tests.json and /tmp/g2-main-*-tests.log.
- UNFINISHED merge work: Bridge's three tools/list size ratchets fail because API 1.2 is 32,172/24,000 bytes; legacy discovery is 23,152 bytes and all other Bridge tests pass. No limit raised, constraint weakened, or failure waived. The requested zero-deferred-work condition is not met; moderator must not commit this as a completed integration. No Git write command used.
