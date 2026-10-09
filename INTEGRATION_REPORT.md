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
