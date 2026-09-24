> **Status:** current — work order (charter §3 level 4), quick-wins backlog derived from docs/audits/2026-09-24-ASTRA_USER_TEST.md, docs/PAINS_LOG.md and docs/foundation/STATUS.md; sequenced around docs/specs/DOCUMENT_FILE_SYSTEM.md (owner decisions D1–D3 recorded 2026-09-24).

# Quick-wins backlog — Astra F08 / F10 / F11, open pains, Mac gaps

**Date:** 2026-09-24 · **Branch read:** `claude/sweet-cerf-1sg30t` (head `56516a9`) · **Planner:** Claude subagent. It read the code and wrote this backlog. It changed no code, built nothing and ran no tests.

## 1. Goal & acceptance

Close the small, well-bounded items that the Document & File System stages (S1–S6) do **not** absorb. Run them as parallel worktree pieces, like Astra batch 1.

- **Headless proof:** every piece adds GPU-free tests that fail before the change and pass after it. The gates are the same as batch 1: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, Windows-target clippy (`x86_64-pc-windows-msvc`) and Mac-target clippy (`aarch64-apple-darwin`, type-check only in the cloud).
- **Ahmed's hand test (batch 3 list, §6):** thick strokes are clickable where they are painted. A marquee inside a hollow ring leaves the ring alone. Open-path round caps are smooth at 4000 %. Layer rows can be renamed through a visible route. Align and Pathfinder icons are readable at rest. The Mac window reopens where it was. The Edit menu shows Select All, Deselect and Delete.
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
- The root cause of Astra's miss is **not established**. Possible causes: the commit happens in the same frame as the one that opens the editor (`te.lost_focus()` right after `request_focus()` on a freshly allocated `ui.put` id), or the synthetic double-click timing. It needs a headless egui reproduction before any fix is claimed.
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

### Wave 1 — start now (no piece touches `build_topbar` or tabs; only QW2 touches `main.rs`, in a small hunk)

**QW1 — Thick strokes hit where they are painted (Astra F08)** · `opus` · **M** · risk: medium (hover, click and marquee all share this code) · S1 conflict: none
- **Owns:** `varos-core/src/editor.rs` (only `path_under`, `path_in_rect`), `varos-core/src/model.rs` (`nearest_seg`, `edge_dist`, plus one private helper), `varos-core/src/tools/pen.rs:55`, `varos-core/tests/occlusion.rs` (extend), new `varos-core/tests/hit_thick_stroke.rs`. **Must not touch:** `scene.rs`, `varos-app`, `varos-render-wgpu`.
- **Steps:**
  1. In `nearest_seg` and `edge_dist`, measure the distance to the **polyline segments** between the 25 samples (point-to-segment projection) instead of to the samples. Keep the signature `(seg index, t, d)` and interpolate `t` along the projected sub-segment, so Pen add-anchor still gets a valid `t`.
  2. In `path_under`: `reach = EDGE_R / ppu + if p.stroke.solid().is_some() { p.stroke_width * 0.5 } else { 0.0 }`. The unit transform is rotation only (`model.rs:29-34`), so the width is not scaled. Keep the top→down occlusion walk (A31), so a thick band on top now correctly occludes what lies beneath it.
  3. In `path_in_rect`: (a) replace "a sample vertex is inside the rect" with "a polyline segment intersects the rect **expanded by the painted half-width**" (Liang–Barsky or a clip test); (b) keep the centre-inside fallback only for **filled** paths (`p.fill.solid().is_some()`, dropping `p.closed ||`).
  4. In `pen.rs:55`, change `d <= EDGE_R` to `d <= EDGE_R / ed.ppu`. Add-anchor keeps using the **centreline** distance, so clicking the band away from the centre selects rather than inserts. Direct reshape (`direct.rs:83`) keeps centreline gating too, so a band click there becomes a path-level selection.
- **Tests (new):**
  - `painted_band_click_selects_away_from_centreline`: ring r = 400, sw = 80, ppu = 3.27. A click at r+35 returns Some; r+45 returns None.
  - `centreline_between_flatten_samples_is_hit`: W 20000 circle, click on the true arc midway between samples.
  - `hollow_ring_marquee_inside_does_not_catch_ring`: a marquee overlapping the band is caught; one inside the inner edge is not.
  - `thin_marquee_crossing_long_edge_selects`: 1000-wide rect, 4-wide marquee across its top edge between samples.
  - `fill_only_shape_reach_unchanged`: no stroke means reach stays `EDGE_R / ppu`.
  - `thick_top_band_occludes_lower_path`.
  - `pen_add_anchor_tolerance_is_screen_constant`: same screen distance at ppu 0.25 and 40.
  - All existing `occlusion.rs`, `layers.rs`, `live_transform.rs` and `input_feel.rs` tests must pass unchanged. Report any that needed editing, with the reason.

**QW2 — Mac remembers the window, crash log on Mac (Known Mac gap)** · `sonnet` · **S** · risk: low · S1 conflict: small `main.rs` hunk; S2's "one app data-path resolver" should **adopt** this helper, not duplicate it
- **Owns:** new `varos-app/src/app_paths.rs` (declared as `mod app_paths;` in `main.rs`), `main.rs:446-452` (both path fns call the helper) and `main.rs:800-812` (restore clamp). **Must not touch:** `ui.rs`, `chrome.rs`, the event-loop arms beyond those lines.
- **Steps:**
  1. `pub fn data_dir_from(appdata: Option<OsString>, home: Option<OsString>, macos: bool) -> Option<PathBuf>` (pure, so it is testable): Windows → `%APPDATA%\Varos`; macOS → `$HOME/Library/Application Support/Varos`; other platforms → `None` (unchanged behaviour). Add `pub fn data_dir()`, which reads the env.
  2. `window.txt` and `crash.txt` live under it.
  3. `pub fn on_screen(pos, size, monitors: &[(i32, i32, u32, u32)]) -> bool`: at least a 64×64 px corner of the title strip must lie on some monitor. Otherwise fall back to the existing first-run centring (`main.rs:804-811`).
  4. Keep the `target/panic.txt` breadcrumb as it is (dev-only).
- **Tests:** `mac_data_dir_is_application_support`, `windows_data_dir_is_appdata`, `missing_env_gives_none`, `offscreen_saved_position_is_rejected`, `partly_visible_position_is_kept`.

**QW3 — Rename you can find + honest dock while drawing (Astra F10, P4, FB6 nit)** · `opus` (root cause unknown) · **S/M** · risk: low · S1 conflict: `ui.rs` rebase only (Layers/Properties regions, not topbar/tabs)
- **Owns:** `varos-app/src/ui.rs`: `panel_layers` (`4150-4600`), `Snap` construction (`640-700`), `panel_props` header (`4666-4672`), plus a new `#[cfg(test)] mod layer_rename_tests`. **Must not touch:** `build_topbar`, `board_ctlbar`, tokens, `main.rs`, core.
- **Steps:**
  1. **Reproduce first.** Drive `panel_layers` in a bare `egui::Context`. `LayerIcons` holds only `Option` textures (`ui.rs:843-851`), so pass `None`s. Send a synthetic double-click on a `<Path>` row's name, then run the next frame, then type and press Enter. Record whether the editor opens, keeps focus and emits `Op::LayerRename`. If it fails, fix the cause, likely an unstable `TextEdit` id or a same-frame `lost_focus`. Give the field an explicit `id_salt(("lay-rename", row.id))` and skip the commit on the frame the editor opens.
  2. Make rename discoverable in its **one home** (the Layers panel): right-clicking a row opens a small `menu_below`-style menu with **Rename** (same `*rename = Some(..)` path) and nothing else yet. Add the hover hint "Double-click to rename" on the name cell.
  3. FB6 nit: when `drawing`, `Snap` takes its paint and name from `ed.active` (the in-progress path, name "Drawing path…") instead of `repr_path()`. The dock header is then MUTED, not TEXT.
- **Tests:**
  - `double_click_opens_rename_and_enter_commits`
  - `rename_field_keeps_focus_on_the_frame_it_opens`
  - `right_click_rename_opens_the_same_editor`
  - `escape_or_blur_with_unchanged_name_is_harmless`: no `LayerRename`, or a no-op one. Check `RenameNode` with the same name leaves `rev` alone. If it does not, record the finding and leave the behaviour unchanged.
  - `drawing_snap_reports_active_path_not_stale_selection`: `Snap` built from an `Editor` with `objsel` = {A} and `active` = B.

### Wave 2 — after wave 1 merges; QW5 lands **before S1 implementation starts**, otherwise fold QW5 into S1's "menu parity"

**QW4 — Smooth round caps (PAINS_LOG P13)** · `opus` (render geometry next to three rounds of join fixes) · **S** · risk: low–medium · S1 conflict: none
- **Owns:** `varos-render-wgpu/src/tess.rs` (`stroke_cap`, the cap calls in `stroke_poly`, and its tests module). **Must not touch:** `lib.rs` draw routing, `scene.rs`.
- **Steps:**
  1. Replace the 24-gon full disc with a **half-disc cap built as a 180° `stroke_join`**. `incoming` is the end quad's frame; `outgoing` is the same frame reversed (`dir = -dir`, `normal = -n`, corners swapped). This reuses the existing sagitta ≤ 0.25 px budget (max 128 steps) and pivots on the quad's own corners, so there is no T-junction crack.
  2. **Verify first** how closed outlines arrive (`scene.rs:509` passes `geom.outline`). If a closed ring is delivered with `pts[0] == pts[last]`, emit the missing **seam join** (last segment → first segment) instead of two caps. The old full discs were hiding that seam.
  3. Keep `width < 1.6` behaviour (no joins, no caps).
- **Tests:**
  - `round_cap_hugs_true_circle_at_huge_radius`: r = 1600, every chord within `JOIN_TOL_PX`.
  - `cap_shares_the_end_quad_corners_bit_exact`
  - `closed_ring_seam_is_joined_not_capped`: coverage samples around the seam.
  - `cap_step_count_is_bounded`: ≤ 128; a thin stroke gets ≥ 4 steps.
  - `thin_strokes_unchanged`
  - Report harness D/E vertex counts before and after, as the join fix did (STATUS `b37f78b` entry).

**QW5 — Edit ▸ Select All / Deselect / Delete rows (Known Mac gap)** · `opus` (touches `editor.rs` selection state) · **S/M** · risk: medium (a plain-key menu row must not steal Backspace from text fields) · S1 conflict: **high** in `main.rs` menu dispatch (`main.rs:947-968`), hence the ordering above
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

**QW6 — Readable small controls, same constitution (Astra F11)** · `sonnet` · **S** · risk: low (easy to revert; the visual verdict is Ahmed's) · S1 conflict: `ui.rs` rebase only
- **Owns:** `varos-app/src/shell/tokens.rs`; `ui.rs` `icon_btn` (`1873-1887`), `pf_btn` (`4983+`), the Align/Pathfinder micro-labels (`4892-4956`) and the informational FAINT labels listed at `2003`, `2260`, `4956`; a new `#[cfg(test)] mod token_contrast_tests` in `tokens.rs`. **Must not touch:** ACCENT use, layout sizes of boxes/bars, the rail, `build_topbar`.
- **Steps:**
  1. Add `pub const ICON: Color32 = rgb(0xa8a39f)`. That is MUTED plus 0x19 on every channel, so it keeps the same warm hue. It measures 7.0:1 on PANEL and 6.4:1 on SURFACE. Use it as the **resting** colour in `icon_btn` and `pf_btn`. Hover stays white; active stays azure where it already is.
  2. Draw the `icon_btn` glyph at 18 px inside the unchanged 26×24 chip.
  3. Raise the section micro-labels from 10 px to 10.5 px.
  4. Move FAINT to MUTED **only** where the text carries information (the three sites above). FAINT stays for placeholders and disabled text. **Do not change FAINT's value.**
  5. No shadows, no animation, no new radii.
- **Tests:** `icon_rest_contrast_at_least_4_5_on_panel_and_surface`, `icon_stays_warm` (r ≥ g ≥ b, like the ramp), `muted_and_text_unchanged` (guards against accidental ramp edits).

### Wave 3 — **after S1** (`ui.rs` top bar and `main.rs` view code are S1's); both pieces own `ui.rs`, so they run **sequentially** (one agent, two commits)

**QW7 — Search pill tells the truth (Astra ⌘K observation)** · `sonnet` · **S** · risk: low · **after S1** (`build_topbar`)
- **Owns:** `ui.rs` `search_pill_width` / `search_pill` (`3153-3183`) and its call site in `build_topbar` (`3359-3361`); `chrome.rs` top-bar test only if the pill width changes.
- **Steps:** remove the "⌘ K" keycap, which promises a path that does not exist. Render the pill as disabled (FAINT icon and text, no hover fill), with the hover text "Search isn't available yet". **Do not** bind ⌘K. Apply the same honesty to the **Share** button (`ui.rs:3357`) if S1/S6 have not given it a home.
- **Tests:** `search_pill_advertises_no_shortcut`, which asserts `layout_no_wrap` text excludes the key label; the existing `mac_topbar_controls_share_the_native_traffic_light_centre` stays green.

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
| QW6 look | Are the ICON colour, 18 px glyphs and 10.5 px labels right? | Ship QW6; revert is a one-token change |
| A3, #2 icons, P6 branch, A7 feel, artboard canvas-name chip, A16/A17/A14 design, "Codex colours" | eye / clarification (see §2 table) | none; waits on his hand test |
| Batch-1 open question: is activating a board an unsaved change? | `GATE_LOG.md` Astra batch 1 | S1 decides (view vs content dirty) |
| Mac screen eyedropper | Worth the macOS screen-recording permission prompt? | Defer |

## 6. Merge order, conflict hot-spots, hand-test list

- **Order:** wave 1 (QW1, QW2, QW3 in parallel) → wave 2 (QW4, QW5, QW6 in parallel; QW5 before S1 code starts) → S1 → wave 3 (QW7 then QW8). The moderator merges each piece `--no-ff` onto the work branch, runs the gates on the merged head, and records the result in `GATE_LOG.md`.
- **Hot-spots:**
  - `main.rs`: QW2 (paths, lines 446-452 / 800-812), QW5 (`apply_key`, menu match), QW8 (`fit_to_board`) and S1 (everything). Never run two of these at once.
  - `ui.rs`: QW3 (Layers/Snap/props), QW6 (`icon_btn`/`pf_btn`/labels), QW7 and QW8 (topbar/hands), S1 (topbar/tabs). The regions are disjoint, but it is the same file, so rebase before merging.
  - `editor.rs`: QW1 (hit functions) and QW5 (`select_all`) are in separate waves.
- **For Ahmed to try (batch 3):**
  1. Click the painted edge of an 80-wide ring at ~300 %: it selects. Marquee inside the ring only: the ring stays unselected.
  2. An open Pen path, width 80, at 4000 %: the end caps are round and smooth.
  3. Layers: double-click a `<Path>` name, or right-click → Rename, then type and press Enter.
  4. Pen drawing with an old selection: the dock says "Drawing path…".
  5. The Align and Pathfinder icons are readable without hovering.
  6. Move and resize the window, quit, reopen: it comes back in the same place.
  7. Edit ▸ Select All ⌘A, Deselect ⇧⌘A, Delete. Backspace in a text field still deletes a character.
  8. (after S1) The Search pill promises no ⌘K; Fit keeps the page clear of the bar and the rail.

## 7. Risks / open questions (with defaults)

1. **QW1 changes feel everywhere** (hover highlight, click, marquee, the rotate-ring gate at `editor.rs:712`). Default: ship the painted-band reach. Near a thick stroke the rotate ring gives way to selection; if hand tests dislike it, remove the band from the rotate gate only.
2. **QW4 closed-ring seam:** if closed outlines are not delivered with a repeated first point, the seam handling is a no-op. Default: the agent proves the delivery shape with a test before choosing.
3. **QW5 plain-key menu row on macOS:** a no-modifier key equivalent could hijack Backspace while typing. Default: no key equivalent is shown or bound; the row is click-only and runs the same key path.
4. **QW3 root cause unknown.** Default: the reproduction test decides. If the headless double-click works, ship the discoverability part (context menu + hint) and note that the Astra miss is unexplained.
5. **QW2 vs S2:** S2 will introduce "one app data-path resolver". Default: S2 extends `app_paths.rs` instead of adding a second one.

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
