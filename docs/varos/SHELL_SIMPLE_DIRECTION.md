# Shell, simplified — direction after the v15 hand test (2026-10-28)

Owner words: «مش عاوز أبقى زيهم بس عاوز البساطة في القوائم والشكل والألوان… نلغي شكل البوكس سيستم ونحتفظ
بالقوائم العايمة». Reference read (read-only, outside the repo): `reference/artcraft/figcraft` =
storytold/figcraft 90054a8 (MIT OR Apache-2.0, egui 0.37 fork, eframe, craft-ui — same stack as ours).

## Decisions
1. **Owner amendment (2026-10-28, after the mockup): the panels are NOT fixed columns.** «مش عاوز
   القوائم ثابتة، عاوز القوائم بالنظام بتاعنا العايم». Every panel group in the mockup (Pages+Layers on
   the left, Properties on the right) is a docked group of OUR shell: drag its tab out → floating card,
   drag back to an edge → azure line → docked again; groups = tabs; strips + flyouts as stages 2–5.
   What changes is only the LOOK of docked groups: flat, no rounded frames, no 12 pt seams, no grips,
   one hairline between neighbours, dividers invisible until hover. The mockup's columns are the
   default Design workspace, not a fixed layout. Split/swap/maximize stay as they are.
2. **Floating panels stay** (Illustrator): strip icon → flyout; tear-off → floating card (1 px line,
   radius 10, no shadow); groups = tabs; the azure line is the only drop preview; light motion as S5.
3. **Simplicity rules (from FigCraft, not its look):** one 40 pt top bar only (brand/Home, document
   tabs, presence + Share, search, workspace); 11 pt labels / 12 pt body / 14 pt semibold titles;
   28×28 icon tiles with 16–18 pt glyphs of one optical size; 1 px hairlines as the only separators;
   sections = title + `+`/`⋯` on the right; fields 24 pt, rows 32 pt, 16 pt side padding.
   Colours = the Varos warm-black ramp (`#141313` base), azure only for selection/focus/active.
4. **Collaboration as a shape, not a feature:** the top bar carries presence avatars and a Share
   button; Home has "Join with a code"; comments is a tool slot. All are placeholders that explain
   "coming later" — the layout is designed around them so adding live sharing later moves nothing.
   (FigCraft: `crates/collab`, `collab-net`, `ui-egui/src/live.rs` — read later, never copied blind.)
5. **Home like FigCraft's structure, Varos' look:** sidebar (Recents · Projects · Shared · Templates ·
   Getting started), search, "New board / Open… / Join with a code", recent files as cards with
   real thumbnails, projects as rows, Getting started tips (hideable). Drop a file to open.
6. **Project → Pages → Artboards.** A `.varos` file is a project; it holds Pages (FigCraft/Figma
   "pages"); each Page is a canvas with artboards. Pages list lives above Layers in the left column.
   Format: next bump adds `pages` with migration (today's document = one page named "Page 1").
7. **What we lift from FigCraft (attribution + NOTICE row each):** the shape of `theme.rs` Tokens
   (light/dark pairs, contrast-checked), the widget kit constants and field/row helpers
   (`widgets.rs`), the toolbar flyout mechanics only if better than ours, the Home sections. NOT:
   their colours, their green, their fixed-only layout, their icons' style.

## Mockup
`docs/varos/mockups/shell-simple-2026-10-28.html` — Editor view + Home view (toggle top-right).
The owner approves/edits it before any code. Then one lane: "shell-simple" (replaces the look rounds
in NEW_SHELL_PLAN.md "Look round"; their items 1–2 and 5 (tool columns) still apply).
