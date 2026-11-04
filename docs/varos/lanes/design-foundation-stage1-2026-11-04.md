# Shared UI foundation — first slice, 2026-11-04

Status: provisional; local work, awaits owner. Previous sizing/picker changes preserved in the
working tree; starting patch and untracked Rust copies saved under `plan/design-system-stage1/`.

`varos/theme_components.rs` owns the compile-time foundation for tab typography, logical heights,
padding, control/box radii and shared panel text defaults. Existing Palette/Tokens remain the color
authority. The production panel workspace and shell document tabs share tab surface painting and
12-point semibold typography. Document height remains 28 points; panel tabs remain 24 points.
Document tabs now use the panel family's flat surfaces/separators. Document identity, dirty mark,
close placement, input IDs, reorder logic, overflow and shortcuts remain owned by their callers.
The only new upstream change is the marked geometry/font lookup in `chrome.rs`.

Panel text defaults in the old shell and hosted Tools/Control share an adapter; the old shell's
heading override is retained explicitly, while Tools/Control retain inherited headings.

This slice does not claim full typography migration: upstream panel labels, dialog tabs and old
box-shell capsules remain later work. Runtime theme/font loading, density controls and import/export
are deferred. With compile-time immutable defaults there is no new cache epoch or runtime setter;
introducing mutable metrics requires invalidating sizing/font caches in that later slice.

Local checks passed: UI suite 2,787 unit + 2 integration tests, zero failed, 26 ignored;
software capture test passed and `plan/design-system-stage1/tabs.png` was inspected; workspace
Clippy with `--all-targets -- -D warnings`, `cargo xtask wasm`, fmt and diff checks passed.
The follow-up build includes the shared container height/font/ghost corrections. Release build passed in 4m 25s (`VAROS_PRODUCT_VERSION=0.20.2-panels.5 CARGO_INCREMENTAL=0
CARGO_BUILD_JOBS=4 cargo build --locked -j 4 --release -p vectorcraft`). Build **20261104.3**
installed while the app was closed, all five installed files match the manifest and deep/strict
codesign verification passed. Build 20261104.2 retained for rollback; settings retained; no app launch.
Receipt and exact dirty Rust source patch: `dist/design-foundation-stage1-2026-11-04/`.
Source patch SHA-256: `f3381c861631ee1bdd219f225867fc205c277ac614a79db62167d14be6837d09`.

Two specialized agents reviewed typography and tab interaction; their heading-preservation finding
was fixed, and real-consumer tests were added. Claude Fable reviewed twice; final read-only review
found no remaining blocker and verified preceding sizing work is preserved. Review evidence:
`plan/design-system-stage1/fable-review.md` and `fable-final.md`. All checks are local.

Deferred document-specific close/title spacing remains explicit in chrome/tab_reorder. The old
upstream-to-Varos style adapter remains; this slice consolidates its Varos definitions without
refactoring the upstream initializer.
Owner hand test: compare document/panel tabs, switch and close documents, reorder them, drag/dock
panels, exercise shortcuts, and confirm existing Color sizing and picker behavior remain usable.
