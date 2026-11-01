# VSTAB.2 — panel movement and Color palette behavior

Status: all 12 code gates passed, 2026-11-01. Native acceptance blocked.

## Owner-reported behavior

The owner supplied seven screenshots of Varos and described fixed rail placement, oversized
edge drops, floating collapse returning panels to old homes, a misplaced Control panel,
per-tab collapse-like arrows, and a Color picker that blocks the canvas and cannot move.
The images remain local; no screenshot assets, palette changes or external artwork were added.

## Changes

- Drag the docked icon rail by its top toggle or empty background, float it, and dock it at
  either workspace edge. Each group can also be dragged by its separator grip. Long floating
  rails scroll so their lower icons stay reachable. The existing serialized strip records the side.
- A floating panel collapses to an icon at its own position, retaining expanded dimensions.
  Moving and expanding that icon uses its new position. Close retains the separate return behavior.
- A floating panel dropped on the rail becomes icons there instead of returning to an old tab row.
- Docked groups have one collapse-to-icon control. Character/Paragraph tabs select and drag;
  their extra options remain in panel menus. Empty header space also starts a drag.
- Edge docking starts with a compact share instead of half the available workspace. A Control-only
  group dropped at the workspace top spans the docked layout and takes a bounded content height.
- The shell Color picker is a movable nonmodal palette with a Close control. Live edits remain
  undoable; Escape cancels its own open edit group. Canvas actions are allowed, and opening or
  using the picker does not auto-collapse its Color flyout. The classic shell keeps its modal behavior.

## Evidence and limitations

- Current upstream fetched again: `1c4a7831ebdd9746b0f253413fe4f1fefc543bfc`, unchanged
  from VSTAB.1. No additional upstream changes to integrate.
- The existing local `plan/STATUS.md`, `plan/execution-plan.md` and `plan/architecture.md` are
  absent. Tracked architecture, stability handoff and the owner's behavior report guide this task.
- `CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true RUST_TEST_THREADS=2 cargo xtask ci` passed
  all 12 gates. Aggregate test results: 11018 passed, 0 failed, 34 ignored;
  the UI suite passed 2600 tests with 19 ignored. Ignored captures are opt-in, not native passes.
- The opt-in `capture_panel_behavior_followup` passed and its 1440×900 software-rendered
  image was inspected: bounded top Control bar, movable Color palette without OK/Cancel,
  and a collapsed floating Swatches icon. Local receipts: `dist/panels-2026-11-01/`.
- Regression coverage uses headless egui pointer events for rail detachment/left docking and
  save restoration, floating collapse/move/expand, empty-tab-row dragging, compact edge drops,
  group collapse, Control top docking, and palette movement with canvas input. A separate
  canvas drag test verifies that palette Escape cancels only its color edit and leaves the
  object's move as a separate undo step.
- Native Computer Use still rejects `com.varos.app` with
  `Computer Use was not approved to use Varos`, including after the owner explicitly tagged it.
  Native mouse behavior, installed-bundle acceptance and actual relaunch remain unverified.
- `/Applications/Varos.app` (`com.varos.editor`) is a separate legacy application and is untouched.

## Native acceptance still required

On the fork bundle, save the open document before replacing the running executable. Recheck
each owner-reported gesture with a mouse, including rail scrolling, left/right docking,
floating icon placement, Control placement in the existing saved workspace, and drawing or
moving objects while the palette is open. Close/reopen the application and confirm layout
persistence. A passing code suite or software-rendered image does not substitute for these steps.

## Revision history

- 2026-11-01: owner report mapped to shell/picker fixes; all 12 gates and software capture passed.
  Native mouse and installed-bundle acceptance remain pending.
