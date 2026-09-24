> **Status:** reference — independent plan review, 2026-09-24.
# Review — DFS S1: AppCommand + lifecycle + real tab identity

Reviewed: `docs/foundation/work_orders/DFS_S1_LIFECYCLE_TABS.md` @ `c1c7de9`. Code at `475a676`; `git diff 56516a9 HEAD -- varos/crates` is empty, so the WO's line references apply.

**Verdict: APPROVE WITH CHANGES.**
1. The dirty rule is right. `content_eq` against a cloned checkpoint is the simplest exact rule that meets spec §2/§3 (undo back to saved, no-op commits, preference steps). I found nothing better.
2. The quit guard from batch 1 is reused sensibly, not needlessly rewritten. `quit_answer` moves as-is, `QuitAnswer` becomes `SaveDecision` (same three answers), and `may_quit` becomes the multi-document loop the spec needs.
3. Four S1-A contract points must go to A now (F1–F3 below). Each one is a doc-comment or `Option` change, so it is cheap while A is still in flight.
4. One piece cannot compile as written (F4), and native menu rows contradict spec §4 while duplicating S6-C (F5).

## P1 — frozen S1-A API (forward to the S1-A agent immediately)

**F1 (P1) — `FileKey::same_file` gives a false "different file" after every save, so the same file opens twice with two writers.**
- Evidence: every save swaps the file's inode. `save_vrs` → `write_atomic` writes `path.with_extension("vrs.tmp")`, then `rename`s it over the target (`varos-core/src/file.rs:50–54`, `varos-pdf/src/lib.rs:28–30`).
- The frozen rule is "compare dev_ino when both are Some, else compare path" (WO §3.3). So open A, press ⌘S, press ⌘O on A again → the inodes differ → a second tab of A opens.
- The same thing happens after any external app rewrites the file atomically. Spec §2: "aliases must not create competing writers".
- Change §3.3 `// compare dev_ino when both are Some, else compare path` → `// same file if the canonical paths are equal, OR both dev_ino are Some and equal (path first: atomic saves change the inode)`.
- Add to B's Save rules: "after a successful save, recompute `store.key(path)` and pass that key to `mark_saved`."
- Add a B test: `open_after_save_still_focuses_the_same_tab` (FakeStore changes the dev_ino on each save).
- Nit (P3): `FileKey` derives `PartialEq`, so `==` compares path + dev_ino and is a trap. Tell A to document that callers use `same_file`.

**F2 (P1) — `ReorderDocument(id, to)` / `Workspace::reorder(id, to)` do not say what `to` means.**
- C produces `tab_drop_index` (an insertion slot 0..=len in the current order) and A consumes `to`.
- If A reads `to` as the final index, dragging a chip one place to the right is off by one.
- Add to §3.3 `reorder`: `// to = insertion slot 0..=len in the CURRENT order (tab_drop_index); dropping on its own slot or the next is a no-op`.
- Extend `reorder_keeps_identity_and_state` with the move-right-by-one and the no-op cases.

**F3 (P1) — `Workspace.active: SessionId` is non-optional, but S2 needs an empty workspace.**
- Spec §3 defines `active: Option<SessionId>`, and spec §2 says "A clean last tab closes to Start". `DFS_S2_S3…md:110` assumes it "may be empty" and says piece E will otherwise relax it.
- Relaxing it later means E re-edits every `ws.active_mut()` that D writes in `main.rs`, a second pass over the riskiest file.
- With `Option` now, the compiler forces D's event handlers to handle "no document", which is exactly what E needs.
- Change §3.3 to `pub fn active_id(&self) -> Option<SessionId>; pub fn active(&self) -> Option<&DocumentSession>; pub fn active_mut(&mut self) -> Option<&mut DocumentSession>;` and `Ui::set_tabs(.., active: Option<SessionId>)`.
- S1 behaviour stays "never empty" (`remove` still makes a fresh Untitled-N). Only the type is future-proofed.
- If the moderator keeps the current API, record in S2 §3.9 that E owns this rewrite.

## P1 — other pieces (must change before B/C/D start)

**F4 (P1) — S1-C cannot compile as written.**
- C must "Replace `tabs: Vec<String>` / `tab_active` with `doc_tabs` / `doc_active`" but "Must not touch … `set_doc_tab`".
- `set_doc_tab` writes `self.tabs` (`ui.rs:1144–1150`), and `main.rs:1292` still calls it until D lands.
- Change C's "Must not touch" to: "`set_doc_tab` body only: make it update `doc_tabs[0].label` (a shim D deletes)." This also keeps a visible tab on the intermediate branch.

**F5 (P1) — Native File rows as `MenuCmd::Key` contradict spec §4 and duplicate S6-C.**
- Spec §4: "Menus and physical keys dispatch once through command IDs, not synthetic key events."
- With `Key`, File ▸ Save / New / Close Tab do nothing while a text field has focus, because `main.rs:952–958` forwards the key to egui. §3.6 extends this to N (`every_menu_key_can_be_handed_to_a_text_field (now with N)`).
- S6-C (`DFS_S4_S6…md:166–168`) then plans to replace the same rows with `MenuCmd::File(FileCmd)` and `to_app_command`, so the work is done twice.
- Change §3.6: "Native File menu: New ⌘N · Open… ⌘O · ─ · Close Tab ⌘W · Save ⌘S · Save As… ⇧⌘S (all `MenuCmd::Key`, the same key path)" → "(all `MenuCmd::File(FileCmd)`; `FileCmd` lives in `chrome.rs`; they bypass the text-field forward)".
- D step 3: `lifecycle_key(..) -> Option<FileCmd>` (no separate `LifecycleKey`), plus one `fn to_app_command(FileCmd, active) -> AppCommand`. S6-C then only adds Export.
- A's `MenuCmd::Close → Quit` rename can stay; C folds it into `File(FileCmd::Quit)`. Do not forward this to A.

## P2 — should change

**F6 (P2) — dead-code allows in `ui.rs`.**
- A's new `Ui` fields and methods (`doc_tabs`, `doc_active`, `app_cmds`, `set_tabs`, `take_app_commands`, `settle`, `document_switched`) have no caller until D. In a binary crate that fails `clippy -D warnings` on A, B and C.
- Step 4 names only the three new modules. Add: "…and `#[allow(dead_code)]` on the new `Ui` items". D's cleanup list: "…and those `ui.rs` allows."

**F7 (P2) — the active tab can be invisible.**
- `topbar_layout` stops placing chips when the row is full (`chrome.rs:83–91`). Open six files with one multi-select → the active tab (the last one) is not drawn. Spec §4: "Active document name always matches canvas/layers."
- Add to C: "`topbar_layout` always places the ACTIVE tab (it takes the last visible slot on overflow)."
- Add test `active_tab_is_placed_when_tabs_overflow`.

**F8 (P2) — move `file_ports.rs` from D to B.**
- D is the only L piece, merges last and carries the day-cap risk (§6.11). `RfdDialogs` and `DiskStore` are headless-testable and do not depend on `main.rs`.
- `DiskStore::key` is where F1's identity rule lives, next to B's rules.
- B then owns `lifecycle.rs` + `file_ports.rs` and adds one `mod file_ports;` line in `main.rs`, a trivial conflict with D.

**F9 (P2) — switch detection misses in-place replacement.**
- §3.5 calls `gui.document_switched()` only "If the active id changed". `add_loaded` replaces a pristine active tab under the SAME id, so layer caches, `lay_collapsed` and the scene-cache mix are not reset.
- Change to: "After every lifecycle command (not only on id change): `gui.document_switched()` and `last_scene_signature = None`."
- The `(active_id, signature)` hash can stay as a second guard.

**F10 (P2) — two S5 dialogs have no owner.**
- `DocStore::load -> Result<Document, String>` + `open_failed(name, reason)` cannot show spec §4's separate titles ("This file needs a newer Varos.", "This PDF has no editable Varos document.") or the migration notice.
- S5 R8 (`DFS_S5…md:350`) says "wired by S1's open pipeline", but S1 never mentions it.
- Do not change A now. Add to §7: "Typed open errors + the migration notice: a small integration piece after S5 changes `DocStore::load` to return `Loaded`/`LoadError` (2 implementations + the Open rule)."

**F11 (P2) — cross-order drift worth recording, not forwarding.**
- S3 wants a checkpoint identity (`Probe.checkpoint: u64`, `DFS_S2_S3…md:89`), plus `mark_saved(.., saved_snapshot)` for async saves (spec §3 "Saved checkpoint becomes C").
- S2 wants outcome events (`…:115`). S4/S6 assume `dispatch(cmd, &mut AppPorts)`, `OpenPaths { paths, origin }` and `find_by_identity(&Path)` (`DFS_S4_S6…md:51–63`).
- Add one line to §5: "Later orders adapt to S1's names (`Lifecycle::run`, `find_file(&FileKey)`, tuple `OpenPaths`, `transaction_open`); the moderator ticks S2 §3.9 against the merged API."

## P3 — nits
12. §3.1 rationale contradicts itself. It rejects state-IDs because they "would also miss any content write that skipped `commit`", yet the dot is memoised on `rev`, which has the same blind spot. Only the per-frame dot is affected, because decisions use `is_dirty_exact`. Say that. `mark_saved` must also reset the memo; state it in §3.3.
13. Add B tests `quit_untitled_save_as_cancel_aborts` and `close_inactive_untitled_suggests_its_own_name`.
14. F3 finding text is inaccurate: `egui_key` already maps KeyW (`chrome.rs:336`).
15. The top-bar **Share** button is also an enabled dead button (`ui.rs:3357`), which spec §2 forbids. Give it the same disabled look and tooltip in C: 2 lines, no wiring.
16. D: land the mechanical `ed`/`view` → `ws.active…` swap as its own commit (the app must behave identically) before adding dispatch/ports. That way a day-cap stop still leaves a reviewable diff.
17. The stub `Lifecycle::run` Quit rule in A is thrown away by B, and the host never calls it before D. A plain `// S1-B replaces` no-op is enough.

## Checked and fine (no change)
- **Core seam:** `content_eq`, `transaction_open`, `take_clipboard`/`set_clipboard` and the undo carry-over are pure. The carry-over is needed: `Ui::run` reads `ed.doc.snap` at frame start (`ui.rs:1244`) and writes it back (`:1408`), so without carry an undo would roll snapping back.
- **Headless tests:** `Dialogs` + `DocStore` port traits cover every decision the spec lists. No test builds a Renderer or an EventLoop.
- **Tab switch with a text field open:** egui gives up focus on the mouse *press*, and a chip click fires on *release*. So an inspector field or layer rename commits into the outgoing document before `ActivateDocument` runs. `settle` dropping those buffers is only a safety net.
- **Caches:** the flatten cache is per-Editor and keyed by value. Layer thumbnails are keyed by content. Only the scene cache and `layer_rows_key` need the reset in F9.
- **Split:** A → (B ∥ C) → D is right. The regions A, C and D own in `ui.rs` are distinct once F4 is fixed. Model choices are right.

## Needs Ahmed (recommended default in brackets)
1. Red traffic light / Close Window runs the whole Quit until the Start page (S2) exists. [yes, as the WO says]
2. Current fill / stroke / stroke weight: per tab (WO) or shared across tabs like the tool? [per tab; check it at the hand test]
3. Tabs that don't fit: only the active one is guaranteed visible, the others are reachable with Ctrl+Tab, and an overflow menu comes later. [yes]
4. Share button shown disabled with "not available yet", like Export. [yes]
