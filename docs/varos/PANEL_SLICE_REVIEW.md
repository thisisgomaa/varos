# VPANEL.S1 — Layers / Swatches native review

Status: implementation authorized 2026-11-01; owner acceptance pending. This is an isolated trial, not a released workspace migration.

## Scope

Only Layers and Swatches own slots in the new layout. The existing real panel content and engine commands are reused. The native frame implements full tabs, icons with names, icons, tab detach/merge/reorder, separate stacked/adjacent groups, whole-block dragging, edge resizing, and X/Window hide/restore. Other panels remain outside this trial. The existing tool strip and canvas are retained for working with the two contents; toolbar docking is disabled in this trial.

The approved HTML remains untouched:
`~/.codex/visualizations/2026/11/01/01a195da-fde8-72e3-9d95-ac60000e350a/varos-panel-icon-review.html`
SHA256 `681ec7a3618337ea12d81d81bb9ab087d12b8563e38a9c3ee734e6d783ce8bb8`.

## Architecture

- `panel_system/model.rs`: validated, transactional ownership tree; each panel has one owner even when hidden. Moves validate a candidate before replacing the layout. Groups retain expanded extents through stacking, detaching and compact modes.
- `geometry.rs`: the same measured rectangles supply painting, interaction and drop targets.
- `paint.rs`: shared opaque Varos chrome; header20, tabs36, controls18, panel icons24 logical points. Existing original/licensed icons and an original geometric Swatches grid; no new asset files.
- `mod.rs`: one live body per panel, stable body scope across hosts, pointer gesture preview and commit, Escape/focus-loss rollback, Window routing. Native UI commands expose actions and read-only inspection.
- The versioned model supports a tested serde round trip. Saving/migrating the user's working layout is deliberately deferred. Trial layout changes are session-only.

## Build isolation

Trial bundle: `dist/panel-slice-2026-11-01/Varos Panels Preview.app`.
Bundle identifier: `com.varos.panels-preview`.
Version: `0.20.0-panel-slice.1`.
Separate configuration: `~/Library/Application Support/Varos Panels Preview` and its `agents` subfolder.
`VAROS_PANEL_REVIEW=1` opts in at startup. The bundle registers no document associations and does not replace either existing Varos app.

The working tree already contained picker clipping and shortcut/undo changes before this slice. They are preserved and recorded in the source patch; they are not claimed as newly implemented or accepted in this task.

## Upstream review

Remote VectorCraft HEAD checked: `05d5f99fd7e7dc00db3a9754d594b1214945d855`.
Craft-libs HEAD checked: `f8f8829cc7be65d466d687dae381d1ca60e31242`; project pin unchanged at `a0916808f7d78bed8c7b90cdf937176015454a7f`.

Backported the scoped Swatches anchor reset and regression test from [a0da3faff](https://github.com/storytold/vectorcraft/commit/a0da3faffebafc0772b5a1902d66fd55b60501e6). It prevents Shift-click from carrying a range anchor across documents. DockArea/OS-window fixes do not directly apply to this new in-workspace renderer; no broad dependency upgrade was made.

## Verification

Verified on the trial source:

- 29 panel-system tests passed (model, retained sizes, raw-pointer merge/reorder/detach, free movement, front order, full flyout detachment, both compact edges, minimum widths, Escape/focus loss, X/Window, stacked groups, stable origin after edge-constrained movement).
- 45 existing Layers tests and 28 Swatches tests passed, including the upstream anchor regression.
- All-target clippy for UI and desktop passed with `-D warnings`; formatting and diff whitespace checks passed.
- Layering: 30 crates, no violations. Asset attribution: all 537 asset files covered; no new asset files.
- Actual-code software captures of the full pair and compact stack were produced and inspected. These are headless raster captures, not native pointer acceptance.

Test builds use `CARGO_INCREMENTAL=0` after the pre-existing incremental artifacts caused undefined-symbol linker failures. The final UI test crate used opt-level=0 to shorten test compilation; the native candidate uses the unchanged release profile. No compiler/dependency version was changed.

WASM check passed. Release packaging and native startup records are added to the candidate's verification artifacts. Automated model and pointer tests establish code contracts, not the owner's approval of native feel or appearance. Full workspace CI/commit and release approval are not claimed for this trial.

## Owner review order

1. Drag the Swatches tab onto Layers, reorder the two tabs, pull one out, then move each block from its dark header. Stop for feedback.
2. Only after the first review: collapse/expand and drag either side edge between icons and names.
3. Next review: stack via the bottom line, attach side by side, move the combined block, X then Window restore.

Do not migrate Control, Tools, Color, Properties or other panels until the owner explicitly accepts this slice and authorizes the next one.
