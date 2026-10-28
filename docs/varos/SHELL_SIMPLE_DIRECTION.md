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

## Colour round (owner decision 2026-10-28; brief `docs/varos/lanes/brief-colours.md`)
Supersedes "the Varos warm-black ramp (`#141313` base)" in decision 3: neutrals are Figma UI3 dark/light
exactly; azure `#0C8CE9` only for selection / focus / active / drop preview; agent orange `#F29A3E`
(was `#C76A20`); no shadows. The roles live in `crates/ui-egui/src/varos/theme.rs` (`DARK`, `LIGHT`);
`tokens_for` maps them onto the unchanged `Tokens` names:

| `Tokens` field | role | Dark | Light |
|---|---|---|---|
| `app_bar`, `tab_strip`, `divider` | tab_bar (top band, seams, in-panel dividers) | #1E1E1E | #E8E8E8 |
| `panel`, `ruler`, `measure_bg` | panel (= tab_active) | #2C2C2C | #FFFFFF |
| `panel_darker`, `pasteboard` | canvas | #1E1E1E | #F5F5F5 |
| `input` | field | #383838 | #F5F5F5 |
| `input_border` | field_hover_border | #4D4D4D | #E6E6E6 |
| `border` | border | #444444 | #E6E6E6 |
| `button_border` | border_strong | #555555 | #D9D9D9 |
| `text`, `text_strong` | text | #FFFFFF | #1E1E1E |
| `text_dim` | text_secondary | #B3B3B3 | #6E6E6E |
| `text_disabled` | text_tertiary | #888888 | #8A8A8A |
| `icon` | icon | #F5F5F5 | #1E1E1E |
| `ruler_tick` | icon_secondary | #B3B3B3 | #757575 |
| `hover` | hover | #383838 | #F5F5F5 |
| `tool_active`, `button` | pressed (derived for `button`: hover = field in Figma, so the front pill / lit toggle takes the next step) | #444444 | #E6E6E6 |
| `row_selected` | selected_row | #1E3A52 | #E3F1FC |
| `accent`, `accent_strong`, `selection`, `anchor_selected`, `flyout_active` | accent / focus | #0C8CE9 | #0C8CE9 |
| `error` | danger | #F24822 | #F24822 (no light value in the brief: kept) |
| `warning` | warning | #FFCD29 | #FFCD29 (same) |
| `guide`, `smart_guide` | guide | #F24822 | #F24822 (same) |

Roles with no `Tokens` field yet (kept in `Palette`, no consumer): menu #1E1E1E / menu_text #FFFFFF /
menu_dim #9A9A9A (both modes), icon_secondary beyond rulers, accent_hover #2E9EEE / #0A7BCE, accent_on
#FFFFFF, selected_row_child #243445 / #F0F7FD. egui's own widgets: field tone at rest, pressed on hover/press;
faint striped rows = halfway panel→hover (derived). Upstream-only colours (bleed, highlight, axes, caption
close, artboard marks) are unchanged.
Medium Dark / Medium Light = each endpoint's surfaces, lines and selected rows moved 8 % towards the other
endpoint (`MEDIUM_MIX`, the largest step that keeps azure ≥ 3:1 on their panels); text and icons stay the
endpoint's. Contrast (unit test `palettes_keep_contrast_with_one_documented_exception`): body ≥ 4.5:1 and
secondary ≥ 3:1 on every surface, all brightnesses / schemes / accents; azure ≥ 3:1 on the panel everywhere
and on canvas / field / hover at the endpoints. The one exception, recorded not fixed: azure on the
pressed tone — 2.76:1 dark, 2.83:1 light, 2.88:1 on the light tab bar; 2.25:1 / 2.50:1 at the medium levels.
