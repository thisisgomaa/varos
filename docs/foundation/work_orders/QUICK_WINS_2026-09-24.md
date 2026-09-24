> **Status:** current — work order (charter §3 level 4), quick-wins backlog derived from docs/audits/2026-09-24-ASTRA_USER_TEST.md, docs/PAINS_LOG.md and docs/foundation/STATUS.md; sequenced around docs/specs/DOCUMENT_FILE_SYSTEM.md (owner decisions D1–D3 recorded 2026-09-24).
> Amended 2026-09-24 after the independent plan review (see reviews/QUICK_WINS_2026-09-24.review.md); P1/P2 applied, "Needs Ahmed" items carry their default.

# Quick-wins backlog — Astra F08 / F10 / F11, open pains, Mac gaps

**Date:** 2026-09-24 · **Branch read:** `claude/sweet-cerf-1sg30t` (head `56516a9`) · **Planner:** Claude subagent. It read the code and wrote this backlog. It changed no code, built nothing and ran no tests.

## 1. Goal & acceptance

Close the small, well-bounded items that the Document & File System stages (S1–S6) do **not** absorb. Run them as parallel worktree pieces, like Astra batch 1.

- **Headless proof:** every piece adds GPU-free tests that fail before the change and pass after it. The gates are the same as batch 1: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Windows-target clippy (`x86_64-pc-windows-msvc`) and Mac-target clippy (`aarch64-apple-darwin`, type-check only in the cloud).
- **Ahmed's hand test (batch 3 list, §6):** thick strokes are clickable where they are painted. A marquee inside a hollow ring leaves the ring alone. Open-path round caps are smooth at 4000 %. Layer rows can be renamed through a visible route. Align and Pathfinder icons are readable at rest. The Edit menu shows Select All, Deselect and Delete. (Mac window memory is out of this batch — see QW2-lite in §5.)
- **Merge rule (STATUS):** a piece reaches `main` only after green gates, an independent Codex review and a hand test. Pieces merge `--no-ff` onto the work branch first.

## 2. Current-code findings (evidence)

### F08 — thick-stroke hit-testing (`varos-core`)
- `Editor::path_under` (`varos/crates/varos-core/src/editor.rs:479-495`) counts a click as an outline hit only when `edge_dist ≤ EDGE_R / ppu` (`EDGE_R = 8.0`, `editor.rs:15`). **It never adds `stroke_width / 2`.** With an 80-unit ring at 327 %, reach is 8 / 3.27 ≈ 2.4 units, while the painted band spans ±40 units. That is why the band click missed and the centreline click selected.
- `Document::nearest_seg` / `edge_dist` (`varos/crates/varos-core/src/model.rs:743-790`) measure the distance to **25 sample points** per cubic segment, not to the curve between them. On a quarter-circle of radius 400, the samples sit about 26 units apart, so a click on the true centreline between two samples can be about 13 units from the nearest sample, far more than 2.4. The centreline hit is therefore luck, and it gets worse on large shapes (Astra's W/H 20000 circle).
- `Editor::path_in_rect` (`editor.rs:672-690`) runs during the object marquee (`editor.rs:3708-3724`). It accepts a path when (a) an **outline sample vertex** is inside the rect, or (b) the rect centre is inside the path and `p.closed || fill`. Test (b) is the hollow-ring bug: a stroke-only **closed** ring is "caught" by any marquee inside it. Test (a) misses a thin marquee that crosses a long edge between two samples.
- Side bug found while tracing: Pen add-anchor (`varos/crates/varos-core/src/tools/pen.rs:54-55`) compares `d <= EDGE_R` in **world** units, with no `/ ppu`. Direct (`tools/direct.rs:83`) correctly uses `EDGE_R / ed.ppu`. So at high zoom the Pen adds anchors far from the path, and at low zoom it almost never does.

### F10 — layer / object rename is hard to find (`varos-app/src/ui.rs`)
- Inline rename exists: a double-click, or two clicks within 0.4 s, opens a `TextEdit` (`ui.rs:4446-4471`, `4522-4536`). It commits `Op::LayerRename` → `EditCommand::RenameNode` (`ui.rs:5704`). PAINS_LOG P4 (`docs/PAINS_LOG.md:37`, `:191`) has been waiting for a re-test since 2026-07-11, and Astra's double-click did not expose it.
- There is **no right-click menu** on Layers rows. Right-click falls through to `resp.clicked()` only. The only other rename entry point in the app is the Artboards panel ⋯ menu (`ui.rs:5395`). Nothing on the row hints that a double-click renames it.
- **Root cause found (approved for QW3): it is in core, not focus.** A `<Path>` row shows `Path::name` (`ui.rs:394`, `:455`). Its rename commits `RenameNode` (`ui.rs:5704`), which writes the **leaf node's** name (`model.rs:1922`) — and nothing displays that name. The rename "works" but is invisible, while still creating an undo step and a dirty mark. That is Astra F10, not a focus bug.
- PAINS_LOG FB6 follow-up nit (`docs/PAINS_LOG.md:119`) is still open. While the Pen is drafting with an old selection still active, the Properties dock shows the stale object. `Snap` takes `repr_path()` (`ui.rs:672`, `editor.rs:4266-4274`, which never looks at `ed.active`), and the dock header prints `s.name` (`ui.rs:4666-4672`). The bar is already correct (`ui.rs:3732-3735`, `3828`).

### F11 — small muted controls (`shell/tokens.rs`, `ui.rs`)
- Tokens (`varos/crates/varos-app/src/shell/tokens.rs:33-34`): `MUTED #8f8a86`, `FAINT #6e6a66`. Measured WCAG contrast: MUTED is 5.12:1 on PANEL `#1b1919` and 4.68 on SURFACE `#242121`. **FAINT is 3.26:1 on PANEL and 2.98 on SURFACE**, below the 4.5:1 small-text level.
- Align, distribute and bar icons are drawn by `icon_btn` (`ui.rs:1873-1887`): a 16 px glyph, MUTED at rest, white on hover, 14 call sites. The Pathfinder glyphs come from `pf_btn` (`ui.rs:4983+`, MUTED at rest). The section micro-labels are 10 px (`ui.rs:4892`, `4912`, `4934`, `4952`). The Pathfinder caption carries information but is FAINT at 10.5 px (`ui.rs:4956`).

### Astra "additional observations" not closed by batch 1
- **⌘K:** the top-bar Search pill is a visual mirror with no home (`ui.rs:3153`, `3162-3183`, `3359-3361`), yet it advertises "⌘ K". `apply_key` has no `KeyK` arm (`main.rs:195-273`), so ⌘K does nothing. The letters typed next are ordinary tool keys (`main.rs:242-252`). This is correct behaviour but a dishonest affordance. It lives in `build_topbar` (`ui.rs:3294`), so it comes **after S1**.
- **Floating control bar:** `board_ctlbar` is pivoted `CENTER_TOP` at `board.center().x, board.top() + 22` (`ui.rs:3675-3678`). Its width follows its content, so the edges move. That is PAINS_LOG P8 (`docs/PAINS_LOG.md:149`, 🟡 **Ahmed's choice**, (a) centred vs (b) fixed left edge). Separately, Fit (`fit_to_board`, `main.rs:344-350`) centres the page in the whole canvas hole (`gui.board_px`, `ui.rs:1458`) and ignores the two floating hands, so fitted art lands under the bar and the rail.

### PAINS_LOG — everything still open
| Item | Evidence | Disposition |
|---|---|---|
| P12 Pen click on a middle anchor deletes it **and opens** the path | `tools/pen.rs:38-41` calls the same `delete_anchor` as Delete | **Decision needed** (Ahmed), §5 |
| P13 open-end round caps are 24-gons | `varos/crates/varos-render-wgpu/src/tess.rs:120-141` (`stroke_cap`, fixed 24-step ring), called at `tess.rs:176-179`; joins already use a sagitta-bounded fan (`tess.rs:183-238`) | Piece QW4 |
| FB6 follow-up nit (stale dock while drawing) | above | Folded into QW3 |
| P4 double-click rename re-test | above | Folded into QW3 |
| P8 control-bar anchoring | above | Needs Ahmed, §5 |
| A3 "menus too long", which menu? | `PAINS_LOG.md:148` | Needs Ahmed (clarify) |
| P6 box-header redesign on `codex/p6-header` | `PAINS_LOG.md:61` | Needs Ahmed (hand test) |
| #2 clipped icons, fixed twice, "confirm visually" | `PAINS_LOG.md:35` | Needs Ahmed (eye) |
| A7 live-rotation final feel | `PAINS_LOG.md:64-65` (merged `138b512`; the row at `:84` is stale) | Needs Ahmed (hand test) |
| P5 single instance "not working at all" | `PAINS_LOG.md:76`, `:192` | Absorbed by S4 |
| Artboard name on canvas: double-click rename + size chip | `PAINS_LOG.md:75` (parked by Ahmed) | Needs Ahmed |
| "Codex colours" discussion; A16 full icons / A17 picker / A14 bar design | `PAINS_LOG.md:74`, `:83` | Needs Ahmed |
| P11.3 hover/snap cost, undo-clone storage | `PAINS_LOG.md:18`, STATUS "Earlier" | Measure first, not a quick win |
| Masks stages 3–6 (Astra could find no way to make a mask) | `PAINS_LOG.md:69-71`; no mask command in `command.rs:12-129` | Out of scope (feature) |
| Stale rows: `:84` A7 "decision", `:85` "T5/T9/T10/T11/A8a queued", `:196`/`:198`/`:199` P7/P9/P10 "confirm/small" (done in w1/`P10` merged) | as cited | Moderator doc hygiene, not a piece |

### STATUS "Known Mac gaps"
- **Window position/size not remembered:** `win_state_path` (`main.rs:446-448`) and `crash_log_path` (`main.rs:450-452`) read only `%APPDATA%`, which does not exist on macOS, so they return `None`. Save and restore (`main.rs:687-705`, `763`, `800-812`) are wired but never persist. Consequence: on Mac the **crash log is not written either**. The panic breadcrumb `target/panic.txt` (`main.rs:714`) is cwd-relative, which is `/` inside a bundle. The restore also never checks that the saved position is on an attached monitor.
- **Menu rows** (`chrome.rs:245-259`). A row may only mirror an existing path (`chrome.rs:6-7`, test `chrome.rs:482-489`).
  - **Delete:** the key path exists (`main.rs:265`, plain `Delete`/`Backspace` → `EditCommand::DeleteSelected`). However, `Accel` always means ⌘ (`chrome.rs:112-127`, `mac_menu.rs:42-50`), so this row needs a small no-⌘ variant. **Gets its row for free.**
  - **Deselect:** the plain `Escape` key path exists (`main.rs:264` → `Editor::escape`, `editor.rs:3980-3989`). A row on that key path is free. The Illustrator key ⇧⌘A is a one-line new arm.
  - **Select All:** **no path.** There is no `KeyA` under ⌘ in `apply_key` and no `select_all` in core, so it needs a small core method first.
  - **Duplicate:** **no path and no command.** ⌘D is Transform Again (`main.rs:230`), so it needs Ahmed (§5).
- Screen eyedropper disabled on Mac: needs screen-capture permission work, so it is not a quick win (§7).

## 3. Design (seams)

- **Core only:** hit geometry (QW1) and `Editor::select_all` (QW5). Selection is editor state, not a document write, so there is no `EditCommand`. This matches `escape()`.
- **Render only:** caps (QW4). **App only:** paths, menus, panels and tokens. No new dependencies. No GPU, window or `EventLoop` in any test. egui UI tests run a bare `egui::Context::run_ui` with a synthetic `RawInput`, as `ui_zoom_tests` does (`ui.rs:5820-5856`).

## 4. Pieces

Model rule: `opus` for hit-test or render geometry and for anything touching `editor.rs` interaction state; `sonnet` for mechanical app or UI work.

### Wave 1 — start now (no piece touches `build_topbar` or tabs; QW5 touches `main.rs`, in a small hunk, and must merge before S1-A merges)

**QW1 — Thick strokes hit where they are painted (Astra F08)** · `opus` · **M** · risk: medium (hover, click and marquee all share this code) · S1 conflict: none
- **Owns:** `varos-core/src/editor.rs` (`path_under`, `path_in_rect`, and `transform_hit` — see step 2a), `varos-core/src/model.rs` (`nearest_seg`, `edge_dist`, plus one private helper), `varos-core/src/tools/pen.rs:55`, `varos-core/src/tools/convert.rs:50` (same world-unit bug as Pen), `varos-core/tests/occlusion.rs` (extend), `varos-core/tests/live_transform.rs` (extend), new `varos-core/tests/hit_thick_stroke.rs`. **Must not touch:** `scene.rs`, `varos-app`, `varos-render-wgpu`.
- **Steps:**
  1. In `nearest_seg` and `edge_dist`, project onto the **chords** between the 25 samples, then refine on the cubic (clamped Newton), so the distance is to the true curve, not the polyline — a polyline alone still misses on a large arc (e.g. the Astra W 20000 circle's chord sagitta is ≈5.35 units at r 10000, bigger than the reach at typical zoom). Keep the signature `(seg index, t, d)` and interpolate `t` from the refined point, so Pen add-anchor still gets a valid `t`.
  2. In `path_under`: `reach = EDGE_R / ppu + if p.stroke.solid().is_some() { p.stroke_width * 0.5 } else { 0.0 }`. The unit transform is rotation only (`model.rs:29-34`), so the width is not scaled. Keep the top→down occlusion walk (A31), so a thick band on top now correctly occludes what lies beneath it.
  2a. **The selection's own band must not block its rotate ring.** `transform_hit` (`editor.rs:712`) gates the corner ring on `self.path_under(pos).is_none()`. With the new reach, a selected object's own painted band now covers its corner ring at ordinary zooms (e.g. sw 80 at any zoom ≥ 35%), so rotating from the corner would become impossible, not merely "give way". Change that guard to `self.path_under(pos).is_none_or(|id| self.objsel.contains(&id))`: the selection's own band never blocks its rotate ring; another object's band still does (matches `SNAP_TRANSFORM_SPEC.md:154`'s intent — nearby *other* objects stay clickable).
  3. In `path_in_rect`: (a) replace "a sample vertex is inside the rect" with "a polyline segment intersects the rect **expanded by the painted half-width**" (Liang–Barsky or a clip test); (b) keep the centre-inside fallback only for **filled** paths (`p.fill.solid().is_some()`, dropping `p.closed ||`).
  4. In `pen.rs:55` and `convert.rs:50`, change `d <= EDGE_R` to `d <= EDGE_R / ed.ppu`. Add-anchor keeps using the **centreline** distance: for the **Pen** a band click away from the centre draws or extends rather than inserts; the **Direct** tool (`direct.rs:83`, already `/ppu`) keeps centreline gating too, so a band click there becomes a path-level select **and drags the whole path** (`direct.rs:77-100`), not a marquee.
- **Tests (new):**
  - `painted_band_click_selects_away_from_centreline`: ring r = 400, sw = 80, ppu = 3.27. A click at r+35 returns Some; r+45 returns None.
  - `centreline_between_flatten_samples_is_hit`: W 20000 circle at ppu 3.27, stroke-less (or sw ≤ 1), click on the true arc midway between samples (a plain polyline reach at ppu 1 would prove nothing here — see step 1).
  - `hollow_ring_marquee_inside_does_not_catch_ring`: a marquee overlapping the band is caught; one inside the inner edge is not.
  - `thin_marquee_crossing_long_edge_selects`: 1000-wide rect, 4-wide marquee across its top edge between samples.
  - `fill_only_shape_reach_unchanged`: no stroke means reach stays `EDGE_R / ppu`.
  - `thick_top_band_occludes_lower_path`.
  - `pen_add_anchor_tolerance_is_screen_constant`: same screen distance at ppu 0.25 and 40.
  - `thick_stroked_selected_rect_still_rotates_from_corner` (sw 80, ppu 3.27 and 1.0 — pins step 2a).
  - All existing `occlusion.rs`, `layers.rs`, `live_transform.rs` and `input_feel.rs` tests must pass unchanged. Report any that needed editing, with the reason.
- **Also feeds:** `nearest_seg` also drives object-geometry snapping (`editor.rs:2860`), `hover_path` (`editor.rs:3518`) and the ⌥-copy cursor (`main.rs:94`) — all now light up on the painted band too, which is intended. Hover's added cost (25 samples + ≤5 Newton steps per segment, per path, per pointer move, no bbox reject) is logged under PAINS_LOG P11.3 "measure first", not optimised here.

**QW2 — dropped.** S2/S3-A already builds the one app-data resolver (`storage::paths::{data_root, AppLayout}`) and turns the Mac crash log on; a QW2 `app_paths.rs` would be a second resolver under a different name. Mac window memory (`win_state_path`, the off-screen clamp) has no owner in S2/S3-A (it stays `None` on macOS there), so it becomes **QW2-lite**, a small follow-up piece after S2/S3-A merges — see §5.

**QW5 — Edit ▸ Select All / Deselect / Delete rows (Known Mac gap)** · `opus` (touches `editor.rs` selection state) · **S/M** · risk: medium (a plain-key menu row must not steal Backspace from text fields) · **moved here from Wave 2 (P1-3): starts now, merges before S1-A merges** — S1-A is already in flight, `DFS_S1_LIFECYCLE_TABS.md` has no "menu parity" item to fold into, S1-C step 4 says "no ⌘A", and S1-D deletes the `OpenDocContext`/`shortcut` this piece would otherwise dispatch through. QW2 is dropped, so `main.rs` is free now. If QW5 misses that window, it waits for S1-D instead — it is not "folded" into anything.
- **Owns:** `varos-core/src/editor.rs` (new `pub fn select_all(&mut self)` next to `escape`), new `varos-core/tests/select_all.rs`, `varos-app/src/chrome.rs` (Edit menu, `MenuCmd`, `egui_key`, tests), `varos-app/src/mac_menu.rs` (`muda_code` `KeyA`, and no key equivalent for the plain row), `varos-app/src/main.rs` (`apply_key` ⌘ arm + the macOS `M::…` match arm). **Must not touch:** `ui.rs`, the `hit`/marquee code.
- **Steps:**
  1. `select_all` behaviour:
     - Object and other tools: `objsel` = every path that is not `eff_hidden` and not `eff_locked`; clear anchors; `refresh_obj_angle()`.
     - Direct tool: `selected` = all anchors of those paths.
     - Artboard tool: `absel` = all boards.
     - Pen mid-draft: end the draft first (`active = None`).
  2. In `apply_key` ⌘ arm: `"KeyA" => if shift { ed.escape() } else { ed.select_all() }`. In text fields, egui already receives ⌘A (select text) because `wants_keyboard()` short-circuits (`main.rs:1246`), and `forward_shortcut` passes ⌘A as a key event (`ui.rs:910-927`).
  3. Menu rows: `key("edit.selectall", "Select All", cmd(K::KeyA))` and `key("edit.deselect", "Deselect", cmd_shift(K::KeyA))`.
  4. Delete row: add `MenuCmd::Plain(KeyCode)`, shown with **no accelerator** so AppKit never intercepts Backspace. When `!gui.wants_keyboard()` it dispatches through `shortcut(code, false, false, false)`; otherwise it does nothing.
  5. Drop `KeyA` from the "missing" list in the chrome test (`chrome.rs:484`).
- **Tests:**
  - core: `select_all_takes_visible_unlocked_paths_only`, `select_all_in_direct_selects_anchors`, `select_all_in_artboard_tool_selects_boards`, `select_all_ends_pen_draft`, `select_all_is_not_an_edit` (`rev` unchanged).
  - app: `cmd_a_selects_all_and_shift_cmd_a_deselects` (via `apply_key`), `edit_menu_mirrors_select_all_deselect_delete`, `plain_delete_row_has_no_native_key_equivalent`, `delete_row_runs_delete_selected`. The existing `every_shortcut_item_shows_exactly_the_keystroke_it_sends`, `ids_and_accelerators_are_unique` and `every_menu_key_can_be_handed_to_a_text_field` stay green.

**QW3 — Rename you can find + honest dock while drawing (Astra F10, P4, FB6 nit)** · `opus` (core fix + UI) · **S/M** · risk: low · S1 conflict: `ui.rs` rebase only (Layers/Properties regions, not topbar/tabs)
- **Owns:** `varos-app/src/ui.rs`: `panel_layers` (`4150-4600`), `Snap` construction (`640-700`), `panel_props` header (`4666-4672`), plus a new `#[cfg(test)] mod layer_rename_tests`; **also `varos-core/src/command.rs`** (one `RenamePath` variant + dispatch arm, reusing the existing but uncalled `Editor::rename_path`, `editor.rs:4380`) and a core test in `varos-core/tests/edit_command.rs` — approved as an exception to "Must not touch core", because the root cause is that `RenameNode` writes the leaf node's name while the row displays `Path::name`, and only a core command can fix that. **Must not touch:** `build_topbar`, `board_ctlbar`, tokens, `main.rs`.
- **Steps:**
  1. Wire the Layers row rename to `EditCommand::RenamePath` instead of `RenameNode`, so the commit updates the name the row actually displays.
  2. Reproduce and harden the focus path: drive `panel_layers` in a bare `egui::Context`. `LayerIcons` holds only `Option` textures (`ui.rs:843-851`), so pass `None`s. Send a synthetic double-click on a `<Path>` row's name, then run the next frame, then type and press Enter. If the editor does not open or keep focus, fix the cause, likely an unstable `TextEdit` id or a same-frame `lost_focus`. Give the field an explicit `id_salt(("lay-rename", row.id))` and skip the commit on the frame the editor opens.
  3. Make rename discoverable in its **one home** (the Layers panel): right-clicking a row opens a small `menu_below`-style menu with **Rename** (same `*rename = Some(..)` path) and nothing else yet. Add the hover hint "Double-click to rename" on the name cell.
  4. FB6 nit: when `drawing`, `Snap` takes its paint and name from `ed.active` (the in-progress path, name "Drawing path…") instead of `repr_path()`. The dock header is then MUTED, not TEXT.
- **Tests:**
  - `rename_path_row_updates_the_displayed_name` (core, `edit_command.rs`)
  - `rename_path_undo_restores_auto_name` (core, `edit_command.rs`)
  - `double_click_opens_rename_and_enter_commits`
  - `rename_field_keeps_focus_on_the_frame_it_opens`
  - `right_click_rename_opens_the_same_editor`
  - `escape_or_blur_with_unchanged_name_is_harmless`: no `RenamePath`, or a no-op one. Check `RenamePath` with the same name leaves `rev` alone. If it does not, record the finding and leave the behaviour unchanged.
  - `drawing_snap_reports_active_path_not_stale_selection`: `Snap` built from an `Editor` with `objsel` = {A} and `active` = B.

### Wave 2 — after wave 1 merges (QW5 has moved to Wave 1, see above)

**QW4 — Smooth round caps (PAINS_LOG P13)** · `opus` (render geometry next to three rounds of join fixes) · **S** · risk: low–medium · S1 conflict: none
- **Owns:** `varos-render-wgpu/src/tess.rs` (`stroke_cap`, the cap calls in `stroke_poly`, and its tests module) and `varos-render-wgpu/src/tess_round3_tests.rs` (vertex-count updates only — the cap change shifts fixed counts at `tess.rs:696` (168), `:712` (162), `:1009` (…+2·72), and round3 `:201`/`:229` (…+144…); these five edits are expected, not a scope breach). **Must not touch:** `lib.rs` draw routing, `scene.rs`.
- **Steps:**
  1. Replace the 24-gon full disc with a **half-disc cap built as a 180° `stroke_join`**. `incoming` is the end quad's frame; `outgoing` is the same frame reversed (`dir = -dir`, `normal = -n`, corners swapped). This reuses the existing sagitta ≤ 0.25 px budget (max 128 steps) and pivots on the quad's own corners, so there is no T-junction crack. Coverage rationale: for any turn ≤ 180° the end half-cap alone covers the outer wedge, because the wedge lies within ±90° of the last direction — so half-caps cannot open a seam by themselves; a seam still needs its own join because the old full discs were only hiding the seam **gap**, not proving coverage (their gap was a 24-gon facet error, ≈13.7 px at r 1600, not a true opening).
  2. **Verify first** how closed outlines arrive (`scene.rs:509` passes `geom.outline`). If a closed ring is delivered with `pts[0] == pts[last]`, emit the missing **seam join** (last segment → first segment) instead of two caps.
  3. Keep `width < 1.6` behaviour (no joins, no caps).
  4. Handle the zero-length open path (a single point): still draws a round dot.
- **Tests:**
  - `round_cap_hugs_true_circle_at_huge_radius`: r = 1600, every chord within `JOIN_TOL_PX`.
  - `cap_shares_the_end_quad_corners_bit_exact`
  - `closed_ring_seam_is_joined_not_capped`: samples the **seam vertex itself** (bisector + both normals, 25–95%) — both existing coverage helpers skip run endpoints (`assert_band_intact` loops `1..len-1`; `measure` uses `windows(3)`), so the seam vertex has never actually been sampled before this test. Cover a rectangle with anchor 0 at a corner at 4000%, and a smooth circle.
  - `cap_step_count_is_bounded`: ≤ 128; a thin stroke gets ≥ 4 steps.
  - `thin_strokes_unchanged`
  - `zero_length_open_path_still_draws_a_round_dot`.
  - Report harness D/E vertex counts before and after, as the join fix did (STATUS `b37f78b` entry).

**QW6 — Readable small controls, same constitution (Astra F11)** · `sonnet` · **S** · risk: low (easy to revert; the visual verdict is Ahmed's) · S1 conflict: `ui.rs` rebase only
- **Owns:** `varos-app/src/shell/tokens.rs`; `ui.rs` `icon_btn` (`1873-1887`), `pf_btn` (`4983+`), the Align/Pathfinder micro-labels (`4892-4956`) and the informational FAINT labels listed at `2003`, `2260`, `4956`; a new `#[cfg(test)] mod token_contrast_tests` in `tokens.rs`. **Must not touch:** ACCENT use, layout sizes of boxes/bars, the rail, `build_topbar`.
- **Steps:** (no new `ICON` token: the order's own numbers show MUTED icons already pass WCAG 1.4.11's 3:1 for non-text — 5.12:1 on PANEL, 4.68:1 on SURFACE, per §2 F11 — and `tokens.rs:1` says the palette is transcribed verbatim from the mockup, so a second icon grey is not backed by a measured failure and would leave the rail/toggle icons at MUTED while `icon_btn`/`pf_btn` moved, splitting the icon language. Only FAINT fails, at 3.26/2.98.)
  1. Draw the `icon_btn` glyph at 18 px inside the unchanged 26×24 chip.
  2. Raise the section micro-labels from 10 px to 10.5 px.
  3. Move FAINT to MUTED **only** where the text carries information (the three sites above). FAINT stays for placeholders and disabled text. **Do not change FAINT's value.**
  4. No shadows, no animation, no new radii.
- **Tests:** `muted_and_text_unchanged` (guards against accidental ramp edits).

### Wave 3 — **after S1**

**QW7 — folded into S1-C.** S1-C already owns `build_topbar`, already gives Export a "disabled look, hover only" with a tooltip, and already adds `menu_row_disabled` (S1 WO §3.6/S1-C) — the Search pill's "⌘K honesty" fix (remove the keycap that promises a path that does not exist; disabled look, FAINT, hover text "Search isn't available yet"; do not bind ⌘K) is the same 10-line pattern in the same function, so S1-C does it instead of a separate wave-3 agent and rebase. S1-C's Share-button disabled treatment (added in the S1 amendment, §3.6) already covers the other half of this observation. See `docs/foundation/work_orders/DFS_S1_LIFECYCLE_TABS.md` §3.6.
- **Test (added to S1-C's list):** `search_pill_advertises_no_shortcut`, which asserts `layout_no_wrap` text excludes the key label; the existing `mac_topbar_controls_share_the_native_traffic_light_centre` stays green.

**QW8 — Fit keeps the art clear of the floating hands (Astra control-bar observation, placement part)** · `sonnet` · **S** · risk: low · **after S1** (`fit_to_board` moves with the session/view work)
- **Owns:** `ui.rs` `board_rail` and `board_ctlbar` (they record their shown `Area` rects), plus a new `pub fit_px: Option<egui::Rect>` on `Ui` next to `board_px` (`ui.rs:839`, `1458`); `main.rs` `fit_to_board` (`344-350`, one line: prefer `gui.fit_px`).
- **Steps:** `fit_px` = `board_px` minus the bottom of the control bar + 8 px at the top and minus the right edge of the rail + 8 px at the left, each only when that hand is shown. ⌘1, ⌘± and the paste centre keep `board_px` (their behaviour is unchanged). **Does not** change the bar's anchoring (P8 stays Ahmed's choice).
- **Tests:** `fit_rect_excludes_shown_hands` (pure helper `fit_area(board, rail: Option<Rect>, bar: Option<Rect>)`), `hidden_hands_leave_fit_unchanged`.

## 5. Needs Ahmed (not pieces; each has a recommended default so work never blocks)

| Item | Question | Recommended default |
|---|---|---|
| **P12** Pen on a middle anchor | Keep "delete and open" or match Illustrator (Pen removes the anchor and **joins** its neighbours; Direct + Delete opens) | Illustrator: add a `remove_anchor_join` for the Pen only; Delete keeps A32 |
| **P8** control-bar anchoring (+ Astra "width changes") | (a) centred, grows from the middle; (b) fixed left edge, Illustrator-style | (b), left edge at a fixed inset from the rail |
| **Duplicate** menu row | No path exists; ⌘D is Transform Again | Add nothing now. Later option: Edit ▸ Paste in Front ⌘F / Paste in Back ⌘B on the in-app clipboard |
| QW6 look | Are the 18 px glyphs and 10.5 px labels right? (no new ICON token — see F11 below) | Ship QW6 as specified; "revert is a one-token change" was not true — 14 call sites change size, so a revert is a small multi-site change, not one token |
| A3, #2 icons, P6 branch, A7 feel, artboard canvas-name chip, A16/A17/A14 design, "Codex colours" | eye / clarification (see §2 table) | none; waits on his hand test |
| Batch-1 open question: is activating a board an unsaved change? | `GATE_LOG.md` Astra batch 1 | S1 decides (view vs content dirty) |
| Mac screen eyedropper | Worth the macOS screen-recording permission prompt? | Defer |
| **QW2-lite** (new follow-up piece) | Enable `window_state()` on macOS + the `on_screen` clamp, now that S2/S3-A owns the resolver | Yes — schedule right after S2/S3-A merges · `sonnet` · **S** |

## 6. Merge order, conflict hot-spots, hand-test list

- **Order:** wave 1 (QW1, QW5, QW3 in parallel; QW5 merges before S1-A merges — QW2 is dropped, so `main.rs` is free) → wave 2 (QW4, QW6 in parallel) → S1 → wave 3 (QW8; QW7 is folded into S1-C, so nothing separate lands here). The moderator merges each piece `--no-ff` onto the work branch, runs the gates on the merged head, and records the result in `GATE_LOG.md`. QW1 and QW4 sit on the pre-batch-1 commit `bd24627` in their worktrees; the moderator merges both onto the current work-branch head and runs the full gates there before any review, rather than reviewing them against the stale base.
- **Hot-spots:**
  - `main.rs`: QW5 (`apply_key`, menu match; before S1-A), QW8 (`fit_to_board`, after S1) and S1 (everything). Never run two of these at once.
  - `ui.rs`: QW3 (Layers/Snap/props), QW6 (`icon_btn`/`pf_btn`/labels), QW8 (topbar/hands), S1 (topbar/tabs, plus QW7's search-pill honesty). The regions are disjoint, but it is the same file, so rebase before merging.
  - `editor.rs`: QW1 (hit functions, including `transform_hit`) and QW5 (`select_all`) both land in wave 1; disjoint functions, rebase before merging.
  - `command.rs`: QW3 (`RenamePath`), separate from any other piece's scope.
- **For Ahmed to try (batch 3):**
  1. Click the painted edge of an 80-wide ring at ~300 %: it selects. Marquee inside the ring only: the ring stays unselected. On a thick rectangle, Direct (A) on the band selects the path and drags it; hover shows a highlight on the band; V (rotate) on a thick rectangle's corner still rotates.
  2. An open Pen path, width 80, at 4000 %: the end caps are round and smooth.
  3. Layers: double-click a `<Path>` name, or right-click → Rename, then type and press Enter.
  4. Pen drawing with an old selection: the dock says "Drawing path…".
  5. The Align and Pathfinder icons are readable without hovering.
  6. Edit ▸ Select All ⌘A, Deselect ⇧⌘A, Delete. Backspace in a text field still deletes a character.
  7. (after S1) The Search pill promises no ⌘K; Fit keeps the page clear of the bar and the rail. (Mac window memory is QW2-lite, a later follow-up after S2/S3-A — not in this batch.)

## 7. Risks / open questions (with defaults)

1. **QW1 changes feel everywhere** (hover highlight, click, marquee, the rotate-ring gate at `editor.rs:712`). Default: ship the painted-band reach. The selection's own band never blocks its rotate ring; another object's band still does (step 2a).
2. **QW4 closed-ring seam:** if closed outlines are not delivered with a repeated first point, the seam handling is a no-op. Default: the agent proves the delivery shape with a test before choosing.
3. **QW5 plain-key menu row on macOS:** a no-modifier key equivalent could hijack Backspace while typing. Default: no key equivalent is shown or bound; the row is click-only and runs the same key path.
4. **QW3 root cause found:** rename committed `RenameNode` (leaf node name) while the row displays `Path::name`, so the rename was invisible. Default: ship the `RenamePath` fix plus the discoverability part (context menu + hint); the reproduction test still decides whether a focus fix is also needed.
5. **QW2 is dropped, not merged with S2.** S2/S3-A already builds the one resolver (`storage::paths::{data_root, AppLayout}`) and turns the Mac crash log on, but keeps `win_state_path` `None` on macOS. Default: Mac window memory + the on-screen clamp become **QW2-lite**, a small follow-up piece scheduled right after S2/S3-A merges (§5).

## 8. Out of scope

- F01/F02/F03/F09 and New/Export menu rows: S1–S6.
- OS clipboard.
- Masks authoring (stages 3–6).
- Stroke caps/joins/dashes as user options (A19); caps stay round.
- P11.3 and undo-storage performance.
- Mac screen eyedropper.
- Accessibility tree for canvas controls.
- Any Windows-specific work.
- Icon-library redesign.
- Rewriting the PAINS_LOG stale rows: moderator doc hygiene after the merges, not a piece.

## Needs Ahmed (from review)
1. **Marquee vs painted band.** Astra asked for a marquee that "match[es] the visible target or exposes a clear preference"; Illustrator has "Object Selection by Path Only". **Working assumption: the marquee touches the painted band (as QW1 ships it); no preference toggle yet.**
2. **Rotate near a thick stroke: own band vs the rotate ring.** **Working assumption: the selection's own band never blocks its own rotate ring (QW1 step 2a); hand-test it (§6 item 1).**
3. **Mac window memory is now unowned** by S2/S3-A. **Working assumption: QW2-lite, scheduled right after S2/S3-A merges (§5).**
4. **Resting icon grey: a new token, or keep MUTED?** **Working assumption: keep MUTED — no new `ICON` token (QW6 step list above); revisit only after seeing the FAINT→MUTED change in the real window.**
