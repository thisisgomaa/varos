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
