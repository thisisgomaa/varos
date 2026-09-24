> **Status:** reference — independent code review, 2026-09-24.

# Code review: QW5, Edit ▸ Select All / Deselect / Delete rows (`feat/qw5-edit-menu-rows` @ `93a4ac6`, base `e456093`)

**Verdict: APPROVE WITH NITS. No P1. One P2 (drop the unneeded `EditCommand::SelectAll` wrapper); the rest are P3.**
1. `select_all` correctly respects hidden/locked at both the path level and the ancestor chain (Layer and Group nodes), because it reuses `eff_hidden`/`eff_locked`, which already walk `Node::parent` to the root. Verified with a throwaway layer-nesting test (removed after the run, not committed).
2. The Direct-tool variant does not special-case clip-mask source paths, and neither does today's Direct marquee (`Drag::Marquee`, `editor.rs:3658`) — both filter only on `eff_hidden`/`eff_locked`. `select_all` matches existing behaviour exactly; this is not a new gap.
3. `MenuCmd::Plain(Backspace)` is wired correctly: the row's `accel` is `None`, so `mac_menu.rs`'s `fill()` never calls `muda_code(Backspace)` (which isn't even mapped) and AppKit gets no key equivalent. The `M::Plain` arm only fires `!gui.wants_keyboard()`, so a focused text field's own Backspace is untouched; `Delete`/`Backspace` both still hit `EditCommand::DeleteSelected` via the pre-existing `apply_key` arm at `main.rs:274`.
4. No other ⌘A/⇧⌘A binding exists. The raw-keyboard path already skips canvas shortcuts while `wants_keyboard()` is true (`main.rs:1273`, pre-existing, feeds `WindowEvent`s to egui separately via `state.on_window_event`), and the menu path explicitly hands ⌘A to egui through `forward_shortcut` (`ui.rs:910`) when a field is focused — confirmed with `egui_key(KeyCode::KeyA).is_some()`.
5. `select_all` "ending a Pen draft" is `active = None` only — identical to `escape()`. `active` is just the id of a path that Pen's `down()` has already pushed into `doc.paths` on the very first click (`pen.rs:69-72`); no separate draft buffer exists. So no geometry is discarded — the open path stays in the document (and becomes selectable, as the test proves). This matches Illustrator, which commits the open path.
6. `EditCommand::SelectAll` is a one-production-caller wrapper around `ed.select_all()` (`command.rs:173`) — I would drop it. See P2.

## Gates (measured by me in the worktree, `varos/`)
- `cargo test --workspace -j 4`: **383 passed, 0 failed** (39 result lines summed by hand). Matches the report.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean (verified after touching the 5 changed source files to force a real re-lint, not a cache hit).
- `cargo fmt --all -- --check`: clean.
- `cargo check -p varos-app --target x86_64-pc-windows-msvc -j 4` and `--target aarch64-apple-darwin -j 4`: both clean.
- Diff stat: 6 files, +329/−8, no `Cargo.toml` change (no new deps).

## Findings

**P2-1 — `EditCommand::SelectAll` is unneeded indirection and contradicts the plan's own rationale.**
- `Editor::execute` is a pure dispatch (`command.rs:210`: `command.apply(self)`), and `Self::SelectAll => ed.select_all()` (`command.rs:173`) adds nothing over calling `ed.select_all()` directly. Its only production call site is `main.rs:242` (`ed.execute(EditCommand::SelectAll)`); everywhere else it appears only in tests.
- The work order itself says the opposite: "Selection is editor state, not a document write, so there is no `EditCommand`. This matches `escape()`." (`QUICK_WINS_2026-09-24.md:68`). The commit is described as "the mirror of `escape`" (`editor.rs:3990`), yet in the exact same two-branch `if shift {...} else {...}` block (`main.rs:238-244`) Deselect calls `ed.escape()` directly while Select All goes through `EditCommand`. A reader hits that asymmetry immediately.
- Copy/Cut are wrapped in `EditCommand` too, but they are genuine document actions (Cut edits history); Deselect is the true sibling of Select All and stays unwrapped, so "consistency with Copy" doesn't justify the indirection here.
- **Fix:** delete the `SelectAll` variant and its `apply` arm; call `ed.select_all()` directly at `main.rs:242`, matching `ed.escape()` on the line above it. Update the two `select_all.rs` tests (`ed.execute(EditCommand::SelectAll)` → `ed.select_all()`) — they already assert `rev`/dirty are untouched, so the coverage doesn't change.

**P3-1 — Menu label.** Illustrator's equivalent row is *Edit ▸ Clear*, not *Delete* (`chrome.rs:263`). Cosmetic; the standing rule (no-accelerator, no key theft) is about the shortcut, not the label, so this is Ahmed's call.

**P3-2 — `cmd_a_selects_all_and_shift_cmd_a_deselects` partly re-tests an unrelated, pre-existing path.** Its last two lines (`main.rs:1923-1925`) assert plain `"KeyA"` still switches to the Direct tool and selects nothing — useful as a regression guard against the new `if !alt` ⌘-arm accidentally widening its match, but it isn't itself QW5 behaviour. Not vacuous, just worth noting it's guarding an adjacent path rather than the piece's own change.

**P3-3 — Not a QW5 regression, flagged for awareness only.** `select_all`'s group-widening (`editor.rs:4022`, `group_members`) can pull in an individually hidden/locked path that shares a *visible, unlocked* group with a pickable one — but `ObjMarquee` (`editor.rs:3708-3722`) has the identical property today, so this is existing product behaviour that QW5 correctly mirrors, not a new bug.
