> **Status:** reference — UI audit, 2026-09-24 (input to the UI system spec).

# 01 — UI architecture and state flow

Scope: `varos-app/src/ui.rs` (read in full, 6,533 lines), `shell/` (`boxtree.rs`, `registry.rs`, `tokens.rs`), the host loop in `main.rs`, and the seams into `varos-core` (`editor.rs`, `command.rs`, `scene.rs`). Branch `claude/sweet-cerf-1sg30t` at `4821cf0`. The top bar and tab strip are audited as **being replaced by S1**, not as broken.

## Executive summary (plain English)

1. The foundation idea is right. Each frame, panels read a copy of the editor's state (`Snap`) and hand back a list of requests (`Op`). The requests are applied after drawing. The F4.1 gate holds: the UI code assigns no editor or document field directly (measured: 0).
2. That idea is not enforced, so it leaks. Two values are copied from the UI back into the editor every frame (`SetSnapConfig`, `set_constrain_wh`). Eight request kinds skip `EditCommand` and call editor methods directly. There are three separate "tell the host" mechanisms.
3. The worst bug found is the colour picker's undo transaction. It stays open across frames, but the canvas, ⌘Z, the panels and ⌘O all remain usable while the picker is open. Any of them silently replaces the picker's saved "before" state. After that, Cancel does not revert and OK makes no undo step. The change also does not mark the file as unsaved.
4. The screen scale factor exists twice: `main.rs` reads it once at launch, and egui reads the live value. After moving the window to a display with a different scale, the canvas overlays (rulers, artboard names, the measurement readout) can be misplaced. Found by reading the code, not tried in a window.
5. `ui.rs` is one 6,533-line file with 129 production functions. The 15 longest are 128–515 lines each. There are 48 fields on `Ui`, and `run()` begins by declaring 48 local variables to work around the borrow checker.
6. About 38 small drawing helpers ("widgets") are written by hand, plus many inline copies. The colour swatch with a red "none" slash is painted in 8 places, the section label in 9, and the X/Y/W/H block in 5. Two menu systems and two widget kits coexist.
7. The control bar is supposed to mirror the Properties panel (the ONE-HOME rule), but it shows different numbers. It ignores the 9-point reference point and the W/H proportion lock.
8. Renaming an artboard has three editors with three different rules for Escape and for an unchanged name. One of them adds an undo step and an unsaved `*` just for clicking into the name field and out again.
9. Only the Layers panel can be tested headlessly today, and only because its function signature happens to allow it. `Ui` cannot be built without a real window, and the shell's tests draw only dummy panels.
10. Proposed fix: split `ui.rs` by home, add one shared widget kit, give every piece of state exactly one owner, and make a panel a pure function of a snapshot. A panel built that way can be tested in a bare egui context.

**Could not verify without a window:** findings B1–B4 and B6 were traced line by line through the code, but I did not reproduce them on screen. I ran no builds or tests. Every number below was measured with grep or wc, or with a brace-matching script for function lengths.

## Measurements

| Metric | Value | How |
|---|---|---|
| `ui.rs` lines (production / tests) | 6,533 (5,912 / 621) | `wc -l`; tests start at `ui.rs:5913` |
| Functions (production incl. nested / tests) | 129 / 40; median 16 lines; 17 > 80 lines; 9 > 150 lines | brace-matching script |
| Top 15 longest | `panel_layers` 515 · `build_color_modal` 418 · `Ui::run` 297 · `build_topbar` 194 · `panel_artboard` 179 · `num_field` 176 · `build_layer_rows` 176 · `board_ctlbar` 172 · `board_rulers` 170 · `panel_properties` 144 · `Ui::new` 141 · `fill_stroke_control` 135 · `build_wheel` 133 · `build_ab_chrome` 133 · `build_splash` 128 | same |
| `#[allow]` in `ui.rs` | 16 = 8 `too_many_arguments` + 7 `dead_code` (S1 stubs) + 1 `cfg_attr(dead_code)` | grep |
| `#[allow]` in all of `varos-app` | 30, incl. 3 module-wide `#![allow(dead_code)]` (`workspace.rs:18`, `app_command.rs:13`, `lifecycle.rs:9`) + `#![allow(deprecated)]` `main.rs:2` | grep |
| `Ui` struct fields | 48 (`ui.rs:798-855`): 4 egui plumbing, 17 icon handles, ~15 panel state, 5 host in/outbox, 7 caches/shell | count |
| `run()` borrow juggling | 48 `let` in the prologue, 7 `mem::take(&mut self.…)`, 23 `self.x = x` write-backs | `ui.rs:1238-1460` |
| `Op` variants → dispatch | 48 variants: 42 → `ed.execute(EditCommand)`, 8 → direct `Editor` methods, 1 intercepted (`OpenPicker`) | `ui.rs:95-148`, `5768-5850` |
| Direct `ed.<field> =` writes, `ui.rs` production | **0** (F4.1 holds) | grep |
| Unconditional per-frame Editor writes from `run()` | 2 (`SetSnapConfig(copy)` `ui.rs:1461`; `set_constrain_wh(lock)` `ui.rs:1462`) | read |
| Editor reads in `ui.rs` production | 39 `ed.doc` refs + 18 other fields, all in `Snap::read` / `build_layer_rows` / keys / picker seed; **0 in panel bodies** | grep |
| Direct `ed.<field> =` writes, `main.rs` (non-test) | 9 (`ppu` ×4, `mods` ×4, `space` ×1) | grep |
| Mutable loop-state locals in `main.rs` | 22 (`main.rs:863-947`); `OpenDocContext{..}` literal built 6× | grep |
| `pub` items in `ui.rs` + `shell/` with ≤ 1 use | 20 (14 in `ui.rs`); 4 S1 stubs with 0 production callers (`set_tabs`, `take_app_commands`, `settle`, `document_switched`) | grep per name |
| Widget-builder fns in `ui.rs` | 38, plus a 2nd dummy kit in `registry.rs` (`icon_btn`, `fake_field`, `swatch`, `micro`, `section`) | read |
| Duplicated patterns | "none" red slash 8 sites · `checker()` 13 · section micro-label `.size(10.0).strong()` 9 · inline `egui::Frame{..}` 7 · X/Y/W/H blocks 5 · alpha-rail mesh 2 · inline `TextEdit` 6 | grep |
| `num_field` call sites / id source | 26 / `make_persistent_id(("numf", tip))`, i.e. keyed on the **tooltip text** | `ui.rs:1730` |
| Raw literals in `ui.rs` | 74 font sizes (11 distinct values), 37 `CornerRadius::same(<n>)`, 59 raw `Color32` constructors | grep |
| egui id sources | `Id::new` 9 · `make_persistent_id` 10 · `ui.id().with` 18 · temp-data `insert_temp` 10 | grep |
| UI tests | `ui.rs` 18 · `boxtree.rs` 5 (dummy bodies only) · `main.rs` 23 · `chrome.rs` 8; real panel bodies with interaction tests: 1 of 6 (Layers) | grep `#[test]` |
| `egui_tiles` confined to `boxtree.rs` | yes, CI-enforced (`tools/check_dep_directions.ps1:126`) | grep |
| Vendor-delta check (ADR-0006) in CI | **no** (`.github/workflows/ci.yml` runs dep-directions only; the `.ps1` needs `pwsh`, which is absent on the Mac) | read |

**Where state lives today.** Document: `ed.doc` (snap config, ruler origin, artboards). Editor session: tool, selection, paint focus, `recent_colors`, `show_rulers`, `guides_hidden`, `constrain_wh` (mirror), `ppu` (mirror of zoom), `mods`, `space`, and the picker transaction (`pending`). `Ui`: rail/dock visibility, box tree, reference point, W/H locks, align target, Layers collapse/search/rename/drag, artboard rename, picker modal, fake tab strings, splash, caches. `main.rs` locals: `view`, `cur_file`, `saved_rev`, window geometry, pan/space/gesture flags, cursor kind, scene cache key. Hidden in egui memory: `num_field` edit buffers, menu open flags, the double-click timestamp, the ghost position, the pill scroll position.

## Findings

### Bug

- **B1 · P1 · The picker's one-undo-step transaction breaks.** `run()` opens it with `PickerBegin` (`ui.rs:1475` → `editor.rs:3126`, `pending = doc.clone()`). The modal then pushes `PickerLive` every frame (`ui.rs:2912-2925`). But `pending` is a single slot that `begin()` overwrites without checking. A canvas click calls `pointer_down` → `begin()` (`editor.rs:3506`), and so does every panel edit (for example `set_opacity`, `editor.rs:4437`). While the modal is open, only Esc and Enter are blocked from the canvas (`main.rs:1255-1258`), so ⌘Z, ⌘O, V and Delete all still run. *Repro (by code):* select a red square → double-click Fill → drag to blue → click empty canvas → Cancel: the square stays blue. With OK instead of Cancel: `commit()` finds `pending == None` (`editor.rs:3130-3143`), so there is no undo step and `rev` is not bumped, which means no `*` appears and quitting will not warn. Variants: ⌘Z while the picker is open, then Cancel, brings back the undone state. Opening a second picker while one is open abandons the first session (`ui.rs:1464-1491`). Page-colour picker + ⌘O: the newly opened file's artboard *i* gets repainted every frame, and Cancel cannot restore it (`replace_doc` clears `pending`, `editor.rs:3211`).
- **B2 · P1 · Stale scale factor.** `main.rs:861` reads `window.scale_factor()` once and passes it as `ppp` every frame (`main.rs:936,1293`). There is no `ScaleFactorChanged` handling. The overlays divide by this `ppp` (`ui.rs:5410-5412` artboard chrome, `5753` measurement readout, `5725` origin crosshair, `5555-5567` rulers and guide drags), while `board_px` uses egui's live `out.pixels_per_point` (`ui.rs:1511-1516`). So one frame uses two different scales. *Repro:* move the window between a Retina and a 1× display → overlays land at half or double position, and guide drags map to the wrong world point.
- **B3 · P2 · The canvas stops updating during a live page-colour preview.** `ab_color_live` changes `page_color` without bumping `rev` (`editor.rs:4322-4327`). `scene_signature` hashes `rev` and `dirty` but not the artboards (`scene.rs:75-175`), while `build_scene` does read `page_color` (`scene.rs:305`). After the first live frame the cache keeps hitting, so dragging in the page-colour picker shows a stale page until OK. Anchor-only Direct selections may be affected the same way (not verified).
- **B4 · P2 · Escape in a picker field cancels the whole picker.** The modal reads `key_pressed(Escape)` globally (`ui.rs:2898-2905`), but Enter is guarded by "no field focused" (`ui.rs:2906`). Pressing Escape to leave the hex or H field cancels the whole colour session.
- **B5 · P3 · The picker shows internal ids as tooltips.** `num_field` uses `tip` both as the id key and as the tooltip (`ui.rs:1730,1859-1862`). Hovering the picker's A, H, S, B, R, G or B field shows "cm-a", "cm-h" and so on (`ui.rs:2803,2816-2818,2839`).
- **B6 · P2 · Unchanged artboard rename marks the file dirty.** `name_field` commits on every blur (`ui.rs:5153-5156`), and `ab_rename` always sets `dirty` (`editor.rs:2214-2224`). Click into the Artboard name field and click out → an undo step and a `*` appear with nothing changed.
- **B7 · P3 · Unstable ids in the Layers panel.** The eye/lock ids come from screen coordinates (`ui.rs:4184`, `("col", left, top)`), so a click that spans a wheel-scroll is lost. The disclosure id `("disc", row.id)` (`ui.rs:4454`) is not salted by section, so a straddling object's mirror rows reuse the same id.

### Inconsistency (breaks a law or convention)

- **I1 · P2 · The control bar does not mirror its home.** In the control bar, X/Y always use reference (0,0) (`ui.rs:3794-3805`), while Properties uses the 9-point reference point (`ui.rs:4777-4803`). The W/H proportion lock applies only in Properties (`ui.rs:4784-4801`). With the reference point at the centre, the two show different X for the same object.
- **I2 · P2 · One action, three editors.** An artboard can be renamed in the inspector (`ui.rs:5125`; Esc and blur commit), in the on-canvas chrome (`ui.rs:5431-5445`; Esc commits, and it re-requests focus every frame, which is the exact pattern QW3 removed from Layers), or in the Layers board header (`ui.rs:4505-4545`; Esc cancels, empty or unchanged is ignored).
- **I3 · P2 · The no-animation law is broken in the shell.** The drag ghost eases toward the cursor with a per-frame lerp plus `request_repaint` (`boxtree.rs:117-120`), and the fork adds a 0.25 s post-dock glide plus smoothing (`vendor/egui_tiles/src/tree.rs:334,504-509,558-601,889`). Both were owner-approved on 07-05 but contradict CLAUDE.md "No animations". This needs an owner ruling.
- **I4 · P3 · Two popup systems.** The hand-rolled `menu_below` (`ui.rs:1596-1660`) coexists with egui's `ui.menu_button` / `ui.button`, which use the default egui look, for the box "☰ change panel" menu (`boxtree.rs:496-501`).
- **I5 · P3 · Nested scroll areas.** The Properties and Artboard bodies create their own `ScrollArea` (`ui.rs:4749,5214`) inside the box's `hostbody` ScrollArea (`boxtree.rs:433`), even though `registry.rs:46-48` warns that nested scrolls fight over the wheel.
- **I6 · P3 · Code contradicts its comment.** The ⋮ artboard menu is documented as "the ungated way to edit a page from ANY tool" (`ui.rs:5384-5386`) but is `.interactable(tool_ab)` (`ui.rs:5468`). The file header says "one light GPU shadow" (`ui.rs:4`), which contradicts the no-shadow rule.

### Glue (special cases, duplicates, dead paths)

- **G1 · P2 · Widget duplication** (see Measurements): 10 icon-button builders (`icon_button`, `icon_btn`, `icon_toggle`, `topbtn`, `mini_btn`, `eyedropper_btn`, `pf_btn`, `col_toggle`, `winctl`, `shape_slot`) plus an inline `fbtn`. 6 text/segment buttons plus 3 inline pill groups (`ui.rs:2316-2337,2442-2488,2544-2565`). 5 row types. 4 separators plus 2 inline. The swatch is painted in 8 places (`ui.rs:2099,2714,3910,3939,3984,4005,5317` …). The X/Y/W/H block is repeated 5 times (`ui.rs:3754-3769,3791-3806,3858-3873,4768-4806,5267-5311`).
- **G2 · P3 · Dead parameters and placeholders.** `num_field`'s `_step` is ignored at all 26 call sites (`ui.rs:1697`). `let _ = col;` (`ui.rs:5339`). `build_splash`'s `_e` and its `ca = 1.0` alpha plumbing (`ui.rs:2972-2975`).
- **G3 · P3 · Sandbox leftovers ship in the app.** `registry.rs:61-312` has dummy bodies (Swatches/History/Assets, fake widgets) and `boxtree.rs` has `draw_board`/`draw_hands`. `lib.rs:3-4` still names a `shell-sandbox` binary that no longer exists (no `src/bin`).
- **G4 · P3 · Pure logic inside the UI file.** Colour math and harmony (`ui.rs:185-238,1950-2018,2139-2196`) and the icon SVG data (`ui.rs:29-92`) belong elsewhere. `install_fonts` reads only `C:/Windows/Fonts` (`ui.rs:1547-1560`), which is a no-op on the Mac, the official platform.

### Architecture (state ownership, coupling, testability)

- **A1 · P1 · No single owner for mirrored state.** The W/H lock lives in `Ui.lock` but is copied into `ed.constrain_wh` every frame (`ui.rs:1462`). The snap config is copied out at frame start and written back every frame (`ui.rs:1297,1461`), which only works because of an ordering comment (`ui.rs:5844`). Zoom lives in `view.zoom` and is copied into `ed.ppu` from 4 places (`main.rs:935,1121,1192,1289`). The tab label is pushed in from `main.rs:1299`, while the fake tab strip mutates its own copy (`ui.rs:3445-3461`).
- **A2 · P1 · Multi-frame edits are not modelled.** The picker, and potentially field scrubs, span frames but are just `Option<ColorModal>` plus a core `pending` slot. There is no "session" type that settles itself when another command arrives. `Ui::settle` exists (`ui.rs:1187`) but is unwired.
- **A3 · P2 · Three outboxes.** `pub fit_request` (`ui.rs:822`), `pub win_action` (`ui.rs:824`) and `app_cmds` + `take_app_commands` (`ui.rs:836,1180`). There are also 4 command surfaces: `apply_key` (`main.rs:198`), the native menu table (`chrome.rs:200`), the egui menus (`ui.rs:3468-3528`), and `Op`. Nothing in the code ties them to one registry. The ONE-HOME rule is unenforced.
- **A4 · P2 · Panels have global side effects.** `build_topbar` writes Win32 caption exclusions (`ui.rs:3538`). The modal polls the global cursor and mouse button (`ui.rs:2886-2896`). `ui.rs` depends on the binary root (`crate::tool_name`, `ui.rs:3880`). `Ui::new` needs a `winit::Window` and `egui_winit::State` (`ui.rs:958,1047`). Together these make `Ui` impossible to build in a test.
- **A5 · P2 · Panel bodies are re-entered by the drag ghost.** `render_drag_ghost` runs the real host a second time in the same frame (`boxtree.rs:338`). This is safe today only because widgets are disabled. It is an unwritten contract: a panel that reads raw input would double-fire.
- **A6 · P2 · Per-frame cost grows with the document.** `layer_rows_key` hashes every node name and sorts the selection every frame (`ui.rs:322-361`), and rebuilds all rows on every drag frame (`ui.rs:356-359`). The Layers list is not virtualized (`ui.rs:4309` uses `.show`; `show_rows` count 0), and thumbnails re-allocate `Vec<Pos2>` per ring per row per frame, filled with `convex_polygon`, which is wrong for concave shapes (`ui.rs:4490-4495`). `Snap::read` scans for document colours every frame (`ui.rs:716`). Not profiled.
- **A7 · P3 · Per-document UI state is not per-document.** Layers collapse/search/rename, the picker and the caches sit on `Ui`, and `document_switched` (`ui.rs:1197`) is unwired.

### Missing rule

- **M1** No rule on which layer (Document / Session / UI / Host) owns a value. Hence A1.
- **M2** No rule that a transaction is exclusive (`begin()` silently replaces `pending`). Hence B1.
- **M3** No rule that live previews must invalidate the scene cache (the signature is maintained by hand). Hence B3.
- **M4** No widget-id rule (ids come from tooltip text or screen coordinates). Hence B5 and B7.
- **M5** No keyboard-precedence rule (field → menu → modal → canvas). Hence B4 and the ⌘Z-during-picker path.
- **M6** No "no dead control" rule: New/Open/Save/Export rows ignore their return value (`ui.rs:3470-3474`), and Share, Export and the search pill do nothing (`ui.rs:3410-3414`). S1 is replacing these.
- **M7** No third command category. Tool, selection and view preferences are neither `EditCommand` nor `AppCommand` (ADR-0002 names only two), so 8 `Op`s bypass both.

## What S1 must leave behind (top bar / tabs / host)

Delete `tabs`, `tab_active`, `set_doc_tab` and the fake `+` / `×` handlers (`ui.rs:827-828,1162-1168,3433-3462`), so the strip draws only `TabView`s. Remove all 7 S1 `dead_code` allows. Route New/Open/Save/Export, `WinAction` and `fit_request` through one `AppCommand` queue. Build `OpenDocContext` once. Wire `settle()` before **every** lifecycle command, including the existing ⌘O path (see B1). Wire `document_switched()`, and move `view` from a `main.rs` local into the session.

## Proposed module split (`ui.rs` → `varos_app::ui`, moved into the lib crate)

```
ui/mod.rs        winit adapter only: Ui::new/on_event/run → calls UiFrame (≤250 lines)
ui/frame.rs      UiFrame (headless): frame(RawInput, &FrameSnap, &mut UiState) -> FrameOut{intents, platform}
ui/snap.rs       FrameSnap{Snap, AbSnap, AbInfo, LRow} + row/thumb caches — the ONLY file that names `Editor`
ui/intent.rs     Intent → EditCommand | SessionCommand | AppCommand (replaces Op + apply_ops)
ui/state.rs      UiState{ chrome, per-doc DocUiState{layers, picker session, rename} }
ui/icons.rs      IC_* data + IconSet (one struct; IconSet::none() for tests)
ui/kit/          field (num/name/text, one commit/cancel rule) · button (Icon{Rail|Bar|Inline}, Text{Primary|Secondary|Ghost}, Segmented)
                 · swatch · row{Info|Action|Toggle|Menu|Check} · menu (one popup system) · layout (section label, separators, frames)
ui/homes/        transform · appearance · align · pathfinder · layers/{header,row,dnd} · artboard · document  (each Home(ui, snap, state, Density))
ui/hands/        rail · ctlbar (mirrors = homes called with Density::Compact; no own controls)
ui/overlays/     rulers · ab_chrome · snap_hud · origin  (take CanvasXform{view, ppp_from_egui, hole})
ui/picker/       modal · plane · wheel (+ colour math → pure module with tests)
ui/chrome/       topbar (S1-owned) · statusbar · splash
```
Order: land S1 first, then do move-only commits one module at a time behind green gates (no behaviour change per commit), then deduplicate into `kit/`, then add `Density` mirrors.

## Rules this suggests

1. Keep `Editor` out of panels: only `ui/snap.rs` may name `Editor`, and a panel reads one immutable `FrameSnap` built once per frame (test: grep `Editor` in `ui/homes`, `ui/kit`, `ui/hands` = 0).
2. Send every UI mutation as one `Intent` that dispatches to exactly one of `EditCommand`, `SessionCommand`, or `AppCommand`, so no UI file calls an `Editor` method (test: grep `ed\.` outside `intent.rs` = 0).
3. Give every value exactly one owner (Document, Session, UI, or Host) and never copy it into another owner each frame (test: `run()` performs no unconditional writes).
4. Treat a multi-frame edit as an explicit session: any other command settles the open session first, and `Editor::begin` asserts that no transaction is open.
5. Make every uncommitted live mutation bump a `live_rev` that the scene cache keys on.
6. Give every stateful widget an explicit, stable id from its domain key, and never derive it from display text, tooltips, or screen coordinates.
7. Paint controls only in `ui/kit`: homes and hands use no raw colour, radius, size, or font literal (test: grep literals outside `kit/` and `tokens.rs` = 0).
8. Implement a home once and mirror it with a `Density`, so the control bar and menus show the same values and obey the same locks as the home.
9. Write each panel body as a pure function of (snapshot, its own state, egui input) that is safe to run twice per frame and never touches OS globals.
10. Let host data in only through `FrameInput` (view, scale factor from egui, tabs), and let effects out only through one `AppCommand` queue.
11. Keep per-document UI state in a `DocUiState` keyed by session id, and swap it on tab change.
12. Settle keys innermost-first (focused field → open menu → modal → canvas); a surface acts on Esc or Enter only if nothing inner consumed it.
13. Draw no control without a handler: a registry test checks every menu row and bar button against a command id.
14. Use no time-based easing or per-frame lerp in UI code (subject to the owner's I3 ruling).
15. Virtualize every list with unbounded rows (`show_rows`).

## What makes a panel headless-testable

A panel can be tested headlessly if all of the following hold. Its signature is `fn home(ui, &XSnap, &mut XState, &mut Vec<Intent>)`. Its snapshot is a plain struct a test can build without an `Editor` (`Default` plus field literals). Its icons are optional (`IconSet::none()`). Its ids are stable so a test can aim at a widget. It makes no OS, clipboard or caption calls. And there is a `UiFrame` that runs from `egui::RawInput` alone, with no `Window`. The Layers rename tests (`ui.rs:6137-6533`: a bare `egui::Context` with synthetic events) are the proof and the template. Today 1 of the 6 real panel bodies meets these conditions. `boxtree`'s tests render only the dummies (`boxtree.rs:1033-1040`). `egui_kittest`, to find widgets by label, is an option but would be a new dev dependency, which needs owner approval.

## What to fix first (top 10)

| # | Fix | Size |
|---|---|---|
| 1 | Picker transaction integrity: settle on any foreign command, ⌘O and a second open; debug-assert non-nesting `begin`; add a test for click-canvas-then-Cancel (B1, A2, M2) | M |
| 2 | One scale factor: take `ppp` from egui (`ctx.pixels_per_point`), drop `main.rs:861`'s `scale` (B2) | S |
| 3 | Add `live_rev` to `scene_signature` for page-colour and paint previews (B3, M3) | S |
| 4 | Remove the per-frame write-backs: the W/H lock goes into the session, and snap edits become intents (A1) | S |
| 5 | `num_field` takes an explicit id plus a separate tooltip; fix the Layers id salts (B5, B7, M4) | S |
| 6 | One text-commit rule in `kit/field`, used by all three artboard renames; skip no-op renames (B6, I2) | S |
| 7 | Extract `kit/` (swatch, icon button, rows, separators, section label, frames) and make the control bar a `Density::Compact` transform/appearance home (G1, I1) | M |
| 8 | Keyboard precedence (Esc in fields; block or settle ⌘Z while the modal is open) (B4, M5) | S |
| 9 | Module split plus `UiFrame` headless core in the lib crate, after S1 lands (A3, A4, A5) | L |
| 10 | Virtualize the Layers list; cache thumbnail points; replace the per-frame node-name hash with `rev` plus a UI key (A6) | M |

Also queue: an owner ruling on the shell animations (I3); `check_vendor_patches` in CI (ADR-0006); deleting the sandbox dummies and the stale `lib.rs` doc (G3).
