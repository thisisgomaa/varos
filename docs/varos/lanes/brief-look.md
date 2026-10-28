# Shell look round — make the new shell match the approved look (geometry, type, icons, frames)

Repo: varos-next (Rust, egui 0.37). Work ONLY in this worktree (branch lane-look off main 5609bac7+).
The owner hand-tested v15 with the new shell ON and said: «شغالة كويس بس شكلها بشع» — it works,
it looks ugly. This round is LOOK ONLY: no behaviour changes, no new features, no colour-palette
change (colours stay `theme::Tokens`; a separate colour round comes later).

Read first: docs/varos/ui-handoff-2026-10-27/approved-html/index.html (the approved look — open the
CSS: sizes, radii, weights, spacing), docs/varos/ui-handoff-2026-10-27/architecture/IMPLEMENTED_DESIGN.md
(typography 11/12 pt, label 11 medium, title 14 bold −0.3; radii field 5 / button 6 / tile 7 /
popover 10 / panel 12; spacing 4/6/8/12; icon button 28×28 with 16 px icon), docs/varos/mockups/
ui-handoff-1440.png (target), docs/varos/mockups/shell5-capture.png (what we have), then
`crates/ui-egui/src/varos/shell/*.rs`, `chrome.rs`, `toolbar.rs`, `varos/picker/`, `theme.rs`
(tokens + how icons are loaded: the 117 Lucide overrides, `render_svg` textures — NEVER text glyphs
for chrome controls, never egui default widgets).

## What the owner sees (from his screenshots) → what to do
1. **Grip dots everywhere.** The `⋮⋮⋮` dot rows between every tool group and every panel-icon group
   on the strips are noise. Keep ONE grip per strip at its top (30×3 bar, `t.muted`, shown within
   15 px / hover like the handoff's approach-revealed grip). Group separators = 1 px hairline
   (`t.line`) with 6 px above/below. Nothing else between groups.
2. **Text glyphs as controls.** `<<`, `>>`, `×`, `≡`, `◇`, `‹ ›` drawn as text look cheap. Use icon
   textures: chevrons-left / chevrons-right / x / ellipsis (or menu) / and for ◇ the smallest
   suitable Lucide icon (e.g. `chevrons-up-down` or `diamond` if present in assets). 16 px icon on a
   28×28 (strip) or 20×20 (tab row) hit area; hover = `t.hover` tile, radius 6.
3. **Icon sizes uneven.** Every strip icon renders at 16 px inside a 28×28 tile, 2 px gap between
   tiles, 3 px inset from the strip edge; the 3×3-dots Swatches-style icon and the tool icons must
   not be bigger than the rest. Tool flyout triangle: 4 px, bottom-right corner of the tile, `t.quiet`.
4. **Double frames.** Legacy panel bodies (Artboards list, Layers list, Properties sections…) draw
   their own frame/background inside the box frame → a frame inside a frame. The shell host passes a
   "frameless" mode to `dock::panel_body` / the panel bodies (one flag threaded through, default
   off so OFF is byte-identical): inside the shell the body has no outer frame and no inner
   background, 8 px inset, lists run edge to edge within that inset. Same for flyouts and floats.
5. **Box frame.** 1 px `t.line` on `t.panel` fill, radius 12 (handoff), 12 px seams (unchanged);
   the strips get the same 1 px hairline on the Board side only, no double outline, no rounded pill
   outline around the whole strip.
6. **Tab rows.** 28 px tall; labels 11 pt medium; active tab = pill (`t.active` fill, radius 6,
   8 px horizontal padding), inactive = `t.muted` text, hover `t.hover`; the `≡` (ellipsis icon)
   16 px at the right with 6 px margin. Tabs must never be clipped mid-word ("Color Guide"): if the
   row is too narrow, the overflowing tabs collapse into a `…` menu at the right (click = list), the
   active tab always visible. Document tabs in the Board header: same row style, close icon 12 px.
7. **App bar / workspace row / control bar.** App bar 40 pt: brand 16 px, doc title 12 pt medium
   centred, undo/redo/search 16 px icons in 28×28 tiles. Workspace row 28 pt: same pill tabs as 6.
   Control bar (when shown): labels 11 pt `t.muted`, NO underlines, fields 24 pt tall radius 5,
   icon tiles 24×24. Status bar 24 pt: 11 pt.
8. **Flyouts and the picker popover.** Flyout/popover panel: `t.panel` fill, 1 px `t.line`, radius
   10, 8 px inner padding, opens 6 px away from its strip and NEVER overlaps the strip or the
   Board's tool strip (clamp to the central region); width 240–280 pt (the picker 240). Picker
   wheel scales to the available width (max 200 pt); the mode-icon row at the top uses 28×28 tiles
   at 16 px. Scrollbars inside flyouts/boxes: 4 px, `t.quiet`, auto-hide (egui ScrollBarVisibility
   VisibleWhenNeeded + thin style), never covering content or flyout triangles.
9. **Typography in the chrome:** nothing above 12 pt except nothing; labels 11 pt; body 12 pt;
   metadata 10 pt `t.quiet`. Fonts stay Inter (UI) as today.
10. **Laws:** no shadows; azure (`t.accent`) only for active/selection/focus/drop preview; corners
    3 px controls / 6 px tiles / 10 px popovers / 12 px boxes; OFF stays byte-for-byte today's
    window (every change behind `shell::enabled` or the frameless flag).

## Tests (tests_shell.rs / toolbar tests; Harness + kittest)
- Strip: exactly one grip hit rect per strip; icon tiles all 28×28 with 16 px glyph rects; no
  `⋮` text drawn (walk the paint shapes for text containing "⋮", "<<", ">>", "≡", "◇" → none).
- Frameless: a Layers box paints no nested frame rect (count rect-stroke shapes inside the box
  body at the box's inset = 0).
- Tab overflow: a 160 pt box with 4 tabs shows the active tab + `…`; clicking `…` lists the rest;
  no tab label is clipped (galley width ≤ allotted).
- Flyout/popover never intersects its strip rect or the tools strip rect; width in [240, 280].
- Control bar has no underline shapes under its labels when the shell is ON.
- Captures (ignored tests): `VAROS_SHELL_CAPTURE_LOOK` → 1440×900, shell ON, Layers|Artboards box,
  Properties box with two sections, a panel flyout open, the picker popover open from the fill well,
  to /tmp/varos-look/shell-look-capture.png; `VAROS_SHELL_CAPTURE_LOOK_NARROW` → a 160 pt box with
  tab overflow to shell-look-narrow.png. Put the final PNGs also in docs/varos/mockups/.

## Gates / rules — as stage 5
ONE cargo lane; `-j 2`; `--test-threads=2`; `--offline`; `CARGO_TARGET_DIR="~/Documents/AI
workspace/lanes/_target-shared"`; ui-egui release lib tests (known 10 release-only failures: 9
`panels::gradient::tests::*` + `tests_rotateview::escape_after_closing_a_popup…`); clippy
--workspace --all-targets -D warnings; rustfmt --check on touched files; `git diff --check`; release
build of `vectorcraft`. No commits/stash. Forbidden: engine, render, text crates; `theme.rs` colours
(you may ADD icon entries, not change colours). Report: docs/varos/lanes/look-report.md — what changed
per item 1–10 with file:line, gate table with exact result lines, capture paths, 5 hand-test steps.

Icons live in crates/ui-egui/assets/varos/icons/*.svg (Lucide, ISC — see the LICENSE files there); add any missing Lucide SVG the same way (same licence, no other icon sources).
