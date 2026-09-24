> **Status:** current — work order (charter §3 level 4), derived from docs/specs/DOCUMENT_FILE_SYSTEM.md; owner decisions D1–D3 recorded 2026-09-24.
# DFS S1 — AppCommand + lifecycle + real tab identity

Date: 2026-09-24 · Base: branch `claude/sweet-cerf-1sg30t` @ `56516a9` · Spec: `docs/specs/DOCUMENT_FILE_SYSTEM.md` §2 "Lifecycle, identity and tabs", §3 "State machine" + "ONE-HOME", §4, §5 row S1.
Owner decisions (Ahmed, 2026-09-24): **D1** yes (one system, real tabs, F4.2 absorbed; the Windows exception concerns S4 only — S1 does no Windows-specific work), **D2** yes (Start/recovery are S2/S3, not here), **D3** yes (format v2 is S5, not here).

## 1. Goal & acceptance

Every tab is a real, independent document: its own `Editor` (document + history + selection), `View`, file path, saved-content checkpoint and dirty state. New / Open / Save / Save As / Close Tab / Quit go through one `AppCommand` path from keys, the native macOS menu, the burger menu and the tab strip.

**Ahmed hand-test (Mac, after all four pieces are merged):**
1. ⌘N (and `+`, and File ▸ New) → a new clean, boardless `Untitled-2` tab; the first tab keeps its art. Close it, ⌘N again → `Untitled-3` (numbers are never reused).
2. Draw red in A, ⌘N, draw blue in B. Switch tabs by clicking: each tab shows only its own art. ⌘Z in A undoes only A. Drag B's chip before A → the order changes and nothing else does.
3. Save A and B (the first Save opens **Save Varos Document**). Quit, reopen both files → correct tab names and content.
4. Pan, fit (⌘0), zoom, select, click another artboard, toggle rulers/guides/snapping, change units → no dot on the tab and no `*` in the title. Edit a saved file → dot appears; ⌘Z back to the saved state → dot disappears.
5. Close a dirty tab (× / middle-click / ⌘W) → **Save changes to “name”?** Save · Don't Save · Cancel — try all three. Close an inactive dirty tab → the prompt names that tab, and it is that tab that gets saved.
6. Save As, then cancel → nothing changes. Save to a folder you cannot write to → **Couldn't save “name”.** with Try Again · Save As… · Cancel; the tab stays dirty.
7. Two dirty tabs, ⌘Q → prompt “Document 1 of 2”, Save; on the second press Cancel → the app stays open, the first file stays saved, the second tab is still dirty. The same happens with the red traffic light.
8. ⌘O a file that is already open → its tab is focused and not reloaded. ⌘O a broken file → **Couldn't open “name”.** and all tabs stay as they were.
9. Export button and burger ▸ Export… look disabled and say “not available yet” on hover. Clicking them does nothing.

**Headless proof (no GPU, no EventLoop):** tests use fake dialog and file-store ports. They cover save failure and cancel, Save As cancel, the quit transaction (Cancel on the second document), undo back to saved content = clean, view-only actions never dirty, independent history per tab, open dedup, a failed open changing nothing, the clipboard surviving tab switch and close, and never-reused untitled numbers. Golden/F3 fixtures stay unchanged (`varos-core/tests/golden.rs`, `history_lifecycle.rs`, `ui.rs::characterization_tests`).
**Closes:** Astra F01 (extended to all tabs) and F02. It also isolates and resolves the view-dirty observation (§3 F10).

## 2. Current-code findings (evidence at `56516a9`)

- **F1: one document, as locals.** `main.rs:857` `let mut ed = Editor::new()`; `:917–918` `cur_file`, `saved_rev`; `view` at `:885`. Every event arm borrows these directly. `OpenDocContext` (`:561–700`) bundles `ed/gui/window/view/cur_file/saved_rev` and is rebuilt at 5 call sites (`:957`, `:973`, `:1004`, `:1050`, `:1335`).
- **F2: fake tabs.** `ui.rs:820–821` `tabs: Vec<String>`, `tab_active: usize`, initialised `["Untitled-1"]` (`:1062`). `set_doc_tab` (`:1144–1150`) overwrites index 0 only. `build_topbar` (`:3294–3470`): `+` pushes `Untitled-{len+1}` (`:3394`); the × removes a label (`:3402–3409`). No document changes behind either.
- **F3: ⌘N does nothing.** `apply_key` (`main.rs:195–271`) and `OpenDocContext::shortcut` (`:657–690`) have no KeyN/KeyW branch. The chrome test *requires* KeyN to be absent (`chrome.rs:466–473`). `egui_key` (`chrome.rs:323–346`) and `mac_menu::muda_code` (`mac_menu.rs:16–40`) have no KeyN.
- **F4: burger rows are visual only.** `ui.rs:3415–3422`: `menu_row` returns a click `bool` (`:3235`) that is ignored for New/Open/Save/Export. The Export top button (`:3358`) is drawn enabled and does nothing. This is an "enabled dead button", which spec §2 forbids.
- **F5: ⌘W = quit.** The native File menu row `file.close` “Close Window” ⌘W and `app.quit` ⌘Q both use `MenuCmd::Close` (`chrome.rs:231,242`), which runs `confirm_quit` (`main.rs:971–995`). The test pins this (`chrome.rs:456–465`).
- **F6: Open replaces.** `load_path` (`main.rs:636–655`) calls `Editor::replace_doc`. That clears history (`editor.rs:3106–3123`) and bumps `rev`. The guard `confirm_discard_unsaved` (`:477–486`) is a Yes/No **discard** prompt. The Open dialog is single-file (`:675`).
- **F7: Save.** `OpenDocContext::save` (`:573–603`) offers `.vrs` **and** `.pdf` filters. It keeps a `.pdf` extension and records `saved_rev = ed.rev`. Failure shows “Save failed: …” with OK only.
- **F8: dirty = revision inequality.** `rev` bumps on commit (`editor.rs:3049–3062`), undo (`:3064–3071`), redo (`:3072–3079`) and `replace_doc`. `unsaved = ed.rev != saved_rev` (`main.rs:1288`, `:608`). Undo back to the saved state therefore stays dirty. So do no-op commits: GATE_LOG batch 1 names a board-handle drag without movement and an unchanged board-name commit. So does `CycleUnits` (`editor.rs:2170–2175`, a history step) and `SetMoveArtWithArtboard` (`:2176–2184`, a history step pinned by `tests/boards.rs:112–125`).
- **F9: preferences live in the serialized Document.** `Document` (`model.rs:498–548`) holds `active_layer`, `ids`, `units.display`, `active`, `move_art_with_ab`, `snap`, `ruler_origin` and `guides_locked` next to content. `SetSnapConfig`, `ToggleSnapping`, `ToggleGuidesLocked`, `ToggleSmartGuides`, `SetRulerOrigin` and `SetActiveArtboard` write the doc without history (`command.rs:174,187–197`; pinned by `tests/edit_command.rs:45–60`). `Ui::run` executes `SetSnapConfig` **every frame** (`ui.rs:1408`). Undo restores the whole snapshot, so it also rolls back these non-history preferences. `Document` derives `PartialEq` (`model.rs:498`).
- **F10: view-dirty observation.** The board-click cause was fixed in batch 1 (`83363df`). The remaining false dirties are the no-op commits and preference history steps in F8. A content checkpoint removes all of them (§4.1).
- **F11: the clipboard is per-Editor.** `editor.rs:372–374` `clipboard: Clipboard` (private). Its comment says it survives `replace_doc`. With several Editors it would be lost on tab switch.
- **F12: caches that would lie after a switch.** `scene_signature` hashes `ed.rev`, the view, the tool and the selection, and only the *live* paths' content (`scene.rs:75–135`). Two tabs with equal `rev`/view can therefore collide, and `render_ui_cached` (`main.rs:1386–1395`) would draw the other tab's art. `layer_rows_key` has the same flaw: it starts from `ed.rev` (`ui.rs:320–345`, used at `:1221–1224`). Ui-side per-document state would also leak across tabs: the colour-picker modal holds an open `PickerBegin` transaction on *its* Editor (`ui.rs:1422`), plus `lay_rename`, `ab_name_edit`, `lay_drag`, `lay_collapsed` and `lay_search` (`:1392–1397`).
- **F13: file arguments.** The first instance never opens its own file argument. `file_arg` (`main.rs:746`) is only forwarded (`single_instance.rs:90–111`). Forwarded paths arrive through `take_pending_file_paths` → `open_path` (`main.rs:1003–1015`).
- **F14: quit guard.** The quit guard from `953bca2` is single-document: `may_quit` / `QuitAnswer` / `quit_answer` (`main.rs:488–528`) plus 4 tests (`:1701–1761`). Its rfd result mapping is reusable.
- **F15: tab overflow.** `topbar_layout` stops placing tabs when the row is full and drops `+` (`chrome.rs:54–96`), so extra tabs become unreachable.

## 3. Design (minimal, faithful to spec §3)

Seam: `varos-core` gains only pure helpers: content comparison, clipboard hand-off, and preference carry-over on undo. Everything else lives in `varos-app`. No new crate, no dependency edge change, no ADR (ADR-0002 already authorises `AppCommand`).

### 3.1 Content checkpoint (core) — the chosen dirty rule
- `Document::content_eq(&self, other: &Document) -> bool` in `model.rs`. It compares **authored content**: `paths`, `groups`, `group_of`, `nodes`, `roots`, `artboards`, `guides` and `units.ppi`. It **excludes** `active_layer`, `ids`, `units.display`, `active`, `move_art_with_ab`, `snap`, `ruler_origin` and `guides_locked`. This is spec §2's dirty list. NaN compares unequal, so it can only produce a false *dirty*, never a false clean.
- A session keeps `saved: Document` (a clone taken at open, save and New). Dirty = `!editor.doc.content_eq(&saved)`, plus an overlay: `editor.transaction_open() && editor.dirty` (a changed gesture still in flight). The result is memoised on `editor.rev`. Every Save / Close / Quit decision recomputes it exactly and ignores the memo.
- **Why this and not undo-stack position or a hash:**
  - One rule handles all of these without touching history semantics: undo or redo back to saved, no-op commits, preference-only history steps (units, move-art) and any view action.
  - It is exact, so there is no hash-collision risk of a false clean.
  - Cost: one O(n) `PartialEq` walk per `rev` change (no serialization) and one extra `Document` per tab. History already holds up to 200.
  - A state-ID scheme would still mark no-op commits and units changes dirty. It would also miss any content write that skipped `commit`.
- Undo/redo carry the **non-history** preferences forward (`snap`, `guides_locked`, `ruler_origin`) from the current doc into the restored snapshot (spec: "undo must preserve current navigation/preferences"). `active`/`active_layer` stay history-restored because tests pin them. Units and move-art stay undoable steps; they just never dirty.
- Also: `Editor::transaction_open(&self) -> bool` (`pending.is_some()`), `Editor::take_clipboard(&mut self) -> Clipboard`, `Editor::set_clipboard(&mut self, c: Clipboard)`.

### 3.2 App vocabulary — `varos-app/src/app_command.rs` (new, frozen by piece A)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)] pub struct SessionId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum OpenOrigin { Dialog, CommandLine, OsHandoff }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum WindowCmd { Minimize, ToggleMaximize, ToggleRail, ToggleDock, TogglePanel(varos_app::shell::PanelId) }
#[derive(Clone, Debug, PartialEq)]
pub enum AppCommand {
    NewDocument, OpenDialog, OpenPaths(Vec<std::path::PathBuf>, OpenOrigin),
    Save(SessionId), SaveAs(SessionId), CloseDocument(SessionId), Quit,
    ActivateDocument(SessionId), ActivateNext, ActivatePrevious, ReorderDocument(SessionId, usize),
    Window(WindowCmd), // F4.2 window/panel effects, performed by the host
}
#[derive(Clone, Debug, PartialEq)] pub struct TabView { pub id: SessionId, pub label: String, pub dirty: bool, pub tooltip: String }
```
There is no Export command: Export stays unavailable until S6 (§3.6). In S1, Close Window = Quit (one window, §6 Q2).

### 3.3 Workspace — `varos-app/src/workspace.rs` (new; implemented fully by piece A)
```rust
#[derive(Clone, Debug, PartialEq, Eq)] pub struct FileKey { pub path: PathBuf, pub dev_ino: Option<(u64, u64)> }
impl FileKey { pub fn same_file(&self, o: &FileKey) -> bool } // compare dev_ino when both are Some, else compare path
pub struct DocumentSession { pub id: SessionId, pub editor: Editor, pub view: View, pub path: Option<PathBuf>,
    pub key: Option<FileKey>, pub untitled: Option<u32>, pub fit_pending: Option<f32>, saved: Document, memo: Cell<Option<(u64, bool)>> }
impl DocumentSession {
    pub fn display_name(&self) -> String;   // "Untitled-3" | file name WITH extension ("Logo.vrs")
    pub fn is_dirty(&self) -> bool;         // memoised on editor.rev (+ open-transaction overlay)
    pub fn is_dirty_exact(&self) -> bool;   // fresh compare, used for every decision
    pub fn is_pristine(&self) -> bool;      // path None && !is_dirty_exact() && !editor.transaction_open()
    pub fn mark_saved(&mut self, path: PathBuf, key: FileKey); // path/name set; checkpoint = current content; untitled = None
    pub fn settle(&mut self);               // drag/ab_drag in flight → editor.pointer_up() (finish the gesture)
}
pub struct Workspace { sessions: Vec<DocumentSession>, active: SessionId, next_id: u64, next_untitled: u32 }
impl Workspace {
    pub fn new() -> Self;                                   // exactly one pristine Untitled-1, fit_pending Some(0.45)
    pub fn active_id(&self) -> SessionId; pub fn active(&self) -> &DocumentSession; pub fn active_mut(&mut self) -> &mut DocumentSession;
    pub fn get(&self, id: SessionId) -> Option<&DocumentSession>; pub fn get_mut(&mut self, id: SessionId) -> Option<&mut DocumentSession>;
    pub fn sessions(&self) -> &[DocumentSession]; pub fn index_of(&self, id: SessionId) -> Option<usize>;
    pub fn new_untitled(&mut self) -> SessionId;            // appended + activated; numbers never reused
    pub fn add_loaded(&mut self, doc: Document, path: PathBuf, key: FileKey) -> SessionId; // replaces the ACTIVE tab only if pristine, else appends; activated; fit_pending Some(0.9)
    pub fn find_file(&self, key: &FileKey) -> Option<SessionId>;
    pub fn activate(&mut self, id: SessionId) -> bool;      // hands over clipboard + tool + recent_colors; resets incoming mods/space
    pub fn activate_relative(&mut self, step: isize) -> bool; // wraps around
    pub fn reorder(&mut self, id: SessionId, to: usize) -> bool;
    pub fn remove(&mut self, id: SessionId) -> bool;        // right neighbour else left; last tab → a fresh pristine Untitled-N (no-op if it already was pristine); clipboard handed over first
    pub fn tabs(&self) -> Vec<TabView>;                     // equal file names get " — <parent folder>"; tooltip = full path or "Not saved yet"
}
```
The loaded Editor is `Editor::new()` + `replace_doc(doc)`. The checkpoint is cloned **after** `replace_doc` (post `sync_tree`).

### 3.4 Lifecycle coordinator — `varos-app/src/lifecycle.rs` (API frozen by A; bodies by B)
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum SaveDecision { Save, DontSave, Cancel }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum SaveFailChoice { TryAgain, SaveAs, Cancel }
pub trait Dialogs {
    fn pick_open(&mut self) -> Vec<PathBuf>;                                  // multi-select; empty = cancelled
    fn pick_save(&mut self, suggested: &str, dir: Option<&Path>) -> Option<PathBuf>;
    fn ask_save_changes(&mut self, name: &str, progress: Option<(usize, usize)>) -> SaveDecision; // Escape = Cancel
    fn save_failed(&mut self, name: &str, reason: &str) -> SaveFailChoice;
    fn open_failed(&mut self, name: &str, reason: &str);
    fn confirm_replace(&mut self, name: &str) -> bool;                        // only when WE changed the chosen name
    fn notice(&mut self, title: &str, body: &str);
}
pub trait DocStore {
    fn load(&mut self, path: &Path) -> Result<Document, String>;
    fn save(&mut self, doc: &Document, path: &Path) -> Result<(), String>;
    fn key(&self, path: &Path) -> FileKey;   // absolute + canonical (parent canonical for a new file) + dev/ino on unix
    fn exists(&self, path: &Path) -> bool;
}
#[derive(Debug, Default, PartialEq)] pub struct Effect { pub exit: bool }
pub struct Lifecycle<'a> { pub ws: &'a mut Workspace, pub dialogs: &'a mut dyn Dialogs, pub store: &'a mut dyn DocStore }
impl Lifecycle<'_> { pub fn run(&mut self, cmd: AppCommand) -> Effect; } // Window(_) is ignored here (host-owned)
```
Rules (spec §2/§4):
- **Open:** each path gets a key. If a tab already has that file, that tab is focused and nothing is reloaded, even when it is dirty. Otherwise the file is loaded into a candidate. Success → `add_loaded`. Failure → `open_failed`, and no tab, path, selection or history changes.
- **Save:** if there is no path, or the path's extension is not `vrs` → Save As (a `.pdf`-opened doc suggests `<stem>.vrs`).
- **Save As:**
  - Suggest `<display stem>.vrs` in the file's folder.
  - If the chosen name lacks `.vrs`, append it. Never silently drop part of the name. If the appended name exists → `confirm_replace`.
  - A target whose key matches **another** open tab is refused with `notice` (“… is open in another tab…”).
  - Path, name and checkpoint change only after the store succeeded.
  - On failure → `save_failed` loop (TryAgain / SaveAs / Cancel).
- **CloseDocument(id):** clean → remove. Dirty → ask about **that** tab by name, without activating it. Save (may go through Save As) → remove only on success. DontSave → remove. Cancel → keep.
- **Quit:** collect the dirty tabs in tab order. For each (i of n): activate it and ask. Save must succeed, or the quit aborts. DontSave is recorded. Cancel aborts, earlier saves stay saved, and nothing is removed. `exit` is true only when every decision resolved.

### 3.5 Host (main.rs) — one dispatch
- Every source produces an `AppCommand`: keys, the native menu, `gui.take_app_commands()`, `WinAction`, `CloseRequested`, OS handoff (`OpenPaths(.., OsHandoff)`) and the startup `file_arg` (`OpenPaths(.., CommandLine)`, once, after the first framed frame).
- Before any lifecycle command: `gui.settle(&mut active.editor)` then `active.settle()`.
- After it:
  - Reset the editor's `mods` (native dialogs eat key releases).
  - If the active id changed → `gui.document_switched()`, `canvas_gesture = false`, `panning = false`.
  - If `exit` → save window state, then `elwt.exit()`.
- The scene cache key becomes `hash((active_id, scene_signature(..)))` (F12).
- Every frame:
  - `gui.set_tabs(ws.tabs(), ws.active_id())`.
  - Window title `"{name}{*} — Varos"`; on macOS also `set_document_edited(dirty)`.
  - Apply `fit_pending` once `gui.board_px` is known. This replaces `board_fit_pending`.

### 3.6 UI (ui.rs / chrome.rs)
- **Tab chips:**
  - One chip per `TabView`, with a neutral dirty dot (MUTED, never azure) before the name and a tooltip.
  - Click → `ActivateDocument`. × or middle-click → `CloseDocument`. `+` → `NewDocument`.
  - Drag a chip → `ReorderDocument(id, tab_drop_index(..))`, with a 1 px TEXT insertion mark (no animation, no shadow).
  - `topbar_layout` reserves `+` before placing tabs.
- **Burger:** New / Open… / Save / Save As… emit commands. Export… is a disabled row.
- **Top Export button:** disabled look (FAINT, hover only), tooltip “Export isn't available yet — PDF export comes in a later update. Save keeps an editable .vrs.” There is no native Export row until S6.
- **Native File menu:** New ⌘N · Open… ⌘O · ─ · Close Tab ⌘W · Save ⌘S · Save As… ⇧⌘S (all `MenuCmd::Key`, the same key path). Varos ▸ Quit ⌘Q → `MenuCmd::Quit`. Ctrl+Tab / Ctrl+⇧Tab switch tabs (keyboard only in S1).
- **ONE-HOME:** the tab strip is the home for activate/close/reorder. New/Open/Save rows are mirrors whose homes (Start S2, Document section S2/S3) do not exist yet. They stay mirrors of the same commands.

## 4. Pieces

Run A first. Then B, C and D run **in parallel** in separate worktrees branched from A's merge. D merges last.

### S1-A — Core checkpoint + frozen app API + workspace model · **opus** · M · merges first
- **Owns:**
  - `varos-core/src/model.rs` (`content_eq`) and `varos-core/src/editor.rs` (`transaction_open`, `take_clipboard`, `set_clipboard`, preference carry in `undo`/`redo`).
  - New `varos-core/tests/content_checkpoint.rs`.
  - New `varos-app/src/app_command.rs` and `workspace.rs` (full), and `lifecycle.rs` (types, traits, `Lifecycle`, stub `run`).
  - `ui.rs` additions next to `set_doc_tab` (`:1144`): fields `doc_tabs: Vec<TabView>`, `doc_active: Option<SessionId>`, `app_cmds: Vec<AppCommand>`, and:
    - `pub fn set_tabs(&mut self, tabs: Vec<TabView>, active: SessionId)` — stub: also mirrors the labels into the old `tabs: Vec<String>`.
    - `pub fn take_app_commands(&mut self) -> Vec<AppCommand>`.
    - `pub fn settle(&mut self, ed: &mut Editor)` — full: `PickerCancel` if the modal is open, then drop the modal, `lay_rename` and `ab_name_edit`.
    - `pub fn document_switched(&mut self)` — full: clear `layer_rows_cache`, `lay_drag`, `lay_anchor`, `lay_collapsed` and `lay_search`.
  - `chrome.rs`: rename `MenuCmd::Close` → `MenuCmd::Quit` and its test assertion. `main.rs`: the three `mod` lines and the `M::Close` arm rename only.
- **Must not touch:** `build_topbar` / `tab_item` / menus content, `apply_key` / `shortcut`, golden fixtures, `characterization_tests`.
- **Steps:**
  1. Add the core API and its tests. Existing core tests must stay green unmodified. If one needs a change, stop and report.
  2. Add `app_command.rs` exactly as in §3.2. Add `workspace.rs` exactly as in §3.3.
  3. Add `lifecycle.rs` exactly as in §3.4. The stub `run` exits on `Quit` only when no session `is_dirty_exact()`, and does nothing for everything else. Mark it `// S1-B replaces`.
  4. Put `#![cfg_attr(not(test), allow(dead_code))]` at the top of the three new app modules (D removes it). Add the Ui stubs.
- **Tests:**
  - Core:
    - `content_eq_ignores_view_and_preference_fields`
    - `content_eq_sees_every_authored_change` (path geometry, paint, node hide/lock/name, artboard rect/colour/clip/hidden, guide, PPI)
    - `noop_commit_and_cancelled_picker_stay_content_equal`
    - `undo_and_redo_back_to_a_checkpoint_are_content_equal`
    - `undo_keeps_current_snap_guide_lock_and_ruler_origin`
    - `clipboard_moves_between_editors`
  - Workspace (in `workspace.rs`):
    - `starts_with_one_pristine_untitled_1`
    - `untitled_numbers_increase_and_are_never_reused`
    - `activate_hands_over_clipboard_and_tool`
    - `remove_picks_right_then_left_and_never_leaves_zero_tabs`
    - `reorder_keeps_identity_and_state`
    - `add_loaded_reuses_only_a_pristine_active_tab`
    - `find_file_matches_same_file_key`
    - `tabs_disambiguate_equal_file_names`
    - `dirty_follows_edit_undo_and_mark_saved`
    - `view_only_actions_never_dirty` (view pan/zoom, `SetActiveArtboard`, rulers/guides toggles, `SetSnapConfig`, `ToggleGuidesLocked`, `CycleUnits`, `SetMoveArtWithArtboard`, board-handle click with no movement, unchanged `RenameArtboard`)
    - `settle_finishes_an_in_flight_drag`

### S1-B — Lifecycle rules + fake-port tests · **opus** · M · depends on A
- **Owns:** `varos-app/src/lifecycle.rs`: `run` bodies, private helpers, and a `#[cfg(test)]` module with `FakeDialogs` (a scripted answer queue that records every prompt) and `FakeStore` (`HashMap<PathBuf, Document>`, per-path load/save failure injection, alias → key map).
- **Must not touch:** any other file. Frozen signatures change only through the moderator.
- **Steps:** implement §3.4 with exact spec §4 copy passed to the ports (the name, the “Document i of n” progress). No rfd, fs or egui in this file.
- **Tests:**
  - `new_command_adds_clean_boardless_untitled`
  - `open_lands_in_a_new_tab_and_dirty_tab_survives`
  - `open_reuses_pristine_untitled`
  - `open_already_open_file_focuses_without_reload` (dirty and aliased)
  - `open_failure_changes_nothing`
  - `open_multiple_paths_mixed_success`
  - `save_untitled_goes_through_save_as`
  - `save_as_cancel_changes_nothing`
  - `save_failure_keeps_path_and_dirty_then_try_again_succeeds`
  - `save_failure_save_as_route`
  - `save_as_moves_path_only_after_success`
  - `save_as_onto_other_open_tab_is_refused`
  - `save_on_pdf_path_offers_vrs`
  - `appended_extension_asks_before_replacing`
  - `undo_back_to_saved_is_clean_end_to_end`
  - `two_tabs_have_independent_history` (red A / blue B / undo A)
  - `close_clean_tab_no_prompt`
  - `close_dirty_tab_save_dont_save_cancel`
  - `close_inactive_dirty_tab_saves_that_tab_only`
  - `close_save_as_cancel_keeps_tab`
  - `quit_clean_asks_nothing`
  - `quit_cancel_on_second_keeps_everything_and_first_stays_saved`
  - `quit_dont_save_all_exits_and_writes_nothing`
  - `quit_save_failure_aborts`
  - `clipboard_survives_close_of_active_tab`

### S1-C — Tab strip, burger, Export honesty, native File rows · **sonnet** · M · depends on A
- **Owns:**
  - `varos-app/src/ui.rs`: `build_topbar`, `tab_item`, the burger `menu_below` block, the Export button, a new `menu_row_disabled`, and the real bodies of `set_tabs` / `take_app_commands`. Replace `tabs: Vec<String>` / `tab_active` with `doc_tabs` / `doc_active`; the tab pushes `AppCommand`s into `app_cmds`.
  - `varos-app/src/chrome.rs`: `menus()` File rows per §3.6, `egui_key` + KeyN/KeyW, `topbar_layout` reserving `+`, tests.
  - `varos-app/src/mac_menu.rs`: `muda_code` + KeyN.
- **Must not touch:** `main.rs`, `lifecycle.rs`, `workspace.rs`, `Ui::settle` / `document_switched`, `set_doc_tab` (D deletes it), `characterization_tests`, `shell/tokens.rs` values (reuse MUTED/FAINT/TEXT/SOLID_PANEL; no new colours).
- **Steps:**
  1. Chips per §3.6, reusing `tab_item` geometry; `Sense::click_and_drag`.
  2. Pure `pub(crate) fn tab_drop_index(tab_rects: &[egui::Rect], pointer_x: f32) -> usize`.
  3. Burger rows → commands. Export disabled with tooltip.
  4. Update the chrome tests: ⌘Q → `Quit`; ⌘W → `Key(W)`; KeyN is now allowed; still no Export row and no ⌘A.
- **Tests:**
  - `tab_drop_index_before_between_after`
  - `plus_is_always_placed_even_with_overflowing_tabs`
  - `file_menu_rows_are_new_open_close_save_saveas_on_their_keys`
  - the updated `the_bar_has_the_standard_mac_menus_…`
  - `every_menu_key_can_be_handed_to_a_text_field` (now with N)
  - mac `every_menu_key_has_a_native_key_equivalent` (type-checked by the Mac-target clippy)

### S1-D — Host integration + real ports · **opus** · L · depends on A; merges LAST (after B and C)
- **Owns:** `varos-app/src/main.rs` and a new `varos-app/src/file_ports.rs`.
  - `RfdDialogs` implements `Dialogs` with spec §4 copy: titles **Open Varos Document** / **Save Varos Document**, filters *Varos documents (.vrs)* + *Varos PDF documents (.pdf)* for Open and *Varos document (.vrs)* for Save, `pick_files` for multi-select. Buttons use `YesNoCancelCustom`. The `quit_answer` mapping and its test move here as `decision_from`.
  - `DiskStore` implements `DocStore` via `varos_pdf::{load_vrs, save_vrs}` + `std::fs::canonicalize` + `MetadataExt` dev/ino under `cfg(unix)`.
- **Post-rebase cleanup, allowed only here:** delete `Ui::set_doc_tab` in `ui.rs`, and the `allow(dead_code)` lines in the three A modules.
- **Must not touch:** anything else in `ui.rs` / `chrome.rs`, and `lifecycle.rs` / `workspace.rs` bodies.
- **Steps:**
  1. Replace `ed` / `view` / `cur_file` / `saved_rev` with `ws: Workspace`. Each event arm uses `ws.active_mut()`.
  2. Delete `OpenDocContext`, `confirm_discard_unsaved`, `may_quit`, `QuitAnswer`, `doc_stem`, the old `full_title` and `board_fit_pending`.
  3. Add a pure `fn lifecycle_key(code: KeyCode, ctrl: bool, shift: bool, alt: bool) -> Option<LifecycleKey>` (N, O, S, ⇧S, W, Tab, ⇧Tab). The shortcut path maps it to an `AppCommand` with `ws.active_id()`. The other keys go to `apply_key` / paste / fit as today.
  4. Add a single `dispatch` per §3.5. Map all sources: `MenuCmd::{ToggleRail, ToggleDock, TogglePanel}` and `WinAction::{Minimize, ToggleMaximize}` go through `AppCommand::Window`; `WinAction::Close`, `CloseRequested` and `MenuCmd::Quit` go through `Quit`.
  5. Add the per-frame tabs/title/fit logic, the scene-key mix, and the startup `file_arg`.
- **Tests:**
  - `lifecycle_key_maps_file_and_tab_keys` (and plain N/O/S/W stay tool keys)
  - `every_file_menu_row_reaches_its_lifecycle_command` (walks `chrome::menus()`)
  - `window_title_names_active_document_and_dirty`
  - `scene_key_differs_between_sessions_with_equal_signature` (two Editors with equal rev/view and different art → equal `scene_signature`, different mixed key)
  - `decision_from_dialog_results_and_unknown_is_cancel`
  - `disk_store_round_trips_through_varos_pdf` (temp dir)
  - `disk_store_key_sees_symlink_alias` (`cfg(unix)`)
  - Existing `menu_mirror_tests` / `clipboard_key_tests` / `instant_zoom_tests` keep passing.

## 5. Merge order and conflict hot-spots

Order: **A → (B, C in either order) → D**.
- Each piece: an own worktree from A's merge commit, moderator diff review, and a `--no-ff` merge into the work branch.
- Gates on every merge:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`
  - `cargo clippy -p varos-app --all-targets --target aarch64-apple-darwin -- -D warnings`

Hot spots:
- `ui.rs`: A adds fields and 4 methods near `:1144`; C rewrites topbar internals and the two stub bodies; D deletes `set_doc_tab`. These are distinct regions. D rebases onto B + C before its cleanup.
- `chrome.rs`: A renames one variant plus one test line; C edits `menus()`, `egui_key` and the tests. C rebases on A (trivial).
- `main.rs`: A touches 4 lines, D rewrites. Only D edits after A.
- `lifecycle.rs`: A writes the skeleton, B fills the bodies, D removes the top `allow`. Take B's file, then apply D's one-line removal.

No behaviour is hand-testable until **all four** are merged. The intermediate branch is safe but incomplete: the stub quit refuses to exit while dirty. The moderator adds GATE_LOG/STATUS entries and the Ahmed checklist (§1) after D. Independent Codex review is required before `main`; it is not available in the cloud, so record it as pending, as in batch 1.

## 6. Risks / open questions (recommended default; execution never blocks)

1. **No Start page yet (S2):**
   - The last tab closes to a fresh pristine `Untitled-N`; closing an already-pristine last tab is a no-op.
   - The red traffic light / Close Window runs the Quit transaction, then exits.
   - The "window closes, app stays" behaviour on macOS comes with S2.
2. **Blocking rfd prompts:** the tab activated behind a quit prompt repaints only after the prompt, because the event loop is blocked. The prompt therefore names the document and “Document i of n”. Async continuations (spec `DialogResolved`) are deferred to S3, where background jobs appear.
3. **App-wide vs per-document state:**
   - App-wide (handed over on activate): clipboard, current tool, recent colours.
   - Per tab: current fill/stroke/weight, rulers/guides visibility, view.
   - Unsaved inline rename buffers are discarded on switch and close; an open colour picker is cancelled (the preview is not a commit).
4. **Undo carry-over** is limited to non-history preferences (`snap`, `guides_locked`, `ruler_origin`). If an existing test breaks, A stops and reports; do not edit the test.
5. **File identity:** canonical path + dev/ino on unix; canonical path only on Windows (compile-only platform). Case-insensitive aliases on APFS are covered by dev/ino.
6. **Tab overflow:** `+` is always reachable. Tabs that do not fit are reachable only with Ctrl+Tab. An overflow menu is a later polish (log it for Ahmed).
7. **Title format change:** the tool name is dropped from the window title (spec naming). The title is hidden on macOS; the tab chip is the visible signal.
8. **`.pdf` Save:** it now routes to Save As `.vrs` (D3). Existing `.pdf` files still open.
9. **Cost:** one `Document` clone per tab and an O(n) compare per `rev` change. Measure on the P11.2 harness scene E before claiming that large files are fine. Record the number in GATE_LOG.
10. **Batch-2 hand-test overlap:** S1 rewrites the `953bca2` quit guard. Batch-2 items 1–2 are superseded by §1 items 5–7; keep 3–10.
11. **Day cap:** if D cannot finish within the stage day, merge A+B+C (tests green, the UI emits commands into a no-op host) and report S1 as **not passed**. Never claim F01/F02 closed without D.

## 7. Out of scope

- Start page, Recent, Home, no-document state (S2).
- Recovery / autosave, durable-write rework, background save jobs, fingerprint conflict checks (S3).
- Finder / Explorer association and macOS open-document events (S4; the existing Windows handoff is only re-routed).
- Format v2, migration, validation, load limits (S5).
- Export home and pure-PDF deliverable (S6); Share button wiring.
- Multiple windows, tab tear-off, cross-window drag.
- Tab context menu and keyboard reorder; tab overflow menu; Window-menu tab rows.
- Accessibility labels beyond tooltips.
- F09 (Artboards) and Astra F04–F08/F10/F11.
- Any change to `shell/tokens.rs` values.
