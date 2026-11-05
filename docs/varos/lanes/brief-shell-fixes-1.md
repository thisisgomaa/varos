# Shell fixes round 1 — owner's v16 hand test (worktree lane-shell-fixes-1, branch off main)

You are an implementer in this git worktree. Do NOT commit; the orchestrator commits. Rust, egui 0.37.
Read first: docs/varos/SHELL_SIMPLE_DIRECTION.md, docs/varos/NEW_SHELL_PLAN.md (stage records + "Simple look
implementation record"), then crates/ui-egui/src/varos/shell/{mod,strip,drag,floats,boxes,divider,tree}.rs,
crates/ui-egui/src/varos/picker/mod.rs, crates/ui-egui/src/varos/tests_shell.rs, crates/ui-egui/src/prefs
(where the pref "Varos shell (new)" = `varos_shell.enabled` lives).

The owner hand-tested v16 (main ac7515fa) and decided / found:

1. **The new shell becomes the DEFAULT.** The pref "Varos shell (new)" defaults to ON for everyone (fresh
   configs and existing configs where the key is absent); the switch stays so the old dock can be chosen.
   Tests: default state ON; an existing ui.json without the key loads ON; an explicit `false` stays OFF.
   Update the pref label to "Classic layout (old dock)" semantics only if that is trivial; otherwise keep it.

2. **An icon dragged out of the right strip never becomes an icon again.** Dragging a panel icon out of the
   strip opens it as a floating/docked panel, but the owner finds no way back to an icon. Required: (a) dropping
   a tab or a floating panel ONTO a strip (azure line on the strip) docks it back as an icon — make the strip a
   first-class drop target along its whole length, with the azure preview; (b) the `<<` control on a floating
   panel and on a lone floating icon returns it to the strip it came from (if that path exists, make sure it
   is visible and works after a drag-out, not only after a click-open); (c) a panel's `⋯` menu gets
   "Collapse to icon". Tests at pointer level for a, b, c.

3. **Resizing a column makes panels overlap / draw into each other** (e.g. the Color wheel spills into the
   Character panel; columns can be dragged below their content width). Required: every box/flyout/float
   clips its body to its own rect (`ui.set_clip_rect` / child UI with clip), and panels declare a minimum
   width (Color: 240, Properties/Character/Paragraph: 260, Layers: 200, default 180); dividers refuse to go
   below the sum of minimums; the legacy bodies scale their contents (the picker wheel shrinks to the width,
   min 160) instead of overflowing. Tests: drag a divider to 50 pt → it stops at the minimum; no paint shape
   of box A lies inside box B's rect after a narrow resize.

4. **The colour picker popover from the fill/stroke wells opens at a small, odd size, is cut off, and closes
   on any outside click.** Required: the popover opens at its full size (240 × content; never clamped so that
   content is cut — if there is no room below, open beside/above, and as a last resort make it scroll);
   it does NOT close on outside clicks any more: it closes only with an **OK** button (applies), **Cancel**
   (reverts the live preview to the value at open) or **Escape** (= Cancel). The well that opened it stays
   highlighted while open. Tests: open from the fill well at the bottom-left of a 900-pt-high window → the
   whole wheel + hex field + OK/Cancel visible; outside click leaves it open; Cancel restores the colour;
   OK keeps it.

5. Keep OFF (classic dock) byte-identical except for the pref default; no colour/token changes
   (`varos/theme.rs` palette is final); icons only from crates/ui-egui/assets/varos/icons (Lucide, ISC).

HEAT RULE (hard, this Mac): another cargo lane is running. Implement + write tests FIRST without cargo.
Before your FIRST cargo command wait until
<scratchpad>/go-shellfix-cargo
exists (`test -f`; if absent `sleep 60`, up to 240 times). Then ONE cargo command at a time, `-j 2`,
`--offline`, tests `-- --test-threads=2`, CARGO_TARGET_DIR="<workspace>/lanes/_target-shared".
Gates: `cargo test -p vectorcraft-ui-egui --lib --release` (0 failures; never weaken/ignore tests),
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check -p vectorcraft-ui-egui`, `git diff --check`.
Captures (ignored tests, 1440×900): `VAROS_SHELL_CAPTURE_SIMPLE` → /tmp/varos-fix1/shell.png and a new
`VAROS_SHELL_CAPTURE_PICKER_OK` showing the picker popover open with OK/Cancel → /tmp/varos-fix1/picker.png;
copy both into docs/varos/mockups/ (shell-fixes-1-*.png).

Report contract (final message): per item 1–5 → DONE / PARTIAL (what is missing) with file:line; the exact
`test result:` lines of each gate; capture paths; files touched; anything you decided that the brief did not say.

## Round-1 additions (owner, second screenshots, same hand test)

6. **The strip's `<<` collapse control sits at the BOTTOM of the right strip and is oversized.** Move it to
   the TOP of the strip (Illustrator puts `<<`/`>>` at the top of the icon column), normal 16 px glyph in a
   28×28 tile, same tile as every other control. Nothing else at the bottom except `⋯`.

7. **Resizing a column should shrink panels to their minimum, then collapse them to icons** (Illustrator
   dock behaviour), not shove all panels around. When the divider is dragged below the column's minimum
   width, the whole column collapses into the strip as icons (azure hint while dragging past the threshold);
   dragging the strip edge outward past ~120 pt expands it back. Tests at pointer level.

8. **Narrow tab rows overlap and show two identical `⋯` buttons** (the tab-overflow menu and the panel
   menu look the same). Overflow must never overlap: the row shows as many whole pills as fit, then ONE
   overflow control drawn as `»` (chevrons-right icon) with the hidden count in its tooltip; the panel
   menu stays `⋯`. Tabs are never clipped mid-word.

9. **Lone floating icons look wrong**: a tall pill with `×` and `>>` on top and a big empty area. Make them
   Illustrator-like and compact: a column of 28×28 icon tiles with 2 pt gaps, a 6 pt grip strip at the top
   (drag handle; no `×` — closing is by dragging back to a strip or "Collapse to icon"/"Close" in the right-
   click menu), `>>` as a 16 px icon in the grip strip's right end to expand to a full panel; the pill's
   height = content, width 40 pt, no empty space. Tests: a lone icon's rect height == 6 + n×30 + 6.

10. **Dragging a whole floating panel/icon group shows a ghost of ONE icon** (e.g. dragging a lone-icon
    pill with ¶ and T shows a single ¶ ghost). The ghost must depict what is being dragged: the whole pill
    (all its icons) for a lone-icon group, the tab row for a panel group, the single tab for a tab. Keep the
    ghost at 60 % opacity following the cursor; the azure line stays the only drop preview. Test: dragging a
    two-icon pill paints a ghost whose rect height covers both icons.

11. **The fill/stroke wells at the bottom of the tools strip look bad: tiny and unlike the picker's wells.**
    Redraw the wells cluster with the same look as the picker's fill/stroke pair (same proportions, 1 px
    `border_strong` outline, overlapping fill-over-stroke, swap arrow and default-colours mini control at the
    picker's proportions), scaled to the strip: 1 column → wells 28 pt, 2–3 columns → 40 pt, centred, with the
    Color/Gradient/None chips below at tile size. No new colours.

12. **Tool strip columns by dragging its edge, not (only) by menu.** Dragging the strip's canvas-side edge
    (8 pt hit area, resize cursor) snaps the strip between 1 / 2 / 3 columns (40/68/96 pt) with a short
    azure hint line at the snap point while dragging; releasing applies the nearest column count and
    persists it per workspace. Keep the `⋯` menu items as a secondary way. Test at pointer level: drag from
    the 1-column edge by +30 pt → 2 columns; by +60 → 3; dragging back → 1.
