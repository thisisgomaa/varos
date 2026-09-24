> **Status:** reference — independent plan review, 2026-09-24.
# Review — DFS S2 + S3 (Start, recents, durable writes, autosave/recovery)

**Verdict: APPROVE WITH CHANGES.**
1. The heavy parts (durable writer, two generations with the manifest written last, orphan detection through locks, a pure scheduler plus one I/O thread) are what the accepted spec asks for (§2 "Start, recent files and recovery", §3 storage safety). They are not over-engineering, and seven pieces is a fair size for two spec stages.
2. It is over-built in four places: the three recovery locks (owner, claim, adopt), the manifest blob-scan fallback, the `FileEvent` filter, and E repeating S1-C's menu work. Cutting them loses no acceptance criterion.
3. The seams with S1 are under-specified. S1's workspace is never empty, S1 has no `Copy` checkpoint, and S1's `Dialogs`/`DocStore` traits are frozen. The seams with QW2 and S4/S6 are the same: three names for one resolver, and two job runners.
4. Piece A (in flight) needs **one behaviour fix** (P1-1). Its API stays as it is. The other P1 changes the plan text around A, not A.

Reviewed against: this work order (read in full); the spec; CLAUDE.md; STATUS top; the S1, S4/S6, S5 and QUICK_WINS work orders; the code at `475a676`; piece A's worktree `feat/dfs-s3-a-storage` (uncommitted); and S1-A's worktree `feat/dfs-s1-a-workspace`.

## Findings

**P1-1 — A: the Apple full-sync needs the SQLite fallback, or Save starts failing on some volumes.** *(changes A's behaviour, not its API)*
Evidence:
- A's `durable.rs` header cites std on Apple: `sync_all` = `fcntl(F_FULLFSYNC)`, with no fallback.
- `write_replace` turns any sync error into a failed write (A worktree `durable.rs:267-268`). `RealFs::sync_dir` does `File::open(dir)?.sync_all()` (`:88`).
- SQLite's `full_fsync` (os_unix.c) retries with `fsync()` when F_FULLFSYNC fails, because some non-local file systems don't support it.
- Today's save has no sync at all (`varos-core/src/file.rs:50-54`), so saves to those volumes work now. After F1 routes Save through `write_replace`, they would say "Couldn't save".

Change §3.3 step 3 from "`write_all` → `sync_all` (std on Apple targets issues `fcntl(F_FULLFSYNC)`…)" to:
"`write_all` → `sync_all`. On Apple, if F_FULLFSYNC fails with ENOTSUP/ENOTTY/EINVAL, retry with plain `libc::fsync`. That needs a macOS-only direct `libc = "0.2"`, already in Cargo.lock through winit/objc2, so zero new crates. Use the same helper for `sync_dir`."
Mac hand check: save to a USB stick or SMB share.
While A is open, also fix this A nit (P3): `temp_path` adds about 45 bytes to the file name, so a name over about 210 bytes hits ENAMETOOLONG. Cap the name part.

**P1-2 — One resolver with one owner. QW2 must not build a second one.**
Evidence:
- QW2 (wave 1, not started) creates `app_paths.rs::data_dir_from` and edits `main.rs:446-452`.
- A has already rewritten those same lines (A worktree diff, `win_state_path`/`crash_log_path` → `storage::paths::AppLayout`).
- A keeps Mac window memory **off**, and QW2 turns it **on**.
- S4/S6 WO:72 expects a third name, `app_paths::data_root() -> Result<PathBuf, AppPathError>`, which S4-C uses (WO:213-220).

Add to §3.2: "**Frozen for other work orders:** `varos_app::storage::paths::{data_root() -> Option<PathBuf>, AppLayout}`. QW2 does not create `app_paths.rs`. It runs after A merges and only removes the macOS `None` guard in `win_state_path` and adds the on-screen clamp (`main.rs:800-812`). S4-C and S6-B call `storage::paths::data_root()`."
In §6 item 8, replace "Default: stay disabled on macOS in this stage (separate Mac-polish item)" with "Default: A keeps it off. QW2, after A, turns it on together with the on-screen clamp."
The moderator also edits QUICK_WINS QW2 and S4/S6 WO:72 to match.

**P2-3 — Recovery store: three lock mechanisms where one will do.**
The spec asks only for "per-process/session ownership locks" (§3, data-root paragraph). The plan has `owners/<owner>.lock`, a `claim.lock` per rid, an `adopt` that rewrites the owner, stale-owner cleanup, and an `owner` field in the manifest.
With one `Recovery/<rid>/session.lock` there is less to build and test:
- The live session holds it (try_lock when the dir is created).
- `scan` try-locks each dir. Success means an orphan that is **already claimed** by this process.
- Recover keeps the lock, and later `write_generation` calls rewrite the manifest meta, so no separate adopt is needed.
std `try_lock` on Unix is `flock`, which conflicts between two opens inside one process, so the "two stores in one process" tests still work.
Change §3.5 layout to `Recovery/<rid>/{session.lock, manifest.json, snap-<seq>.json}`. Delete `owner()`, `claim()`, `adopt()`, `ClaimGuard` and `Manifest.owner`. `scan() -> Vec<OrphanEntry>` holds each orphan's lock until Recover/Discard or exit. Tests: replace `claim_is_exclusive`/`adopt_rewrites_owner…` with `scan_claims_orphan_exclusively` and `recovered_session_keeps_writing_same_rid`.

**P2-4 — Data-loss path: a snapshot written after a failed retire is deleted at the next launch.**
- `rid` is "per session" for the session's whole life (§3.5).
- `retire` writes a `retired` marker first, then deletes the files.
- `cleanup_completed` removes every dir that has the marker.
If a delete fails (the marker stays) and the user then edits, the next snapshot goes into the same marked dir. `scan` skips it and the next launch deletes it.
Change `retire` to "atomic `rename(<rid>, <rid>.discarded-<nonce>)`, then best-effort `remove_dir_all`; cleanup removes `*.discarded-*`". Add: "after every Retire the session takes a fresh rid". New test: `snapshot_after_partial_retire_survives_cleanup`.

**P2-5 — Drop the unreadable-manifest blob-scan fallback.**
The spec asks only for a visible fallback from a corrupt newest generation, and for damaged copies to be kept (§2). The manifest is written through `write_replace`.
Replace "manifest unreadable ⇒ scan `snap-*.json` newest-first (metadata "unknown")" with "manifest unreadable ⇒ `OrphanState::Damaged(reason)`, kept". Rename the test to `unreadable_manifest_is_damaged_and_retained`.

**P2-6 — §3.9's assumptions about S1 are already wrong. Fix them now; don't leave it to "adapt".**
(a) S1 has no `content_checkpoint() -> Copy`. S1 keeps `saved: Document` plus `content_eq` (S1 WO §3.1).
- In §3.6/§3.9 S1-3, set `Probe.checkpoint = editor.rev` (a change signal) and `clean = !session.is_dirty()`.
- `Completion.checkpoint` also becomes `rev`.
(b) S1 names the probe `Editor::transaction_open()` (S1 worktree `editor.rs:3094`). Delete "If absent, F1 adds this 3-line read-only method to core" and use S1's name.
(c) S1's `Workspace` is never empty: `active: SessionId`, and `remove` of the last tab creates a fresh Untitled (S1 worktree `workspace.rs:197`, `:344-355`). E's ownership list doesn't include `workspace.rs`/`lifecycle.rs`.
Default that needs **no S1 edits**: `AppView::Start` treats a sole pristine untitled session as "no document":
- no chip, title "Varos";
- New from Start reveals it;
- Open reuses it (S1's `add_loaded` already does this);
- clipboard hand-over keeps working.
Replace S1-1's "If S1 keeps a non-empty invariant, piece E relaxes it" with that rule.
(d) S1's `DocStore::save -> Result<(), String>` and its `Dialogs` trait (S1 WO §3.4) can express neither `ReplacedUnconfirmed` nor the external-change prompt. Add `lifecycle.rs` to F1's Owns with `DocStore::save -> Result<SaveOutcome, String>` and `Dialogs::external_change(name) -> ExternalChoice`.

**P2-7 — B duplicates S1's file identity. Its case folding is also wrong.**
- S1 already has `FileKey { path, dev_ino }` and `DocStore::key` (canonical path + dev/ino, S1 WO §3.3).
- B adds a second `normalize` (lexical clean + case-fold on macOS). That is wrong on case-sensitive APFS and ignores NFD names.
Change: "Dedupe = equal `dev_ino` when both are known, else exact path equality. The caller passes S1's `DocStore::key(path)` values." Delete `case_insensitive_dedupe_on_mac_and_windows`.
Also drop the `FileEvent` enum. It has one caller, and it "proves" a property the caller can bypass. Use `Recents::record(path, dev_ino, now)`, called only at the Opened/Saved sites. The integration tests already prove "never pollute": E `failed_open_leaves_recents…` and S6 `export_success_leaves_…recents_unchanged`.

**P2-8 — E repeats S1-C and is too big for one agent.**
S1-C already adds native New ⌘N, updates the `KeyN` test (`chrome.rs:482-488`), and wires the burger New/Open/Save/Save As (S1 WO §3.6, S1-C steps 3-4).
- Cut E's chrome/burger scope to "Open Recent ▸ (≤10) + Clear Menu" only.
- Split E:
  - **E1**, sonnet or opus, wave 1, no S1 dependency: the pure `start.rs` model plus a `start_ui::draw(&mut egui::Ui, &StartModel) -> Vec<StartAction>` tested on a bare `egui::Context`.
  - **E2**, opus, after S1: AppView, Home chip, Board-leaf mode, key gating and menus.

**P2-9 — F1 and F2 cannot run in parallel as written.**
- F2's Recover must build a session that carries the adopted rid in `SessionRecovery`, a field F1 owns (§3.9 S1-2).
- Both edit `main.rs`, `ui.rs` and the S1 session module.
Change: D freezes `pub struct SessionRecovery { rid, last_ok, last_err, … }` in `storage/scheduler.rs`. Otherwise the order becomes F1 → F2.

**P2-10 — D's worker should be the app's only job runner.**
- S6-B expects "a job runner that returns `JobId`… completes through a proxy-woken `AppCommand`" and a `durable::replace_file(dest,&bytes,&CancelToken)` (S4/S6 WO:72, :156).
- D's typed `Job::{Snapshot, Retire}` holds a `RecoveryStore` inside, so it cannot run Export. That leads to a second thread and a second writer (S6 also plans an inline fallback writer).
Change D: `IoWorker::submit(key, Box<dyn FnOnce() -> Completion + Send>)`, with the recovery jobs built in F1. S6 then checks its `CancelToken` and calls `write_replace`. Fix S6's text to match.

**P2-11 — The spec's macOS "window close keeps the app" was dropped between two work orders.**
The S1 WO §6.1 defers it "to S2". This WO never mentions it, and spec §2 requires it. Add it to E2 or to §7 as an explicit deferral (see Needs Ahmed).

**P3-12** — §2 says "no proxy, no user events". That is false:
- `main.rs:837` passes `event_loop.create_proxy()` to `MacMenu::build`.
- `mac_menu.rs:63-70` uses an mpsc channel plus `send_event(())`.
In §3.6, say: "`wake` clones the existing proxy; one drain point in `AboutToWait` serves menu, OS-open (S4-B) and worker queues."

**P3-13** — The Document home already exists: `ui.rs:4794 document_section` (in the Properties dock when nothing is selected). S1 builds none (S1 WO §3.6). Replace "home = Document section from S1" with "F1 adds the Recovery rows to `document_section` (`ui.rs:4794`)".

**P3-14** — Ahmed's hand tests need copy-paste commands. A Finder launch passes no env var, so step 5 should read `VAROS_DATA_DIR=/tmp/ro /Applications/Varos.app/Contents/MacOS/varos`. Steps 4 and 6 should give the exact `kill -9 $(pgrep varos)` and `: > "…/Recovery/<rid>/snap-N.json"` lines, plus "Finder ▸ Get Info ▸ Modified unchanged" instead of "compare bytes/mtime".

**P3-15** — The `exists` probe of 20 recents on the UI thread can hang on a stale SMB mount. Run it on the worker, or accept and log it.

**P3-16** — Rename-replace drops macOS xattrs (Finder tags) on the real `.vrs`. Today's `write_atomic` does the same, so this is no regression. Log it for Mac polish.

**P3-17** — `FsPort` has already grown to 12 methods in A (the plan had 8). It is justified: cloud tests run as root, so real permission faults and disk-full can't be produced. Freeze it: no new methods without moderator OK. `FaultFs` shipping in the release lib is acceptable, because bin tests can't see `cfg(test)` lib items.

**P3-18** — `chrono`: verified. `Cargo.lock:559-567` has `iana-time-zone`, and only `lopdf` pulls chrono, so zero new crates. Keep it. CRC-32: the spec asks for a checksum. `crc32fast` is already locked (via flate2/png), but A's 65-line version is written and tested, so no change. Keep `out_of_order_completion_is_ignored` (the spec lists it), but note it is unreachable with one-in-flight + FIFO.

## Is there a better approach overall?
No, apart from the cuts above.
- A synchronous snapshot on the main loop would mean serialising up to 32 MiB of JSON plus F_FULLFSYNC every 30 s on the UI thread. That is a periodic stall, which breaks "a work tool answers instantly".
- std has no non-blocking file write, so the choice is a thread or a stall.
- One FIFO thread plus the existing mpsc+proxy pattern is the smallest correct design.

## Needs Ahmed
1. **Async real Save** (spec §3: "Save/Export I/O runs off the UI thread"). The plan keeps Save synchronous (§6 Q2). Recommended default: accept for S3, because the Save dialogs already block. Record it as a known deviation from the accepted spec, and revisit it if a 10 MB save measures >100 ms on the Mac.
2. **macOS red traffic light keeps the app running** (spec §2). Recommended default: defer. The single-window app keeps S1's "red light = Quit transaction" until multi-window work. Write the deferral into STATUS.
3. **Where the Recovery on/off switch lives.** Recommended default: the Properties-dock DOCUMENT section (`ui.rs:4794`), labelled "Recovery (all documents)".
