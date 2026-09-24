> **Status:** reference — independent spec review (architecture), 2026-09-24.
# Review — `docs/specs/UI_SYSTEM.md` · angle: architecture & state flow

**Verdict: REQUEST CHANGES.** The direction is right: one owner per value, panels as pure functions of a snapshot, one outbox, a kit, headless goldens. It matches audit 01 and fits S1.
1. As written, the kit cannot compile in its place: it sits in the lib crate but names bin-crate types, and U0-D builds it before those types exist.
2. L4 and L5 contradict each other. Core has no generic undo-coalescing primitive, so "a scrub is one undo step" has no mechanism.
3. The per-frame `Snap` is claimed cheap but was never measured. I measured it: at 10k paths with Select All, it costs about 360 ms per frame today. The cache key the spec proposes would also serve stale values.
With the 3 P1s and the P2s below fixed, I would approve. `Op` should die **first** (U2), not last. Keep ADR-0002's two levels; do not add a third and fourth category.

Evidence base: spec read in full; audits 01 (full) and 04 (rules); S1 WO §3 and S2/S3 §3.7/§3.9; code at `130bc85` (`ui.rs`, `registry.rs`, `boxtree.rs`, `app_command.rs`, `workspace.rs`, core `editor.rs`/`command.rs`/`model.rs`); ADR-0002; F4_DESIGN. **Measured** in a throwaway release-mode crate that depends only on `varos-core` (scratchpad, not committed). The document had 10,000 closed 4-anchor rects, 3 colours, and `sync_tree` had been run. Per call, over 2 runs:
- `obj_bbox`: **180 ms**; `obj_local_dims`: **179 ms**. Both at Select All; with 1 object selected, both together take 0.001 ms.
- `set_opacity` at Select All: 66–73 ms.
- `doc.clone()` (the cost of `Editor::begin`): 2.6–3.3 ms.
- Node hash as in `layer_rows_key`: 0.22–0.28 ms.
- `document_colors`: 0.09–0.18 ms.

The cause is `Document::pidx` (`model.rs:669-671`), a linear `position` inside a per-selected-object loop (`editor.rs:626-627`), which makes the cost O(sel·n).

## Findings

**1 · P1 · The kit layering does not compile, and the build order is inverted.**
- §3.1 puts `shell/kit/*` in the **lib** crate. §3.3 has the kit take `Act::Cmd(CommandId)`, `Act::Req(Request)`, `Tip::Cmd(CommandId)`, `m.cmd(id)`, `more: Option<HomeId>` and `FieldKey`. All of those are defined in **bin** modules (`ui/commands.rs`, `ui/request.rs`, `ui/homes.rs`), and `Request` wraps `AppCommand`, which lives in the bin (`app_command.rs` imports `varos_app::shell::PanelId`). A lib cannot name bin types.
- The order is also wrong: U0-D ships `Act` before `CommandId` exists (U1-A) and before `Request` exists (U2-A). U1-A's `resolve() -> Result<Request, _>` and `CommandSpec.home: HomeId` come before U2-A defines `Request` and `HomeId`.
- *Change* (§3.1 kit line, §3.3 first line, U0-D, U1-A):
  - The lib kit becomes action-agnostic: `pub type Enabled = Result<(), &'static str>;` The builders become `icon_button(ui, key, icon, size, on, en: Enabled, tip: &str) -> Response` and so on; they cannot draw `Err` without its reason.
  - The binding lives in a new **bin** `ui/act.rs`: `pub enum Act { Cmd(CommandId), Req(Request) }` plus `cmd_button(ui, id, &CmdCtx, &mut Out)`. It resolves label, hint and enablement from the table, so "a dead control cannot be written" still holds.
  - `ui/request.rs`, `ui/homes.rs` and `ui/act.rs` land in **U1-A**, not U2-A.
  - *Alternative:* move `ui/` and `app_command.rs` into the lib, as audit 01 proposed. It is rejected because it moves the S1 files (`app_command.rs`, `workspace.rs`, `lifecycle.rs` users) under in-flight S1-C/D, E2, F1 and F2.

**2 · P1 · L4 × L5 contradict each other, and core has no live-span primitive.**
- L4 says "a whole scrub is one `EditSession` = one undo step" (§4 numeric field). But every inspector command runs its own `begin()`…`commit()`: `set_opacity` at `editor.rs:4471-4484`, and 54 `self.begin()` sites.
- The result depends on L5:
  - Without L5, a scrub makes one undo step per frame.
  - With L5's assert, any scrub or picker frame that runs such a command panics in debug.
- The picker already works around this with 4 ad-hoc commands (`PickerBegin/LivePaint/Commit/Cancel`, `command.rs:33-46`). §3.2's "Core additions" names no generic mechanism.
- There are also two owners of "a transaction is open": the UI `EditSession` and core `Editor::pending` (S1's `transaction_open()`). `SessionKind::Drag` duplicates core `Drag` and S1's `DocumentSession::settle()`.
- *Change* (§3.2 core additions, L4, L5):
  - Add a core live span: `EditCommand::{LiveBegin, LiveCommit, LiveCancel}`. Inside a span, `execute(cmd)` absorbs the command's inner `begin/commit`: one snapshot, no history until `LiveCommit`, and a restore on `LiveCancel`. The 4 `Picker*` commands become thin aliases, deleted in U6.
  - L5 → "Make `Editor::begin` debug-assert that no transaction is open **outside a live span**."
  - L4: `SessionKind { Picker(PickerTarget), Scrub(FieldKey), Typing(FieldKey), Rename(RenameTarget) }`, with no `Drag`. The **only** code that issues `LiveBegin`/`LiveCommit`/`LiveCancel` is the dispatch that creates and ends the UI `EditSession`. Core stays the single owner of "a transaction is open".
  - `SessionOp::Open` must carry `(SessionKind, modal)`.
  - Order: this core piece comes **before** U3-A (the scrub needs it), not inside U3-E.

**3 · P1 · The per-frame `Snap` cost is unmeasured, and the proposed cache key is wrong.**
- Today `Snap::read` (`ui.rs:656-724`) calls `obj_bbox` and `obj_local_dims` on every frame. As measured above, that is ≈360 ms per frame at a 10k-path Select All.
- R7's key `(session, rev, live_rev)` leaves out the **selection**, and a selection change does not bump `rev`. That is why `layer_rows_key` hashes `objsel` and `selected` explicitly (`ui.rs:345-351`).
- So there are two outcomes:
  - Cache per the spec, and X/Y/W/H and the Layers highlights go stale after a click.
  - Don't cache, and the app runs at about 3 fps.
- L24's 10k test ("builds ≤ visible rows") covers widget building, not `build_layer_rows` flattening. With `live_rev` in the rows key, every drag frame re-flattens all 10k rows (audit 01-A6 again).
- *Change:* add a law to §2A:
  - **"L9b · An idle frame costs O(visible), not O(document): `Snap` parts are memoised on `(SessionId, rev, live_rev, sel_rev)`, Layers rows on `(SessionId, rev, sel_rev, DocUi key)` without `live_rev`, and thumbnails are built lazily for visible rows."**
  - *Test:* memo-hit counters show zero recomputes on a second no-input frame. A release-mode 10k Select All `Snap` build ≤ 1 ms is recorded in GATE_LOG (a measurement, not a CI timing assert).
  - Core additions: `Editor::sel_rev`, bumped by every selection mutation. `pidx` becomes O(1): an id→index map rebuilt in `sync_tree`, or an equivalent.
  - R7 ("measure … after U3-C") moves to U2-A as a stop condition.

**4 · P2 · `Op` should die first, and no new command categories should be added.**
- `apply_ops` (`ui.rs:5768-5850`) is 48 arms. 40 are pure `EditCommand` 1:1. The only real translation is:
  - drop zone `u8`→`DropPos`;
  - rename-by-node-kind, which reads `ed.doc`;
  - `MTarget`→live command;
  - 8 declared `Editor` calls.
- L3 adds `Transient(TransientCmd)` and `View(ViewCmd)`, which makes 4 categories. ADR-0002 names two, and audit 01-M7 flagged exactly this gap. `EditCommand` already carries non-history state commands (`SetSnapConfig`, `ToggleSnapping`, `ToggleSmartGuides`, `ToggleGuidesLocked`). ADR-0002 defines it as "deterministic `Document`/`Editor` edits".
- *Change:*
  - `pub enum Request { Edit(EditCommand), App(AppCommand) }`.
  - The 8 transient setters become no-history `EditCommand`s: `SetTool`, `SetPaintFocus`, `SelectLayers`, `ToggleLayerSelected`, `SetGuidePreview`, `ClearOriginPreview`, `ToggleGuidesVisible`, `ToggleRulersVisible`. Also add `SetConstrainWh` (see 8).
  - `ViewCmd` goes under `AppCommand::View(ViewCmd)`, added the same way S2/S3 add variants. `Lifecycle::run` ignores it, just as it ignores `Window(_)`.
  - Drop "`Op` kept as a translation shim until U3 ends" (U2-A). Replace it with **U2-A0 (S)**, a mechanical `Op`→`Request` rewrite: the drop zone becomes `DropPos` at the panel, `LRow` carries `RenameTarget`, and `MTarget` becomes `PickerTarget`.
  - Without this, all six parallel U3 pieces edit the same `Op` enum and `apply_ops` block.
  - This reopens F4_DESIGN's gray-state ruling. Record it in F4_DESIGN with the date. No ADR change is needed.

**5 · P2 · `resolve(id, &Snap)` cannot be called where it is needed.**
- The host key path (`route()` in `main.rs`) and native-menu enablement (L11) run outside a UI frame. `Snap<'a>` borrows UI caches.
- *Change* (§3.2):
  - `resolve(id: CommandId, cx: &CmdCtx) -> Result<Request, Disabled>`. `CmdCtx` is a `Copy` struct of about 10 bools built by the host from `Workspace` + `FocusSnap`: `start_shown`, `text_focused`, `modal_open`, `popup_open`, `has_doc`, `has_selection`, `can_undo`, `can_redo`, `tx_open`, plus `os`.
  - `lookup`'s `KeyCtx` is folded into it, and `Snap.frame` embeds the same value.
  - This is also audit 04's "`dispatch(cmd, ctx)` … without an EventLoop".

**6 · P2 · "DocUi … swapped by `document_switched`" (L8, U2-A) conflicts with frozen S1 semantics.**
- `document_switched()` takes no id. It runs after **every** lifecycle command, including Save, and before the next `set_tabs` reveals the new id (S1 §3.5). `add_loaded` also reuses the same id for a pristine tab.
- So the swap cannot pick the right entry. And today's body (`ui.rs:1197-1203`), which clears collapse and search, would wipe per-tab state on every ⌘S, breaking L8's own test.
- Also, `num_field` edit buffers live in egui memory keyed by field, not by session. A typed-but-uncommitted X in tab A reappears in tab B after Ctrl+Tab.
- *Change* (L8): "`set_tabs` selects `DocUi` by the active id and drops entries whose id left the strip. `document_switched` clears only caches and in-flight gestures (rows, thumbs, `lay_drag`, anchor), never user state. Every stateful widget id is salted with `SessionId` (L24)."

**7 · P2 · L4 contradicts §3.2 about rename and typed fields on a lifecycle command.**
- L4: "a non-modal one (scrub, drag, rename) **commits** before any foreign Request runs". §3.2: `settle` "discards rename buffers — as S1 defines" (`ui.rs:1187-1194`).
- *Change:* pick one and write it in both places. **Default:** a non-modal session commits, and `settle` cancels only modal sessions. This matches the Mac field editor, where ⌘S keeps what you typed. It changes the documented behaviour of the S1-A stub, not its signature. (Needs Ahmed #1.)

**8 · P2 · The W/H lock needs a single owner in core, not a command argument.**
- L23 and §3.2 put `keep_ratio` in `SetObjectBounds`. But canvas scale drags read `Editor::constrain_wh` (`editor.rs:3908`, `:3997`). Moving the lock into the command alone regresses A12, and copying it every frame breaks L1.
- `SetObjectBounds` **already** carries `anchor_x/anchor_y` (`command.rs:15-22`). The bar bug 01-I1 is UI-only: it passes (0,0).
- *Change:* "`Editor` owns `constrain_wh`, set by `EditCommand::SetConstrainWh` on toggle. `SetObjectBounds` reads it; the reference point is already an argument." Delete "`SetObjectBounds { rect, anchor: RefPoint, keep_ratio }`" from the core additions.

**9 · P2 · Remove the `Panel` trait: it cannot back a registry, and it duplicates the metadata.**
- `trait Panel { const ID; const SCROLL; type Local; fn min_size() }` is not object-safe, so a "PANELS registry" of trait objects is impossible. It would be a `match` with one consumer per impl.
- `registry.rs:44-55` already owns `min_size()` and `self_scrolling()`. The trait would make two homes for the same facts.
- *Change* (§3.2 `ui/panel.rs`): "`pub fn draw(id: PanelId, ui, doc: &mut DocUi, app: &mut AppUi, snap: &Snap, out: &mut Out)` — one `match`. The metadata lives only in `registry.rs`. The drag-ghost re-entry (`boxtree.rs:119,307`) passes a scratch `Out` that is dropped." L21 keeps the contract as a law.

**10 · P2 · The plan's pieces are over the day cap and collide with in-flight work.**
- U2-A (5 modules, a render hook, `DocUi`, and a `Snap<'a>` rewrite of the 48-`let` prologue at `ui.rs:1238-1300`) is L, not M. U3-E mixes a core primitive with the 418-line picker.
- *Change:* U2-A0 = `Op`→`Request` (S, finding 4); U2-A1 = `snap.rs` + memo + core `sel_rev`/`pidx` (M, finding 3); U2-A2 = frame, panel, `DocUi` (M). U3-0 = the core live span (M, finding 2), before U3-A.
- E1 (`start_ui::draw -> Vec<StartAction>`, in flight) is a second outbox. Name one adapter in `ui/dispatch.rs`, `StartAction → Request`, owned by E2.
- The caches that `Snap` borrows must sit outside the `&mut UiState` that `frame()` takes (for example a separate `SnapCache`), or the borrow juggling at `ui.rs:1238-1300` comes back.

**11 · P3 · Sharpen the text of several laws and tests.**
- **L2:** "the only UI file that names `Editor`" is false. `ui.rs::settle(&mut Editor)` (frozen) and `ui/dispatch.rs` must name it, and the grep `ed\.` matches `selected.`. Change it to "outside `ui/snap.rs`, `ui/dispatch.rs` and the S1 adapter methods; grep `\bEditor\b|\bed\.`".
- **L3:** the grep `Op` matches `Option`. Use `\benum Op\b|\bapply_ops\b`.
- **L1:** the test should say "a `UiFrame::frame` with no input". The host legitimately applies `fit_pending`.
- **§3.2:** `Snap::default()` cannot default a `&Platform`. Use `Platform::FIXTURE: &'static`.
- **§3 flow:** add "after dispatching a non-empty batch the host requests one repaint". Immediate mode drew this frame from the pre-dispatch `Snap`.
- **§3.3 testkit:** egui shapes carry no widget key. Either the kit records keys through a side channel, or the golden reads the AccessKit tree. egui already compiles `accesskit` 0.24.1 (`Cargo.lock`), so this costs no new dependency and prepares R8. Verify the egui 0.35 API in U0-D.

## Whole-spec answers (from this angle)
1. **Laws:**
   - Merge L5 into L4 (finding 2). Add L9b (finding 3).
   - Sharpen L2, L3, L8 and L23 (findings 6, 8, 11).
   - The contradictions I found are L4↔§3.2 (finding 7), L8↔S1 `document_switched` (finding 6), L3↔ADR-0002 (finding 4) and L23↔canvas lock (finding 8).
   - Every other §2A–B law traces to a finding and is testable.
2. **Simplest architecture:**
   - Drop `Transient`/`View`, keeping `Request` = 2 variants.
   - Drop the `Panel` trait.
   - The lib kit becomes action-agnostic.
   - `UiFrame` earns its place only for cross-surface tests (the Esc ladder, popups). Per-panel goldens need `draw()` alone.
   - No new dependency is needed; I agree that egui_kittest is not needed.
3. **Plan:** re-cut per finding 10. U0-A…C can start now. U0-D is blocked by finding 1 until U1-A lands `act.rs`.
4. **Decisions Q1–Q5:** these are visual and UX questions, and they are fair. From this angle, Q1's "U0 starts now" must say "U0-A…C". One question is missing (below).
5. **World-class reviewer.** A world-class reviewer would ask for four missing pieces:
   - A written **performance budget** (idle frame = no document-sized work; interaction frame ≤ 16 ms at a stated document size). Figma and Blender treat this as a law.
   - A first-class **undo-coalescing model**: the live span is *the* primitive of a pro editor.
   - A **semantic tree** (AccessKit) as the test surface.
   - **Cached enablement** for native menus (the `validateMenuItem` pattern) instead of recomputing it on every frame.

## Needs Ahmed
1. You are typing a layer or artboard name, or a number, and press ⌘S or switch tabs. Should Varos **keep what you typed** (the Mac way) or **throw it away** (as S1 does today)? **Default: keep it** (finding 7).
2. No other new questions. Findings 1–6 and 8–11 are technical, and the moderator and Codex can settle them.
