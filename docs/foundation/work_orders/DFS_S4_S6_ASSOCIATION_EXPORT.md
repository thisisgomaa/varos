> **Status:** current — work order (charter §3 level 4), derived from docs/specs/DOCUMENT_FILE_SYSTEM.md; owner decisions D1–D3 recorded 2026-09-24.
> Amended 2026-09-24 after the independent plan review (see reviews/DFS_S4_S6_ASSOCIATION_EXPORT.review.md); P1/P2 applied, "Needs Ahmed" items carry their default.
# DFS work order — S4 file association (macOS + Windows) and S6 Export + system acceptance

Date: 2026-09-24 · Branch baseline: `claude/sweet-cerf-1sg30t` · Planning only: nothing below has been built or run.
Spec rows: §5 S4 and S6; §2 "Association and export boundary"; §6 "macOS process/events" and "Privacy and fidelity".
D1 = YES, including the **narrow Windows exception for S4**. That exception covers the handoff code and the Windows-target compile gate only. **No Windows machine is available for hand-testing.** So the Windows half of P5 stays *pending* until real Windows access exists. A green compile gate does not prove it works at runtime (spec §5 S4).

## 1. Goal & acceptance

**S4 — file association.** A Finder double-click, Finder "Open With", a drop on the Dock icon, an Explorer double-click, or a command-line path all feed one queue. That queue feeds S1's `OpenPaths`. On a cold launch the paths are held until the workspace is ready, so a file is never lost because it arrived early. On a warm launch the same event opens the file in a new tab, or focuses its tab if the file is already open.
- *Ahmed's Mac hand test:* with a dirty A open, double-click B in Finder, both cold (Varos not running) and warm. B opens in its own tab and A survives with its dot. Double-click B again: the same tab is focused and nothing reloads. Select 3 files → Open: 3 tabs. Also try a name with Arabic letters and spaces, and a broken `.vrs`: a readable error, no blank tab, and A is untouched. "Open With ▸ Varos" and a drop on the Dock icon behave the same way. Get Info on a `.vrs` shows Varos as the owner.
- *Windows:* the same list on Explorer. **Pending. There is no test machine.** Only the compile gate runs.
- *Headless proof:* queue tests for early events, duplicates, multiple files, and payload/argv parsing. Workspace tests with S1's fake ports for cold, warm, duplicate, multi-file, bad-file and Unicode paths. Mac and Windows target clippy stay clean.
- *Closes:* P5 (the Mac half; the Windows half stays pending) and the Mac load gap. It re-checks F02. Duplicate-process write safety = S3's fingerprint prompt (`external_change_prompts_before_overwrite`), not a separate S4 lock.

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

S2: `varos_app::storage::paths::data_root() -> Option<PathBuf>` (S2_S3 WO §3.2 — the ONLY data-folder resolver; frozen name, not `app_paths`). S3: `durable::write_replace(fs, dest, &bytes, nonce) -> Result<WriteOutcome, WriteError>` plus `IoWorker::submit(key, job: Box<dyn FnOnce() -> Completion + Send>)`, a generic job runner on one FIFO thread that completes through a proxy-woken `AppCommand` (S2_S3 WO §3.3/§3.6). S6-B depends on S3-F1 merging first; there is no fallback writer — building a second durable writer and a second thread duplicates work S3 already does.

### 3.2 S4 — one queue, three producers (all in `varos-app`, no core change)
New `varos-app/src/os_open.rs`. It is pure std, compiled on every platform, and has full unit tests:
```rust
pub const MAX_PATH_UTF16: usize = 32_768;   // Windows long-path ceiling incl. NUL → cbData ≤ 65_536 bytes; this is the spec's payload-length bound, so there is no separate pending-count cap
pub struct OsOpenQueue { pending: Vec<(PathBuf, OpenOrigin)> }
impl OsOpenQueue {
    pub fn push(&mut self, path: PathBuf, origin: OpenOrigin);   // rejects empty/relative; exact dup → skipped
    pub fn drain(&mut self) -> Option<OpenBatch>;                 // None when empty; order preserved
}
pub struct OpenBatch { pub paths: Vec<PathBuf>, pub origin: OpenOrigin }
pub fn enqueue(paths: impl IntoIterator<Item = PathBuf>, origin: OpenOrigin); // global Mutex<OsOpenQueue>, poison-tolerant
pub fn take_batch() -> Option<OpenBatch>;
pub fn file_args<I: IntoIterator<Item = S>, S: Into<OsString>>(args: I, cwd: &Path) -> Vec<PathBuf>; // all positionals, skips -psn_*, absolutizes
pub fn decode_handoff(tag: usize, words: &[u16]) -> Result<Vec<u16>, HandoffError>; // tag, len ≤ MAX_PATH_UTF16, strip 1 trailing NUL, no interior NUL, non-empty
```
- **CLI (all OS):** `main()` calls `os_open::enqueue(file_args(args_os, cwd), CommandLine)` before the event loop starts. S1-D already routes the startup `file_arg` and the handoff drain into `OpenPaths` (S1 WO §3.5, S1-D step 5); S4-A swaps S1-D's `file_arg`/`take_pending_file_paths` for `file_args` + `os_open`, using S1's frozen names (`OsHandoff`, no `Recent`, tuple `OpenPaths(Vec<PathBuf>, OpenOrigin)`).
- **Windows:**
  - The sender forwards **every** positional path, one `WM_COPYDATA` each, using `std::path::absolute` (not `canonicalize`). On a zero return (`SendMessageTimeoutW(.., 2_000)` times out), the sender retries until a 10 s deadline before exiting — the first instance does not pump messages until `run`, which comes after `Renderer::new`, so a short single timeout can lose files at startup. This stays compile-gate only (R3).
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
  - After adding the method, re-assign the same object with `app.setDelegate(app.delegate())`. This is sound: winit keeps the strong reference in `EventLoop.delegate`. It makes AppKit re-read any responds-to cache (AppKit caches "delegate responds to X" at `setDelegate:` time, which winit calls inside `EventLoop::new`, *before* `install()` runs). The IMP is `extern "C-unwind"`, never panics, and keeps the proxy in a static `Mutex`, as `mac_menu.rs:66` does. If the IMP still never fires on the bundled app, S4's gate stays open (R1). **There is no swizzle fallback:** delegate replacement is fatal, not just stale docs — `ApplicationDelegate::get` panics unless the delegate `is_kind_of` winit's class (`winit-0.30.13/src/platform_impl/macos/app_state.rs:174–184`, called on every run-loop wake), and a subclass swizzle would still need the same selector resolution as `class_addMethod`, so if the added method is never called, the swizzle would not be called either.
- **Delivery** happens in `main.rs` on `AboutToWait`, replacing `:1002–1015`:
  - `while let Some(b) = os_open::take_batch()`, dispatch `AppCommand::OpenPaths { paths: b.paths, origin: b.origin }`.
  - For `OsEvent` batches, also call `window.set_minimized(false)` and `window.focus_window()`.
  - Start is decided at the first `AboutToWait` drain, not before `run` — a cold Finder open arrives *inside* `run()` (odoc is delivered before `applicationDidFinishLaunching:`), so "a batch is pending at startup" would see an empty queue on the Mac.
- **Duplicate-process write safety = S3's fingerprint prompt, not a separate lock.** There is no `doc_lock.rs`. The spec asks only to "protect duplicate-process writes" (§2 Association), and S3 already builds this: the fingerprint check (`external_change_prompts_before_overwrite`, S2_S3 WO §3.3/F1) plus per-session `Recovery/<rid>/session.lock` ownership (S2_S3 WO §3.5). On macOS a second process only happens when someone execs the binary directly — `open`, Finder and the Dock all reuse the running app. See R4.
- **Bundle:** `bundle.sh` already registers the type, so **no plist change is required**. S4 adds only a post-install check line, `mdls`/`lsregister -dump | grep com.varos.editor.document`, printed for Ahmed. Conformance to `com.adobe.pdf` (for Quick Look thumbnails) is deliberately **not** added; see §6 R8.

### 3.3 S6 — pure PDF, one home, one command path
**varos-pdf API.** New `src/export.rs` and `src/write.rs`. `lib.rs` keeps its read side untouched, because S5 owns it.
```rust
pub enum ExportScope { AllVisibleArtboards, ActiveArtboard, ArtworkBounds }
pub struct PageSpec { pub rect: [f32; 4] /* world x,y,w,h */, pub background: Option<Rgba> }
pub struct ExportPlan { pub scope: ExportScope, pub pages: Vec<PageSpec> }
pub enum ExportUnavailable { NoVisibleArtboards, ActiveArtboardHidden, NotBoardless, NeedsArtboards,
    NothingToExport }            // each has `fn reason(&self) -> &'static str` (user copy); masks are clipped, not refused (see Masks below)
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
- **Masks (decided: clip, per `docs/MASKS_PLAN.md` Stage 5 — refusal is only the fallback):** `write_pages` implements MASKS_PLAN Stage 5 directly, reusing the existing accessors (`Document::clip_group_of`, `node_mask_child`, `node_paths`, `unit_xform`, `model.rs:657–672, 1081, 1687`). For a painted path with `clip_group_of = Some(c)`: `q`, the rings of every path in `node_paths(node_mask_child(c))` (each with its own `unit_xform`), `W* n`, the member's usual paint (knockout XObject included), then `Q`. Cull a member whose bbox misses the mask bbox. Single-level only, matching the canvas (`scene.rs:622`, which also uses even-odd over all mask rings — exactly `W*`). This also fixes the same fidelity gap in the `.vrs` preview pages, at no extra cost. **Fallback only if the clip cannot pass inside the day:** refuse with `ClipMasksNotSupported` / "Clipping masks can't be exported to PDF yet." (kept as a documented fallback path, not the default).

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
- **There is no `file_routes.rs`, `FileRoute` or `TopbarCtl`, and no second route table.** S1-D already adds `FileCmd` (`chrome.rs`) + `lifecycle_key()` + one `to_app_command(FileCmd, active) -> AppCommand` mapper, and native File rows already use `MenuCmd::File(FileCmd)` (S1 WO §3.6, S1 review F5). S6-C only extends the existing enum: `FileCmd` gains `Export`, and `to_app_command` gains that one arm. Hamburger rows and the top-bar Export button emit `FileCmd::Export`, as does keyboard `shortcut()` if the spec ever gives Export a chord (it does not, §6 R6). All of them go through the same `to_app_command`.
- New native row: `file.export` "Export…" with no accelerator (§6 R6), added to `chrome::menus()` next to S1's existing File rows.
- The parity test reads the real sources directly: `chrome::menus()`, the burger's row list, and `lifecycle_key`/`to_app_command` — not a separate declarative table that could drift from them.
- Enabled state: `MacMenu::sync_enabled(|FileCmd| bool)` mirrors availability, for example Export disabled when there is no session or a job is in flight. Add it if S1 has not.

## 4. Pieces

S4 has 2 pieces (S4-C is deleted, §3.2) and S6 has 3 (S6-D is folded into S6-C, see S6-C below). **S6-A can start now**, in parallel with S1, because it touches `varos-pdf` only. Every other piece needs S1 merged (S6-B also needs S3-F1).

**S4-A — OS-open queue, CLI and Windows handoff.** · model **opus** (app loop plus unsafe glue) · **M** · depends: S1 merged.
- *Owns:* new `varos-app/src/os_open.rs`; modifies `single_instance.rs`, `main.rs` (startup enqueue, early subclass install, `AboutToWait` drain → `OpenPaths`, OsEvent focus) and `lib.rs`/mod list.
- *Must not touch:* `mac_*.rs`, `ui.rs`, `chrome.rs`, `varos-core`, `varos-pdf`.
- *Steps:*
  1. `os_open.rs` with the API in §3.2.
  2. Replace `first_file_arg` with `file_args`, keeping the two old tests passing through a wrapper, and add the `-psn_` skip.
  3. Windows: `decode_handoff`, forward all paths, `std::path::absolute`, move the subclass install up, delete `PENDING_FILE_OPENS`, retry the send until a 10 s deadline on timeout.
  4. Drain into S1 `dispatch`, using S1's frozen names (`OsHandoff`, tuple `OpenPaths(Vec<PathBuf>, OpenOrigin)`).
  5. Workspace-level tests with S1's fakes.
- *Tests:*
  - Queue behaviour:
    - `early_events_wait_until_drained` — pushes before the workspace exists survive.
    - `duplicate_paths_coalesce_in_order`, `multi_file_order_preserved`.
  - Parsing:
    - `relative_and_empty_rejected`.
    - `file_args_all_positionals_skip_psn_and_flag_values`.
    - `decode_handoff_rejects_oversize_interior_nul_bad_tag_empty` and `decode_handoff_strips_one_trailing_nul`.
  - Workspace (S1 fakes):
    - `cold_os_open_skips_start_and_opens_b` (enqueue **after** the workspace is built and before the first `AboutToWait` drain — a cold Finder open arrives inside `run()`, not before it).
    - `warm_os_open_keeps_dirty_a_and_adds_tab`.
    - `reopen_same_file_focuses_without_reload` (history length unchanged).
    - `bad_file_among_good_opens_the_rest_no_blank_tab`.
    - `unicode_and_space_paths_open`.
  - Gates: Windows-target clippy clean and Mac-target clippy clean.

**S4-B — macOS open-documents bridge (the spike).** · model **opus** · **M** · depends: S4-A's `os_open::enqueue` signature only. It can be written concurrently; merge after S4-A.
- *Owns:* new `varos-app/src/mac_open.rs`; the `Cargo.toml` features (`objc2-app-kit`: `NSApplication`; `objc2-foundation`: `NSURL`, `NSArray`, `NSString`); `main.rs`, one call after `EventLoop::builder()…build()`; the `tools/mac/bundle.sh` post-install check line.
- *Must not touch:* `single_instance.rs`, `os_open.rs` (it calls the API only), `mac_menu.rs`, `ui.rs`.
- *Steps:*
  1. `install()` as in §3.2, with `// SAFETY:` comments on every unsafe block, factored as `fn add_open_urls(cls: &AnyClass) -> AddResult /* Added | AlreadyPresent */` so it is testable without NSApp.
  2. A pure helper `fn file_url_paths(urls) -> Vec<PathBuf>`.
  3. A startup log line `[varos] open-documents bridge: installed|skipped (<why>)`.
  4. Record in the PR how the spike went: cold, warm, and Dock drop tried on the bundled app; if the IMP never fires, S4's gate stays open (there is no swizzle fallback — see R1).
- *Tests:* only on macOS (`#[cfg(target_os = "macos")]`, built by Mac-target clippy, run on Ahmed's Mac):
  - `file_url_paths_keeps_file_urls_only` (NSURL objects need no NSApp).
  - `add_open_urls_adds_once_then_reports_already_present` (on a throwaway `NSObject` subclass — no NSApp, window or EventLoop needed: the method is added; a second call returns `AlreadyPresent`).
  - `add_open_urls_imp_enqueues_file_urls_only` (`msg_send` with `[file://…, https://…]` enqueues exactly one path).
- *Honesty:* this bridge is only proven by the Mac hand test. Cloud gates type-check it and nothing more.

S4-C (cross-process document lock) is **deleted**: duplicate-process write safety is S3's fingerprint prompt plus its per-session `Recovery/<rid>/session.lock`, not a separate S4 lock (§3.2, R4).

**S6-A — Pure-PDF writer and scope planner (varos-pdf only).** · model **opus** (privacy-critical refactor with byte-identity) · **M** · depends: none; **start now**.
- *Owns:* `varos-pdf/src/lib.rs` (write side only: move the page loop into `write.rs`, re-export), new `src/write.rs`, `src/export.rs`, `tests/export_pdf.rs`, `tests/fixtures/native_demo.pdf`.
- *Must not touch:* `extract_model`/`load_vrs` (S5 owns reading), `varos-core`, `varos-app`.
- *Steps:*
  1. **Before any edit**, write `native_demo.pdf` from `container.rs::demo_doc` **without** a mask; the masked, rotated, translucent two-board fixture is regenerated in the clip commit (step 4), and the PR says so.
  2. Move the loop into `write_pages`. `write_pdf` must stay byte-identical (proven against the unmasked fixture).
  3. Add the §3.3 API (bounded `has_embedded_model`, `outline_bbox`-based Artwork bounds — see P2 changes below).
  4. Implement the MASKS_PLAN Stage 5 clip in `write_pages`: `q`, mask rings via `unit_xform`, `W* n`, the member's paint, `Q`; cull a member whose bbox misses the mask bbox.
- *`has_embedded_model` must not run a full lopdf parse on an arbitrary file:* S6-B calls it on whatever file the user picks as the export destination, up to 256 MiB, and an unbounded lopdf load is exactly the risk S5-D exists to remove. It is a bounded byte scan for `/VAROS_Model` or `model.varos.json`, never a PDF parse; it feeds a warning only, so a false positive just asks one extra question.
- *Artwork bounds use the canvas's own extent, not the raw control hull:* `world_bbox` is the hull of control points (handles included), which oversizes curved art. Use the union of `doc.outline_bbox(pi)` over visible painted paths (the flattened, xform-aware extent the canvas already uses), padded by half the stroke width.
- *Tests, `tests/export_pdf.rs`, all through lopdf:*
  - `native_write_is_byte_identical_to_fixture` (unmasked fixture).
  - Scope:
    - `all_visible_two_boards_two_pages_sized_like_boards`.
    - `hidden_board_excluded`, `all_hidden_is_unavailable_not_a_dummy_page`.
    - `active_scope_one_page`, `active_hidden_unavailable`.
    - `boardless_artwork_bounds_page_matches_padded_outline_bbox`, `boardless_empty_is_nothing_to_export`.
  - Privacy:
    - `export_has_no_embedded_file_filespec_af_names_or_varos_keys` (walks every object).
    - `export_leaks_no_hidden_coordinates_or_names`: a hidden path, a hidden layer and a path on a hidden board each carry unique coordinates such as `777.125`, and there are the names "SECRET-LAYER" and "Board Secret". None of these appear in any decompressed stream or in the raw bytes.
    - `exported_pdf_is_refused_by_load_vrs`.
    - `has_embedded_model_is_a_bounded_scan_not_a_parse` (a large non-PDF garbage file with no marker returns `false` without allocating unbounded memory).
  - Content and behaviour:
    - `visible_content_present`: the fill operator and the expected page coordinates.
    - `opacity_and_rotation_survive_export`.
    - `clip_member_is_wrapped_in_w_star_n`, `member_outside_mask_is_not_emitted`.
    - `export_is_deterministic`, `cancel_before_first_page_returns_cancelled`.
    - `has_embedded_model_true_for_native_false_for_export`.

**S6-B — Export home and export job.** · model **opus** · **M** · depends: S1, S3-F1, and S6-A merged.
- *Owns:*
  - New `varos-app/src/export_home.rs`: `ExportHomeModel::from(&Document, scope, job_state)` → rows, reasons, page count and status copy. It is pure and testable, and renders with egui. Per-session scope choice lives here, in a `HashMap<SessionId, ExportScope>` (S6-B does not own S1's `workspace.rs`, so it is not stored on the session).
  - New `varos-app/src/export_job.rs`: adds `Job::Export { sid, dest, doc, plan, cancel: Arc<AtomicBool> }` to S3's `IoWorker` (§3.6 of S2_S3 WO: `submit(key, job: Box<dyn FnOnce() -> Completion + Send>)`, a generic job runner S6 reuses rather than standing up a second thread and writer). The write uses S3's `durable::write_replace` — there is no separate S6 durable writer.
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
  6. The job (submitted to S3's `IoWorker`), completion, and status. Compute the export plan only when `(session, rev, scope)` changes, not on every panel frame.
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

**S6-C — Entry points, F4.2 file-effect parity audit, and system journey test.** · model **sonnet** · **M** · depends: S1-C, S2-E2, S6-B. Merges **last** (S6-D is folded into this piece: the release gate is Ahmed's manual journey on the Mac, and a separate one-test piece is not worth its own agent — the moderator still edits `STATUS.md`/`GATE_LOG.md` with the hand-test list, the subagent does not).
- *Owns:*
  - `chrome.rs`: extend S1's `FileCmd` with `Export` and its native row (no `file_routes.rs` — see §3.3).
  - `mac_menu.rs`: `sync_enabled`.
  - `ui.rs` `build_topbar`: Export button and hamburger rows wired; a disabled look when unavailable.
  - `main.rs`: the menu-drain mapping and the keyboard `shortcut()` file chords going through S1's `to_app_command`.
  - New `varos-app/tests/…` or `main.rs` `#[cfg(test)] mod system_journey_tests` (headless, fake ports and clock).
- *Must not touch:* `export_home.rs`, `export_job.rs`, `shell/*`, `varos-pdf`.
- *Tests:*
  - `every_file_command_has_every_route_the_spec_lists` (reads the real sources: `chrome::menus()`, the burger row list, and `lifecycle_key`/`to_app_command` — not a separate table).
  - `all_routes_of_a_command_map_to_the_same_app_command`.
  - `export_routes_all_emit_show_export_for_active_session`.
  - `no_enabled_file_row_is_dead`: every hamburger/native file row maps to a `FileCmd`.
  - `export_rows_disabled_without_session`.
  - `menu_accel_equals_keyboard_chord` (extends `menu_mirror_tests`).
  - The PR includes the audit table: route × command, with evidence, for GATE_LOG.
  - `system_journey_headless` follows spec §5:
    1. New A, draw, save.
    2. New B, draw, switch/reorder/undo.
    3. `OpenPaths(C, OsEvent)`.
    4. Dirty-quit cancel.
    5. Snapshot B, then a simulated crash (drop the workspace without closing, and drop S3's `RecoveryStore` so its `session.lock` is released before the rescan).
    6. Relaunch scan, recover B, Save As.
    7. Export the active scope (inspect the PDF with lopdf).
    8. Reopen A and B, then quit cleanly.
    - It asserts content, titles, paths, dirty marks and the PDF's page count and privacy.

## 5. Merge order and conflict hot-spots

Order — a dependency graph, not a line (S6-B/C need nothing from S5, and S5 has its own gate on ADR-0008; chaining S6 behind it would block S6 for no reason):
- **S6-A**: any time now.
- **S4-A**: after S1-D. **S4-B**: after S4-A.
- **S6-B**: after S1 + S3-F1 + S6-A. **S6-C**: after S1-C + S2-E2 + S6-B.
- **S5** is independent. The only ordering is the `varos-pdf/src/lib.rs` rebase between S5-D and S6-A.

Use `--no-ff` for every merge, with the full gates after each: test, clippy (host, Windows target, `aarch64-apple-darwin`), and fmt.

Hot spots:
- `varos-pdf/src/lib.rs`: S6-A (write side) against S5 (read side, plus catalog `VAROS_SchemaVersion` → 2).
  - S6-A moves writing into `write.rs` so that S5's reader edits don't collide.
  - Whichever lands second rebases.
  - **If S5 lands after S6-A, S5 intentionally regenerates `native_demo.pdf`** (the version key changes) and must say so in its PR.
- `main.rs`:
  - S4-A takes the startup and `AboutToWait` hunks. S4-B takes one line after the event-loop build.
  - S6-B takes the job-completion hunk. S6-C takes the menu drain and `shortcut()`.
  - S2-E2 also touches `main.rs` (AppView, launch-to-Start, Recents) and S3-F1 owns the `AboutToWait` completion drain — both land before S6-B/S6-C per the order above.
  - These are separate regions. The moderator resolves them by keeping both sides.
- `ui.rs`: S6-B only the panel-host arm (`~:1344`), S6-C only `build_topbar` (`~:3290–3425`). No overlap is expected.
- `chrome.rs`/`mac_menu.rs`: S1-C and S2-E2 land first (New ⌘N, Open Recent ▸). S6-C only adds the `Export` row/arm to what they leave; it adapts and does not duplicate.
- `Cargo.toml` (varos-app): S4-B only (features). `Cargo.lock` should not change; if it does, the moderator checks it.

## 6. Risks / open questions (each with a default)

- **R1 — macOS bridge mechanics (the spec's S4 spike).**
  - Adding a method to winit's delegate class is an ObjC-runtime patch that depends on a winit implementation detail. The docs even contradict the code.
  - *Default:* `class_addMethod` guarded by `respondsToSelector`, followed by `app.setDelegate(app.delegate())` to force AppKit to re-read its responds-to cache. There is no swizzle fallback (delegate replacement is fatal — see §3.2). Pin `winit = "=0.30.13"` in the PR comment and re-check on any winit bump.
  - If it does not work within the day, S4's gate stays **open** and S4-B's partial result is recorded. The Windows/CLI half still merges.
- **R2 — Masks in PDF.** Decided: S6-A implements MASKS_PLAN Stage 5's `q … W* n … Q` clip directly in `write_pages` (§3.3 Masks), which also fixes the same gap in today's `.vrs` preview pages at no extra cost.
  - *Fallback only:* if the clip cannot pass inside the day, S6-A refuses documents whose chosen pages contain masked art (`ClipMasksNotSupported`) instead of shipping a silent misrender. Refusal would otherwise block Export for any file with a mask (Astra's files use masks, STATUS batch-2 item 6) and a warning-then-export would show art outside the mask, so the fallback is a last resort, not the plan.
- **R3 — Windows unverifiable.**
  - *Default:* compile gate only. P5's Windows half stays **pending** in STATUS and GATE_LOG. The "Show in Explorer" and IFileSaveDialog overwrite prompt are unverified. Nobody claims Windows works.
- **R4 — Terminal duplicate processes on macOS.** There is no single-instance mutex (per the spec).
  - *Default:* S4-C is deleted (§3.2). S3's fingerprint prompt (`external_change_prompts_before_overwrite`) plus its per-session `session.lock` refuse a second writer's silent overwrite; a second process only happens when someone execs the binary directly (Finder/Dock/`open` reuse the running app). Forwarding the paths to the running instance through `NSWorkspace` is **later**.
- **R5 — Artwork bounds or pages above 14,400 pt** may be clamped by some viewers.
  - *Default:* export as-is, and the Export home shows "Pages larger than 200 in may not open in every PDF viewer."
- **R6 — Export shortcut.** The spec lists none.
  - *Default:* no accelerator. The parity table allows `chord: None`.
- **R7 — Share button and search pill** (not file actions, outside this system). The Share button gets S1-C's disabled "not available yet" look; the search pill stays drawn-only.
  - *Default:* untouched here. S6-C's audit lists the search pill as an out-of-scope dead control for Ahmed's queue.
- **R8 — UTI conforming to `com.adobe.pdf`** would give Quick Look thumbnails, but other apps might treat `.vrs` as PDF and strip the model on re-save.
  - *Default:* leave `bundle.sh` conformance unchanged. Do not register Varos for `.pdf`.
- **R9 — Selection scope** (raised in the brief) is not in the spec's scope list.
  - *Default:* not offered. The scopes are All visible / Active / Artwork bounds.
- **R10 — Window drag-and-drop** (`DroppedFile`) is not required by the spec.
  - *Default:* out of scope. It is trivial later, through `os_open::enqueue(.., OsEvent)`.
- **R11 — Upstream API drift** (S1/S2/S3 names).
  - *Default:* adapt the call sites and keep the §3.1 behaviour contract. If a contract item is missing (e.g. dedup inside a batch), the S4-A/S6-B piece adds a failing test and asks the moderator. It does not re-implement S1 logic.

## 7. Out of scope

PNG and SVG engines and their settings; batch export; Export for Screens; a Selection scope; print profiles, bleed/TrimBox, CMYK, fonts and text; multi-level nested clip masks (single-level only, matching the canvas — R2); foreign PDF/AI/SVG import; registering Varos for `.pdf`; installers, signing, notarization and updaters; cross-window tab drag and extra windows; forwarding terminal launches to a running macOS instance; Windows hand-testing and any Windows-specific UX beyond the handoff; file drag-and-drop onto the window; wiring the Share button and the search pill (S1-C gives Share a disabled look only); F09 (the Artboards system).

## Needs Ahmed (from review)
1. **Default export name for a document opened from a `.pdf`.** The spec's default is `<name>.pdf`, which is the open file itself, so the destination guard (§3.3, "an open Varos document" refusal) would always refuse it. **Working assumption:** suggest `<name> export.pdf` in that one case only.
