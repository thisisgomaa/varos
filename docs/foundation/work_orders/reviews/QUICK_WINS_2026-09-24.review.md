> **Status:** reference — independent plan review, 2026-09-24.

# Review — `QUICK_WINS_2026-09-24.md`

**Verdict: APPROVE WITH CHANGES.**
1. QW1's core idea (reach = `EDGE_R/ppu` + painted half-width; distance to the true curve) is right and minimal: the renderer paints exactly "distance ≤ w/2" (round joins `tess.rs:183-238`, round caps).
2. But QW1 as written kills the corner rotate ring on every thick-stroked selection (P1-1). QW3's real root cause is in core, which the order forbids it to touch (P1-2).
3. QW5's ordering is already broken: S1-A is in flight, S1 has no "menu parity" item, and S1-C asserts "no ⌘A" (P1-3).
4. QW4 is sound. Half-disc caps pivoting on quad corners cannot reopen a seam. The only gap is that no test samples the seam vertex (P2-3).
5. The QW2 drop is right for the resolver and crash log, but Mac window memory is now orphaned (P2-5). QW6's new `ICON` token is not supported by its own contrast numbers (P2-4).

Evidence was read at the work-branch head `475a676` unless noted. In-flight worktrees were checked read-only: `fix/qw1-stroke-hit`, `fix/qw3-layer-rename`, `fix/qw4-round-caps`.

## Findings

### P1 — must change before execution (in-flight scope first)

**P1-1 · QW1: the rotate ring dies on a thick-stroked selection.**
- The rotate gate is `transform_hit` (`editor.rs:712`): `… && self.path_under(pos).is_none()`. The ring is ≤ `22/ppu` from a frame corner. The frame is the geometric bbox with no stroke (`obj_local_bbox`, `editor.rs:592`).
- On a rectangle the painted band covers the corner out to `sw/2`. With the new reach, `path_under` returns the selected object itself everywhere in the ring when `22/ppu ≤ sw/2 + 8/ppu`. For sw 80 that is **every zoom ≥ 35 %**. Rotating from the corner becomes impossible, not merely "gives way".
- The spec's intent is "only over empty space so **nearby objects** stay clickable" (`SNAP_TRANSFORM_SPEC.md:154`), not the selection's own band.
- The in-flight QW1 diff leaves this line unchanged (worktree `editor.rs:761`). No existing test catches it: `live_transform.rs:932` uses the default sw 2 (reach 9 < 14.1).
- **Change:** add `editor.rs` `transform_hit` to QW1's Owns. Replace `self.path_under(pos).is_none()` with `self.path_under(pos).is_none_or(|id| self.objsel.contains(&id))`. Add the test `thick_stroked_selected_rect_still_rotates_from_corner` (sw 80, ppu 3.27 and 1.0).
- In §7.1, replace "Near a thick stroke the rotate ring gives way to selection; if hand tests dislike it, remove the band from the rotate gate only." with: "The selection's own band never blocks its rotate ring; another object's band still does."

**P1-2 · QW3: the root cause is found, and it is in core.**
- A `<Path>` row shows `Path::name` (`ui.rs:394`, `:455`). Its rename commits `RenameNode` (`ui.rs:5704`), which writes the **leaf node's** name (`model.rs:1922`), and nothing displays that name.
- So the rename "works" but is invisible, while still creating an undo step and a dirty mark. That is Astra F10, not a focus bug.
- The QW3 agent already adds `EditCommand::RenamePath` in `varos-core/src/command.rs` (+8 lines, reusing the existing but uncalled `Editor::rename_path`, `editor.rs:4380`). This breaks "Must not touch: … core" but is the correct EditCommand-only fix.
- **Change:** ratify it. Amend QW3 Owns to add "`varos-core/src/command.rs` (one `RenamePath` variant + dispatch arm) and a core test in `tests/edit_command.rs`".
- Add these tests:
  - `rename_path_row_updates_the_displayed_name`
  - `rename_path_undo_restores_auto_name`
- Replace §2 "The root cause of Astra's miss is **not established**." with the finding above.

**P1-3 · QW5: the ordering claim is unachievable as written.**
- "QW5 lands **before S1 implementation starts**, otherwise fold QW5 into S1's 'menu parity'" does not hold:
  - S1-A is already running (worktree `feat/dfs-s1-a-workspace`).
  - `DFS_S1_LIFECYCLE_TABS.md` has no menu-parity item.
  - S1-C step 4 says "still no Export row and **no ⌘A**".
  - S1-D deletes `OpenDocContext` (whose `shortcut` QW5 step 4 dispatches through) and adds one `dispatch`.
- The real constraint is that QW5 must be on the base that S1-C and S1-D branch from, which is A's merge. QW2 is dropped, so `main.rs` is free now.
- **Change:** move QW5 to "start now, merge before S1-A merges". Edit S1-C step 4 to read "⌘A/⇧⌘A rows exist (QW5); still no Export row". Add to S1-D: "route `MenuCmd::Plain` through `dispatch`, keeping the `wants_keyboard` guard".
- If QW5 misses that window, it waits for S1-D. It is not "folded".

### P2 — should change

**P2-1 · QW1 step 1 wording allows an inadequate fix.**
- A polyline between the 25 samples is **not** the true curve. On the Astra W 20000 circle (r 10000, 4 cubic spans) the chord sagitta is ≈ r·(π/48)²/8 ≈ **5.35 units**. At 327 % the reach is 8/3.27 ≈ 2.45 units, so a centreline click can still miss.
- The in-flight agent already refines with Newton (`cubic_nearest`), which is good. The order should require it.
- **Change:** replace "measure the distance to the **polyline segments** between the 25 samples … instead of to the samples" with "project onto the chords between the 25 samples, then refine on the cubic (clamped Newton), so the distance is to the true curve".
- Pin `centreline_between_flatten_samples_is_hit` to **ppu 3.27 with a stroke-less or sw ≤ 1 path**. At ppu 1 it proves nothing.

**P2-2 · QW1 feel changes that the order omits.**
- `nearest_seg` also feeds object-geometry snapping (`editor.rs:2860`). Snaps now engage between samples.
- With the Direct tool, a band click becomes a path-level select plus whole-path move (`direct.rs:77-100`). Before, it was a marquee.
- `hover_path` (`editor.rs:3518`) and the ⌥-copy cursor (`main.rs:94`) now light up on the band.
- `tools/convert.rs:50` has the same world-unit bug as Pen (`d <= EDGE_R`, no `/ppu`).
- **Change:**
  - Add `convert.rs:50` to QW1 (one token).
  - Add these hand-test lines to §6: "A on a thick band selects the path and drags it"; "hover highlight appears on the band"; "V on a thick rectangle's corner still rotates".
  - Fix §4 QW1 step 4: "clicking the band away from the centre selects rather than inserts". For the **Pen** it draws or extends; only Direct selects.

**P2-3 · QW4: seam coverage is untested, and the rationale is off.**
- Both coverage helpers skip run endpoints: `assert_band_intact` loops `1..len-1` (`tess.rs:976`), and `measure` uses `windows(3)` (`tess_round3_tests.rs:88`). So the seam vertex of a closed ring has **never** been sampled.
- The coverage argument: for any turn ≤ 180° the end half-cap alone covers the outer wedge, because the wedge lies within ±90° of the last direction. So half-caps cannot open a seam, and the seam join is a vertex-count choice.
- "The old full discs were hiding that seam" is true only of their 24-gon facet error (≈ 13.7 px at r 1600).
- Five existing tests pin cap vertex counts and must change: `tess.rs:696` (168), `:712` (162), `:1009` (…+2·72), and round3 `:201` and `:229` (…+144…). The in-flight agent edits `tess_round3_tests.rs`, which is outside the listed Owns.
- **Change:**
  - Add `tess_round3_tests.rs` (count updates only) to Owns, and list those five tests as expected edits.
  - Require `closed_ring_seam_is_joined_not_capped` to sample the **seam vertex** (bisector + both normals, 25–95 %) on a **rectangle with anchor 0 at a corner, at 4000 %**, and on a smooth circle.
  - Add `zero_length_open_path_still_draws_a_round_dot`. The agent handles this case; pin it.

**P2-4 · QW6: the `ICON` token is not needed, and it splits the icon language.**
- The order's own numbers show MUTED icons already pass: 5.12:1 on PANEL and 4.68:1 on SURFACE, against WCAG 1.4.11's 3:1 for non-text. Astra F11 says "not a measured contrast-ratio failure". Only FAINT fails (3.26/2.98, verified).
- `tokens.rs:1` says the tokens are "transcribed VERBATIM from … `UI_VISION_MOCKUP.html`". S1-C says "no new colours". `UI_DIRECTION.md` rule 7 makes the ramp Ahmed's.
- Changing only `icon_btn`/`pf_btn` leaves the rail and toggle icons at MUTED (`ui.rs:1866`, `2376`, `2424`, `2507`, `4078`, `4880`), so resting icons end up in two greys.
- **Change:**
  - Delete QW6 step 1 and the two `icon_*` tests.
  - Keep step 4 (FAINT→MUTED where the text carries information, the only measured failure) and its guard test.
  - Make the 18 px glyph and the 10.5 px labels an Ahmed eye-check.
  - In §5, replace "Ship QW6; revert is a one-token change" (untrue: 14 call sites change size) with "Ship FAINT→MUTED only; a brighter icon grey is Ahmed's call, applied to every resting icon or none".

**P2-5 · QW2 drop: correct for paths, but window memory is orphaned.**
- S2/S3-A builds the one resolver (`VAROS_DATA_DIR` → Application Support → `%APPDATA%` → XDG) and turns the Mac crash log on (`DFS_S2_S3…md:44-45`, `:125-127`). A QW2 `app_paths.rs` would be a second resolver, so dropping it is right.
- But S2/S3-A says "**`win_state_path` stays `None` on macOS**" (`:45`; Q4 `:170`, "stay disabled"). The Mac window memory and the off-screen clamp now have no owner, yet §1 acceptance and §6 hand test 6 still promise them.
- Separately, `DFS_S4_S6…md:72` expects `app_paths::data_root() -> Result<…>`, while S2/S3 builds `storage::paths` / `AppLayout`. That is one resolver under two names.
- **Change:**
  - Remove "The Mac window reopens where it was" from §1 and hand test 6.
  - Add a follow-up "QW2-lite (after S2/S3-A): enable `window_state()` on macOS + `on_screen` clamp · sonnet · S" to §5 with default "yes".
  - Moderator: make S4/S6 use S2's name.

**P2-6 · Base commit.**
- The QW1 and QW4 worktrees sit on `bd24627`, before Astra batch 1. They lack 245 changed lines of `editor.rs` and the order itself, so the cited line numbers differ.
- The hit functions are untouched by batch 1, so conflicts are unlikely. The batch-1 tests (`direct_inspector.rs`, `clipboard.rs`, `boards.rs`) are stroke-less and unaffected.
- **Change (moderator):** merge both onto the work-branch head and run the full gates there before any review.

**P2-7 · Resources: QW7 duplicates S1-C.**
- S1-C already owns `build_topbar`, makes Export "disabled look, hover only" with a tooltip, and adds `menu_row_disabled`. The ⌘K pill honesty is the same 10-line pattern in the same function.
- **Change:** fold QW7 into S1-C, and drop the separate wave-3 agent and its rebase. QW8 stays after S1-D, since S1-D rewrites fit.

### P3 — nits
1. QW1 marquee: growing the rect by `hw` on every side is a square Minkowski sum. It over-catches near rect corners by up to 0.41·hw. That is acceptable; say so in a comment.
2. After the fix, `path_in_rect` test (b) is unreachable from the real gesture: a marquee cannot start inside a fill (`object.rs:12` starts a move instead). Keep it only for `occlusion.rs:86`.
3. Hover now costs 25 samples + ≤ 5 Newton steps per segment × every path on each pointer move, with no bbox reject (`path_under`). Log it under P11.3 "measure first"; don't optimise now.
4. QW5 Delete row: Illustrator's label is **Edit ▸ Clear**. The standing rule concerns shortcuts, so the label choice is minor.

## Needs Ahmed (additions; the order's list is otherwise honest)

| Item | Question | Recommended default |
|---|---|---|
| Marquee vs painted band | Astra asked for a marquee that "match[es] the visible target **or expose[s] a clear preference**". Illustrator has "Object Selection by Path Only". | The marquee touches the painted band (as QW1); no preference yet |
| Rotate near a thick stroke | Own band vs rotate ring (P1-1) | The own band never blocks rotate; hand-test it |
| Mac window memory | Now unowned (P2-5) | QW2-lite right after S2/S3-A |
| Resting icon grey | New `ICON` token, or keep MUTED (P2-4) | Keep MUTED; decide after seeing FAINT→MUTED |
| QW6 revert claim | "one-token change" is not true | Corrected in P2-4 |
