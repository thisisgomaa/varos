> **Status:** reference — independent code review, 2026-09-24.
# Code review — DFS S1-A: core content checkpoint + frozen app API + workspace model

Reviewed: branch `feat/dfs-s1-a-workspace` @ `9b8c741`, base `c1c7de9` (`git diff c1c7de9...HEAD`, 9 files, +1555/−8), against `DFS_S1_LIFECYCLE_TABS.md` §3.1–3.4 / piece S1-A and its plan review (F1, F2, F3, F6).

**Verdict: APPROVE WITH NITS.**
1. `content_eq` is right. The destructure is exhaustive, the field split matches spec §2, `paths` is compared by value in Vec order (z-order is content), and `ids` is ignored. Every authored change I tried is caught.
2. The workspace semantics match the contract and the review fixes. F1 (path OR inode + `set_key`), F2 (insertion slot, with the move-right-by-one test), F3 (`Option` active) and F6 (item allows in `ui.rs`) are all applied and tested.
3. There is one real hole in the dirty memo (P2-1). It comes from a pre-existing core bug: the Pen tool can change content with no `rev` bump and no history step. The fix is two lines in `editor.rs`, a file A owns, so do it before merge.
4. Cost is acceptable and measured (see Gates). It adds about one extra Document clone per edit, and nothing per frame.
5. There are no law violations: core stays pure, no GPU/EventLoop in tests, UI document writes still go through EditCommand, and no tokens/visuals are touched.

## Gates (measured by me in the worktree)
- `cargo test --workspace -j 4`: **365 passed, 0 failed** (matches the report).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `clippy --workspace --all-targets --target x86_64-pc-windows-msvc`: clean. `clippy -p varos-app --all-targets --target aarch64-apple-darwin`: clean.
- **Cost probe** (a throwaway test, deleted afterwards; 10 000 paths × 8 anchors with handles, 10 001 nodes):
  - Release: `Document::clone` 3.8 ms · `content_eq` (equal, full walk) 3.9 ms · `content_eq` (last path differs) 0.5 ms.
  - Debug: 11.8 ms / 12.1 ms / 5.6 ms.
  - `begin()` already clones the whole document on every gesture/commit, so the checkpoint roughly doubles the per-edit overhead. It is paid once per `rev` change on the active tab, never per frame (the memo holds).
  - Memory: one extra Document per tab. History already holds up to 200.
  - WO §6.9 asks to measure on "P11.2 scene E". Scene E is only 500 rectangles, so this probe is the stricter number. Record it in GATE_LOG.
- **Silent-write probe** (throwaway): every tool (12) × selection on/off × 7 points × click/tiny-drag, flagging `rev` unchanged && `!content_eq(before)`. Only one path was found (P2-1). A code search of `self.doc.` writes outside `begin`/`commit` in `editor.rs` / `tools/` / `command.rs` agrees: every other hit is a preference (`active`, `snap`, `ruler_origin`, `guides_locked`) or runs inside a transaction that sets `dirty`.

## Findings

**P2-1 — Pen "continue path" reverses a path with no `rev` bump and no history. The tab then shows clean while Close/Quit asks to save.**
- Evidence: `varos-core/src/editor.rs:3240–3252` `resume()`. When the clicked end is the path's first anchor, it calls `self.reverse(pi)` (anchor order is saved content) but never sets `self.dirty`. `pointer_up` → `commit()` (`:3049`) then pushes no undo step and does not bump `rev`.
- Reproduced: Pen tool, open 3-anchor path selected, click its FIRST anchor, release. Result: `rev 0 → 0`, `content_eq(saved) == false`, order `[12, 11, 10]`.
- Effect on S1:
  - `DocumentSession::is_dirty` (`workspace.rs:119`) returns the stale memo `(rev, false)`, so there is no dot and no `*`.
  - `is_dirty_exact` returns true, so ⌘W on a tab that shows no dot asks “Save changes?”.
  - Pre-existing core bug: ⌘Z cannot restore the order either.
- Fix (`editor.rs:3246–3248`): `if last != Some(end_aid) { self.reverse(pi); self.dirty = true; }`. Add a test in `content_checkpoint.rs`: `pen_resume_from_the_first_anchor_is_a_history_step`. It asserts that `rev` bumps, `!content_eq`, and that Undo restores the order and makes `content_eq` true again.
- Also fix the `is_dirty` doc comment: it says "the memo holds for every EditCommand path", but pointer gestures are not EditCommands.

**P3-1 — `settle()` misses a transaction with no drag.** `workspace.rs:161`: the gate is `drag != None || ab_drag != None`. Pen delete-anchor / add-anchor / close clicks open a transaction in `pointer_down` (`editor.rs:3419`), change the doc and return with `drag == None` (`tools/pen.rs:22–41`). A ⌘S/⌘W between press and release leaves the transaction open. Nothing is lost: the overlay reports dirty and the release commits. Fix: gate on `|| ed.transaction_open()` (after `Ui::settle` has cancelled any picker, the only open transaction left is a pointer one).

**P3-2 — Per-frame full compare while a transaction is open and still unchanged.** `content_dirty` (`workspace.rs:172`) never writes the memo mid-transaction. If `rev` changed and a gesture starts before the next `tabs()` call (for example commit and press in one event batch), every frame of a marquee/no-op drag re-walks the document (~4 ms release on 10k paths) until `dirty` turns true. Fix: when a transaction is open and `!ed.dirty`, use the pre-transaction content, which is `pending`. Or cheaper: memoise `(rev, dirty)` even mid-transaction when `!ed.dirty`, because the doc equals `pending` then.

**P3-3 — Case/symlink alias after an EXTERNAL atomic rewrite.** `same_file` (`workspace.rs:45`) matches by path OR stored inode. If another app rewrites the file (new inode) and the user opens it through a symlink or a differently-cased spelling, neither matches, so a second tab opens. `path` is compared raw (`PathBuf ==`, case-sensitive); canonicalisation is `DocStore::key`'s job (S1-D). On macOS, `std::fs::canonicalize` does not guarantee the on-disk case. Forward to B: on Open, compare against a FRESH `store.key(session.path)`, not the stored key. This costs one stat per open tab per Open.

**P3-4 — Twin labels can stay ambiguous.** `tabs()` (`workspace.rs:408–422`) appends only the parent folder. `/a/x/Logo.vrs` and `/b/x/Logo.vrs` both read `Logo.vrs — x` (the tooltip still disambiguates). Acceptable for S1; log it.

**P3-5 — A NaN in a saved document makes its tab permanently dirty.** `content_eq` uses `PartialEq`, so `saved != saved`. This errs to the safe side, as documented. Loads with NaN are an S5 validation concern; note it there.

## Answers to the moderator's checks
1. `content_eq`: exhaustive `let Document { .. }` plus `DocUnits { ppi, display: _ }` (`model.rs:618–635`). `paths` is compared in Vec order; `ids`, `active`, `active_layer`, `snap`, `ruler_origin`, `guides_locked`, `move_art_with_ab` and `units.display` are ignored. The test covers anchor handle, hole, node colour, bleed, guide add/move, xform, clip role/mask, z-order and ppi (`content_checkpoint.rs:71–126`). `Path`/`Node`/`Artboard` hold no UI-state fields, so there is no false dirty from Layers expand etc.
2. Content changes without a `rev` bump: exactly one (P2-1). The rest are preferences.
3. Cost: see Gates.
4. `same_file`: path-equal OR both-inode-equal. The path is NOT canonicalised here (by design: D's `DiskStore::key`). A live symlink or case alias is caught by inode (`same_file_survives_an_atomic_save`, unix). For the stale-inode gap, see P3-3.
5. `remove` of the last tab: a fresh `Untitled-N` from a monotonic counter (tests show Untitled-4, then -5). A pristine last tab is a no-op. The clipboard is **moved** and follows the active tab, so it is never duplicated: A→B→A keeps it, and copy-in-A / paste-in-A after a round trip works (`activate_hands_over_clipboard_and_tool`). Closing the active holder hands it over first (`remove_picks_right_then_left…`).
6. Undo/redo carry of `snap`/`guides_locked`/`ruler_origin` (`editor.rs:3087–3092`): `undo_selection.rs` and all other existing core tests pass unmodified. The view-only law holds (`view_only_actions_never_dirty`).
7. Ports: every method is plain data in/out (`Vec<PathBuf>`, `Option<PathBuf>`, enums, `Result<_, String>`, `FileKey`), so all are fakeable headlessly. `Effect { exit }` is enough for D: D detects an id change itself and, per plan-review F9, calls `document_switched` after every command. `add_loaded` also always mints a new id, which makes an in-place replacement visible.
8. `ui.rs`: the four methods and three fields sit next to `set_doc_tab` (`ui.rs:1164–1198`). `build_topbar` / `tab_item` / menus are untouched. The `set_tabs` stub only mirrors labels into the old `tabs`/`tab_active`.
9. Waste: none material. Every public item has a named consumer in B/C/D (`set_key`/`get`/`display_name`/`activate_relative` → B, `tabs`/`fit_pending`/`index_of` → C/D, `OpenOrigin` → frozen §3.2). The deviation from `cfg_attr(not(test), allow(dead_code))` to a plain `allow(dead_code)` is justified in-file (test builds would fail clippy) and D removes it. Test square helpers are duplicated between `workspace.rs` and `content_checkpoint.rs` (crate boundary; acceptable).
