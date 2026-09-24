> **Status:** reference — UI audit, 2026-09-24 (input to the UI system spec).

# 04 — Input handling, focus and feel

Auditor scope: every path a key, click, wheel or menu row takes from winit to the `Editor`, plus focus, modifiers, thresholds and the feel tests. Branch `claude/sweet-cerf-1sg30t` @ `4821cf0`. Read-only: no code edited, nothing built or run. QW5 (`feat/qw5-edit-menu-rows`) is reviewed but **not merged** here, so it is counted as "coming", not as present.

## Executive summary (plain English)

1. The keyboard reaches the canvas in three ways (key press, Mac menu row, key forwarded to a text field), and each works out the modifiers in its own way.
2. There is no single key table. ⌘S = Save lives in 5 places in the code. V = Selection lives in 4 places, with 2 different names.
3. Menu rows pretend to be keystrokes. Clicking View ▸ Rulers while a text field is focused does nothing. Our own spec forbids this.
4. Pressing **Tab** once can silently turn off every canvas shortcut (V, A, P, Delete, ⌘Z) until you press Esc or click. egui gives the focus to a button, and the app treats any focused widget as "typing".
5. **Probable P1 (from reading the code, not yet seen in the window):** after any mouse click inside the colour picker, **Cancel no longer undoes the colour**. The reason: every mouse release goes to the canvas, even when the press happened on a panel.
6. The drag threshold is measured in world units, not screen pixels. At 4000 % you must drag 160 px before a pen handle appears. At 5 % every click becomes a drag. Object moves have no threshold at all.
7. All canvas sizes (hit radii, handles, snap reach, double-click reach, wheel step) are in device pixels. On a Retina Mac, the platform we ship, they are all **half size**.
8. There is no trackpad pinch-zoom. Two-finger pan moves the canvas at 0.75× finger speed.
9. Escape can do two jobs with one press: it closes a menu **and** clears the selection. There are 4 different double-click timings. Mac shows "Alt" and "Shift+O" where it should show ⌥ and ⇧.
10. Only small pure helpers have tests. The routing itself (focus gate, release routing, wheel, Space-pan, double-click) is a 490-line closure in `main.rs` with no headless coverage.

## Measurements (counted in the source)

| What | Number | Where |
|---|---|---|
| Places that encode ⌘S = Save | 5 (menu table, shortcut arm, dead burger label, `egui_key`, `muda_code`) | `chrome.rs:241`, `main.rs:670`, `ui.rs:3472`, `chrome.rs:334`, `mac_menu.rs:25` |
| Places that encode V = Selection | 4, two spellings ("Select (V)" / "Selection (V)") | `main.rs:174,245`, `ui.rs:965,3617` |
| KeyCode translation tables | 3 (`egui_key` 19 keys, `muda_code` 19 keys, `format!("{:?}")` strings) | `chrome.rs:326-350`, `mac_menu.rs:16-40`, `main.rs:665` |
| `apply_key` arms (⌘ / plain) + `shortcut` arms | 14 + 17 + 4 | `main.rs:198-271`, `664-690` |
| Native menu rows with an accelerator | 26, every one a synthetic `MenuCmd::Key` except Quit/Close | `chrome.rs:230-305` |
| Double-click definitions | 4 (350 ms/6 device px · 350 ms/4 pt · 300 ms/6 pt egui · 400 ms same-row) | `main.rs:1185-1190`, `mac_caption.rs:16-18`, egui default, `ui.rs:4600-4606` |
| Sources of modifier truth | 3 (`ed.mods`, egui `i.modifiers`, menu hard-codes ⌘=true) | `main.rs:1126-1131`, `ui.rs:1755-1838`, `main.rs:969` |
| Native dialog sites / sites that reset `ed.mods` after | 8 / 3 | dialogs `main.rs:476,486,582,601,618,653,682,727`; resets `633,674,686` |
| Sources of scale-factor truth | 2 (startup `scale` vs live `pixels_per_point`) | `main.rs:861` vs `ui.rs:1511-1515` |
| Platform `cfg` branches in `main.rs` | 33 (28 macOS, 5 Windows), mostly inside the event closure | `main.rs:950-1437` |
| Event-loop closure length | ≈ 490 lines, 0 headless tests of its routing | `main.rs:950-1437` |
| Tab stops (focusable `Sense::click()` widgets) | 41 in `ui.rs`, plus every TextEdit | `grep Sense::click()` |
| Canvas px constants read as **device** px | 11 (`DRAG_THRESH`* `CLOSE_R ANCHOR_R HANDLE_R EDGE_R AB_HANDLE_R radius_px` 7.0 22.0, dbl 6.0, wheel 30) | `editor.rs:11-17,774,794`, `model.rs:459`, `main.rs:1189,1241` |
| Feel tests | core `input_feel` 10, `hit_thick_stroke` 16; app `main.rs` 23, `chrome.rs` 8, `mac_caption` 3, `ui.rs` rename 9 + characterization 3 | — |

\* `DRAG_THRESH` is worse: it is not even divided by `ppu` (B2).

## Findings

### Bug

- **B1 · P1 · Colour-picker Cancel can stop reverting (release is routed to the canvas).** `main.rs:1201-1211` calls `ed.pointer_up()` on **every** left release. That includes releases whose press was over a panel and returned early (`main.rs:1176-1179`). `pointer_up` ends in `commit()` (`editor.rs:3573`, `3130-3144`). Meanwhile the picker holds the same `pending` slot opened by `PickerBegin` (`ui.rs:1475`, `editor.rs:4300`), and `PickerLive` marks it dirty every frame. **Repro (predicted):** select a shape → double-click Fill → click once in the colour field → click Cancel. The first release commits the picker's snapshot, so `picker_cancel` finds `pending = None` (`editor.rs:4341-4346`) and the colour stays. OK only works by accident. The same collision happens with Delete, ⌘Z or a canvas click while the picker is open: `main.rs:1255-1257` gates only Esc/Enter, which contradicts `ui.rs:1152` ("canvas shortcuts must be fully gated off"). Status: code reading only. A hand test takes 1 minute.
- **B2 · P1 · The drag threshold is in world units.** `DRAG_THRESH = 4.0` (`editor.rs:11`) is compared with world distances at `editor.rs:3627,3651,3831,3966,4017` (pen handle, pen close, Alt-duplicate, Rotate/Scale tool, Convert). At zoom 40 you need 160 device px. At 0.05 you need 0.2 px, so a click to set the Rotate pivot or place a corner point becomes a drag. The size filters have the same fault: a shape under 2 world units is deleted (`editor.rs:3548`), which is a 79 px shape at 4000 %; a page under 4 units is dropped (`editor.rs:2172`). The only test runs at zoom 1 (`tests/layers.rs:223`).
- **B3 · P2 · There is no threshold at all for moving an object or a page.** `Drag::Object` moves on any delta and then snaps within 8 px (`editor.rs:3864-3875`). An artboard counts as moved at `> 0.001` world units (`editor.rs:2090`). Result: a click with a little mouse jitter can move the object, snap-jump it, and add an undo step plus a dirty dot. Unverified in the window.
- **B4 · P1 · The Tab key is a focus trap.** egui 0.35 gives the focus to the first focusable widget when Tab is pressed and nothing is focused (`egui memory/mod.rs:580,656-662`). `wants_keyboard()` is `egui_wants_keyboard_input()`, which means *any* focused widget (`ui.rs:1149-1151`), even though its doc says "text field". From then on, KeyboardInput is dropped (`main.rs:1253`) and Mac menu ⌘-rows are forwarded to egui and lost (`main.rs:961-965`). So V, A, P, Delete, arrows, ⌘Z and even **clicking Edit ▸ Undo** do nothing until Esc or a click. Tabbing out of the last number field lands in the same state. egui's own `text_edit_focused()` (`context.rs:2889`) is the check the comment describes.
- **B5 · P2 · Escape and Delete fire twice.** An open popup (`menu_below`, `ui.rs:1652`) closes on Escape. The same key also reaches `apply_key` → `ed.escape()` (`main.rs:267`), which drops the whole selection. PAINS_LOG P7 records the first half as intended ("main.rs feeds egui first"). Delete and ⌘Z with a menu open also act on the canvas. A click outside a menu both closes it and presses the canvas.
- **B6 · P2 · The scale factor goes stale when the window moves to another display.** `scale` is read once (`main.rs:861`) and passed as `ppp` every frame (`main.rs:1293`) to the rulers, guide drag-out, zero point, snap HUD and page labels (`ui.rs:1349,1418,1428-1429,5412,5555-5654`). `board_px` uses the live value (`ui.rs:1511-1515`), and there is no `ScaleFactorChanged` arm. On a Retina Mac with a 1× monitor, these overlays would sit 2× off. Unverified.
- **B7 · P2 · Modifiers go stale or get stuck.** The reset after a native dialog clears `ed.mods` only. It runs at 3 of the 8 dialog sites; the OS hand-off `open_path` (`main.rs:637` → `486`) is missed. It leaves `space_down` and `panning` alone. A ⌘ that is still held reads as released, so ⌘O → Cancel → S picks the Scale tool. On macOS, winit resets modifiers when the window loses key status (`winit window_delegate.rs:225`), but Space is not a modifier. The `Focused(false)` arm (`main.rs:1047-1054`) resets nothing, so ⌘-Tab away while holding Space can leave the hand stuck.
- **B8 · P3 · Space-pan beats chrome.** `space_down` is checked before `over_panel` (`main.rs:1144`). Space + click on a panel button therefore pans the canvas *and* clicks the button, because egui was fed first.
- **B9 · P3 · Leftovers when the pointer leaves the canvas.** `hover_path` updates only when `!over_panel` (`main.rs:1119`), so the hover highlight stays when the pointer moves onto chrome. During a canvas drag over a panel, the cursor switches to the chrome arrow, because `resolve_ck` ignores `canvas_gesture` (`main.rs:1368`).
- **B10 · P3 · Escape or a tool key during a gesture does not cancel it.** `escape()` / `set_tool()` set `drag = None` (`editor.rs:4035-4093`), but the edit so far stays in the document and the next release commits it. `event.repeat` is never checked (`main.rs:1266`), so holding ⌘O or ⌘S can queue dialogs.

### Inconsistency

- **I1 · P2 · Menus are synthetic keys.** `MenuCmd::Key(Accel)` (`chrome.rs:155`) sends ⌘-keystrokes through `shortcut(…, true, …)` (`main.rs:960-977`). This breaks `docs/specs/DOCUMENT_FILE_SYSTEM.md:185`: "Menus and physical keys dispatch once through command IDs, not synthetic key events." Native rows are always enabled (`mac_menu.rs:143-146`), which breaks the same line of the spec ("enabled state mirrors command availability"). S1 fixes the **File** rows only.
- **I2 · P2 · Four double-click definitions** (see the measurements). None of them reads the OS double-click interval. The canvas one is in device px, the caption one in points. The Mac caption always zooms (`main.rs:1164`) and ignores the user's "double-click title bar" system setting.
- **I3 · P2 · Key hints are not from one table and are not platform-aware.** "Select (V)" (window title) vs "Selection (V)" (rail). "Direct Select (A)" vs "Direct Selection (A)". "Shift+O" (`ui.rs:967`, `main.rs:181`) and "Alt" (`ui.rs:3621`) appear on Mac. Only `shortcut_label` (`tokens.rs:16`) knows the platform, and only for ⌘ combos.
- **I4 · P2 · Step modifiers differ on every surface.** Canvas nudge: 1 / Shift 10 (`main.rs:243`). Field arrows: 1 / 10 / physical-Ctrl 0.1 (`ui.rs:1755-1771`). Field scrub: ×5 / physical-Ctrl ×0.2 (`ui.rs:1831-1838`). Everywhere else Ctrl ≡ ⌘ (`main.rs:1130`, `ui.rs:4570`). On a Mac, Ctrl+↑/↓ is Mission Control by default, so the fine step is probably out of reach (unverified).
- **I5 · P3 · Windows vs Mac run different code for the same key.** On Mac a ⌘-row goes menu → synthetic key → `shortcut`; on Windows it goes KeyboardInput → `shortcut`. Ctrl+Q and Ctrl+W do nothing on Windows (Quit exists only as `MenuCmd::Quit`). On Windows, Undo, Redo, Arrange, Ungroup, Transform Again, Copy and Paste have **no pointer path** (no menu bar; the burger rows `ui.rs:3470-3474` are dead). ⌘S inside a focused field does nothing on either platform today.
- **I6 · P3 · Enter = deselect everything** (`main.rs:267`). Illustrator uses Enter to end a path or confirm, not to clear the selection.

### Glue

- **G1 · P2 · Dispatch is stringly typed.** `format!("{:?}", code)` (`main.rs:665`) is matched against string literals in `apply_key`. A typo such as `"Keyv"` compiles and silently does nothing. The tests go through the same Debug strings.
- **G2 · P2 · Debug file writes in the frame loop.** Every cursor change writes `target/cursor-debug.txt` synchronously (`main.rs:1382-1388`), plus a startup file (`main.rs:880`). The path is relative to the working directory.
- **G3 · P3 · Magic numbers.** Space-click zoom repeats `1.5` instead of `ZOOM_KEY_STEP` (`main.rs:1149`). Wheel uses `/40.0` and `*30.0` (`main.rs:1234-1244`). Transform handle `7.0` and rotate ring `22.0` are raw literals (`editor.rs:774,794`). Double-click uses `350` and `6.0` inline.
- **G4 · P3 · The same toggle is written 2–3 times.** Smart Guides is flipped by `EditCommand` (`command.rs:206`), by `toggle_smart_guides` (`ui.rs:3542`), and a test pins the two equal (`ui.rs:6124`). The snap rows exist twice (`main.rs:296` and the magnet menu `ui.rs:3500-3507`). Tool switching has two vocabularies, `Op::Tool` (48-variant `Op`, `ui.rs:95-150`) and `apply_key`.

### Architecture

- **A1 · P1 · One `pending` history slot is shared** by the picker session, canvas gestures and key commands (`editor.rs:3126-3144`). A "floating palette that leaves the canvas live" (`main.rs:1250-1252`) cannot work on top of one slot. This is the root of B1.
- **A2 · P1 · There is no pointer capture.** Presses are routed on `over_panel`, but releases are not (`main.rs:1201-1211`). Whoever received a press must receive its release.
- **A3 · P2 · The routing cannot be tested.** Focus gate, modal gate, eyedropper swallow, Space-pan, wheel mapping, double-click detection, release routing and `canvas_gesture` all live inline in the winit closure, with 33 platform `cfg`s. Only helpers (`resolve_ck`, `zoom_step`, `apply_key`, `caption_double_click`) are pure and tested.
- **A4 · P2 · Nobody owns "who has the keyboard now".** The state is spread over egui focus (last frame), `color_modal`, popup temp-data, `space_down`, `panning` and `canvas_gesture`, all separate booleans. Esc and Enter are handled by up to four independent checks (`main.rs:1253-1257`, `ui.rs:1652,2898,2906,4524`).

### Missing rule

- **M1 · P1 · No rule for canvas pixel units.** The pointer is physical (`main.rs:1112`), `ed.ppu = view.zoom`, and the overlay uses `size_scale 1.0` (`render lib.rs:949,1112`), so every "screen px" constant is really a **device** px. On a 2× Retina display: edge reach 8 → 4 pt, anchor 12 → 6 pt, transform handle 7 → 3.5 pt, snap 8 → 4 pt, canvas double-click 6 → 3 pt, wheel 30 → 15 pt per notch. Marks and hits shrink together, so they still agree, but both are half the designed size. The seam already exists (`build_fg(size_scale)`) and is unused. The "full Retina hover sweep" is still open in Astra §98.
- **M2 · P2 · No gesture vocabulary for Mac.** Pinch, rotate and smart-zoom are not handled (0 hits for `PinchGesture`). Trackpad `PixelDelta` goes through the notch mapping at 0.75× (`main.rs:1234`). Right-click on the canvas does nothing (`main.rs:1221`, `_ => {}`), while Layers rows have a context menu.
- **M3 · P2 · No Escape/Enter ladder** (field → popup → modal → gesture → selection) and no Tab rule. See B4, B5 and B10.
- **M4 · P2 · No rule that every action has a command id and a visible home.** Key-only actions: Arrange ×4, Ungroup, Transform Again, Lock Guides, Actual Size (Mac menu only). Button-only actions with no key: Flip, Align ×6, Distribute, Pathfinder, page add/duplicate/delete, units, snap master, Triangle/Polygon/Convert tools.

## Already being fixed — not counted above

- **S1 (DFS_S1 §3.5/§3.6, S1-C/S1-D):** ⌘N (F3); ⌘W = Close Tab instead of Quit (F5); File rows become `MenuCmd::File(FileCmd)` command ids, which bypass `forward_shortcut`; pure `lifecycle_key()` for N/O/S/⇧S/W/Ctrl+Tab; one `dispatch` for keys, menu, burger, tab strip, `WinAction`, `CloseRequested`, OS hand-off and `file_arg`; `mods` reset after **every** lifecycle command (this covers the `open_path` gap in B7 for dialogs); `canvas_gesture` / `panning` reset on tab switch; burger rows get wired; Export/Share disabled; ⌘K keycap removed (QW7); `OpenDocContext`/`shortcut` deleted. The top bar and tab strip are audited as "being replaced", not as broken.
- **QW5 (branch, reviewed, not merged):** ⌘A / ⇧⌘A in `apply_key`; Edit ▸ Select All / Deselect / Delete; `MenuCmd::Plain` Delete row with no key equivalent, guarded by `wants_keyboard`. Note: that guard inherits B4.
- **QW1 (merged `b9a8b2c`):** `EDGE_R / ppu` for Pen add-anchor, `pen_hint` and Convert; painted-band hit; rotate ring no longer blocked by the selection's own band. B2 and M1 are what QW1 left untouched (`DRAG_THRESH`, size filters, device px).
- **Astra F05/F06 (merged):** ⌘± / ⌘1 zoom the canvas around the centre; UI keyboard zoom disabled (`ui.rs:1565`, tested).

**What S1 should leave behind for the input contract:**
1. Route `lifecycle_key` **before** the `wants_keyboard` gate on the keyboard path. Otherwise Ctrl+S in a field works on Mac (through the AppKit menu) but not on Windows.
2. Make `FileCmd` the template for `EditCmd`, `ObjectCmd` and `ViewCmd`, not a one-off.
3. Keep `dispatch` pure enough that a test can drive it without an `EventLoop`.

## Rules this suggests (the INPUT CONTRACT)

1. **One key table.** Each `(physical key, modifiers, platform)` maps to exactly one `CommandId` in one const table. Menus, tooltips, the status bar and the window title render their hints from that table, and a test asserts that no string literal key hint remains.
2. **One dispatch.** Keys, menu rows, buttons and gestures all produce a `CommandId` that runs through one `dispatch(cmd, ctx)`. A menu row never sends a synthetic keystroke (spec DOCUMENT_FILE_SYSTEM §4).
3. **Every action has a command id and at least one visible home.** A test walks the `CommandId` enum and fails when a command has no key-table row or no menu/button home (a documented exception list is allowed).
4. **The focus rule.** Canvas shortcuts yield only while a **text** editor has focus (`text_edit_focused`). Tab and arrow focus traversal is off unless a field is focused. The native menu's command ids never yield to focus.
5. **The Escape ladder.** One Escape (and one Enter) goes to exactly one owner, in this order: text field → open popup → modal/palette → live gesture (cancel = restore `pending`) → selection. A test presses Esc once per state and asserts that exactly one layer changed.
6. **Pointer capture.** The surface that received a press receives its moves and its release. A release never reaches the `Editor` unless the press did.
7. **Transactions do not nest silently.** While a modal edit session (the picker) holds `pending`, any other command either waits, commits that session first, or is refused. It never overwrites `pending`.
8. **Screen-px means logical px.** Every canvas tolerance, mark size, threshold and wheel step is written in logical px and converted with the live scale factor and zoom in exactly one function. A test runs the same gesture at scale 1 and 2 and at zoom 0.05, 1 and 40 and expects the same result.
9. **One drag threshold.** Press → drag starts only after `DRAG_THRESH` logical px of screen travel. This covers all tools, pages and panels, and a sub-threshold release counts as a click.
10. **One double-click.** The interval comes from the OS (falling back to 500 ms), the radius is in logical px, and one helper serves the canvas, the caption, Layers and the swatches.
11. **The modifier reset rule.** Focus loss, and every native dialog, clears `mods`, Space, pan and gesture state in one `reset_transient_input()`; the next `ModifiersChanged` restores the truth.
12. **The platform seam.** Platform differences in input (menu bar, caption, gestures, modifier names) live behind one `platform::input` module. The event loop holds no `cfg(target_os)` branch.
13. **Routing is a pure function.** `route(event, InputState, FocusSnapshot) -> Vec<Action>` is headless-testable, and every gesture in this audit has a test through it.

## What to fix first

| # | Fix | Size |
|---|---|---|
| 1 | Pointer capture: give releases only to the canvas when the press went there (A2). This closes B1's main path. Add a headless test: picker Cancel after a click reverts. | S |
| 2 | Gate or settle canvas commands while the picker holds `pending` (A1/B1), or give the picker its own session slot | M |
| 3 | `wants_keyboard` → `text_edit_focused`; disable egui Tab/arrow focus traversal when no field is focused (B4) | S |
| 4 | Divide `DRAG_THRESH` and the size filters by `ppu`; add a threshold to Object/page drags (B2/B3); add tests at zoom 0.05 and 40 | S |
| 5 | Logical-px rule: feed the live scale factor into `ed.ppu`/`size_scale`/tolerances; drop the stale `scale`; handle `ScaleFactorChanged` (M1/B6) | M |
| 6 | Escape ladder + gesture cancel restores `pending` (M3/B5/B10) | M |
| 7 | One key table + `CommandId` for Edit/Object/View rows, extending S1's `FileCmd`; delete the Debug-string matching (I1/G1/I3) | L |
| 8 | `reset_transient_input()` on focus loss and on every dialog (B7) | S |
| 9 | Trackpad: `PinchGesture` zoom at the cursor, `PixelDelta` 1:1 pan (M2) | S |
| 10 | Move routing out of the closure into a pure `route()` behind a platform seam, and remove the frame-loop debug writes (A3/G2) | L |

Could not verify without a window: B1, B3, B6, B7 (Mac Space), I2 (system setting), I4 (Mission Control), and M1's visual size. Each finding above gives a one-line repro to run by hand.
