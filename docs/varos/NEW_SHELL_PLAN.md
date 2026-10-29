# New shell plan — the approved Blender-like Varos window (2026-10-27)

Design of record: `docs/varos/ui-handoff-2026-10-27/` (owner + Codex; HTML approved «Excellent,
this is what is required»). Its `AGREED_ARCHITECTURE.md` carries 12 non-negotiable rules and the
interaction contract; this file says how the native app gets there, in hand-testable stages.

## Findings that shape the plan (inventory 2026-10-27, read-only)

- **The editor is already host-agnostic.** Every panel body is `fn show(app, ui)` dispatched by
  `dock::panel_body(app, ui, id, width, tall)` (`crates/ui-egui/src/dock.rs:11`); the canvas is
  `canvas::show(app, ui)` taking the Ui's available rect; menus are a data table
  (`menus::menu_tree`) shared with the native macOS menu; shortcuts (`shortcuts::handle`) go
  through `menus::invoke`. None of them import craft-ui dock types.
- **Only the arrangement code knows craft-ui**: `panel_docking.rs` (+ `PanelContent`),
  `varos/panel_stack.rs` (our cards), `layout_migration.rs`, `workspaces.rs`, the
  `ui.docking` state, and the `window.panel*` / `window.dock` / `window.workspace*` commands.
  That is the replacement surface.
- **Panel ids are strings** (3 dock tabs + 39 icon panels + `library:*` + protected `canvas`),
  listed in `state.rs` (`all_panels`, `ICON_PANELS`, `ICON_PANEL_GROUPS`).
- **The old box system** (`~/…/varos/varos/crates/varos-app/src/shell/`) is egui 0.35 on a
  vendored `egui_tiles` fork with a closed 11-variant `PanelId` enum. Its *patterns* (lift-and-ghost
  drag, azure drop preview, capsule tab pills, "board is just another box", tokens-only, hand
  painting) carry over; its code does not port cheaply, and `egui_tiles` has no divider rails,
  flyouts or in-area floats, which the design requires.

**Decision:** build the shell natively in varos-next as `crates/ui-egui/src/varos/shell/` —
our own split tree mirroring the prototype's model (`axis`, `ratio`, `rail`, `a`, `b`, leaf =
editor instance) — behind a Preferences switch, beside the current craft-ui shell, until the
owner approves it in the real window. No egui_tiles. The old shell's drag/ghost/tab-pill code is
reference for feel only.

## Stable host API (never changed by the shell work)

`canvas::show` · `dock::panel_body` · `panels::panel_menu_items` · `menus::menu_tree/entry/invoke`
· `shortcuts::handle` · `varos::home::show/is_home` · `chrome::doc_tabs` · `toolbar::control_bar_options`.
Editor crates (engine, doc, text, render, pdf, format) untouched.

## Vocabulary (Rust names)

`Workspace { tree: Area, editors: BTreeMap<EditorId, Editor>, floats: Vec<Float> }` ·
`Area = Split { axis, ratio, rail: Rail, a, b } | Leaf(EditorId)` · `Rail = Vec<EditorId>` (icons on
that divider) · `Editor { kind: PanelId(String), view: ViewState, pinned: Option<ObjectId>,
sections: Vec<SectionId> }` · `Float { editor, owner: AreaPath, rect, return_rail }` ·
`Flyout { editor, from_rail, rect }` (temporary, not persisted). One editor instance has exactly one
owner: an Area leaf, a rail slot, or a float (rule 11).

## Stages (each ends with gates + a bundle + the owner's hand test)

| # | Stage | What the owner sees | Scope / files | Lane |
|---|---|---|---|---|
| S0 | **Seam + switch** | Preferences → "New shell (Varos)" ON: the window shows three areas — Viewport (real canvas, doc tabs, rulers), Layers, Properties — with the approved charcoal tokens, 12 px gutters, rounded 12 px areas, editor-type icon top-left of each. Dividers resize. Nothing else moves yet. OFF = today's window. | `varos/shell/{mod,tree,area,paint,tokens,persist}.rs`; `lib.rs:2360-2365` calls `shell::frame` instead of `panel_docking::show` + `panel_stack::frame` when ON; own `ui.json` key `varos_shell` (version 1); `window.panel` answered (open = add to the right column rail; close = remove). App bar / toolbar / status bar unchanged in S0. | Opus (AppKit-free but judgement-heavy: egui layering, canvas rect, input) |
| S1 | **Divider rails + flyouts** | The 39 icon panels sit as icons on the dividers (right column's vertical divider by default, grouped as today's icon column); empty rails 10 px, populated 34 px; click = temporary flyout beside the icon (reuses `dock::panel_body`); click again/outside = close; icon keeps ownership. | `shell/rail.rs`, `shell/flyout.rs`; rail orientation both ways; `ui.open_panel` shim so toolbar/menus/shortcuts keep working. | Codex Astra, Opus review |
| S2 | **Move, dock, float, duplicate** | Hidden top-edge grip (84×12 hit, 30×3 bar, appears within 15 px / hover / focus). Drag grip or rail icon → live target preview: drop on divider = icon, near area edge = split, inside area = float within it; Escape / blur cancels; Alt at drag start = duplicate (own view state, shared artwork). Float resizes from the lower-right corner, collapses back to a rail. | `shell/drag.rs` (lift-and-ghost from the old shell), `shell/drop.rs`, `shell/float.rs`; tests on the tree (split/merge/owner invariants). | Opus, Codex review |
| S3 | **Editor chooser + Properties as an editor** | Top-left icon opens the chooser (Viewport, Properties, Layers, Color, Typography, Transform, Assets, History, Node board placeholder, + the full native list); choosing replaces only that area's editor. Properties: category icon column, "Following selection" / pin, sections collapse + drag-reorder with insertion cue, per-instance state. | `shell/chooser.rs`; `panels/properties.rs` gains a section model (`SectionId`, order, collapsed) — the first editor-side change, kept behind the shell switch. | Codex Astra, Opus review |
| S4 | **Workspaces + persistence** | One-row workspace bar (Design / Typography / Board / Focus + custom), middle-drag panning, overflow; switching restores layout and keeps the document; maximize/restore an area; layouts survive relaunch; unknown/removed editor ids migrate to the default rail; "Reset layout" keeps artwork. | `shell/workspace.rs` replacing `workspaces.rs` paths when ON; `window.workspace*` commands; migration tests. | Opus |
| S5 | **Chrome to the approved look** | 42 px title bar (brand, menus, doc name, undo/redo, Export), 40 px workspace bar, 25 px status bar; viewport tool palette and navigation palette as repositionable widgets inside the Viewport (reachable when small); Page selector = artboards (not documents; documents stay tabs). Control bar content becomes the viewport header row (Object/View/Select + preview toggles). | `chrome.rs` paths under the switch; `toolbar.rs` → `shell/viewport_widgets.rs`; tokens from `IMPLEMENTED_DESIGN.md`. | Codex Astra, Opus review |
| S6 | **Retire the old shell** (only after the owner's OK on S0–S5) | Switch removed; craft-ui dock, cards, layout_migration, OS-window floats deleted; weekly upstream merges keep the editor, drop their chrome. | big deletion lane + NOTICE/docs refresh. | Opus |

Order inside S1–S5 may be reshuffled by the moderator for lane availability; the owner is asked
only for hand-test results and the decisions below.

## Decisions for the owner (asked once, before S0 lands)

1. **Accent colour.** The approved HTML uses pale blue `#8fb9f7` on `#29394e` for active/focus;
   Varos' constitution says azure `#0c8ce9`. Follow the HTML (recommended, he approved it visually)
   or keep azure?
2. **Pages vs documents.** The prototype's "Page" selector is file-level pages. In Varos a Page =
   an artboard (recommended); documents stay as tabs in the Viewport header. OK?
3. **Tool rail.** The prototype's tool palette floats inside the viewport. S0–S4 keep today's left
   tool rail; S5 moves it into the viewport. OK to keep the old rail that long?

## Acceptance (from the handoff, verified by the owner in the real window)

Same editor: area → vertical rail → horizontal rail → float → new split, exactly one owner after
each step · duplicate Properties, pin one, change selection, both right · cancel / release outside
the window / focus loss / narrow areas / resize / zoom · layouts persist across relaunch, unknown
ids migrate, Reset keeps artwork · keyboard focus, legibility, overflow, menu placement, performance,
multi-display scaling · file formats, rendering, Arabic shaping, recovery untouched.

## Rules for every lane

One cargo lane at a time on the owner's Mac; gates = workspace tests (release, `-j 4`,
`--test-threads=4`, the 10 known ui-egui release-only failures excepted), clippy `--all-targets
-D warnings`, fmt, `git diff --check`; author ≠ reviewer; moderator lands the merge, bundles,
and hands off in plain Arabic with what to test. Shell code lives under `varos/shell/` only;
editor-side changes (S3 sections) are the exception and are listed in the lane report.

## Stage 2 implementation record (2026-10-27)
Version 2 persists the Board strip, groups, dimensions and icon/label mode; version 1 migrates.
Flyouts use the shared tab row, panel menu and real panel bodies; open state is transient.
Window commands and the next-frame `ui.open_panel` shim share shell ownership.
Software captures use `VAROS_SHELL_CAPTURE` and `VAROS_SHELL_CAPTURE_FLYOUT`.
Owner acceptance pending; dragging and floating strips remain outside this stage.

## Stage 3 implementation record (2026-10-27)
Drags (`shell/drag.rs`): tab, icon, strip group (grip dots), floating strip (title strip); ghost at 70 %,
1-point azure source outline, a 2-point azure line as the only preview; release applies on a copy of
`VarosShell` (kept only when whole), Escape / lost focus / lost pointer cancel. Targets in order: tab
row, strip slot, Board-strip dock (floating strips), box edge band (24 pt + half seam), empty space.
Floating panels and lone icons / floating strips (`shell/floats.rs`) persist in version 3 (1 and 2
migrate); double-click on a tab collapses a box or float to its tab row. Alt-drag is the
`DragMode::Duplicate` hook only (moves; status "duplicate: later"). Captures: `VAROS_SHELL_CAPTURE_DRAG`,
`VAROS_SHELL_CAPTURE_FLOAT`. Owner acceptance pending.

## Stage 4 implementation record (2026-10-27)
Properties is an editor of sections (`panels/properties.rs`, the one editor-side change, used only by
the shell): stable `SectionId`s across selection kinds (Appearance and Quick Actions are one section for
objects and the document), one header per section (chevron, 11 pt semibold title, six grip dots, 1 pt
hairline, body inset 8 pt); a click collapses, the grip drags (ghost + azure line, Escape cancels; the
grip is never the tab). `PropertiesView { collapsed, order }` per editor instance, kept in
`VarosShell.views`. Every editor follows the selection ("Following selection · Kind"). More editors:
≡ › Duplicate Panel (a box below; not offered in flyouts) or Alt-drag of a Properties tab
(`properties@2`, …); the window commands and the Window menu reach them by id. Pin deferred: needs
explicit targets passed to the sections (engine commands already accept explicit targets); not a
selection swap (`PropertiesView.pinned` stays in the model, unused).
Workspaces (`shell/workspace.rs`): a 28 pt row on top of the central region replaces the app bar's
switcher — Design, Typography, Print, the user's, `+`; one row, wheel/trackpad and middle-drag scroll,
`‹ ›` mark hidden tabs; a workspace is the whole arrangement (tree, strips, floats, section views,
maximized box), changes remembered per workspace, Reset restores the preset (documents untouched),
rename/delete from a tab's context menu (Manage dialog); saving a workspace keeps the changes of the one
left. Maximize: double-click a box's empty tab-row space or ≡ › Maximize Box; the Board: double-click
its document tabs' empty space or its ≡ (top of its strip) › Maximize Board; the tree never changes,
`maximized: Option<u32>` is saved. ⌘⇧M is Object ›
Transform › Move (Illustrator), so maximize has no key. `VarosShell` version 4, migrations 1→2→3→4 are
pure functions with stage 1–3 fixtures (`shell/fixtures/`). Stage-3 P2 fixed: a side-by-side all-collapsed
pair of unequal heights shows its shorter side whole (no hole). Captures: `VAROS_SHELL_CAPTURE_SECTIONS`,
`VAROS_SHELL_CAPTURE_WORKSPACES`. Owner acceptance pending.

## Stage 5 implementation record (2026-10-28)
Codex Astra implemented, Opus 5.5 reviewed read-only (11 findings → fix round → MERGE). Tools live on
the Board's left edge (`shell/tools.rs`): a 34 pt strip inside the Board's rounded frame with the
groups of the old rail, the same flyouts (long-press / triangle), the fill/stroke wells and, below them,
Edit Toolbar · Color/Gradient/None · Draw mode (visibility flags honoured; Screen mode stays in View);
the strip floats by its grip (rounded fill + hairline, content-derived height) and docks back to the
Board's left edge with the full-height azure line; its position is part of each workspace; the grip is
inert in screen modes 1–2. `VarosShell` version 5 (`tools_pos`), migration 4→5 pure, fixture
`shell/fixtures/v4.json`. App bar (`chrome.rs`, 40 pt): brand + Home, the 9 menus only when no native
menu backend exists (macOS keeps them in the menu bar), document name + dirty dot, undo/redo with
`tl!` tooltips and shortcuts, search; the workspace switcher left the app bar (stage-4 row has it).
Status bar never intersects a box or float (test). Motion (`shell/motion.rs`, the only motion in the
shell): flyout open = 6 pt slide from its rail + fade 90 ms, close = fade 60 ms (input disabled before
the opacity is restored, no 48 % jump); a dropped tab/float settles 80 ms ease-out only when it created
a new container (existing boxes never move); tear-off cancels the docked flyout motion at once;
`animation_time == 0` → instant. Tab pills get the `◇` fewer/more-options toggle for Character and
Paragraph (`ch-hide-options` / `pa-hide-options`; glyph present in Inter); `<<` `>>` `×` hit areas
≥ 16×16. Shared constants in `shell/tokens.rs` (strip width, app-bar padding, chrome glyph size).
Deferred: a docked tools settle can be obscured by the canvas (P3); the docked tools scrollbar is
always hidden (wheel/trackpad reach the overflow). Capture: `mockups/shell5-capture.png`
(`VAROS_SHELL_CAPTURE_FINAL`). 99 shell tests. Owner acceptance pending; stage 6 waits for it.

## Look round — owner requirements after v15 (2026-10-28, PAUSED by the owner for the usage limit)
Owner on v15 with the shell ON: «شغالة كويس بس شكلها بشع». Decided (in order), all shell-ON only:
1. **Round 1 (brief `brief-look.md`, lane `lane-look`, Codex run stopped mid-build, nothing committed):**
   one grip per strip (no `⋮⋮⋮` between groups, hairline separators); chrome controls as Lucide icon
   textures (no `<<` `>>` `×` `≡` `◇` text); every strip icon 16 px in 28×28 tiles; no frame inside a
   frame (legacy panel bodies get a frameless host mode); tab rows 28 pt, 11 pt medium, overflow →
   `…` menu (never clipped); app bar 40 / status 24; control bar without underlines; flyouts and the
   picker 240–280 pt, never over a strip, wheel ≤ 200 pt, thin auto-hide scrollbars; colours unchanged.
2. **Round 2 (brief `brief-look2.md`):** (a) delete the Workspaces row; the switcher returns to the app
   bar's right as a compact control; (b) the Control bar becomes a dockable panel `control` (not open
   by default; the fixed row disappears); (c) tighten the vertical rhythm (app bar → Board directly);
   (d) document tabs = the box tab style; (e) tools strip columns 1 / 2 / 3 from its `…` menu, per
   workspace (34 / 62 / 90 pt); (f) **decision A — flat Illustrator look over the box tree**: no
   rounded box frames, no 12 pt seams, one hairline between boxes, invisible dividers until hover,
   flat `t.panel` everywhere, strips flat, floats the only cards (1 px line, radius 10), no shadows.
3. Colour round after that (owner: clear, minimal, modern), then stage 6 only after his OK.
Briefs live in the session scratchpad; copy them into `docs/varos/lanes/` when work resumes.

## Simple look — implementation record (2026-10-28)

The approved `SHELL_SIMPLE_DIRECTION.md` and `brief-shell-simple-1.md` supersede the
look-round dimensions above: flat docked groups with one-point seams, 40-point strips,
28-point tool tiles, 40/68/96-point tool-column choices, document tabs in the 40-point top
bar, a compact workspace popup, and a dockable Control panel instead of fixed rows.
Floating/docking, split/swap/maximize, section reordering and stage-5 motion remain.
Version 6 adds per-workspace tool columns without discarding old arrangements. The
Design preset places Layers/Artboards left of the Board and Properties on the right.
The tools docking target follows the window edge even with a group left of the Board.
Partial validation and owner hand-test steps are recorded in
[`lanes/shell-simple-1-report.md`](lanes/shell-simple-1-report.md). Captures and final gates remain unfinished at the owner’s request for a draft PR. Owner acceptance
and stage 6 remain pending; this lane changes neither Home/Pages nor theme colours.

## Simple look implementation record (2026-10-28/29)
Direction: `SHELL_SIMPLE_DIRECTION.md` + mockup `mockups/shell-simple-2026-10-28.html`. Landed as #6 (lane-look):
flat docked look (no box frames/gutters, one hairline, dividers invisible until hover), the 40 pt top bar owns
the document tabs + presence/Share placeholders + search + workspace control (capped, ellipsized); the Workspaces
row and the fixed Control-bar row are gone — Control is a dockable `control` panel; strips are flat 40 pt with
one grip, 28×28 tiles and 1/2/3 tool columns per workspace (40/68/96 pt); legacy bodies are frameless inside
boxes; tab rows 36/32 pt with overflow menus; Lucide icon textures replace text glyphs; flyouts/picker 240 pt
never over any strip (floating ones included); VarosShell v6 (+fixture v5). Home draws its own single 40 pt bar
(window drag + caption buttons on custom title bars). Pages (#5) sits above Layers|Artboards in the Design layout
and the status bar reads "Page N of M · Artboard i / n" (shell ON). Captures: `mockups/shell-simple-capture.png`,
`shell-simple-cols-capture.png`. Deferred: colour round (lane-colours), stage 6 removal after the owner's OK.

## Shell fixes round 1 (2026-10-29) — from the owner's v16 hand test
Landed (lane-shell-fixes-1, Cursor/Grok 4.7, Opus reviews): the new shell is the DEFAULT (VarosShell v7: a
one-time migration turns `enabled` on unless `enabled_chosen`); icons return to their origin strip/group/pill on
all three paths (drop on strip, `<<`, Collapse to icon) and the float-drop preview matches; boxes/floats/flyouts
clip to their rects with minimum widths (Color 240, Properties/Character/Paragraph 300, Layers 200, default
180); dragging a column below its minimum (32 pt dead zone) collapses it into the strip; the strip edge
restores it; picker popover 240 wide, never cut, closes only with OK / Cancel / Escape (undo group owned by the
picker only, sealed by foreign commands, Gradient tab included); `<<` at the top of the strip; narrow tab rows
show whole pills + `»`; lone pills = 16 pt grip row + 28×28 tiles, per-group grips, Close ≠ Collapse; wells
cluster drawn like the picker's, centred; tool strip columns 1/2/3 by dragging its edge (snap hints) or the
`⋯` menu; drag ghosts depict the whole group. Captures: `mockups/shell-fixes-1-*.png`.
