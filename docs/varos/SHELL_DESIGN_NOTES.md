# New shell — design notes from the owner (live, 2026-10-27)

Owner's intent in one line: **Blender's window (independent boxes you split and swap, workspaces)
+ Illustrator's panel system (icon strips, flyouts, floating groups, light and calm) + Illustrator's
9 menus.** This file collects his rules, one per observed gesture, in the order he gave them. It is
the design of record for the shell together with the sketch
`mockups/shell-sketch-2026-10-27.png`; it supersedes the rail-on-every-divider idea of the HTML
handoff.

## Structure (agreed 2026-10-27 morning)

1. The Board (canvas) is one big box. Only its edges carry icon strips (tools on the left edge,
   panel icons on the right edge), like Blender's viewport header/side regions.
2. Every other box is an ordinary box: one or more panels as tabs; split it, drag a tab out, close
   it. Dividers between boxes only resize; they carry no icons.
3. An icon strip may also float on its own (not attached to any edge), like Illustrator's torn-off
   dock column.
4. Clicking an icon opens its panel as a flyout beside the strip and closes it again, exactly like
   Illustrator's iconic dock (one flyout per strip at a time; another icon switches).
5. The big menus (File / Edit / Object / Type / Select / Effect / View / Window / Help) stay in the
   top bar.

## Illustrator panel system, element by element (from the owner's screenshots)

| # | Element | What it does | Visual |
|---|---|---|---|
| 1 | Iconic strip | Vertical column of icons in groups; each group has small grip dots on top (drag the whole group); thin separator between groups; `<<` at the top widens the strip to icon + label. | two greys, 1 px separators, monochrome icons |
| 2 | Click on an icon | Opens the panel as a flyout beside the strip (title tab, `>>` collapse, `≡` menu). Click again closes; another icon switches. One open per strip. | flyout is a normal panel body |
| 3 | Expanded strip | Same strip with labels (Appearance, Comments, OpenType…); same groups and separators. Not a second object. | width toggle only |
| 4 | Floating | Anything can be torn off and float: an icon strip (`×`, `>>`), a panel group with tabs, even the tools. Thin grey bar on top to grab, `×` to close, grip dots at the bottom to resize. | almost no shadow |
| 5 | Visual language | Flat: background grey and a slightly darker panel grey; 1 px lines; bold small tab title; the only chrome is `<<` `>>` `×` and grip dots. | no decoration |
| 6 | Drag and drop | Tab out of a group = new floating panel. Near the dock edge = blue insertion line. Onto a group = joins as a tab. Double-click a tab = collapse to its title bar. | blue = azure |
| 7 | Tools | Same idea: icon strip with named groups (Select / Shapes / Draw / Modify / Type / Navigation / Color), expands, floats. | |

## Gestures the owner marked as important

- **Edge glow (2026-10-27, «دي مهمة أوي»):** dragging a floating strip toward the window's right
  edge lights a vertical blue line along the edge = "release here and it becomes a side column".
  Same signal family as the blue insertion line inside a group. Rule: **before release, the
  result is always previewed**; the preview is a thin azure line/glow, never a ghost box.

- **Two states only (owner):** an icon strip is either *icon* or *icon + name*. Icons in a strip
  can be arranged one per row, or dragged onto each other to form a **group**.
- **Group (owner):** a group shows ONE panel at a time; its tabs switch between them (Color |
  Gradient | Color Guide). A tab can be moved to another group or pulled out on its own. The
  flyout of a group opens with the clicked panel's tab active (the group's icon highlights, e.g.
  Color's palette icon lit while the flyout shows Color's tab).
- **While iconic (owner):** as long as a panel lives in the strip as an icon, clicking opens and
  closes it, only one is open at a time, and the flyout has a button (`>>`) that closes it back
  into its icon. The flyout's body is the real panel (Illustrator's Color flyout shows the whole
  picker, the group grows to fit; width is resizable from the bottom grip).

- **The lone floating icon (owner, "the different case"):** drag an icon out of a strip and it
  floats on its own as a tiny one-icon strip (`×`, `>>`, grip dots, the icon). From there:
  - click the **icon** → its panel opens as a flyout beside it; the small icon stays (still
    iconic, still one-click to close);
  - click the **arrows** (`>>`) → it turns into a full floating panel (title tab, `×`, `<<` to
    go back to the icon, `≡`); the icon strip disappears;
  - **double-click the tab** of a floating panel → it collapses to its title bar only; double-click
    again → opens. So a panel has four presentations: icon · flyout · full · collapsed bar, and
    every one is reached by one gesture, reversible with the same gesture.
- *Implementation note (stage 3):* double-click collapse works for stacked boxes and floats; a box
  beside the Board refuses to collapse (status line) until the owner decides what it should become.
- *Implementation note (stage 4):* a box laid out beside a taller all-collapsed stack (one tab row
  beside two) shows whole rather than leave a hole under its row; collapsing it that way is refused
  like a collapse beside the Board.
- **Group look (owner):** the group's tab row holds all its panels' names; names are cut short
  when the group is narrow ("Open | Char | Paragraph | Glyph"), the active tab is lit and wider.
  `>>` and `≡` sit at the row's right. The group's iconic form is a 4-icon strip with the active
  icon lit; clicking an icon in that strip opens the group flyout on that tab. Collapsed to a bar
  = the tab row alone (`×` top-left, `<<` top-right). The active tab carries a small `◇` before its
  name = the panel's own "show fewer / more options" toggle (Character: short view with font,
  size, leading, kerning; full view adds scale, baseline, rotation, language, digits, marks).

## From the owner's screen recording (2026-10-27 10:54, 93 s; frames sampled at 1 s and 0.15 s)

- **Timing: there is almost no animation.** Expand (labels) → collapse (icons) happens within one
  frame (< 150 ms). Flyouts open and close instantly. Docking snaps; nothing slides. The "smooth"
  feel comes from instant response + always-visible previews, not from motion. (Matches Varos'
  `animation_time = 0` law.)
- **Dragging an icon inside a strip:** the dragged icon becomes a small ghost that follows the
  cursor; its source group gets a thin blue outline; a short horizontal blue line between groups
  shows where it will land (above/below a group = join or new group).
- **Dragging a strip to the dock column (89–92 s):** a full-height vertical blue line along the
  column's edge = "release = new column beside this one"; a short horizontal blue line between the
  column's groups = "release = join this column as a group here"; the line moves as the cursor
  moves between groups. Release → the icons join the column; no animation.
- **Dragging an icon out of a strip (25.7–26.4 s):** ghost icon follows the cursor; release on
  empty canvas → a lone floating one-icon strip appears at the cursor.
- **Groups are made by dragging tabs together (84–89 s):** Paragraph + Character + Glyphs became
  one group by dropping each panel's tab onto the other's tab row; the tab row showed the blue
  insertion line between tabs.
- Illustrator's menus in the recording (File, Edit): plain lists, bold-free, with shortcuts right
  aligned, separators by task; nothing else (no icons, no descriptions).
