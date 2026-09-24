> **Status:** reference — UI audit, 2026-09-24 (input to the UI system spec).
# UI audit 03 — box system, panels, docking and the ONE-HOME rule

Branch `claude/sweet-cerf-1sg30t` @ `4821cf0`. Read-only audit. Nothing was run in a window. Every pixel number below
is **computed from code constants** (and estimated text widths), not seen on screen.

## Executive summary
1. The box system works as a mechanism: one tree (egui_tiles), chip tabs, drag-resize, tab drag between boxes,
   edge docking, a Window menu. Ruling 9 holds: only `shell/boxtree.rs` imports `egui_tiles`.
2. The **panel contract is missing**. The registry only holds ids and titles. The real panels are wired through a
   70-line `match` closure inside `Ui::run` (`ui.rs:1332-1402`). Each panel takes its own list of `&mut` state.
3. The **floating control bar is a mirror with its own logic**. Its X/Y ignore the reference point and its W/H
   ignore the proportion lock, so the bar and Properties can show **different X values for the same object**.
4. **The layout is never saved.** `to_json` is only called in a test and there is no `from_json`. Every launch,
   and every closed panel, falls back to `standard()`.
5. **Min sizes are fiction at default size.** The one global minimum (224 pt) is only enforced while dragging.
   The default shares already put the Align box below it (≈188 pt tall). Below a ≈1144 pt window width, the
   right column (20 % share) also drops below it. The per-panel `PanelId::min_size()` is never called.
6. **The control bar changes width from ≈145 to ≈865 pt** depending on context. Nothing limits it to the board.
   On a small board it spreads over the rulers and the panel column. After Fit it covers the top of a portrait
   artboard, including its name label.
7. **ONE-HOME is broken in 6 domains** (Transform, Artboard, Pathfinder, Snapping, Colour, panel presence).
   Snapping has no home at all. Two homes are hidden behind another panel's mode: Artboard shows only with the
   Artboard tool, and Document only when nothing is selected.
8. **Motion breaks the law.** The drag ghost eases 55 % toward the cursor each frame. The vendored fork adds a
   250 ms "glide" after every drop. Both conflict with "No animations" and BOX_SYSTEM_PLAN decision 3.
9. The vendor fork still matches its contract (SHA verified, 5 of 17 files changed). But the checker is
   PowerShell-only and is not in CI. About 90 of the 177 lines added to `tree.rs` are the forbidden glide.
10. Sandbox leftovers still ship in the product: about 335 lines of dummy panels and a dummy board. The box header
    height jumps 40↔34 pt. The ☰ "change panel" menu can put the same panel into two boxes.

## Measurements
| What | Number | How |
|---|---|---|
| `ui.rs` / `boxtree.rs` / `registry.rs` lines | 6533 / 1042 / 312 | `wc -l` |
| Dummy (sandbox) code still compiled in the product | ≈239 (`registry.rs:74-312`) + 96 (`boxtree.rs:859-954`) lines | read |
| Panel builder sizes | layers 517 · artboard 184 · ctlbar 176 · properties 148 · align 61 · rail 50 · pathfinder 14 lines | awk over `fn` spans |
| Host `match` closure inside `Ui::run` | 70 lines (`ui.rs:1332-1402`) | read |
| `to_json` call sites outside tests / `from_json` exists | 0 / no | grep |
| `egui_tiles` imports outside `boxtree.rs` | 0 | grep (ruling 9 holds) |
| Vendor fork vs crates.io 0.16.0 | SHA-256 `9eb8fef6…0a174` ✓, VCS `62ac7471…` ✓, 17 files, **5 modified**, +307/−127 lines | downloaded `.crate`, `diff -ru --strip-trailing-cr` |
| … of which glide/animation code in `tree.rs` | ≈90 of +177 lines (`tree.rs:334-336,504-510,557-618,856-869`) | read |
| Vendor check in CI | **no** (`ci.yml` runs only `check_dep_directions.ps1`) | read |
| Global box minimum / per-panel minimum used | 224 pt (`boxtree.rs:686`) / **never** (`registry.rs:51` has no caller) | grep |
| Right-column width at 1460 / 1024 / 800 pt window | ≈287 / ≈200 / ≈155 pt (<224 below ≈1144 pt window) | `(W−24)×0.2`, gaps 12 |
| Upper box (Align/Pathfinder) height at default 860 pt window (Mac) | ≈188 pt (<224) | `(860−28−31−6−12)×0.24` |
| Properties' built-in width (Transform row / rotation row + 24 margin) | ≈236 / ≈238 pt (> 224 min) | `38+6+66+6+66+6+24`; `150+6+26+6+26` |
| Control bar width by context | idle ≈145 · Direct ≈390 · object ≈800 · Artboard ≈865 pt | sum of fixed widths + gaps 6 + margins 20 (`ui.rs:3716-3891`) |
| Bar vertical band inside board / Fit top margin | y = 22…58 pt / 5 % ≈ 39 pt at default | `ui.rs:3731`, `fit_to_board(...,0.9)` `main.rs:650` |
| Box header height, tabbed vs single | 40 vs 34 pt (`boxtree.rs:538` vs `:639`) | read |
| Surfaces that edit object X/Y/W/H | 2 (Properties, bar), **2 different logics** | read |
| Entry points to colour | 4 (rail foot, bar chips, Properties rows, artboard page) → 1 modal | read |
| Surfaces that change which panels are open | 4 (egui Window menu, native Window menu, box ☰, box ✕), 2 dispatch paths | read |
| Headless tests covering real panels | only `panel_layers`. `every_panel_body_renders_headless` renders the **dummies** | grep |

## BOX_SYSTEM_PLAN, stage by stage
| Stage | Verdict | Evidence |
|---|---|---|
| 0 Tokens | **Done, drifted** | `tokens.rs` exists. The law's values changed without the plan being updated: RBOX 4→8, seam 6→12, a new RCAP=11 for tabs (§3.3 says tabs = `r` 3). `ui.rs` still imports tokens under old names (`LINE as BORDER`, `PANEL as SOLID_PANEL`, `SURFACE as` two names, `ui.rs:22-26`) and has 37 literal `CornerRadius::same(n)`. |
| 1 Void frame | **Done**, top bar **being replaced by S1** | Share / Export / Search do nothing (`ui.rs:3410-3414`). The burger's New/Open/Save/Export rows ignore their click result (`ui.rs:3470-3474`). S1 must not leave any row without a command. |
| 2 Box tree + registry + ⌄ swap | **Done, drifted** | The tree and chip tabs are real. The registry has no `render` hook (§4.2) and `HostFn` replaced it (`boxtree.rs:22`). ⌄ became ☰ "CHANGE THIS PANEL TO" (`boxtree.rs:492-510`) and allows duplicates (F-B3). Tabs are capsules, not `r`. |
| 3 Behaviours | **Partial** | Min size only while dragging, and one global value. Collapsible sections exist **only in the dummy** Properties (`registry.rs:219-249`), not the real one. "Hands shrink/vanish on a tiny board" exists **only in the dummy** `draw_hands` (`boxtree.rs:892-954`). The real hands have no small-board rule. Motion is not 1:1 (F-I4). |
| 4 Migration | **Done, glued** | Real panels come in through the closure (`ui.rs:1332`). Properties hosts three homes: Transform/Appearance, **Artboard** when the tool is Artboard (`ui.rs:1372`), **Document** when nothing is selected (`ui.rs:4756`). The control bar was born as a mirror but has its own logic (F-B1). Swatches/History/Assets are unbuilt stubs. |
| §4.5 serde / workspaces | **Not done** | The tree is serializable but never saved or restored. There is no "Reset workspace". `Cargo.toml:27` still says "BoxState" (the type is gone; the pane is `PanelId`). `lib.rs:3-4` names a `shell-sandbox` bin that no longer exists. |
| 5 (out of scope) | Not started | Correct. |

## Homes and mirrors (the ONE-HOME map as built)
| Domain | Home (Section) | Mirrors | Verdict |
|---|---|---|---|
| Tools | Rail (a hand), shape flyout | keys | OK |
| Transform X/Y/W/H/∠/flip/refpt/lock | Properties › Transform `ui.rs:4764-4830` | Bar `ui.rs:3790-3818` | **Violation**: the mirror computes differently (F-B1) |
| Opacity, stroke weight | Properties › Appearance | Bar (opacity only) | OK. The law says the bar also mirrors weight: missing |
| Fill/stroke colour | **Modal** picker `ui.rs:2513` (not a docked Section) | rail foot (click = focus, double-click = picker), bar chips (**click** = picker `ui.rs:3930`), Properties rows (click = focus, double-click = picker `ui.rs:2119-2123`) | **Inconsistency**: the home is not docked, and the same gesture does different things |
| Paint focus / swap / default | Rail foot `ui.rs:3971` | keys X / ⇧X / D, Properties row click | OK |
| Align / distribute | Align panel `ui.rs:4978` | Bar 4 of 6, carries `align_target` | **Exemplar**: command-only mirror |
| Pathfinder | Pathfinder panel `ui.rs:5039` | Bar `:3849`, **Properties › Shape** `:4867` | **Violation**: a panel mirrors a panel. The plan itself lists "Shape" in Properties (§4.2), so the law is ambiguous |
| Artboard | Properties body only when the tool = Artboard | Bar (Artboard tool), ⋮ canvas menu `ui.rs:5485-5512`, Layers board headers (eye/lock), status i/n + Fit | **Violation**: the home depends on the tool. The bar ignores `ab_lock` (`:3765` vs `:5271`) |
| Document (units, rulers, guides, snap master) | Properties only when nothing is selected `ui.rs:4888` | native View menu, keys | **Hidden home**: gone as soon as anything is selected |
| Snapping options | **none** | magnet popover `ui.rs:3500-3526`, native View `chrome.rs:291-294`, Properties master switch, ⌘U | **No home.** The magnet highlight ignores `snap.enabled` (`ui.rs:3397`) |
| Layers / structure | Layers panel | Object › Group/Arrange keys | Arrange (z-order) has no home anywhere |
| Panel presence | none (the tree) | egui Window menu (calls `shell` directly, `ui.rs:3490-3492`), native Window menu (`MenuCmd`, `main.rs:1002`), ☰, ✕ | Two dispatch paths. On macOS **two Window menus** are visible at once |

## Findings

### Bug
- **F-B1 (P1) The bar's Transform mirror has its own logic.** The bar sends `SetBBox(.., 0.0, 0.0)`, a top-left
  anchor with no ratio (`ui.rs:3794-3805`). Properties sends `(ax, ay)` from `refpt`, displays `s.x + ax·world_w`,
  and applies the W/H lock itself (`ui.rs:4776-4803`). Repro: set the reference point to centre, select a
  100-wide object at x=0 → Properties X = 50, bar X = 0. With the lock on, typing W in the bar breaks the ratio.
  The same split exists for artboards (bar `:3765` has no `ab_lock`, the panel at `:5271` has it). The ratio
  lock is computed in the panel, not in the command, so every mirror has to re-implement it.
- **F-B2 (P2) Nested vertical scroll in Properties and Artboard.** The box wraps every panel that is not
  `self_scrolling` in a ScrollArea and adds `add_space(10.0)` (`boxtree.rs:433-437`). Properties and Artboard
  open their own ScrollArea inside it (`ui.rs:4749`, `:5214`). The inner one fills the viewport, so the outer
  one always has ≈10 pt of overflow. Expected symptom (not seen in a window): an extra scroll of about 10 pt on
  the whole panel once the inner list reaches its end. `self_scrolling` (`registry.rs:46-48`) exists to prevent
  exactly this.
- **F-B3 (P2) ☰ "change this panel" can duplicate a panel.** `switch` writes `*p = panel` without checking
  `is_open` (`boxtree.rs:88-92`, `:500-509`). Switching Properties to Layers while Layers is open gives two
  Layers boxes, and Properties can then only come back through the Window menu. `find_pane` then only sees the
  first copy (`:205`).
- **F-B4 (P2) The artboard ⋮ menu is gated, but the docs say it is not.** `.interactable(tool_ab)` at
  `ui.rs:5468`, against LAYERS_VISION.md:376 "⋮ ungated" and the code's own comment at `:5385`. The dots are
  drawn in every tool but only respond in the Artboard tool.
- **F-B5 (P2) The layout, rail/bar visibility and closed panels are lost on every launch.** No
  save/restore (`boxtree.rs:130`). `show_rail` / `show_dock` live only in `Ui` (`ui.rs:826`).
- **F-B6 (P2, computed) Clipping at the boxes' own minimum.** Properties needs ≈236–238 pt but the minimum is
  224. The default upper box (≈188 pt) is smaller than Align's content plus its header (≈190 pt).
  `panel_layers` reserves `list_h.max(60)` + header ≈43 + footer 34 = 137 pt, but a box may be 120 pt tall
  (`registry.rs` min), so the footer gets clipped (PAINS #2 again).

### Inconsistency
- **F-I1 (P2) Bar width jumps (Astra).** The bar is sized to its content with a centred pivot (`ui.rs:3730-3731`):
  ≈145 / 390 / 800 / 865 pt. Even inside one context the width follows the selection's name label (custom names
  have no length limit). Nothing limits it to the board: at a 1024 pt window (board ≈782 pt) the object bar
  already overflows it. At the 224 pt board minimum it covers the panel column (`Order::Middle`, above the tiles).
- **F-I2 (P2) The bar covers artwork.** A fixed band 22–58 pt from the top of the board. `Fit` leaves a 5 % margin
  (≈39 pt at default size, `main.rs:650`), so a fitted portrait artboard's top edge **and its name label** (drawn at
  y−24, `ui.rs:5412`) sit under the bar. The rail (x 16…≈58) touches the left edge of a fitted landscape artboard.
- **F-I3 (P3) Header height 40 vs 34 pt.** When a box goes from 2 tabs to 1, `prune_single_child_tabs`
  (`boxtree.rs:723`) turns it into a single box and the body moves up 6 pt.
- **F-I4 (P2) Motion.** Ghost `gpos += (cur−gpos)·0.55` per frame, so the speed depends on frame rate and does not
  follow the cursor 1:1 (`boxtree.rs:116-117`). The fork adds a 250 ms glide and an easing "fly-in" from the drop
  point (`vendor/egui_tiles/src/tree.rs:504-510, 557-618`). This is against CLAUDE.md "No animations" and plan
  decision 3. Zoom easing was already removed for the same reason (STATUS, `36d04d4`). **Needs Ahmed**: he asked for
  the glide on 07-05.
- **F-I5 (P3) Two menu systems.** The box ☰ uses egui's `ui.menu_button` / `ui.button` (`boxtree.rs:496-503`).
  Every other menu uses `menu_below` / `menu_row`. Rows, fonts and closing behaviour differ.
- **F-I6 (P3) Window-menu checkmark semantics.** Clicking a ✓ row whose tab is hidden behind another tab brings it
  forward instead of closing it (`boxtree.rs:151-156`). The native checkmark still says "open".
- **F-I7 (P3) Naming.** The control bar is called `dock` everywhere (`show_dock`, `ToggleDock`, `Check::Dock`),
  and so is the Properties "inspector dock". The Properties tab keeps the title "Properties" while it shows the
  Artboard inspector.

### Glue
- **F-G1 (P2)** The host `match` closure inside `run` is the real panel registry. Adding a panel means editing
  `run`, the closure, `DOCKABLE`, `family()` and the Window menus.
- **F-G2 (P2)** Sandbox leftovers ship in the product: dummy bodies, `draw_board`, `draw_hands`, the unhosted
  `ShellState::ui`, and the stubs Swatches/History/Assets (`registry.rs:10-19`). The `render_panel` fallback also
  runs inside the drag ghost.
- **F-G3 (P3)** The tree is changed during rendering: `flatten_nested_tabs` rewrites the tree every frame
  (`boxtree.rs:58`), and so do `switch` / `close` / `set_active`. These are edits with no command behind them.
- **F-G4 (P3)** Each hosted panel sets its own margin, `Frame::inner_margin(12,10)` ×4 (`ui.rs:4750,4979,5040,5215`),
  while Layers uses `add_space(11)`. The "box owns the 12×10 margin" (`boxtree.rs:419-422`) only applies to dummies.
  The micro-label is inlined 9× as `MUTED 10.0 strong`; the law says `faint 9.5`, uppercase, tracking 1.2.
- **F-G5 (P3)** `SetSnapConfig(snap_cfg)` and `set_constrain_wh(lock)` are executed **every frame**, unconditionally
  (`ui.rs:1461-1462`). A UI-held copy is written back into the core instead of sending a command on change.
- **F-G6 (P3)** Hand frames are repeated inline: rail, bar and `panel_frame` each build `fill PANEL + BORDER +
  RBOX` (`ui.rs:3684-3690, 3733-3739, 1662`), while boxes are painted in `boxtree.rs:534`.

### Architecture
- **F-A1 (P1) No panel contract.** Panels get `&Snap` (good) but also take 3–9 `&mut` locals each (`refpt`, `lock`,
  `ab_lock`, `align_target`, `lay_*`, `fit_request`) that are threaded through `run` by hand (`ui.rs:1281-1303`).
  `OpenPicker` is caught inside `run` (`ui.rs:1464`). `fit_request` bypasses `Op`. Nothing enforces "read `Snap`,
  return ops".
- **F-A2 (P2) Homes depend on other panels.** Artboard and Document live inside Properties' mode switch. Close
  Properties and they disappear: there is no artboard inspector, and no Snapping master switch anywhere except
  the magnet's sub-options.
- **F-A3 (P2) Two Window-menu paths.** The egui menu mutates `ShellState` directly. The native menu goes through
  `MenuCmd → main.rs:1002 → gui.toggle_panel`. There should be one `ShellCmd`.
- **F-A4 (P2) The vendor contract cannot be checked on the machines that matter.** `tools/check_vendor_patches.ps1`
  is PowerShell. The Mac has no `pwsh` (STATUS). CI never runs it. Ruling 9 (import confinement) is not
  machine-checked either. Both hold today only because I verified them by hand.
- **F-A5 (P3) Plan and law are out of date.** BOX_SYSTEM_PLAN §3 (seam 6, RBOX 4, tab radius `r`, right column 274)
  no longer matches `tokens.rs`, and the plan is stamped `reference` only. The spec needs one current layout
  table.

### Missing rule
- **F-M1** No uniqueness invariant (one pane per `PanelId`). No "a home may not depend on the tool or on another
  panel's state". No "a mirror may not compute".
- **F-M2** No size policy: nothing says the right column is fixed in points while the board takes the rest, or
  that the default layout must meet every box's minimum at the minimum window size (800×560).
- **F-M3** No hands policy: nothing limits the bar's width to the board, no overflow "…", no stable slots per mode,
  no space reserved for Fit, no small-board rule (the dummy had one and it was dropped).
- **F-M4** No keyboard or accessibility path for docking (focus a box, switch tabs, open or close a panel).
  Astra saw that the canvas controls are missing from the accessibility tree.

### What S1 must leave behind (top bar, tab strip, File menu)
Every bar or burger row either sends an `AppCommand` or is not drawn (today: Share, Export, Search, New/Open/Save/
Export rows). `ShellState` stays **app-wide**, not per document. `settle()` must run before `document_switched()`
(the picker session and rename buffers). The egui and native Window menus use one command path.

## Rules this suggests
1. **Home registry:** each editable domain has exactly one `Home` entry (Section + owner panel + command set) in
   `shell/homes.rs`. A test fails if two homes claim the same `EditCommand` family.
2. **A mirror only sends commands:** it calls the home's shared field function (same value transform, same
   refpoint, same lock), so the mirror and its home always show the same number. A test compares them.
3. **Constraints belong to the command, not the panel:** ratio lock and reference point are fields of
   `SetObjectBounds` / `SetArtboardRect`, resolved in the core.
4. **Panel contract:** `fn ui(local: &mut Self::Local, ui: &mut Ui, snap: &Snap) -> Vec<Cmd>`. No `&mut Editor`,
   no threaded `&mut` from other panels. Local view state belongs to the panel and is reset by a lifecycle hook.
5. **The box owns margins and overflow.** A panel declares `ScrollPolicy::{Box, SelfManaged}`. There is never
   more than one vertical ScrollArea per box.
6. **One pane per `PanelId`.** `ShellState` enforces it. Replace/open of an already-open panel moves or focuses it.
   This is tested.
7. **Every panel declares `min_size`.** The layout enforces it at layout time, not only while dragging. A headless
   test renders every panel at its minimum with no horizontal overflow.
8. **The standard layout meets every minimum at 800×560.** The right column is fixed in points (≈274). The board
   takes what is left.
9. **Layout is saved** at quit and restored at launch (falling back to `standard()` on error), and a
   *Reset Workspace* command exists.
10. **One `ShellCmd` (Open/Close/Focus/Replace)** is used by every menu (egui and native), ☰ and ✕.
11. **Hands never leave the board.** Bar width ≤ board − 2×12 pt. Overflow collapses into "…" that jumps to the
    home. Slots stay stable within a mode (disable, never remove).
12. **Fit reserves the hands' bands** (bar 58 pt top, rail 60 pt left), so no fitted edge or label sits under a hand.
13. **No home depends on the tool or on another panel's empty state.** Artboard and Document are their own
    Sections.
14. **Docking has no motion:** the ghost follows the cursor 1:1 and a drop is instant.
15. **The vendor delta and the import boundary are checked in CI** on every push by a cross-platform check (a Rust
    test or a shell script, not only PowerShell).

## What to fix first
| # | Fix | Size |
|---|---|---|
| 1 | Move the ratio lock and reference point into the command. Bar and Properties call one field function (F-B1). | M |
| 2 | Drop the inner ScrollAreas in Properties/Artboard (or mark them `SelfManaged`), and drop the extra `add_space` (F-B2). | S |
| 3 | Uniqueness in ☰ switch and `toggle_panel`, with a test (F-B3). | S |
| 4 | Un-gate the artboard ⋮ menu as documented (or change the doc, owner decision) (F-B4). | S |
| 5 | Save and restore the layout + `show_rail` / `show_dock`, plus a *Reset Workspace* command (F-B5). | M |
| 6 | Size policy: fixed right-column width, per-panel `min_size` enforced at layout time, default shares that meet the minimums (F-B6, rules 7–8). | M |
| 7 | Hands policy: bar width limit + overflow "…" + stable slots, and Fit reserves the bands (F-I1, F-I2). | M |
| 8 | Remove the ghost easing and the fork glide (this also shrinks `tree.rs` by ≈90 lines). **Needs Ahmed's OK** (F-I4). | S |
| 9 | Vendor and import checks in CI, cross-platform (F-A4). | S |
| 10 | Home registry + `Panel` trait + registry render hook. Delete the sandbox dummies. Split Artboard, Document and Snapping into Sections (F-A1, F-A2, F-G1, F-G2). | L |

**Could not verify without a window:** the ≈10 pt scroll wobble (F-B2), the clipping at the minimum width (F-B6),
the bar overlap at small sizes and after Fit (F-I1/I2), and the 6 pt header jump (F-I3). All come from code
constants and estimated text widths (±10 pt).
