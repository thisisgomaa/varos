# Shell look round 2 — owner decisions after seeing v15 (same lane, same rules as brief-look.md)

Owner (2026-10-28), shell ON only; OFF stays byte-identical:

1. **Remove the Workspaces row** (Design / Typography / Print, 28 pt under the app bar) completely.
   Workspaces keep working: the switcher returns to the app bar's right side as a compact control
   (current workspace name + chevron, 11 pt, 24 pt tall, radius 6; click = the same list the row
   offered: presets, custom, "+ New…", rename/delete/reset via the same dialogs). `window.workspace*`
   commands unchanged. Delete the row's code paths (no dead flag), keep tests by retargeting them
   to the app-bar control; middle-drag scrolling of the row is gone with the row.
2. **The Control bar becomes a panel.** Register a panel id `control` ("Control") in the shell's
   panel list (icon: Lucide `sliders-horizontal`), whose body is today's control-bar content
   (`chrome::control_bar` items: snapping, selection summary, fill/stroke wells, stroke weight/profile,
   opacity, Document Setup, Preferences…) laid out as wrapping rows inside a box/float/flyout like any
   panel. It docks, floats, duplicates and persists like every other panel. While the shell is ON the
   fixed control-bar ROW is no longer drawn at all. Default layout: the Control panel is NOT open
   (reachable from the right strip's icon and Window › Control); migration: `VarosShell` version 6
   (5→6 pure: nothing to add except the registry knows `control`; add fixture v5).
3. **Rows above the Board.** With 1 and 2 gone, the Board box starts directly under the 40 pt app
   bar + 12 pt seam. The owner: «الفراغ بين القوائم كبير جداً» — tighten the remaining vertical
   rhythm: app bar 40, seam 12, Board header row 28 (document tabs), rulers as today; no extra
   padding rows anywhere in the chrome; status bar 24.
4. **Document tabs in the Board header** use exactly the box tab-row style (same pill, 11 pt medium,
   same height 28, close icon 12 px on hover/active) — the owner: «شكل التابس جوه بانل الفيو بورت
   مختلف عن باقي التصميم».

Tests: no workspace row rect in `hit_rects` when ON; app-bar switcher opens the list and switches
(layout restored); `control` panel opens from the strip and from `window.panel control`, body shows
the snapping toggle and the fill well, docks and floats; no control-bar row rect when ON; OFF: the
old row and the old app-bar switcher unchanged (existing tests). Capture `VAROS_SHELL_CAPTURE_LOOK`
updated (no rows, Control panel floating open). Report: append a "Round 2" section to
docs/varos/lanes/look-report.md.

5. **Tools strip columns: 1, 2 or 3** (owner: «عاوزها يكون في أوبشن إنها تكون صفين أو 3 صفوف»).
   The tools strip (docked on the Board's left edge or floating) gets a columns setting 1 | 2 | 3,
   chosen from the strip's `…` menu at its bottom (radio items "1 column / 2 columns / 3 columns",
   `tl!`) and from the Edit Toolbar drawer; persisted in `VarosShell` per workspace next to
   `tools_pos` (`tools_columns: u8`, default 1; part of the v6 migration above). Width = 34 / 62 / 90
   pt (28 pt tiles, 2 pt gap, 3 pt inset); tools fill the columns in rail order left-to-right then
   down (Illustrator's double-column rhythm), groups keep their hairline separators across the full
   width, the wells + bottom controls span the full width centred, the flyout triangle stays at each
   tile's bottom-right. The Board's canvas rect shifts by the strip width; docking preview/azure line
   unchanged. Tests: 3 widths, tile rects fill the grid in order, canvas rect shifts, persisted and
   per-workspace, OFF untouched (`toolbar_double` still drives the old rail only).

6. **Flat Illustrator look instead of visible boxes (owner decision "A", 2026-10-28).** The box
   TREE stays (split/swap/maximize/workspaces unchanged), but boxes stop looking like cards:
   - no rounded box frames, no 1 px outline around each box, no 12 pt seams: adjacent boxes are
     separated by ONE 1 px hairline (`t.line`) and the divider hit area (8 pt) is invisible until
     hovered (then a 2 px `t.hover` line; drag unchanged; azure only while dragging/dropping);
   - all boxes share the same flat `t.panel` fill; the central region's base (`t.base`) shows only
     in the Board's canvas margin, never as gutters between boxes; the outer edge of the window
     has no gutter either (the right column touches the window edge, the Board touches the tools
     strip);
   - tab rows are the only header: flat, 28 pt, hairline below; the Board's header row the same;
   - strips (tools + panel icons) are flat columns with a hairline on the Board side, no pill;
   - floats keep a 1 px `t.line` outline and radius 10 (they are the only "cards"), no shadows;
   - the Board's canvas keeps radius 0 (flat), rulers flush.
   This supersedes brief-look.md item 5 (box frame) and the 12 pt seams/radius 12 everywhere.
   Tests: no rounded-rect stroke shapes for boxes; exactly one hairline between two adjacent boxes;
   divider hover line appears only on hover; canvas rect touches the strip; captures updated.
