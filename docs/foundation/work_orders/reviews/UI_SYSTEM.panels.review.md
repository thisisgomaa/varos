> **Status:** reference — independent spec review (panels), 2026-09-24.
# UI_SYSTEM.md — independent review · angle: homes, panels, docking, layout

Read-only review of `docs/specs/UI_SYSTEM.md` at `130bc85`. Evidence: audit 03; BOX_SYSTEM_PLAN §3.5/§4; `shell/{boxtree,registry,tokens}.rs`; `vendor/egui_tiles/src/{behavior,tree,container/linear}.rs`; ADR-0006; VENDOR_PATCHES; DEPENDENCY_MAP §4; FOUNDATION_CHARTER §3; DFS S1 §3.2; S2/S3 §3.2–3.7 and §4–5; `storage/settings.rs`. All pixel numbers below come from code constants. None was measured in a window.

## Verdict: REQUEST CHANGES (4 P1s; all fixable in the text, no redesign)
1. The homes half is right. The registry, one pane per id, Properties as stacked sections, and mirrors that call their home's field function close every ONE-HOME violation in audit 03.
2. The size law cannot be built as written. egui_tiles splits space by proportional shares and has one global scalar minimum. "Fixed 274", "minimum at layout time" and "persist the Tree serde" contradict each other (F1, F2).
3. Removing the glide changes the contract of an accepted ADR. Charter §3 rule 1 then requires a superseding ADR, not only a VENDOR_PATCHES edit (F3).
4. U3 is not really parallel. Its pieces use kit parts built in sibling pieces, and two pieces write the same unowned function that S3-F1 also edits (F4).
5. With F1–F4 fixed, the P2s can be folded in while U2 and U4 are authored.

## Findings

**F1 (P1) There is no size model, so L22, `RIGHT_COL` and Q5 can't be built or tested.**
- egui_tiles: `Shares::split` gives each child `available × share / Σshares` (`linear.rs:43-52`); `Behavior::min_size(&self) -> f32` is one scalar (`behavior.rs:269`), used only by the drag-shrink code (`linear.rs:455`); here it is 224 (`boxtree.rs:686`).
- `standard()` uses shares 0.80/0.20 and 0.24/0.76 (`boxtree.rs:43-46`). `PanelId::min_size()` has no caller (`registry.rs:51`).
- At 800×560 on the Mac the tree is (800−12)×(560−28−31−6) = 788×495 (`ui.rs:1404-1410`, `chrome.rs:31`, `ui.rs:3608`): board 776−274 = 502; upper box 0.24×483 ≈ 116 pt, below Align's ≈190. Windows (46 pt bar): column 477 pt; S3-F2's recovery strip takes ≈28 more.
- Minimums are met only with point sizes: upper 190 pt, lower fills (≥263 Mac, ≥245 Win). "Fixed" conflicts with UI_DIRECTION "Boxes resize freely". For custom layouts L22 cannot always hold: three 238 pt columns + a 240 pt board is more than 788.

Change L22 from: "Enforce each panel's minimum at layout time and make the standard layout meet every minimum at an 800×560 window."
To: "Every split has exactly one Fill child: the branch holding the Board, else the last child. Every other child keeps its size in points when the window changes, and user drags change those points. A drag or drop that would push a box below its minimum (the max over its tabs, from `shell/registry.rs`) stops at the minimum. The window's minimum = max(`WIN_MIN`, the layout's summed minimums). A loaded layout whose minimum exceeds the monitor falls back to `standard()`. `standard()` meets every minimum at 800×560 with the app bar at 28 and at 46, with and without the S3 recovery strip."
- Tests: resizing 1460→800 changes only the Fill box; a drop that breaks a minimum is refused; plus the 2×2 `standard()` matrix. §3.1 boxtree row: add "pt sizes are applied as shares in a pre-layout pass; no fork change."
- Token "`RIGHT_COL` ★ 274" → "`RIGHT_COL` 274 (standard initial width; user-resizable)". Q5 "right column fixed 274 pt" → "the right column keeps its width (274 pt) when the window changes; you can still drag it."

**F2 (P1) The layout file names the fork's type outside `boxtree.rs` and stores window-relative shares.**
Quote §3.5: "`LayoutFile { version: 1, tree: <egui_tiles Tree<PanelId> serde>, …}`" in `ui/layout.rs`.
- It breaks ruling 9 and the VENDOR_PATCHES isolation contract, and fails the spec's own L33 import check. Shares saved at a 1460 pt window reopen at 800 pt as a ≈155 pt right column (audit 03 table), which breaks F1. Any fork rebase silently changes the format; "`version` is bumped whenever the fork changes it" has no check behind it.

Replace with: "`tree: LayoutNode`, Varos' own schema owned by `shell/boxtree.rs`: `enum LayoutNode { Split { dir, children: Vec<(LayoutNode, Size)> }, Box { panels: Vec<PanelId>, active: PanelId } }`, `enum Size { Pt(f32), Fill }`. `ShellState::{to_layout, from_layout}` converts and validates (Board exactly once, each PanelId at most once, only `DOCKABLE` + Board, one Fill per split). `ui/layout.rs` does file I/O only." About 80 lines; it takes the fork out of the on-disk contract. Add a test that pins the `PanelId` serde names.

**F3 (P1) Removing the glide requires a superseding ADR.**
- ADR-0006 Decision: "supported Varos delta is exactly the five source files … and the behavior ledger in DEPENDENCY_MAP.md:98-115". That ledger's `tree.rs` row says "short post-drop glide state … movement after docking" (`DEPENDENCY_MAP.md:111`).
- Charter §3.1: an ADR is immutable after acceptance; "Any real change = a new superseding ADR." So §6.4 "no ADR supersession — Codex review confirms the contract reading" is a reading the charter does not allow. No piece owns the removal either: §3.7 lists it, and only U6-A sweeps "deletions still standing".

Change §6.4 to: "Removing the glide changes ADR-0006's ledger. U4-B adds ADR-0009 'egui_tiles fork contract v2': same upstream identity and same five files, with a `tree.rs` row without the glide. It supersedes ADR-0006, which gets a Superseded-By line. The same commit updates DEPENDENCY_MAP §4 and VENDOR_PATCHES."
- Q3 must state this cost: "(needs one short ADR)".

**F4 (P1) U3's "parallel worktrees" collide.**
- U3-D's bar mirrors need `num_field` and `swatch`, and U3-E's swatch unification needs `swatch`; U3-A creates both. U3-C's Layers rename needs U3-B's `inline_edit`, and U3-B waits for S3-F1, so U3-C waits for F1 too.
- Neither U3-A nor U3-B owns `ui/panels/properties/mod.rs`, the stack L20 needs. Both rewrite `panel_properties` (`ui.rs:4749-4890`), which F1 also edits ("`ui.rs:4794`", S2/S3 §4), yet the queue runs "U3-A … ↔ E2 → F1". U3-E edits core `editor.rs` (`begin` assert, `live_rev`), but the core queue lists only U3-A → U3-B → U5-A.
- Change: add piece **U2-C kit** (M): `num_field`, `swatch`, `inline_edit`, `list_row`, `section`, `menu`, each with goldens and no call sites, so U3 pieces only consume the kit. U3-A owns `properties/mod.rs` with five section slots (three delegate to the legacy functions until U3-B). `ui.rs` queue: "F1 → U3-A → U3-B"; U3-C, D and E run in parallel after U2-C. Core queue: add U3-E.

**F5 (P2) The `Panel` trait duplicates the registry and has one consumer.**
§3.1 `shell/registry.rs` holds "min sizes, ScrollPolicy"; §3.2's `trait Panel` declares `const SCROLL` and `fn min_size()` again. `boxtree` (lib) needs these values and cannot see a bin trait. The associated const and type make the trait not object-safe, so `PANELS` ends up as a `match` anyway.

Change: remove `ID/SCROLL/min_size` from the trait (the registry is the one source), or drop the trait: "`fn render(id, locals: &mut Locals, ui, &Snap, &mut Out)`: one match whose arms call `panels::x::ui(...)`". The L21 test stays as it is.

**F6 (P2) Closing Properties removes five homes (the second half of audit 03-F-A2 is not addressed).**
Append to L19: "A home jump opens its container at its standard place if it is closed, brings its tab forward and expands the section. Home jumps are: a mirror's '…', Window ▸ panel, and `FocusPanel`." Test: close Properties → bar "…" on X → Properties opens with Transform expanded.

**F7 (P2) Start conflicts with L20 and L13, and the Start kit migration has no owner.**
S2 §3.7: other panels show one line, "No document open", and "Tab/Shift+Tab traverse actions and rows". L20: "Properties always stacks …"; L13 and §7: "Tab moves focus only among fields of the focused box", "Tab on the canvas does nothing". E1's `start_ui.rs` is wave 1 and predates the kit.
Change: L20 add "…while a session is active; with no session, every document panel shows S2's placeholder." L13 add "While Start shows, the Board box holds focus and Tab traverses Start." U3-C also moves `start_ui.rs` rows to `list_row`.

**F8 (P2) Save cadence and store contract.**
Quote: "written … after each `WindowCmd` that changes layout and at quit".
Drag-resize and drag-dock are not `WindowCmd`s, so they are lost on a crash. `write_replace` does a durable write (`durable.rs:323`); on the UI thread it would stall a frame. S3 already has a store contract (`settings.rs:1-4`).

Replace with: "saved when the `LayoutNode` differs from the last saved value, checked at frame end with no pointer button down, and written on S3's `io_worker`. Loaded with the settings.rs contract: corrupt → `.bad` kept; newer version → `standard()` for this run, file not overwritten."

**F9 (P2) Reset Workspace sits in every box's ☰.**
§4: "☰ lists `DOCKABLE` panels + Reset Workspace" puts a global action with no undo one misclick from "change panel". Change to: "Window ▸ Reset Workspace only. It restores tree, sizes, rail/bar and section collapse, never window size or documents. No confirmation; status text 'Workspace reset'."

**F10 (P2) ⌃F6 is taken on the Mac.**
On macOS ⌃F6 is by default "Move focus to the floating window" (System Settings ▸ Keyboard Shortcuts ▸ Keyboard; verify on Ahmed's Mac). F-keys also need fn on Mac laptops. Change L13 "⌃F6" → "the `FocusNextBox` table chord (Windows F6/⇧F6; Mac chord picked in U1 after checking System Settings)".

**F11 (P2) Gaps in the hot-file queue and module map.**
`ui/{commands,dispatch,homes,request}.rs` are written by U1, U2, every U3 piece, U4-A and the E2/S6 rows, but are not in the queue. "U3 ends by deleting `Op`/`apply_ops`; `Ui::run` takes `&Editor`" has no piece and touches `main.rs`. The Board leaf (hole, rulers, overlays, hands, Start switch) has no module, yet E2 edits it.
Change: queue row "`ui/commands.rs`, `dispatch.rs`: U1-A, then append-only rows, one piece at a time"; piece U3-G (S) in the `main.rs` queue after S3-F2; `ui/board.rs` in §3.1, with E2 as its first writer after U2.

**F12 (P2) U3-D and U4-C design the same control bar twice.** U3-D builds `hands/ctlbar.rs`; then U4-C re-slots it (width classes, stable slots, "…"). Change: U3-D owns the slot table (modes, stable slots, width classes, fold order). U4-C owns only placement, clamp/hide and the Fit bands.

**F13 (P2) U4 order.** U4-A saves the layout before U4-B defines point sizes, so a v2 schema would follow a day later. Change to "U4-B (size model + glide/ghost-ease removal + ADR-0009) → U4-A (persist `LayoutNode`) → U4-C".

**F14 (P2) U1 before E2 delays autosave.** The `main.rs` queue is "S1-D → U1-B/C → S2-E2 → S3-F1", and E2 must precede F1 (S2/S3 §5). So about 2 agent-days of `main.rs` work in U1 delay the recovery/autosave work, which is data safety. §6.1 treats this only as a merge collision. Change the default to "E2 → F1 → F2 first; U1 absorbs their rows" (the spec's own fallback). Ask Ahmed.

**F15 (P2) The vendor check cannot see patch drift.** The checker compares only the set of modified files (VENDOR_PATCHES "Machine check"), so it passes with or without the glide. L33 test add: "…and the SHA-256 of each of the five patched files matches the ledger, so any edit to the fork updates the ledger in the same commit."

**F16 (P2) U2-A is bigger than one agent-day.** It covers five new modules, the render hook, `DocUi` per `SessionId`, the headless `UiFrame` and the `Op` shim, all over a 6,533-line `ui.rs`. Split into U2-A1 (types + render hook) and U2-A2 (`UiFrame` + `DocUi`/`document_switched`).

**F17 (P3) Token values.** "`H_BOXHEAD` one value" → `H_BOXHEAD 40` (`boxtree.rs:538` vs `:639`); `STATUS_H` → 31 (`ui.rs:3608`; the plan says 25); add `BOARD_MIN_BAR`. Define the hands' "board" as the leaf minus the rulers (`ui.rs:1347-1351`); otherwise "top = `HAND_INSET`" and "inside the rulers" disagree.

**F18 (P3) The "…" overflow.** "trailing slots fold into '…' (jumps to the home)" → "into a '…' menu of the folded mirrors; each row jumps to its own home." At 800×560 the bar is at most ≈410 pt, against an object bar of ≈800 pt, so about half of it folds. Tell Ahmed.

**F19 (P3) Law documents.** UI_DIRECTION and the plan (§3.5) say the bar is "horizontally centered". Update them in the fixed-left commit, the same way as Q4.

**F20 (P3) Frozen S1 name.** "S1 names unchanged" contradicts the U6-A rename "`ToggleDock` → `ToggleBar`" of S1's frozen `WindowCmd`. Drop the rename, or log it as a reviewed S1 API change.

**F21 (P3) 03-F-G3 has no fix described.** It is marked closed in U6. Add: "normalise the tree once after each tree-changing event (drop, `WindowCmd`, load), not per frame (`boxtree.rs:58`)."

**F22 (P3) Ghost pass.** L21 "safe to run twice per frame" → add "; the drag-ghost pass renders into a disabled `Ui` and its `Out` is discarded (test)."

**F23 (P3) Window-menu ✓ (03-F-I6).** Define ✓ as "visible (frontmost in its box)". Clicking a row whose tab is hidden brings it forward; clicking a frontmost one closes it.

**F24 (P3) Board rule.** L19 test add: "Board exactly once, not closable, never tabbed into" (`boxtree.rs:681-692`).

**F25 (P3) RTL.** Arabic is the long-term moat. State that "the standard layout is not mirrored for RTL in V1; `LayoutNode` stores Split order only, with no left/right fields."

**F26 (P3) Naming.** "`storage/paths.rs + layout()`" reads as `AppLayout::layout()`. Call it `workspace()` → `root/workspace.json`.

**F27 (P3) QW8 vs U4-C.** QW8 measures the shown hand rects; U4-C uses constant bands. State that U4-C replaces QW8's measurement.

## The five whole-spec questions, from this angle
1. **Laws.** L19–L21 are testable. L22 is not (F1). L20 needs the Start precedence (F7), L13 a safe chord (F10), and L19 home reachability (F6). Merge L21's size and scroll declarations into the registry (F5).
2. **Simplicity.** The `Panel` trait has one consumer (F5). The own layout schema adds about 80 lines but removes the fork from the file contract, so the result is simpler overall. No new dependency is needed in this area.
3. **Plan.** Realistic only with F4 and F11–F14 and F16. The real hot spots are `panel_properties` (U3-A/U3-B/F1), `commands.rs` and `main.rs`.
4. **Decisions.** Q3 hides the ADR cost. Q5's "fixed" is misleading, and 800×560 is already the window floor (`main.rs:789`); the real decision is "the board absorbs window changes". The U1-vs-E2 order is missing.
5. **World-class gaps.** An explicit size model (Fill vs point sizes) and refusal of over-constrained docks, as in Blender's area minimums; a layout schema owned by the app, not the toolkit; every home reachable in one command; a keyboard docking path that does not clash with OS shortcuts; an RTL statement.

## Needs Ahmed (recommended default in bold)
1. Right column: **keeps 274 pt when the window changes, still draggable** / locked at 274.
2. Reset Workspace: **Window menu only** / every box menu.
3. Order: **Start + autosave (E2/F1/F2) before the command table (U1)**. The other way delays recovery by about 2 days.
4. Glide removal: **yes, with a short superseding ADR (ADR-0009)** as part of Q3.
5. At an 800 pt window the control bar shows about half of its object controls and puts the rest in "…": **accept**.
