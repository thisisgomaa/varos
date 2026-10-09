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
