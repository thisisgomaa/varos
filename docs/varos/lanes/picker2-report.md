# Picker lane handoff — 2026-10-27

Codex prepared the implementation but could not compile it (its sandbox could not write the shared
cargo target). An Opus 5.5 follow-up made it compile, fixed what the gates and the captures found,
and ran every gate below. No commits, stash operations, native app/window actions, or shell-module
edits were made.

## Follow-up fixes (Opus 5.5, 2026-10-27)

| File | Fix |
|---|---|
| `widgets.rs` | `ProxyClicks` no longer derives `Default` (egui 0.37 `Rect` has none): the compile error. |
| `varos/picker/mod.rs` | The popover sits under the Fill/Stroke *pair* (the anchor plus any well within 12 pt), else beside it (right, then left) when the window has no room below, so it never covers the other well (it used to cover the toolbar's stroke square, so "choose another well" could not work). Escape leaves a focused field or an open dropdown alone first. |
| `panels/mod.rs` | The chip's Swatches access (secondary click) uses the self-managed `widgets::popover` again instead of an egui context menu, which closes with the Swatch Libraries menu (#536 regression). |
| `panels/gradient.rs` | Opacity/Location fields narrow to 42 pt below 240 pt of room, so Gradient mode stays 240 pt wide (it measured 255 pt in the first capture). |
| `canvas.rs` | Comment for the picker gate. |
| `tests_paintchips.rs` | Picker edits are labelled "Color" (the picker's own command label), not "Fill Color"; the #536 test now exercises the real chip (secondary click) instead of a stand-in Area; test renamed to what it checks. |
| `varos/picker/tests.rs` | Placement assertion follows the pair rule and checks the popover never intersects the wells; Gradient mode asserts the measured popover area is at most 240 pt wide. |

## Files and purpose

Paths below are relative to `crates/ui-egui/src/` unless prefixed with `docs/`.

| File | Change |
|---|---|
| `varos/picker/mod.rs` | Transient 240 pt anchored modal with transparent backdrop, window clamping, outside dismissal and well switching; Gradient icon mode; solid conversion through commands; removes the mode-row close button and Gradient navigation text; isolates nested stop-editor mode. |
| `varos/picker/tests.rs` | Updates existing persistence/dismissal/gradient tests; adds real-frame toolbar, target-switch, canvas-selection, shell/docked-panel, AccessKit, stop-edit and drag-undo coverage; adds software capture entry point. |
| `panels/mod.rs` | Routes shared toolbar proxies and Properties/control-bar chips to the picker; records well geometry; retains explicit secondary-click Swatches access. |
| `widgets.rs` | Exposes proxy well rectangles and removes obsolete double-click wording from their accessible names; updates corresponding assertions. |
| `state.rs` | Adds a non-serialized popover anchor. |
| `lib.rs` | Draws the picker after shell/panels and before dialogs. |
| `canvas.rs` | Suppresses canvas tool input while the picker owns the pointer, including the dismissing gesture. |
| `shortcuts.rs` | Gives picker Escape handling priority without changing shortcut mappings. |
| `panels/gradient.rs` | Shares the existing editor with the picker, keeps compact options visible, wraps stop fields, finishes interrupted ramp gestures, and dismisses the picker for canvas editing/eyedropper actions. |
| `panels/color.rs` | Preserves the docked body and routes nested gradient-stop colour editing to its own picker mode; updates the proxy regression assertion. |
| `tests_paintchips.rs` | Updates primary-well tests to edit picker hex values; retains the Swatch Libraries regression separately. |
| `docs/varos/BACKLOG.md` | Records implementation scope and the validation blocker without changing parity scores. |
| `docs/varos/lanes/picker2-report.md` | This evidence-scoped handoff. |

## Review fixes (Codex Astra static review, FIX FIRST; Opus 5.5, 2026-10-27)

Each new test was checked against a temporary build with the five fixes reverted: all five failed
there (mode changed under a dialog; hue 107.8 instead of 180; isolation ended; overlap kept the
fill; header moved 659 → 733), and all pass with the fixes.

| Finding | Fix | Test |
|---|---|---|
| P1 picker interactive behind application dialogs | While `app.ui.dialog` is set the popover is drawn as a plain, non-interactable, disabled Area (no egui Modal), so the dialog and its backdrop own input. | `the_popover_takes_no_input_under_an_application_dialog` (gradient-stop popover placed away, and the modal Rectangle dialog; clicks on Wheel/Sliders/Harmony/eyedropper change nothing) |
| P2 docked panel and popover share one gesture `State` | Gesture state per instance: the popover layer keeps `state_id().with("popover")`, everything else `state_id()`; each records the frame it was drawn; a not-drawn instance's gesture is finished on its own. | `the_release_frames_movement_is_committed_with_the_docked_picker_also_showing` |
| P2 Escape with a picker dropdown open reaches tool routing | `picker::escape` returns true (the caller routes Escape no further) whenever the popover is open and Escape is pressed; with a dropdown open or a field focused it does not consume it, so egui closes that dropdown or field. | `escape_with_a_picker_dropdown_open_never_reaches_the_tools` (isolation mode survives two Escapes; the third ends it) |
| P2 overlap of the toolbar squares cannot switch target | Wells are registered in drawing order (the active square last); a click resolves the topmost well first and keeps the popover only when that is the current well and target. | `a_click_on_the_overlap_of_the_toolbar_squares_is_the_square_in_front` (both target states, after X) |
| P3 mode change moves the popover | Placement is decided for the tallest mode (`tallest`); the current height only clamps. | `switching_modes_keeps_the_popovers_header_where_it_opened` (toolbar wells near the bottom edge: beside; Properties chips: below) |

## Gate results (final code)

All cargo commands: `CARGO_TARGET_DIR=<workspace>/lanes/_target-shared`,
`-j 2 --offline --release`, tests with `--test-threads=2`, one lane, sequential.

| Gate | Result |
|---|---|
| `cargo test -p vectorcraft-ui-egui --no-fail-fast` | lib: `test result: FAILED. 2470 passed; 10 failed; 10 ignored; 0 measured; 0 filtered out` (after the review fixes; 2465 before them); other targets `ok. 2 passed` and `ok. 0 passed`. The 10 failures are exactly the known release-only set: 9 `panels::gradient::tests::*` (panic "widget drawn", debug-only widget ids) + `tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`. |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo fmt --all && git diff --check` | exit 0, no output |
| `cargo build -p vectorcraft` | exit 0 (`Finished release`) |
| Wheel capture | `<scratchpad>/picker-capture.png` |
| Gradient capture | `<scratchpad>/picker-capture-gradient.png` |

Captures (1400×1000, software raster, toolbar Fill well): 240 pt wide, frameless, no ×, no
"Color | Appearance" tabs, no "Gradient…" button; mode row = Wheel · Sliders · Harmony · Gradient +
eyedropper. The toolbar proxy sits low in the window, so the popover opens *beside* the wells
(right of them, clamped for the tallest mode), not under them; its header is at the same place
(y = 548) in Wheel and Gradient mode. Gradient mode shows thumbnail, type, Edit
Gradient, stroke placement, method, angle/aspect, ramp with stops, colour/opacity/location, dither.

No `error.log` in the worktree.

## Remaining

- Because the 9 gradient-panel tests only run in debug builds, nothing here re-checks the
  shared gradient editor (`show_body`, compact options, wrapped freeform row, narrower fields).
  A debug `cargo test panels::gradient` would need a cold debug build; none was run.
- Keyboard shortcuts other than Escape still reach the app while the popover is open (Undo works;
  so do Delete and tool keys). Only pointer input is modal.
- A press-drag outside the popover (not a click) leaves it open; the canvas ignores that drag.
- The docked Color panel and the popover share one picker state; when both show, only the
  popover takes input (egui's modal layer).
- Opening the picker with a primary click while the chip's Swatches popover is open leaves both
  open.

## Five hand-test steps (after a successful build)

1. Select a rectangle and click the toolbar Fill well. Check the frameless 240 pt picker is
   anchored to the well and clamped to the window. Drag the triangle: the fill should update live,
   and one Undo should restore the colour before the drag.
2. Open the Stroke well while the picker is open. Check the target changes. Click empty canvas:
   the picker should close without changing selection. Reopen and press Escape to close it.
3. Repeat opening through Properties → Appearance and the control-bar swatches. Keep Window →
   Color visible: each well must still open the popover, and dismissing it must preserve the panel.
4. On a solid-filled rectangle select Gradient. Check the two-stop default, edit a stop colour,
   drag its position and verify one Undo per gesture. Exercise type, angle/aspect, method, stroke
   placement and dither where supported; return to Wheel and check solid conversion and Undo.
5. Repeat with the Varos shell on and off. Check docked Color still renders the picker, Window →
   Gradient remains available, and the popover has neither a close × nor a “Gradient…” navigation
   button or “Color | Appearance” title tabs. Check the eyedropper remains usable.
