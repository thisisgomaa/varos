> **Status:** current — work order (charter §3 level 4), derived from docs/specs/DOCUMENT_FILE_SYSTEM.md; owner decisions D1–D3 recorded 2026-09-24.
# DFS work order — S4 file association (macOS + Windows) and S6 Export + system acceptance

Date: 2026-09-24 · Branch baseline: `claude/sweet-cerf-1sg30t` · Planning only: nothing below has been built or run.
Spec rows: §5 S4 and S6; §2 "Association and export boundary"; §6 "macOS process/events" and "Privacy and fidelity".
D1 = YES, including the **narrow Windows exception for S4**. That exception covers the handoff code and the Windows-target compile gate only. **No Windows machine is available for hand-testing.** So the Windows half of P5 stays *pending* until real Windows access exists. A green compile gate does not prove it works at runtime (spec §5 S4).

## 1. Goal & acceptance

**S4 — file association.** A Finder double-click, Finder "Open With", a drop on the Dock icon, an Explorer double-click, or a command-line path all feed one queue. That queue feeds S1's `OpenPaths`. On a cold launch the paths are held until the workspace is ready, so a file is never lost because it arrived early. On a warm launch the same event opens the file in a new tab, or focuses its tab if the file is already open.
- *Ahmed's Mac hand test:* with a dirty A open, double-click B in Finder, both cold (Varos not running) and warm. B opens in its own tab and A survives with its dot. Double-click B again: the same tab is focused and nothing reloads. Select 3 files → Open: 3 tabs. Also try a name with Arabic letters and spaces, and a broken `.vrs`: a readable error, no blank tab, and A is untouched. "Open With ▸ Varos" and a drop on the Dock icon behave the same way. Get Info on a `.vrs` shows Varos as the owner.
- *Windows:* the same list on Explorer. **Pending. There is no test machine.** Only the compile gate runs.
- *Headless proof:* queue tests for early events, duplicates, multiple files, caps, and payload/argv parsing. Workspace tests with S1's fake ports for cold, warm, duplicate, multi-file, bad-file and Unicode paths. Mac and Windows target clippy stay clean.
- *Closes:* P5 (the Mac half; the Windows half stays pending) and the Mac load gap. It re-checks F02.

**S6 — Export and system acceptance.** Export has ONE home: an Export panel in a dock box. The top-bar **Export** button, the hamburger **Export…** row and the native **File ▸ Export…** row all send the same `ShowExport` command. PDF is the only enabled format. Scope is *All visible artboards* (the default), *Active artboard*, or *Artwork bounds* when the document has no boards. The flow is destination → progress with Cancel → result. The exported PDF has **no embedded model and no hidden or editable data**. PNG and SVG rows are disabled and say "Not available yet". The F4.2 file-effect parity audit is done.
- *Ahmed's hand test:* export a two-board file through all three routes. Open the PDF in Preview and check the page count and the art. Try with a hidden board, the active board only, a boardless document, overwrite / cancel / failure. The document's path and dot never change. Then run the whole spec §5 system journey.
- *Headless proof:* the emitted PDF is inspected with lopdf. Checks: page count and sizes; no `/EmbeddedFile`, `/Filespec`, `/AF`, `/Names` or `/VAROS_*`; no model JSON, no node or board names, and no hidden coordinates in any stream; the visible content is present. `load_vrs` refuses the exported file. Export does not touch the session (path, rev, history, checkpoint, recents). The parity table test covers every file command on every route. No GPU readback.
- *Closes:* F03. Final regression check of F01/F02. F09 stays with the Artboards work.

## 2. Current-code findings (read on this branch)

**Association**
- **The cold-launch file argument is never opened.** `main.rs:746–750` reads `file_arg` and passes it only to `acquire_or_forward`. The first instance never loads it, and `file_arg` is not used anywhere else. So "double-click with Varos closed" opens an empty document on every platform.
- `single_instance.rs:11–39`: `first_file_arg` returns only the **first** path. It would also treat a macOS `-psn_…` argument as a file (it skips only `--` flags).
- Windows handoff, `single_instance.rs:67–250`. `send_file_path` uses `fs::canonicalize` (`:224`), which gives `\\?\C:\…` paths that would leak into titles. `receive_file_path` (`:161–192`) checks the tag, parity and a null pointer. It has **no upper bound on `cbData`**, does not reject interior NULs, and does not require an absolute path. `PENDING_FILE_OPENS` (`:73`) is a `Vec` with no cap. The subclass is installed at `main.rs:844`, about 50 lines after the window is created at `main.rs:796`. A handoff in that gap depends on message-pump timing.
- The drain happens at `main.rs:1002–1015`: on `AboutToWait` it calls `open_path` once per path. That runs the old discard-guard (`main.rs:630–634`), which replaces the single document. This is exactly what S1's `OpenPaths` replaces.
- **macOS has no open-document handler at all.** winit 0.30.13 installs its own `WinitApplicationDelegate` as `NSApp`'s delegate: `~/.cargo/registry/src/*/winit-0.30.13/src/platform_impl/macos/event_loop.rs:232–240`. That delegate implements only `applicationDidFinishLaunching:` and `applicationWillTerminate:` (`…/macos/app_state.rs:47–74`). Its own docs at `winit-0.30.13/src/platform/macos.rs:29–30` say winit "will not register an application delegate". **That doc is stale for this version.** Replacing the delegate would break winit's launch path, and the pattern shown in those docs would do exactly that.
- `mac_menu.rs:64–93`: muda builds `NSMenu` items and never touches the app delegate. That is good: a bridge that does not replace the delegate cannot interfere with the menu.
- `varos-app/Cargo.toml:36–38`: objc2 0.6 / objc2-foundation / objc2-app-kit 0.3 are already present on macOS. The features `NSApplication`, `NSURL`, `NSArray` and `NSString` are missing.
- `tools/mac/bundle.sh:82–105` **already declares** `CFBundleDocumentTypes` (Editor, Owner, `vrs`) and `UTExportedTypeDeclarations` (`com.varos.editor.document`, conforming to `public.data`/`public.content`). Line 133 runs `lsregister -f`. Registration exists; the missing piece is event handling. This matches the spec: "registering `.vrs` alone is insufficient".
- Nothing handles winit's `WindowEvent::DroppedFile` (grep finds 0 hits in `varos-app/src`).

**Export**
- The top-bar Export is drawn only: `ui.rs:3355–3358` (`bar_btn(... "Export", true)`, and the response is ignored). The layout slot is at `chrome.rs` `topbar_layout` / `ui.rs:3316–3320`.
- In the hamburger menu, `ui.rs:3415–3421`, the rows **New / Open… / Save / Export…** throw away `menu_row`'s `bool` (`ui.rs:3235`). All four are dead. The Share button (`ui.rs:3357`) is also drawn only.
- `chrome.rs:235–243`: native File has Open / Save / Save As / Close Window. It has **no New and no Export**. "Close Window" ⌘W maps to `MenuCmd::Close`, which is the quit guard (`main.rs:971–989`). The spec wants ⌘W = Close Tab; that is S1's job. `mac_menu.rs:126–137` creates every item enabled and has no enabled-state sync.
- Save As still offers a `.pdf` filter (`main.rs:575–587`). The spec says Save writes `.vrs` only (S1).
- In `varos-pdf/src/lib.rs`, `save_vrs` (`:30–32`) goes to `write_pdf` (`:72`), which **always** embeds the model. The embedding is the EmbeddedFile, FileSpec with `/AF` Source, the `/Names` tree, `/VAROS_Model` and `/VAROS_SchemaVersion` (`:251–267`). The page loop (`:84–247`) is shared. When there are no boards, or all boards are hidden, it falls back to a dummy 1080² or first board (`:84–93`). That is fine for the native file and wrong for a deliverable.
- **Mask fidelity gap:** the writer skips mask sources through `paint_list` (`:122`) but emits **no clip operator** (grep finds no `W`/clip). So the members of a clipping mask come out **unclipped**, while the canvas clips them (`varos-core/src/scene.rs:48–58, 525–535`). Today's `.vrs` pages already misrender masks. A pure export would also expose the art outside the mask.
- Artwork that sits partly off-page is written whole and clipped only by the MediaBox. Art entirely off-page is culled by `bbox_hits` (`:316`). Hidden art is skipped by `eff_hidden` (`:126`), which covers node, group and board hiding.
- `write_atomic` (`varos-core/src/file.rs:50–54`) uses a fixed `*.vrs.tmp` sibling and no fsync. S3 replaces it with an app-owned durable writer.
- There is no Export panel. `PanelId` (`shell/registry.rs:10–19`) lists Board, Align, Pathfinder, Properties, Layers, Swatches, History and Assets. `ShellState::toggle_panel` (`shell/boxtree.rs:142`) *closes* a visible panel, so it cannot serve as the "reveal" behaviour. Panel bodies are dispatched at `ui.rs:1317–1349`.
- Existing tests to extend: `menu_mirror_tests` (`main.rs:1550`), `chrome.rs` tests (`:351`), `varos-pdf/tests/container.rs` (with its `pages_of` lopdf helper).

## 3. Design

### 3.1 Assumed upstream APIs (S1, S2, S3)
The pieces below are written against these signatures. If S1/S2/S3 land with different names, only call sites change. **The behaviour contract is what binds.**
```rust
// S1 — varos-app (e.g. app_command.rs / workspace.rs)
pub struct SessionId(pub u64);                         // Copy, Eq, Hash
pub enum OpenOrigin { Dialog, Recent, CommandLine, OsEvent }
pub enum AppCommand { OpenPaths { paths: Vec<PathBuf>, origin: OpenOrigin }, ShowExport(SessionId),
    ExportPdf(SessionId, ExportRequest), CancelJob(JobId), /* … */ }
impl Workspace {
    pub fn dispatch(&mut self, cmd: AppCommand, ports: &mut AppPorts);   // AppPorts = fake-able dialogs/fs/clock
    pub fn active(&self) -> Option<SessionId>;
    pub fn session(&self, id: SessionId) -> Option<&DocumentSession>;   // .path(), .editor(), .is_dirty()
    pub fn sessions(&self) -> impl Iterator<Item = &DocumentSession>;    // tab order
    pub fn find_by_identity(&self, p: &Path) -> Option<SessionId>;
}
impl Ui { pub fn take_app_commands(&mut self) -> Vec<AppCommand>; }    // like today's `win_action`
```
The `OpenPaths` contract that S4 depends on (S1 owns it):
- Paths are processed in order, with duplicates removed inside the batch by identity.
- An already-open identity focuses its tab with no reload.
- A pristine empty tab may be reused. Edited work is never replaced.
- A failing path shows its error and the other paths still open.
- The last path that opened or was focused becomes active. A pending OS open overrides Start.

S2: `app_paths::data_root() -> Result<PathBuf, AppPathError>`. S3: `durable::replace_file(dest, &bytes, &CancelToken) -> Result<(), WriteError>` plus a job runner that returns `JobId`, runs off the UI thread, and completes through a proxy-woken `AppCommand`. **Fallback if S3 has not landed:** S6-B uses a unique sibling temp file, then `sync_all`, then `rename`, inline in `export_job.rs`, and the Export home says nothing about durability.

### 3.2 S4 — one queue, three producers (all in `varos-app`, no core change)
New `varos-app/src/os_open.rs`. It is pure std, compiled on every platform, and has full unit tests:
```rust
pub const MAX_PENDING: usize = 64;          // paths held at once; the extra ones are counted, not kept
pub const MAX_PATH_UTF16: usize = 32_768;   // Windows long-path ceiling incl. NUL → cbData ≤ 65_536 bytes
pub struct OsOpenQueue { pending: Vec<(PathBuf, OpenOrigin)>, dropped: usize }
impl OsOpenQueue {
    pub fn push(&mut self, path: PathBuf, origin: OpenOrigin);   // rejects empty/relative; exact dup → skipped
    pub fn drain(&mut self) -> Option<OpenBatch>;                 // None when empty; order preserved
}
pub struct OpenBatch { pub paths: Vec<PathBuf>, pub origin: OpenOrigin, pub dropped: usize }
pub fn enqueue(paths: impl IntoIterator<Item = PathBuf>, origin: OpenOrigin); // global Mutex<OsOpenQueue>, poison-tolerant
pub fn take_batch() -> Option<OpenBatch>;
pub fn file_args<I: IntoIterator<Item = S>, S: Into<OsString>>(args: I, cwd: &Path) -> Vec<PathBuf>; // all positionals, skips -psn_*, absolutizes
pub fn decode_handoff(tag: usize, words: &[u16]) -> Result<Vec<u16>, HandoffError>; // tag, len ≤ MAX_PATH_UTF16, strip 1 trailing NUL, no interior NUL, non-empty
```
- **CLI (all OS):** `main()` calls `os_open::enqueue(file_args(args_os, cwd), CommandLine)` before the event loop starts. This fixes the cold-launch bug.
- **Windows:**
  - The sender forwards **every** positional path, one `WM_COPYDATA` each, using `std::path::absolute` (not `canonicalize`).
  - The receiver calls `decode_handoff`, then `OsString::from_wide`, then `Path::is_absolute`, then `os_open::enqueue(.., OsEvent)`.
  - The static `PENDING_FILE_OPENS` is deleted.
  - `install_file_open_handler` moves to right after `create_window` (`main.rs:~796`).
  - The mutex, the class name and the timeouts stay unchanged.
- **macOS (the S4 spike):** new `varos-app/src/mac_open.rs`, `#[cfg(target_os = "macos")]`.
  - `install(proxy: EventLoopProxy<()>)` runs **after `EventLoop::build()` and before `run`**, because Launch Services delivers `odoc` during `finishLaunching`, before `applicationDidFinishLaunching`.
  - It looks up `NSApp.delegate()`'s class (`WinitApplicationDelegate`). If that class does **not** already respond to `application:openURLs:`, it adds that method with `objc2::ffi::class_addMethod` (type encoding `v@:@@`).
  - The IMP keeps only `isFileURL` entries, converts each with `url.path()` into a `PathBuf`, calls `os_open::enqueue(.., OsEvent)`, and then `proxy.send_event(())` to wake the loop.
  - It **never calls into winit or the editor**, which rules out re-entrancy, and it never replaces the delegate, so winit's launch handling and muda's menu are untouched.
  - If the selector already exists (a future winit), it logs `[varos] open-documents: delegate already handles openURLs — bridge not installed` and adds nothing.
  - Fallback if the spike fails (e.g. the IMP is never called on a bundled app): an isa-swizzle to a runtime subclass built with `ClassBuilder` over the winit class. That keeps the same IMP and the same queue.
- **Delivery** happens in `main.rs` on `AboutToWait`, replacing `:1002–1015`:
  - `while let Some(b) = os_open::take_batch()`, dispatch `AppCommand::OpenPaths { paths: b.paths, origin: b.origin }`.
  - For `OsEvent` batches, also call `window.set_minimized(false)` and `window.focus_window()`.
  - If `b.dropped > 0`, show "{n} more files weren't opened. Open them from File ▸ Open…".
  - At startup, "a batch is pending" suppresses Start (S2's "OS intent wins").
- **Cross-process write protection** (the spec's "launched unusually from a terminal"). New `varos-app/src/doc_lock.rs`, std only.
  - It holds an exclusive `File::try_lock` (std, stable since Rust 1.89) on `<data_root>/Locks/<fnv1a64(normalized path)>.lock` for every session that has a path.
  - Acquire happens on successful open and on Save As to a new path. Release happens on close and on Save As away.
  - If another process holds the lock, open is refused: **Couldn't open “{name}”.** / "It is already open in another Varos window. Your open documents have not changed." / **OK**.
  - It needs no new crates, and the lock is released automatically if a process crashes, because the OS drops it.
- **Bundle:** `bundle.sh` already registers the type, so **no plist change is required**. S4 adds only a post-install check line, `mdls`/`lsregister -dump | grep com.varos.editor.document`, printed for Ahmed. Conformance to `com.adobe.pdf` (for Quick Look thumbnails) is deliberately **not** added; see §6 R8.

### 3.3 S6 — pure PDF, one home, one command path
**varos-pdf API.** New `src/export.rs` and `src/write.rs`. `lib.rs` keeps its read side untouched, because S5 owns it.
```rust
pub enum ExportScope { AllVisibleArtboards, ActiveArtboard, ArtworkBounds }
pub struct PageSpec { pub rect: [f32; 4] /* world x,y,w,h */, pub background: Option<Rgba> }
pub struct ExportPlan { pub scope: ExportScope, pub pages: Vec<PageSpec> }
pub enum ExportUnavailable { NoVisibleArtboards, ActiveArtboardHidden, NotBoardless, NeedsArtboards,
    NothingToExport, ClipMasksNotSupported }            // each has `fn reason(&self) -> &'static str` (user copy)
pub enum ExportError { Cancelled, Unavailable(ExportUnavailable), Write(String) }
pub fn default_scope(doc: &Document) -> ExportScope;   // boards present → AllVisibleArtboards, else ArtworkBounds
pub fn plan_pdf_export(doc: &Document, scope: ExportScope) -> Result<ExportPlan, ExportUnavailable>;
pub fn export_pdf_bytes(doc: &Document, plan: &ExportPlan, cancel: &AtomicBool) -> Result<Vec<u8>, ExportError>;
pub fn has_embedded_model(bytes: &[u8]) -> bool;       // catalog /VAROS_Model or EmbeddedFiles model name
```
- **Internal refactor.** `fn write_pages(doc, &[PageSpec], model: Option<&str>, cancel) -> Result<Vec<u8>, ExportError>` holds the existing page loop, moved verbatim. `write_pdf(doc)` becomes: build today's native page list (fallback included) plus `Some(blob)`. **Native output bytes must stay identical.** A fixture captured *before* the refactor proves this (`tests/fixtures/native_demo.pdf`).
- **Export** calls `write_pages(.., None, cancel)`. It writes no FileSpec, no `/AF`, no `/Names`, no `/VAROS_*`, no Info dictionary and no names. It checks `cancel` between pages. It is deterministic: the same document gives the same bytes.
- **Scope rules** (spec §2):
  - *All visible:* visible boards in document order. If every board is hidden, `NoVisibleArtboards`.
  - *Active:* `doc.active_artboard()`. If that board is hidden, `ActiveArtboardHidden`. If there are no boards, `NeedsArtboards`.
  - *ArtworkBounds:* only when there are no boards, otherwise `NotBoardless`. The page is the union of the visible paint-list world bboxes, padded by half the stroke width, with a transparent background. If there is no visible art, `NothingToExport`.
  - There is never a dummy board.
- **Masks (the spec §6 fidelity rule):** if any path emitted on a planned page is a *member* of a visible Clip group, `plan_pdf_export` returns `ClipMasksNotSupported`. Reason: "Clipping masks can't be exported to PDF yet." This refusal is the honest behaviour instead of a silent misrender; see §6 R2.

**App.** New `varos-app/src/export_home.rs` (the panel body plus pure view-model) and `export_job.rs` (the job).
- `PanelId::Export` is added to `registry.rs` with title "Export", added to `DOCKABLE`, and its family is Properties. `ShellState::reveal_panel(PanelId)` opens the panel or brings it forward, and **never closes it**.
- `ShowExport(id)` reveals the panel. The panel always renders the **active** session. Scope choice is kept per session in S1 session view state; it is not persisted.
- **Panel content** follows spec §4 copy exactly:
  - **Format:** PDF (selected) / PNG and SVG muted with "Not available yet", not interactive, and the same text as tooltip.
  - **Pages:** radio rows. A row that doesn't apply is disabled and shows its `reason()`.
  - A page count and "Destination: chosen when you export".
  - The note "For sharing. Editable Varos data is not included."
  - The action **Export…**. When `plan_pdf_export` fails, the button is disabled and the reason is shown.
  - Tokens come only from `shell/tokens.rs`, using existing row and button widgets. Azure is only for the selected radio and focus. No shadows, no animation.
- **`ExportPdf(id, ExportRequest { scope })` flow:**
  1. Native save dialog through `AppPorts`: title **Export PDF**, `<stem>.pdf`, a PDF filter. The extension is forced to `.pdf`.
  2. Cancelling the dialog does nothing.
  3. Guards, in order:
     - The destination is an open session's identity → refuse: "Choose another name. “{name}” is an open Varos document."
     - The destination exists, is ≤ 256 MiB, and `has_embedded_model` → extra **Replace · Cancel** with "“{name}” contains an editable Varos document. Replacing it with an export removes the editable data."
     - The native NSSavePanel / IFileSaveDialog shows the plain overwrite prompt.
  4. `Document` clone snapshot, then a job: plan → bytes → durable replace, with a `CancelToken` checked before the replace. There is at most one export per session, and writes to the same destination identity are serialized.
  5. The status line reads "Exporting PDF…" + **Cancel**.
     - Success: "Exported {filename}." + **Show in Finder** (`open -R`) or **Show in Explorer** (`explorer /select,`) through the port.
     - Failure: "Couldn't export PDF. {reason}. Your document has not changed." + **Try Again · Cancel**.
     - A cancel after the replace has happened reports success.
  6. Closing the session cancels its job.
- **Export mutates nothing.** It never touches the session path, `Editor` or history, the checkpoint, the dirty state, recents, or recovery.

**F4.2 file-effect parity.**
- One pure table, `fn file_command_routes() -> &'static [FileRoute]`, in new `varos-app/src/file_routes.rs`. Each row is `{ cmd: FileCmd, native_menu_id: Option<&str>, hamburger: Option<&str>, topbar: Option<TopbarCtl>, chord: Option<Accel> }`.
- `FileCmd` covers New, Open, Save, SaveAs, CloseTab, CloseWindow, Quit and Export. `fn to_app_command(FileCmd, active) -> AppCommand` is the **only** mapper.
- Native menu rows for file actions become `MenuCmd::File(FileCmd)` instead of `Key(..)`. Hamburger rows and the top-bar Export button emit `FileCmd`, as does keyboard `shortcut()`. All of them go through `to_app_command`.
- New native row: `file.export` "Export…" with no accelerator (§6 R6). A `file.new` ⌘N row is added if S1 didn't add it.
- Enabled state: `MacMenu::sync_enabled(|FileCmd| bool)` mirrors availability, for example Export disabled when there is no session or a job is in flight. Add it if S1 has not.

## 4. Pieces

S4 has 3 pieces and S6 has 4. **S6-A can start now**, in parallel with S1, because it touches `varos-pdf` only. Every other piece needs S1 merged (S4-C also needs S2's `data_root`; S6-B also needs S3).

**S4-A — OS-open queue, CLI and Windows handoff.** · model **opus** (app loop plus unsafe glue) · **M** · depends: S1 merged.
- *Owns:* new `varos-app/src/os_open.rs`; modifies `single_instance.rs`, `main.rs` (startup enqueue, early subclass install, `AboutToWait` drain → `OpenPaths`, OsEvent focus) and `lib.rs`/mod list.
- *Must not touch:* `mac_*.rs`, `ui.rs`, `chrome.rs`, `varos-core`, `varos-pdf`.
- *Steps:*
  1. `os_open.rs` with the API in §3.2.
  2. Replace `first_file_arg` with `file_args`, keeping the two old tests passing through a wrapper, and add the `-psn_` skip.
  3. Windows: `decode_handoff`, forward all paths, `std::path::absolute`, move the subclass install up, delete `PENDING_FILE_OPENS`.
  4. Drain into S1 `dispatch`.
  5. Workspace-level tests with S1's fakes.
- *Tests:*
  - Queue behaviour:
    - `early_events_wait_until_drained` — pushes before the workspace exists survive.
    - `duplicate_paths_coalesce_in_order`, `multi_file_order_preserved`, `overflow_counts_dropped`.
  - Parsing:
    - `relative_and_empty_rejected`.
    - `file_args_all_positionals_skip_psn_and_flag_values`.
    - `decode_handoff_rejects_oversize_interior_nul_bad_tag_empty` and `decode_handoff_strips_one_trailing_nul`.
  - Workspace (S1 fakes):
    - `cold_os_open_skips_start_and_opens_b`.
    - `warm_os_open_keeps_dirty_a_and_adds_tab`.
    - `reopen_same_file_focuses_without_reload` (history length unchanged).
    - `bad_file_among_good_opens_the_rest_no_blank_tab`.
    - `unicode_and_space_paths_open`.
  - Gates: Windows-target clippy clean and Mac-target clippy clean.

**S4-B — macOS open-documents bridge (the spike).** · model **opus** · **M** · depends: S4-A's `os_open::enqueue` signature only. It can be written concurrently; merge after S4-A.
- *Owns:* new `varos-app/src/mac_open.rs`; the `Cargo.toml` features (`objc2-app-kit`: `NSApplication`; `objc2-foundation`: `NSURL`, `NSArray`, `NSString`); `main.rs`, one call after `EventLoop::builder()…build()`; the `tools/mac/bundle.sh` post-install check line.
- *Must not touch:* `single_instance.rs`, `os_open.rs` (it calls the API only), `mac_menu.rs`, `ui.rs`.
- *Steps:*
  1. `install()` as in §3.2, with `// SAFETY:` comments on every unsafe block.
  2. A pure helper `fn file_url_paths(urls) -> Vec<PathBuf>`.
  3. A startup log line `[varos] open-documents bridge: installed|skipped (<why>)`.
  4. Record in the PR how the spike went: cold, warm, and Dock drop tried on the bundled app, or the isa-swizzle fallback if the IMP never fires.
- *Tests:* only on macOS (`#[cfg(target_os = "macos")]`, built by Mac-target clippy, run on Ahmed's Mac):
  - `file_url_paths_keeps_file_urls_only` (NSURL objects need no NSApp).
  - `bridge_selector_signature_is_v_at_colon_at_at` (the encoding constant).
- *Honesty:* this bridge is only proven by the Mac hand test. Cloud gates type-check it and nothing more.

**S4-C — Cross-process document lock.** · model **sonnet** · **S** · depends: S1 (session open/save-as/close sites) and S2 (`data_root`).
- *Owns:* new `varos-app/src/doc_lock.rs`, plus ≤ 3 hook lines in S1's workspace open, save-as and close.
- *Must not touch:* `os_open.rs`, `single_instance.rs`, `mac_*`, `ui.rs`.
- *Steps:*
  1. `DocLocks { held: HashMap<SessionId, File> }` with `try_acquire(root, identity) -> Result<File, LockError>` and `release(SessionId)`.
  2. An FNV-1a 64 hash of the normalized path string.
  3. The refusal message in §3.2.
  4. If `data_root` fails: warn once and open without the lock. This matches S2's "Recovery unavailable" style and never blocks editing.
- *Tests:*
  - `second_lock_on_same_identity_fails`, `lock_released_on_drop`.
  - `save_as_moves_lock_to_new_path`.
  - `workspace_refuses_file_locked_by_other_holder`, with a second `File` handle standing in for the other process.
  - `hash_is_stable_known_vector`.

**S6-A — Pure-PDF writer and scope planner (varos-pdf only).** · model **opus** (privacy-critical refactor with byte-identity) · **M** · depends: none; **start now**.
- *Owns:* `varos-pdf/src/lib.rs` (write side only: move the page loop into `write.rs`, re-export), new `src/write.rs`, `src/export.rs`, `tests/export_pdf.rs`, `tests/fixtures/native_demo.pdf`.
- *Must not touch:* `extract_model`/`load_vrs` (S5 owns reading), `varos-core`, `varos-app`.
- *Steps:*
  1. **Before any edit**, write `native_demo.pdf` from `container.rs::demo_doc` plus a masked, rotated, translucent two-board document.
  2. Move the loop into `write_pages`. `write_pdf` must stay byte-identical.
  3. Add the §3.3 API.
  4. Add the mask detector, which walks each emitted path's ancestors for `GroupRole::Clip` where the path is not inside `mask_child`, reusing `Document` accessors.
- *Tests, `tests/export_pdf.rs`, all through lopdf:*
  - `native_write_is_byte_identical_to_fixture`.
  - Scope:
    - `all_visible_two_boards_two_pages_sized_like_boards`.
    - `hidden_board_excluded`, `all_hidden_is_unavailable_not_a_dummy_page`.
    - `active_scope_one_page`, `active_hidden_unavailable`.
    - `boardless_artwork_bounds_page_matches_padded_bbox`, `boardless_empty_is_nothing_to_export`.
  - Privacy:
    - `export_has_no_embedded_file_filespec_af_names_or_varos_keys` (walks every object).
    - `export_leaks_no_hidden_coordinates_or_names`: a hidden path, a hidden layer and a path on a hidden board each carry unique coordinates such as `777.125`, and there are the names "SECRET-LAYER" and "Board Secret". None of these appear in any decompressed stream or in the raw bytes.
    - `exported_pdf_is_refused_by_load_vrs`.
  - Content and behaviour:
    - `visible_content_present`: the fill operator and the expected page coordinates.
    - `opacity_and_rotation_survive_export`.
    - `clip_mask_member_makes_scope_unavailable`, `clip_mask_off_page_does_not_block`.
    - `export_is_deterministic`, `cancel_before_first_page_returns_cancelled`.
    - `has_embedded_model_true_for_native_false_for_export`.

**S6-B — Export home and export job.** · model **opus** · **L** · depends: S1, S3 (or the §3.1 fallback), and S6-A merged.
- *Owns:*
  - New `varos-app/src/export_home.rs`: `ExportHomeModel::from(&Document, scope, job_state)` → rows, reasons, page count and status copy. It is pure and testable, and renders with egui.
  - New `varos-app/src/export_job.rs`.
  - `shell/registry.rs` (`PanelId::Export`) and `shell/boxtree.rs` (`reveal_panel`, family).
  - `ui.rs`: only the panel-host arm `P::Export => …` near `:1344`.
  - `main.rs`: job completion handling and the Show-in-Finder port.
- *Must not touch:* `chrome.rs`, `build_topbar`/hamburger in `ui.rs` (S6-C), `varos-pdf`.
- *Steps:*
  1. The panel and pure model.
  2. `reveal_panel`.
  3. Handle `ShowExport`/`ExportPdf`/`CancelJob` in the S1 coordinator.
  4. The dialog port method `pick_export_destination(default_name) -> Option<PathBuf>`.
  5. The guards in §3.3.
  6. The job, completion, and status.
- *Tests (S1 fake ports, no EventLoop):*
  - Panel model:
    - `home_defaults_to_all_visible_with_page_count`.
    - `png_svg_rows_disabled_with_not_available_yet`.
    - `unavailable_scope_disables_export_with_reason`.
    - `home_shows_active_session_after_tab_switch`.
  - Export flow:
    - `destination_cancel_changes_nothing`, `extension_forced_to_pdf`.
    - `refuses_open_session_path`, `native_model_destination_needs_second_confirmation`.
    - `export_success_leaves_path_rev_history_checkpoint_dirty_recents_unchanged`.
    - `cancel_before_replace_leaves_destination_untouched`, `cancel_after_replace_reports_success`.
    - `write_failure_shows_try_again_and_document_unchanged`.
    - `one_export_in_flight_per_session`, `closing_session_cancels_its_export`.
  - `reveal_panel_never_closes_a_visible_export_panel`.

**S6-C — Entry points and F4.2 file-effect parity audit.** · model **sonnet** · **M** · depends: S1. It can run concurrently with S6-B against `AppCommand::ShowExport`; merge after S6-B.
- *Owns:*
  - New `varos-app/src/file_routes.rs`.
  - `chrome.rs`: `MenuCmd::File(FileCmd)`, File rows New / Open / Save / Save As / Export… / Close Tab / Close Window.
  - `mac_menu.rs`: `sync_enabled`.
  - `ui.rs` `build_topbar`: Export button and hamburger rows wired; a disabled look when unavailable.
  - `main.rs`: the menu-drain mapping and the keyboard `shortcut()` file chords going through `to_app_command`.
- *Must not touch:* `export_home.rs`, `export_job.rs`, `shell/*`, `varos-pdf`.
- *Tests:*
  - `every_file_command_has_every_route_the_spec_lists` (the table against `chrome::menus()`, the hamburger list and the chords in spec §4).
  - `all_routes_of_a_command_map_to_the_same_app_command`.
  - `export_routes_all_emit_show_export_for_active_session`.
  - `no_enabled_file_row_is_dead`: every hamburger/native file row maps to a `FileCmd`.
  - `export_rows_disabled_without_session`.
  - `menu_accel_equals_keyboard_chord` (extends `menu_mirror_tests`).
  - The PR includes the audit table: route × command, with evidence, for GATE_LOG.

**S6-D — System journey test and acceptance kit.** · model **sonnet** · **M** · depends: every S1–S6 piece merged.
- *Owns:* new `varos-app/tests/…` or `main.rs` `#[cfg(test)] mod system_journey_tests` (headless, fake ports and clock); the hand-test list text handed to the moderator (the moderator edits `STATUS.md`/`GATE_LOG.md`; the subagent does not).
- *Test:* `system_journey_headless` follows spec §5:
  1. New A, draw, save.
  2. New B, draw, switch/reorder/undo.
  3. `OpenPaths(C, OsEvent)`.
  4. Dirty-quit cancel.
  5. Snapshot B, then a simulated crash (drop the workspace without closing).
  6. Relaunch scan, recover B, Save As.
  7. Export the active scope (inspect the PDF with lopdf).
  8. Reopen A and B, then quit cleanly.
  - It asserts content, titles, paths, dirty marks and the PDF's page count and privacy.

## 5. Merge order and conflict hot-spots

Order:
1. **S6-A** (any time now).
2. S1 (other WO).
3. **S4-A**.
4. **S4-B**.
5. **S4-C** (after S2).
6. S5 (other WO).
7. **S6-B** (after S3).
8. **S6-C**.
9. **S6-D**.

Use `--no-ff` for every merge, with the full gates after each: test, clippy (host, Windows target, `aarch64-apple-darwin`), and fmt.

Hot spots:
- `varos-pdf/src/lib.rs`: S6-A (write side) against S5 (read side, plus catalog `VAROS_SchemaVersion` → 2).
  - S6-A moves writing into `write.rs` so that S5's reader edits don't collide.
  - Whichever lands second rebases.
  - **If S5 lands after S6-A, S5 intentionally regenerates `native_demo.pdf`** (the version key changes) and must say so in its PR.
- `main.rs`:
  - S4-A takes the startup and `AboutToWait` hunks. S4-B takes one line after the event-loop build.
  - S6-B takes the job-completion hunk. S6-C takes the menu drain and `shortcut()`.
  - These are separate regions. The moderator resolves them by keeping both sides.
- `ui.rs`: S6-B only the panel-host arm (`~:1344`), S6-C only `build_topbar` (`~:3290–3425`). No overlap is expected.
- `chrome.rs`/`mac_menu.rs`: S6-C only. If S1 already added a New row or ⌘W Close Tab, S6-C adapts and does not duplicate.
- `Cargo.toml` (varos-app): S4-B only (features). `Cargo.lock` should not change; if it does, the moderator checks it.

## 6. Risks / open questions (each with a default)

- **R1 — macOS bridge mechanics (the spec's S4 spike).**
  - Adding a method to winit's delegate class is an ObjC-runtime patch that depends on a winit implementation detail. The docs even contradict the code.
  - *Default:* `class_addMethod` guarded by `respondsToSelector`, with the isa-swizzle subclass as fallback. Pin `winit = "=0.30.13"` in the PR comment and re-check on any winit bump.
  - If neither works within the day, S4's gate stays **open** and S4-B's partial result is recorded. The Windows/CLI half still merges.
- **R2 — Masks in PDF.** The existing writer ignores clipping, so an honest export must refuse documents whose chosen pages contain masked art.
  - *Default:* refuse with the reason (S6-A). Offer Ahmed a follow-up piece, "PDF clip masks": a per-path `q … W* n … Q` from the mask rings, with a fixture comparison against the canvas. It is **not** smuggled into S6.
  - Note that today's `.vrs` preview pages have the same gap. That goes in the PAINS/risk log, not in this stage.
- **R3 — Windows unverifiable.**
  - *Default:* compile gate only. P5's Windows half stays **pending** in STATUS and GATE_LOG. The "Show in Explorer" and IFileSaveDialog overwrite prompt are unverified. Nobody claims Windows works.
- **R4 — Terminal duplicate processes on macOS.** There is no single-instance mutex (per the spec).
  - *Default:* S4-C locks refuse a second writer. Forwarding the paths to the running instance through `NSWorkspace` is **later**.
- **R5 — Artwork bounds or pages above 14,400 pt** may be clamped by some viewers.
  - *Default:* export as-is, and the Export home shows "Pages larger than 200 in may not open in every PDF viewer."
- **R6 — Export shortcut.** The spec lists none.
  - *Default:* no accelerator. The parity table allows `chord: None`.
- **R7 — Share button and search pill are drawn-only** (not file actions, outside this system).
  - *Default:* untouched. S6-C's audit lists them as out-of-scope dead controls for Ahmed's queue.
- **R8 — UTI conforming to `com.adobe.pdf`** would give Quick Look thumbnails, but other apps might treat `.vrs` as PDF and strip the model on re-save.
  - *Default:* leave `bundle.sh` conformance unchanged. Do not register Varos for `.pdf`.
- **R9 — Selection scope** (raised in the brief) is not in the spec's scope list.
  - *Default:* not offered. The scopes are All visible / Active / Artwork bounds.
- **R10 — Window drag-and-drop** (`DroppedFile`) is not required by the spec.
  - *Default:* out of scope. It is trivial later, through `os_open::enqueue(.., OsEvent)`.
- **R11 — Upstream API drift** (S1/S2/S3 names).
  - *Default:* adapt the call sites and keep the §3.1 behaviour contract. If a contract item is missing (e.g. dedup inside a batch), the S4-A/S6-B piece adds a failing test and asks the moderator. It does not re-implement S1 logic.

## 7. Out of scope

PNG and SVG engines and their settings; batch export; Export for Screens; a Selection scope; print profiles, bleed/TrimBox, CMYK, fonts and text; PDF clip-mask rendering (R2 follow-up); foreign PDF/AI/SVG import; registering Varos for `.pdf`; installers, signing, notarization and updaters; cross-window tab drag and extra windows; forwarding terminal launches to a running macOS instance; Windows hand-testing and any Windows-specific UX beyond the handoff; file drag-and-drop onto the window; Share and the search pill; F09 (the Artboards system).
