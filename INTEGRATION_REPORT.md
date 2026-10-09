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
