> **Status:** reference — independent code review, 2026-09-24.
# DFS S1-B — lifecycle rules + fake-port tests + real file ports: code review

Piece `DFS_S1_B_LIFECYCLE` · worktree `agent-aaf7e7af83236d739` · branch `feat/dfs-s1-b-lifecycle` · commits `15c0b7c`, `c737a36` on base `0f5bfdd`.
Contract: WO `DFS_S1_LIFECYCLE_TABS.md` §3.4 / §4 S1-B (amended), plan review F1 + F8, S1-A code review P3-3.

## Verdict: APPROVE WITH NITS
1. No data-loss path found. Quit, Close, Open-dedup, Save As refusal and the save-failure loop all behave as the WO says, and a test pins each one.
2. F1 (fresh key after every save) and P3-3 (fresh `store.key` per tab on Open) are both in the code and both tested.
3. The frozen API is unchanged. Only 3 files changed. `main.rs` gets exactly one line (`mod file_ports;`).
4. One P2: the `{reason}` text users see is the raw loader/writer string ("read failed: … (os error 2)"), not plain English (P2-1).
5. The P3s are small cleanups plus hand-offs to D and the moderator.

## Gates (measured by me in the worktree, shared `CARGO_TARGET_DIR=target-s1`, crates touched first)
- `cargo test --workspace -j 4`: **426 passed, 0 failed** (lifecycle 29, file_ports 4, matching the report).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `clippy --workspace --all-targets --target x86_64-pc-windows-msvc`: clean. `clippy -p varos-app --all-targets --target aarch64-apple-darwin`: clean.
- All 31 test names listed in WO §4 S1-B exist. The implementer added two more: `switch_reorder_and_window_commands_route_to_the_workspace` and `prompt_copy_follows_the_spec`.

## The moderator's checks
1. **Quit transaction.** `quit()` (`lifecycle.rs:249–259`) collects the dirty tabs in tab order, activates each one and asks "i of n". Cancel returns `false` right away: nothing is removed and earlier saves stay saved. `quit_cancel_on_second_keeps_everything_and_first_stays_saved` checks that tabs are `[a,b,c]`, `a` is clean, `b` is dirty, and that only `a.vrs` was written. If a save fails, `resolve → save` loops through `save_failed`. Cancel there → `false` → the quit aborts and the second tab is never asked (`quit_save_failure_aborts`, which also covers TryAgain → SaveAs → continue).
2. **Close of the only tab.** `close` → `ws.remove`. The last tab is replaced by a fresh `Untitled-N`, or nothing happens if it is already pristine (`workspace.rs:393–400`). Tested at `lifecycle.rs:958,988`.
3. **Open dedup and pristine reuse.** Every open tab gets a fresh key (`open_tab_of`, `:146–154`). A match only focuses the tab; nothing is reloaded, even when the tab is dirty. Aliases and external rewrites are tested (`:595–626`). Reuse goes through `add_loaded` → `replace_at`, which builds a new `DocumentSession::loaded`: a new id, a new `Editor` (empty history), and a checkpoint taken after `replace_doc`. So dirty state and history really are fresh; only the clipboard, tool and recent colours are handed over.
4. **Save As onto another tab's file** is refused with a notice and goes back to the dialog. This also works through an alias (`:798–822`). Save As onto the tab's own current path is allowed, because `except = Some(id)` (`:824–828`).
5. **F1 / P3-3.** `write()` (`:222–230`) recomputes `store.key(dest)` after the store succeeds and passes it to `mark_saved`, not to `set_key`. The WO says "pass to mark_saved", so this is correct. Tested by `open_after_save_still_focuses_the_same_tab`, where FakeStore swaps the inode on every save.
6. **Dismissed prompt = Cancel.** `decision_from` maps `_ => Cancel`. In rfd 0.15.4 on macOS, an unknown `NSModalResponse` maps to `Cancel` (`backend/macos/message_dialog.rs:135`), and NSAlert binds Escape to the button titled "Cancel", which comes back as `Custom("Cancel")` → Cancel. On Windows, `TDF_ALLOW_DIALOG_CANCELLATION` / MessageBox Escape both give `IDCANCEL`. Correct.
7. **rfd copy and button mapping.** Titles, filters and prompt copy match spec §4 word for word. The batch-1 "Varos" title and "…before quitting?" wording are correctly replaced. The fallback mapping is right: rfd lists the first custom label as "yes" on every backend, so Yes = Save / Try Again and No = Don't Save / Save As. See P3-5 for the Windows labels.
8. **`DiskStore` errors are not plain English.** See P2-1.
9. **Waste.** See P3-1 to P3-4.
10. **`main.rs`.** The diff is exactly `+mod file_ports;`.

## Findings

**P2-1 — `{reason}` shows raw internal strings to the user.** `DiskStore::load` / `save` (`file_ports.rs:166–171`) pass the `varos_pdf` / `varos_core` strings through unchanged:
- `read failed: No such file or directory (os error 2)` (`varos-pdf/src/lib.rs:37`)
- `not a valid .vrs model: expected value at line 1 column 1` (`varos-core/src/file.rs:32,36`)
- `not a readable PDF: …`, `no PDF catalog: …`
- `this file was saved by a newer Varos (v3) — please update`
- `write failed: Permission denied (os error 13)` / `rename failed: …` (`file.rs:52–53`)

`sentence()` only trims them and adds a full stop, so Ahmed would see "Couldn't open “x.vrs”. / read failed: No such file or directory (os error 2). Your open documents have not changed." Spec §4's example reason is a plain sentence ("The editable model exceeds the 32 MiB limit"). `prompt_copy_follows_the_spec` (`file_ports.rs:286–290`) even pins `"… write failed: permission denied."` as the expected body.

Batch 1 had the same problem ("Save failed: {e}"), so this is not a regression. But B owns the copy, and S3 (save errors) and S5 (typed load errors) are several pieces away.

**Fix:** one private `fn plain_reason(raw: &str) -> String` in `file_ports.rs`, applied in `DiskStore::load/save` via `.map_err`:
- `read failed:` / `write failed:` / `rename failed:` → strip the prefix and the ` (os error N)` suffix, then capitalise (the same shaping `varos_app::storage::durable::io_reason` already does — reuse its logic, or better, get a real `io::Error` from a `std::fs::metadata(path)` pre-check on load and call `io_reason`)
- the newer-Varos message → "It was saved by a newer version of Varos. Update Varos to open it"
- any other parse error → "It isn't a Varos document, or it is damaged"

Add 3–4 asserts to a test, and change the pinned `write failed` expectation. S3's switch to `durable::write_replace` (whose `WriteError::reason()` is already plain) replaces the save half later.

**P3-1 — Stored identity is now write-only.** Because the P3-3 fix compares fresh keys, nothing in production reads `DocumentSession.key`, `Workspace::find_file` or `DocumentSession::set_key` any more (`grep`: only tests and `mark_saved`/`add_loaded` writes). This is the frozen A API, so B was right not to touch it. The moderator/D should either delete `find_file`/`set_key` in D's cleanup, or record why they stay (S3 fingerprinting may want the stored key). A side effect of fresh-only matching: a file renamed in Finder while open no longer matches its tab by inode. This is harmless: two different files, no overwrite.

**P3-2 — "Replace?" is asked before the other-tab refusal** (`lifecycle.rs:203–214`). When the appended `.vrs` name is another tab's file, the user answers "Replace" and then gets "open in another tab". **Fix:** compute `key` / `open_tab_of` before the `confirm_replace` check (swap the two blocks).

**P3-3 — Pristine reuse can drop a redo stack.** `is_pristine` (A, `workspace.rs:140`) is content-based, so an Untitled where the user drew and then undid everything counts as pristine. `add_loaded` replaces it, and its redo history is lost. The window is narrow, and the owner is A/the moderator. Consider adding `&& !editor.can_redo()` (or "history empty") to `is_pristine`.

**P3-4 — Small waste.**
- `stem_of` and `with_vrs` each have one caller (`lifecycle.rs:283,290`). They are fine as named helpers, or they could be inlined.
- `decision_from`, `fail_choice_from` and `file_key` are `pub` but only used inside `file_ports.rs` → make them private.
- The test fixtures `art()` (`lifecycle.rs:499`) and `doc_with_art()` (`file_ports.rs:231`) are the same square-on-a-board. One `#[cfg(test)]` helper would do.
- `Scratch` duplicates `storage::testdir::TestDir`, but that type is `pub(crate)` + `cfg(test)` in the lib crate, so the binary cannot reach it. Acceptable.
- Hand-off to D: `quit_answer`, `QUIT_*` and their tests in `main.rs` now duplicate `decision_from`. D deletes them with `QuitAnswer`/`may_quit`.

**P3-5 — Windows fallback labels.** rfd is built without `common-controls-v6`, so `YesNoCancelCustom` / `OkCancelCustom` render as Yes/No/Cancel and OK/Cancel (`rfd win_cid/message_dialog.rs:49`). The mapping is correct, but "Couldn't save … Yes/No/Cancel" does not say that No = Save As. Windows is compile-only, so log this for S4: enable the `common-controls-v6` feature, or add Windows-only body text. Windows identity is the absolute path, compared case-sensitively (`file_ports.rs:199–202`), so `C:\A.vrs` and `c:\a.vrs` would open twice. This is already declared "until S4"; log it with the same S4 item.

## Honesty check
Every claim in the report was reproduced: 426/426; 29 + 4 tests; no frozen signature changed; tabs take `key.path` (`lifecycle.rs:135`; DiskStore resolves symlinks, tested at `file_ports.rs:345`); the Save-As-onto-other-tab notice; "Document i of n" in the body (`file_ports.rs:61`); Windows = absolute path only. Nothing is overclaimed.
