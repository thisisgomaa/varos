> **Status:** reference — independent code review, 2026-09-24.

# Code review: QW3, layer/object rename you can find + honest dock while drawing (`fix/qw3-layer-rename` @ `0679979`, base `c9067d4`)

**Verdict: APPROVE WITH NITS. No P1. Fix P2-1 and P2-2 before merge. P2-3 is a follow-up recommendation.**
1. The F10 root cause is fixed the right way: an EditCommand. A `<Path>` row now writes `Path::name`, which is the name the row displays. The rename is one undo step, and an empty or unchanged name is a no-op (`command.rs:240-249`).
2. The editor state machine is sound on egui 0.35. It cannot stick open on blur, Escape or Enter, it takes focus only once, and while it is focused Enter and Escape never reach the canvas.
3. The right-click menu reuses `menu_below`/`menu_row` (tokens only, no shadow) and lives in Rename's one home, the Layers panel.
4. The FB6 change leaves the dock partly dishonest. With a stale selection it shows zeros and the draft's paint, but its fields still write to the stale object (P2-1).
5. The new UI tests are real (mutations are caught) but slow: 10.8 s for 9 tests (P2-2).

## Gates (measured by me in the worktree, `varos/`)
- `cargo test --workspace -j 4`: **359 passed, 0 failed**, 0 ignored (40 result lines). This matches the report.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `cargo clippy -p varos-app --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` with `-D warnings`: both clean.
- `cargo test -p varos-app layer_rename_tests`: 9/9 in **10.80 s** (debug).
- `git merge-tree main fix/qw3-layer-rename`: clean. The branch is `main` plus two docs commits.

## Verified correct (evidence)
- **Blur commits.** egui surrenders focus on any press outside the widget (`egui-0.35.0/src/context.rs:1485-1494`). `main.rs:1022` feeds every window event to egui first, so a click on the canvas also closes the field.
- **Escape cancels.** `Focus::begin_pass` clears focus on Escape (`memory/mod.rs:~580`). The next `lost_focus()` sees `key_pressed(Escape)`, so the edit is cancelled and not committed. Test: `escape_cancels_an_edited_name`.
- **Enter commits.** Singleline `surrender_focus` (`text_edit/builder.rs:1115`), then `lost_focus()`.
- **No double commit.** `lost_focus` has a two-frame window (`memory/mod.rs:851-857`), but `*rename = None` removes the field on the first frame, so the commit cannot fire twice.
- **No focus theft.** `request_focus` runs only while the field is unfocused and has not just lost focus (`ui.rs:4473-4497`). The old code re-grabbed focus every frame, and because `Response::lost_focus` reads memory live, blur and Escape never closed it. The Escape and blur tests would fail on the old code.
- **Canvas never sees Enter/Escape.** Keys go to egui while `wants_keyboard()` (`main.rs:1246`), which is evaluated from the previous frame's focus.
- **Mutations are caught.** Dropping `!cancel`, `!v.is_empty()`, `v != row.name`, the select-all-on-open or the `!focused` guard fails a test. So does routing Path leaves back to `RenameNode`, or dropping the core no-op guards.
- **Constitution.** `menu_below` uses `Frame { .. Default }`, which has no shadow, with `SOLID_PANEL`/`BORDER`. `menu_row` uses `HOVER`/`TEXT`. There is no animation. The mirror rows have distinct `menu_id`s (`ui.rs:4568` includes `row.sec`).
- **Laws.** Core stays pure. The new tests use a bare `egui::Context` (no Renderer, no EventLoop). The only document write goes through `EditCommand`. The commit is staged by name (3 files).
- **Honest report.** The commit message discloses the core touch, the unchanged `RenameNode` rev-bump finding, and that the target checks are compile-only.

## Findings

**P2-1: Mid-draft, the dock shows zeros and the draft's paint while its fields edit the stale selection.**
- **What happens.** `Snap::read` forces `n = 0` when drawing (`ui.rs:657-658`), which routes into the F07 branch `direct_bb = ed.direct_bbox()` (`ui.rs:664`). But `direct_bbox` checks the *real* `objsel` (`editor.rs:987`). With a stale selection it returns `None`, so X/Y/W/H read 0,0,0,0.
- **Where the fields write.** The fields still dispatch `SetObjectBounds` → `set_obj_bbox`, which acts on the stale `objsel` (`editor.rs:1538`). Stroke weight, opacity and colour target `selected_pids` = stale ∪ draft (`editor.rs:4253`). The dock now shows the draft's paint (`ui.rs:675`), so the stale target is hidden.
- **Example.** Typing X = 100 mid-draft moves an object that the dock says is not selected.
- **The empty-selection case disagrees.** With an empty `objsel`, `direct_bbox` measures the draft's anchors (the Pen inserts each new anchor into `selected`, `pen.rs:90`). So `direct = true` and the fields are live on the draft. The same draft yields two different docks depending on leftover state, which is what FB6 set out to remove.
- **The test misses it.** `drawing_snap_reports_active_path_not_stale_selection` (`ui.rs:6427`) asserts only name, fill, sw and `!sel`. It checks neither x/y/w/h nor `direct`.
- **Fix, preferred.** One core line: the Pen clears `objsel` when it starts a new draft (`pen.rs:72-73`, next to `ed.selected.clear()`). This matches Illustrator, where a new path deselects other art. Every write then targets the draft, and `repr_path()` already resolves the draft through `selected`, so the `drawing ? active : repr_path()` special-case can go. Only the "Drawing path…" name override stays. This needs the same Owns ratification that `RenamePath` got.
- **Fix, minimum within Owns.** In `panel_properties`, wrap the Transform block in `ui.add_enabled_ui(!s.drawing, …)`.
- **Test.** Extend the test to cover both cases: `objsel` = {A} and `objsel` = ∅. Assert that `x/y/w/h` and `direct` agree, and that a `SetObjectBounds` mid-draft does not move A.

**P2-2: The UI tests cost about 10 s of CI per run.**
- **Cause.** `row_y` (`ui.rs:6229`) builds 120 fresh contexts × 5 frames for every lookup. It is called about 9 times (`open_editor` → `row_y`, and 3× in the Escape/blur test), which adds up to about 5,000 egui passes.
- **Fix.** Stop probing once the hit run ends (`take_while` after the first hit), and memoise per row set. Or lay the panel out once and read the row's rect: every row has the same height, so one hit gives the pitch. Target: under 2 s.

**P2-3: There are still two name fields for a path, and the choice between them lives in the app.** This is a recommendation; do not implement it in QW3.
- **Where the choice is made.** `apply_ops` (`ui.rs:5748-5752`) decides that a Path leaf renames through `RenamePath`. Core `RenameNode` on a Path leaf still silently writes `Node::name`, which nothing displays (`model.rs:287` even documents that), and it bumps `rev` even for an identical name.
- **Who will trip on it.** Any future caller that sends `RenameNode` for a row, such as S1 menu parity, a scripting or AI command, or an inspector rename, will reproduce Astra F10.
- **Saved files.** Files saved after an old buggy rename carry an invisible leaf name.
- **Recommendation: unify in core.**
  - `Editor::layer_rename(nid, …)` resolves `NodeKind::Path(pid)` itself and writes `Path::name`, with the same trim, empty and unchanged guards. Container nodes get the same no-op guard.
  - `RenamePath` then becomes internal or can be removed, and `apply_ops` goes back to a single `RenameNode` arm.
  - On load (`sync_tree`), move any non-empty leaf `Node::name` into `Path::name` when that is `None`. This recovers Astra's "lost" renames. Then clear it, so `Node::name` is containers-only.
- **Schema note.** Per ADR-0004, this migration is a schema-behaviour note and needs a line in the ADR log.

**P3-1:** An emptied field can never restore the auto-name `<Path>`.
- `Editor::rename_path`'s `empty → None` branch (`editor.rs:4384`) is now dead.
- The one-caller free function `command.rs:242 rename_path` duplicates it. Fold the guards into `Editor::rename_path` and drop the wrapper.
- The doc-comment claim "Illustrator keeps the old name when the field is emptied" is unverified. Ahmed should confirm the behaviour.

**P3-2:** `str::trim` does not strip bidi or format marks (U+200E/200F, U+061C ALM, U+200B, U+FEFF).
- A pasted name made only of these marks passes as non-empty and renders as a blank row. That matters for an Arabic-first product.
- Fix: treat the name as empty when every char is whitespace or a `Cf` format char. The check belongs in core, next to the existing guard.

**P3-3:** A zombie editor is possible.
- If the edited row disappears without an egui press (for example, a native macOS menu click on Delete), egui's dead-man switch drops focus but `rename` stays `Some`.
- An Undo that restores the same node id then re-opens the field and takes keyboard focus. The pattern was already there before QW3.
- Fix: after the row loop, `if !rename_shown { *rename = None }`.

**P3-4:** The same board name now has two inline editors with different rules.
- The Layers board row cancels on Escape and ignores an empty or unchanged name.
- The canvas board label (`ui.rs:5393-5398`) still commits on Escape and sends empty or unchanged names, which dirties the document.
- Extract one `inline_rename(ui, id, buf) -> Option<String>` helper from `ui.rs:4466-4497` and use it at both sites.

**P3-5:** Tests.
- The review asked for the core tests in `tests/edit_command.rs`. `tests/rename_path.rs` adds another link binary; move the 4 tests.
- `clone_op` maps every other op to a fake `Op::LayerGroup` (`ui.rs:6209`), which is misleading. Collect only `LayerRename`/`AbName` instead.
- `rename_field_keeps_focus_on_the_frame_it_opens` would also pass on the old code. It is a regression guard only.
- There is no test for the "Double-click to rename" hover hint, or for the Board-row (`AbName`) guard.

**P3-6: Existing issues, not caused by QW3.**
- `MENU_R = 4` lives in `ui.rs:3237`, not in `shell/tokens.rs`, and it is neither the 3 px nor the 8 px law value. The new menu inherits it.
- Escape on an open `menu_below` also reaches the canvas and deselects, because no widget is focused.
- Rename from a mirror row's second appearance opens the field on the first appearance (`rename_shown`).
