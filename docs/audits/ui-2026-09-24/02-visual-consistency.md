> **Status:** reference — UI audit, 2026-09-24 (input to the UI system spec).
# 02 — Visual constitution compliance & component consistency

Auditor scope: `docs/UI_DIRECTION.md` + `shell/tokens.rs` vs `ui.rs`, `chrome.rs`, `main.rs`, `shell/*.rs` (and the
canvas colours in `varos-core/src/scene.rs` + `varos-render-wgpu/src/tess.rs`). Read and grep only; no window, no build.
Top bar / tab strip (`ui.rs` 3114–3540, `chrome.rs`) are **being replaced by S1** and are audited as "what S1 must leave behind".

## Executive summary (plain English)
1. The colour palette itself is healthy: every chrome colour comes from `tokens.rs`, no hex codes are scattered around, and there are no shadows.
2. But `tokens.rs` only holds colours and two corner sizes. Text sizes, spacing, control heights and icon sizes have no home, so `ui.rs` makes them up each time.
3. Result: 11 text sizes, 10 corner sizes, 9 control heights and 12 spacing values. 44 of 65 gaps are off the law's 4/8px beat.
4. There is no shared kit of controls. The same control is painted again and again by hand: about 15 icon-button painters, 7 colour-swatch painters, 6 tab/segment styles and 6 separator helpers.
5. Each copy got its own idea of "hover" and "on", so the program feels glued together. Hovered icons go pure white in some places, TEXT colour in others, and don't change in one.
6. On the Mac (the only official build) the law's fonts are never loaded. The app shows egui's built-in Ubuntu-Light and Hack instead. This probably explains part of Astra F11 ("small grey controls are hard to read").
7. The law says "numbers are mono and tabular" and "micro-labels are spaced capitals". The main number field draws proportional 13px text, and no label anywhere has letter-spacing.
8. Azure is mostly used correctly. The exceptions: the dialog OK button, the ruler's pointer tick, and a gradient "glow" while you drag a panel (the law bans glow and gradients).
9. The law documents disagree with each other and with the code: box corners 4 vs 8px, seams 6 vs 12px, capsule pills vs 3px tabs. The canvas guide colour is pink in the tokens but green/cyan in the renderer.
10. Fix: build one small component kit on a larger token set, delete the one-offs, and add a CI grep so no raw colour or size can come back.

## Measurements (all measured on branch `claude/sweet-cerf-1sg30t`, main checkout)
| What | Where | Count |
|---|---|---|
| Raw colour constructors outside `tokens.rs` (`Color32::WHITE/BLACK/TRANSPARENT`, `from_gray`, `from_white/black_alpha`, `rgba_a(…)`) | `ui.rs` | WHITE 23 · BLACK 2 · TRANSPARENT 8 · grey/alpha 14 · `rgba_a` 15 → **62** |
| same | `shell/boxtree.rs` / `registry.rs` / `chrome.rs` / `main.rs` | 6 (4 in the dead sandbox board) / 0 / 0 / 0 |
| Hex literals outside `tokens.rs` (app crate) | `cursors.rs:694` (Win brush `0x00131314`) | 1 |
| Places that hold their own copy of BG/ACCENT | `tokens.rs`, `render-wgpu/lib.rs:14`, `tess.rs:282`, `cursors.rs:694`, `core/scene.rs:15-16` | 5 |
| `CornerRadius` literals | `ui.rs` | 105 uses; **10 distinct radii**: 0,1,2,3,4,5,8,10,11,16 (tokens define 3: R=3, RBOX=8, RCAP=11) |
| Font sizes (`FontId::*(n)` + `.size(n)`) | `ui.rs` | 85 uses; **11 sizes**: 9.5,10,10.5,11,11.5,12,12.5,13,13.5,14,26 (+9.0 in boxtree) |
| `TextStyle` ramp defined in `install_style` (`ui.rs:1569-1590`) but used explicitly | `ui.rs` | **0** (every text uses a literal size) |
| Interactive heights (`allocate_exact_size` y) | `ui.rs` | **9**: 15,16,17,18,22,24,25,26,30 |
| `add_space(n)` values / off the 4px beat | `ui.rs`+`shell` | 12 distinct / **44 of 65** |
| `item_spacing` overrides | `ui.rs`+`shell` | 16 overrides, 9 distinct pairs |
| Icon draw sizes | `ui.rs` | 6 (11,13,14,15,16,17); mockup has 2 (16 `.ic`, 13 `.ic-s`) |
| `vec2(` literals | ui / boxtree / registry / chrome / main | 141 / 21 / 10 / 6 / 1 |
| Float literals before `#[cfg(test)]` (all kinds, incl. geometry maths) | `ui.rs` / `boxtree.rs` / `chrome.rs` | 1211 / 192 / 44 |
| Shadows | whole app | **0** (`tokens.rs:89-90` set `Shadow::NONE`; the splash comment confirms no shadow) ✅ |
| `animation_time` | `tokens.rs:75` | 0.0 ✅, but **1 hand-rolled ease** (`boxtree.rs:117`) |
| `ACCENT*` uses | `ui.rs` / `boxtree.rs` | 26 / 11 (5 `T::ACCENT` + 6 `azure()` gradient calls) |
| Tooltips (`on_hover_text`) / disabled-reason tooltips | `ui.rs` | 22 / 1 |
| Letter-spacing (`extra_letter_spacing`) | everywhere | **0** |
| CI checks that enforce tokens | `.github/workflows/ci.yml` | **0** |

## Control inventory (what exists today)
| Control | Implementations (file:line) | Variants | States present / missing | Azure | Text |
|---|---|---|---|---|---|
| Icon button | `icon_button` 2934 (rail 30²), `shape_slot` 4109 (30²), `icon_btn` 1926 (26×24), `icon_toggle` 1907 (24²), `topbtn` 3165, `mini_btn` 1868 (22², font glyph), `eyedropper_btn` 2401 (24×22), `pf_btn` 5073 (34×28/26²), Layers `fbtn` closure 4699 (30×24), `winctl` 3114, box ✕ `boxtree:482`, box ☰ = egui `menu_button` `boxtree:496`, `scroll_arrow` `boxtree:405`, fill/stroke swap + default pair `ui.rs:4068/4084`, registry `icon_btn` (dead) | **~15 painters, 11 sizes** | hover everywhere. The hovered icon turns **WHITE** in 7 painters, **TEXT** in 5, and does not change in `mini_btn`. Resting rail icons are **WHITE** (`2947`). Pressed: none. Disabled: only `eyedropper_btn` (hand-rolled ×0.4). Keyboard focus: **none** | on = ACCENT fill (rail, toggle, eyedropper) ✅. `topbtn` "active" (magnet) = SURFACE, not azure ✗ | glyphs 14 |
| Text button | `dlg_btn` 2367, `bar_btn` 3189, `pill_btn` 5182, `action_row` 4930 | 4 | the secondary border is LINE2 in two painters and **LINE** in `pill_btn`. Disabled only in `pill_btn` (FAINT text). No focus | `dlg_btn` primary = ACCENT fill ✗ (see V-I3) | 12, 12.5 |
| Tabs / segmented | doc `tab_item` 3238 (28h, RBOX, PANEL), box pills `boxtree:578` (22h, RCAP, SURFACE), picker tabs 2544 (54×22, ACCENT), `target_indicator` 2439 (52×22, ACCENT + outline rest), harmony pills 2316 (52×22, ACCENT + outline rest), `seg_btn` 4955 (22h, ACCENT + SURFACE rest) | **6** in 2 languages ("active = surface" vs "active = azure fill") | hover in all. Disabled/focus: none | mixed | 11, 11.5, 12 |
| Number field | `num_field` 1690 (25h), `dim_field` 3892 wrapper, sandbox `fake_field` (dead) | 1 live | rest/hover/focus(ACCENT + INPUT_WELL) ✅. Disabled = egui's default 0.5 alpha (not a token) | focus ✅ | value **proportional 13** (`1822`); label outside the field, 11.5 |
| Text input | `name_field` 5125 (ACCENT on focus), Layers search 4238 (**no focus state**), layer rename 4511, picker hex 2771 (**stock egui TextEdit** frame) | 4 | hover: none of them | — | 12.5, 13, mono 12.5 |
| Dropdown / menu | `menu_below`+`menu_row`/`check_row`/`menu_sep` 1610/3288 (one careful system), box ☰ uses **egui's stock `menu_button` + `ui.button`** (`boxtree:496-505`) | 2 menu looks | ✅ rows hover. Keyboard navigation: none | — | 12, mono 10.5 |
| Checkbox / switch | `toggle_row` 5161 (iOS switch, RCAP capsule, white knob), `check_row` ✓ 3314, `col_toggle` 4174 (eye/lock), `radio_dot` 2199 | 4 | `col_toggle` "forced" = `from_gray(74)`, a cool grey outside the ramp | ON = ACCENT ✅ | 12, 12.5 |
| Slider | picker rails only (`rail_thumb` 2037: WHITE + black α) | 1 (no panel slider; the mockup's opacity slider is not built) | — | — | — |
| Colour swatch | `paint_row` 2084 (26×18 r4), artboard panel 5317 (26×18 r4, **copy**), `ctl_chip` 3910 (17² r2), `ctl_ab_color` 3939 (17² r2, **copy**), `swatch_strip` 2050 (15² r3), harmony chips 2344 (30×22 rR), `fill_stroke_control` 3971 (20² r4 + r5 halo), sandbox `swatch` (dead) | **7 live painters** | the none-slash is drawn 1.6 / 1.4 / 1.2 px wide. Hover border WHITE in some, none in others | active target edge = ACCENT ✅ | — |
| List row | Layers rows 4268+ (26h), `info_row`/`action_row`/`toggle_row`/`menu_row` (26h) | row height **consistent (26)** ✅ | layers: hover ROW_HOVER · selected ACCENT_TINT + 2px ACCENT edge · active layer ACCENT at **α 0.5** (an untokenised 4th azure) · dimmed 0.42 · dragged black α120 | ✅ | 12, 12.5; auto-names MUTED, named `from_gray(208)` (off-ramp) |
| Separator | `hsep` 1943, `divider` 2952, `bar_sep` 3904, `menu_sep` 3336, Layers `hairline` closure 4230, box header hlines `boxtree:632,655` | **6 helpers** | — | — | — |
| Section label | 10× `RichText(..).color(MUTED).size(10.0).strong()` (TRANSFORM, ALIGN TO…), FAINT 10.5 (`swatch_strip`, HARMONY), FAINT 9.5 (box menu, sandbox `micro`) | 3 | — | — | **no letter-spacing** |
| Tooltip | egui default via `on_hover_text` (22×) | 1 | styled only by `tokens::apply` (PANEL, LINE, radius 8). Font = egui Body 13 | — | 13 |

## Findings

### Bug
- **V-B1 · P1 — The law's fonts never load on the Mac.** `install_fonts` (`ui.rs:1547-1561`) reads only `C:/Windows/Fonts/…`. On macOS it silently falls back to egui's bundled **Ubuntu-Light** (proportional) and **Hack** (mono) (`epaint_default_fonts-0.35`). This is known and deferred in `docs/foundation/MAC_SHELL_PORT.md:40`, but it means the only official build never shows the typography UI_DIRECTION §5 calls "the only decoration". Light weight at 9.5–11.5 px plus MUTED/FAINT on warm black is a likely contributor to Astra F11. *Repro:* run on a Mac and compare glyph shapes with the mockup (not verified by eye here).
- **V-B2 · P2 — Drag ghost eases instead of following the cursor 1:1.** `boxtree.rs:117` does `gpos += (cur - gpos) * 0.55` and repaints every frame. BOX_SYSTEM_PLAN ruling 3 says "Drag follows the cursor 1:1… no eases". *Repro:* drag a panel tab quickly; the ghost lags and then catches up.
- **V-B3 · P3 — Dead controls look live.** Share and Export (`ui.rs:3410-3411`) and the Search pill (`3414`, hover-only) highlight on hover and do nothing. The burger rows New/Open/Save/Export (`3470-3474`) ignore their click result. → **S1 must not carry these over**: a control with no action is not drawn.

### Inconsistency (breaks a law or convention)
- **V-I1 · P1 — Numbers are not mono/tabular** (rule 5, mockup `.v{font:11px mono; tabular-nums}`). `num_field` draws values in `FontId::proportional(13.0)` (`ui.rs:1822`, focus editor `1743`). Mono is used only for hex, zoom % and artboard i/n. Digits change width while you scrub.
- **V-I2 · P2 — Micro-labels are not "spaced uppercase".** No `extra_letter_spacing` anywhere. There are 3 label styles (MUTED 10 bold ×10, FAINT 10.5, FAINT 9.5). The mockup `.slabel` is 9.5 px FAINT, weight 600, letter-spacing 1.2.
- **V-I3 · P2 — Azure outside selection/active/focus.** (a) Dialog OK = ACCENT fill (`ui.rs:2371`), a "primary action". The mockup deliberately makes Share *non*-azure (`.btn`). (b) The ruler pointer tick is ACCENT (`5611`, `5697`), which is cursor tracking, not selection. (c) The panel-drag drop preview is an azure **gradient glow** (`boxtree.rs:741-830`, `fn glow`, `fill_gradient`), and UI_DIRECTION's identity section bans "glass/glow/gradient". (d) There are 4 azure strengths (ACCENT, ACCENT_SEL, ACCENT_TINT, `with_a(ACCENT,0.5)` at `ui.rs:4399`) but only 3 are tokens.
- **V-I4 · P2 — "Active" has two languages.** Box tabs and doc tabs show active = surface/panel fill with no azure (per the mockup). Picker tabs, Align-To, Fill/Stroke target and harmony show active = solid azure fill. The magnet's active state = SURFACE (`3174-3177`). The mockup's `.cbtn.on` (a 10×2 accent underline) exists nowhere.
- **V-I5 · P2 — Warm-ramp leaks (rule 7).** The splash footer uses `rgba_a(0x60,0x60,0x64)`, a cool B>R grey (`ui.rs:3028`). The canvas dot grid is `[0.34,0.34,0.37]`, cool (`tess.rs:283`), while the token says white α.045 (`tokens.rs:52`, used only by the dead sandbox). There are 14 neutral `from_gray` values (74, 208, 24, 160, 90, 140…) off the ramp.
- **V-I6 · P2 — Hover brightness has no rule.** Hovered icons go `Color32::WHITE` (not in the ramp; TEXT is `#e9e6e3`) in 7 places and TEXT in 5. Resting rail icons are painted WHITE (`2947`, `4124`) where the mockup has `.tbtn{color:muted}`, so the tool rail is the loudest chrome on screen.
- **V-I7 · P3 — The splash is a 1.55 s forced hold** (`ui.rs:2961`, `1301-1311` skip building the editor while it shows). That fights "a work tool answers instantly". It also uses radii 16/10/8 found nowhere else.
- **V-I8 · P3 — Canvas guide colours contradict the token law.** `tokens::GUIDE` is magenta `#ff54a8` (mockup `--guide`). The renderer uses green smart guides and cyan ruler guides (`core/scene.rs:24,26`). One of them is wrong; the docs don't say which.
- **V-I9 · P3 — Platform labels half-done.** The status bar hard-codes "Alt+drag duplicates" (`ui.rs:3621`). On the Mac it should be ⌥. `shortcut_label` covers ⌘ only.

### Glue (duplication, one-offs, dead paths)
- **V-G1 · P1 — No component kit.** About 15 icon-button painters, 7 swatch painters, 6 tab/segment painters, 6 separators and 4 text inputs (table above). Pairs of copies: `ctl_chip` / `ctl_ab_color` (`3910` / `3939`), `paint_row` / artboard swatch (`2084` / `5317`), `icon_btn` / `fbtn` (`1926` / `4699`). The two functions named `icon_btn` (`ui.rs:1926`, `registry.rs:96`) look different.
- **V-G2 · P2 — Token aliases.** `ui.rs:22-26` imports tokens under old names (`LINE as BORDER`, `PANEL as SOLID_PANEL`, `SURFACE as BG_SURFACE` **and** `as SWATCH_WELL`), so one colour has two or three names in the file that uses it most.
- **V-G3 · P2 — Local metric constants outside tokens.** `MENU_ROW_H/GUTTER/PAD_V/MENU_R` (`ui.rs:3282-3285`), `RULER` (`5524`), `ICON_RASTER` (`872`), `SPLASH_DUR`, box header heights 40/34 and min width 224 (`boxtree.rs:538,639,687`), chrome bar 28/46 (`chrome.rs:31,33`).
- **V-G4 · P3 — The dead sandbox ships in the product.** Dummy board, hands, dummy panels and fake widgets live in `boxtree.rs:861-960` and `registry.rs:74-312`. Their `bin` was deleted. They hold mockup-faithful pieces that the real app does *not* use (`fake_field` mono 11 with the label inside, collapsible `section`, `micro` label).
- **V-G4b · P3 — Four icon systems.** Lucide stroke SVG (25), filled SVG (9), hand-painted glyphs (pathfinder, caps, ✓, chevrons, triangles) and font glyphs (`"×"` in `mini_btn`, `"☰"` in `boxtree:496`; font glyphs showed as tofu before — `registry.rs:229`).

### Architecture
- **V-A1 · P2 — Canvas overlay colours live in `varos-core`** (`scene.rs:15-26`: ACCENT, HANDLE_COL, SEG_HI, GUIDE, SNAP_GUIDE…) while chrome colours live in `tokens.rs`. BG is copied in 4 places (`tokens.rs:26`, `render-wgpu/lib.rs:14`, `tess.rs:282`, `cursors.rs:694`). There is no single theme source, so "themes" (UI_DIRECTION future list) would need 5 edits.
- **V-A2 · P2 — Visual states are painted inline in every function** (`if hovered {…} else if on {…}`). No `WidgetState → style` function exists, so a state cannot be tested or themed. The headless tests render panels but never check a state's look.
- **V-A3 · P3 — egui defaults still paint some UI.** Tooltips, the picker hex field and the box ☰ menu use egui widgets styled only through `tokens::apply`. Their font (Body 13) and radius (r_box 8) differ from the hand-painted equivalents.

### Missing rule (the system has no rule and needs one)
- **V-M1 · P1 — Tokens cover colour only.** There are no tokens for text sizes, spacing, control heights, icon sizes, stroke widths, disabled alpha or focus ring. That gap is the root of every count above.
- **V-M2 · P1 — The law documents disagree.** Box radius: UI_DIRECTION rule 3 says "0–4px, panels square", the mockup says `--rbox:4px`, and CLAUDE.md + `tokens.rs:57` say 8. Seams: UI_DIRECTION and BOX_SYSTEM_PLAN §3.3 say 6, `tokens.rs:59` says 12. Tabs: BOX_SYSTEM_PLAN §4.5 says radius r (3), the code uses RCAP 11. The code changes were Ahmed's decisions (07-04 / 07-08, recorded in token comments), but the law documents were never updated.
- **V-M3 · P2 — No keyboard-focus look.** Rule 4 names "focus" as an azure role, but only text fields show focus. `Sense::click` widgets can take focus with Tab and show nothing.
- **V-M4 · P2 — No disabled rule.** There are 4 disabled styles: egui's 0.5 alpha (`dim_field`), FAINT text (`pill_btn`), ×0.4 (`eyedropper_btn`) and `from_gray(74)` (`col_toggle`). Only 1 control explains *why* it is disabled.
- **V-M5 · P2 — No CI gate.** Nothing stops a new `Color32::WHITE` or `CornerRadius::same(5)`. BOX_SYSTEM_PLAN §8 "tokens only" is honour-system.

## Where the code drifted from the mockup (`UI_VISION_MOCKUP.html` / BOX_SYSTEM_PLAN §3.5)
| Element | Mockup | Code | Note |
|---|---|---|---|
| Box radius / seam | 4 / 6 | 8 / 12 (`tokens.rs:57,59`) | Ahmed 07-04; docs not updated (V-M2) |
| Box tabs | radius 3, pad 5·12, active SURFACE | capsule RCAP 11, 22h (`boxtree:578-584`) | Ahmed 07-08 |
| Field | h25, label **inside** 9.5 FAINT, value mono 11 | h25, label **outside** 22px column 11.5, value proportional 13 | V-I1 |
| Control-bar field | `.mini` h24, value mono 11 | reuses the panel `num_field` (25h, 13 px) | the bar reads as heavy as the panel |
| Control bar | top 30, h36, content = X/Y/W/H·∠ · chips·Op · align ×4 · insertion ×2 · magnet · ⋯ | top 22 (`3731`), h36 ✅, adds a Pathfinder mirror; magnet moved to the app bar; no ⋯-to-home | P8/A14 history |
| Tool rail | 44 wide, 32² buttons, left 34 / top 104, icons MUTED | 30² buttons, left+16, vertically centred (`3682`), icons WHITE | V-I6 |
| Colour swatch | 18² r2 | 26×18 r4 (Properties), 17² r2 (bar), 20² r4 (rail) | V-G1 |
| Opacity | slider row + value | number field only | — |
| Section header | micro-label + `⋯` (jump to home) | label only; no `⋯`, not collapsible | ONE-HOME "…" mirror is missing |
| Status bar | h25 | h31 (`3608`, gap folded in) | documented 07-07 |
| Measure HUD | azure fill, white mono 10, r2 | PANEL + LINE border, proportional 12 (`5742-5765`) | — |
| Ruler numerals | mono 8.5 | proportional 9.5 (`5541+`) | — |
| Dot grid | white α .045, 22px cell | cool grey, adaptive ~30px (`tess.rs:282-283`) | V-I5 |
| App bar | h40 | Mac 28 / Win 46 (`chrome.rs:31,33`) | S1 area |
| Fonts | Segoe UI Variable / Cascadia (Windows-era choice) | Mac: egui Ubuntu-Light / Hack | V-B1: the law needs a cross-platform font decision |

## Proposed component kit (names · states · tokens consumed)
All states come from one `fn look(kind, state) -> Look`. The five states are **rest · hover · pressed/on · disabled · focus**. Each kit piece is a `pub fn` in `shell/kit/`, is tested headless per state, and appears in a gallery.
| Component | Replaces | States | Tokens |
|---|---|---|---|
| `IconButton{size: Tool32 \| Bar26 \| Inline22, role: Action \| Toggle \| Tool}` | 15 painters | rest MUTED icon · hover HOVER bg + TEXT icon · on ACCENT bg + ON_ACCENT icon (Tool/Toggle only) · disabled α · focus ring | `H_TOOL/H_BAR/H_INLINE`, `IC/IC_S`, `HOVER`, `ACCENT`, `ON_ACCENT`, `FOCUS`, `DISABLED_A`, `R` |
| `Button{Secondary \| Ghost \| Default}` | `dlg_btn`, `bar_btn`, `pill_btn` | same five | `H_CTRL`, `SURFACE`, `LINE2`, `HOVER`, `T_BODY` (+ a rule on whether Default may be azure) |
| `Segmented` | picker tabs, Align-To, Fill/Stroke target, harmony | one active language (pick azure *or* surface) | `H_SEG`, `R`, `T_SMALL` |
| `TabStrip{Doc \| Box}` | `tab_item`, box pills | rest/hover/active/drag/close-hover | `H_TAB`, `RBOX` or `RCAP`, `PANEL/SURFACE`, `VOID_HOVER` |
| `NumField` | `num_field`, `dim_field`, the bar mini field | rest/hover/scrub/edit(focus)/disabled+reason | `H_FIELD`, `T_NUM` (mono tabular), `T_MICRO` label inside, `INPUT_WELL`, `ACCENT` |
| `TextField{prefix icon?}` | `name_field`, Layers search, rename, hex | rest/hover/focus/disabled | same as NumField |
| `Swatch{S17 \| S18 \| S20, none, checker, target-ring}` | 7 painters | rest/hover/target/disabled | `SW_*`, `LINE2`, `NONE_RED`, `CHECKER_A/B` |
| `Menu`, `MenuRow`, `CheckRow`, `MenuSep`, `Flyout` | `menu_below` family + egui `menu_button` | hover · checked · disabled · keyboard focus | `H_ROW`, `MENU_GUTTER`, `RMENU` |
| `Switch`, `Radio`, `EyeLockToggle` | `toggle_row`, `radio_dot`, `col_toggle` | off/on/hover/forced/disabled | `RCAP`, `ACCENT`, `ON_ACCENT` |
| `ListRow` | Layers row + info/action rows | rest/hover/selected/active/dimmed/dragged/drop-target | `H_ROW`, `ROW_HOVER`, `ACCENT_TINT`, `ACCENT_EDGE_W`, `ACCENT_SOFT` |
| `SectionHeader{collapsible?, more→home?}` | 3 label styles | rest/hover/collapsed | `T_MICRO` (9.5, 600, +1.2 spacing, FAINT) |
| `Separator{H \| V \| Rail \| Menu}` | 6 helpers | — | `LINE`, `SP_*` |
| `Frame{Box \| Hand \| Dialog \| Menu \| Hud}` | `panel_frame`, rail/ctlbar frames, splash card, HUD | — | `PANEL`, `LINE`, `RBOX`, `SP_*` |
| `Tooltip` (styled egui) | `on_hover_text` | normal / disabled-reason | `T_SMALL`, `PANEL` |
New token groups: **type** (`T_MICRO 9.5, T_SMALL 11, T_BODY 12, T_TITLE 13, T_NUM mono 11`) · **space** (`SP_1..6 = 2,4,8,12,16,24`) · **heights** (`H_ROW 26, H_FIELD 24/25, H_SEG 22, H_TOOL 32, H_BAR 26`) · **icons** (`IC 16, IC_S 13`) · **state** (`ON_ACCENT #fff, FOCUS, DISABLED_A`) · **canvas** (guide, smart-guide, handle, dot grid — shared with core as plain `[f32;4]`).

## Rules this suggests
1. No `Color32` construction (`from_*`, `WHITE/BLACK/GRAY/TRANSPARENT`) outside `shell/tokens.rs`, except a named colour-maths allowlist (document colour → `Color32`). CI: `rg` count = 0.
2. No numeric `CornerRadius::same(n)`, `Margin::*(n)`, `FontId::*(n)` or `.size(n)` outside `tokens.rs`/`shell/kit/`. CI grep = 0.
3. A panel never paints a control. It calls a kit component; `painter().rect*` in panel code is only for content (thumbnails, rulers, canvas).
4. Every kit component implements rest/hover/pressed-or-on/disabled/focus, and a headless test renders each state.
5. Azure appears only through ACCENT tokens and only for selection, on/active and keyboard focus. Never for primary buttons, gradients or pointer tracking.
6. Every number on screen uses `T_NUM` (mono, tabular).
7. Every gap, margin and spacing comes from the `SP_*` scale (4px beat; 2 is the only half-step).
8. Icons come in exactly two sizes (16/13), are rasterised at the device pixel size, and rest in MUTED.
9. A disabled control uses `DISABLED_A` and carries a tooltip that says why.
10. No motion: chrome never interpolates toward a target (drag = pointer 1:1). `animation_time` stays 0 and is asserted in a test.
11. Fonts are bundled in the binary and identical on every platform; the app never reads OS font paths.
12. One theme source: canvas overlay colours and chrome colours live in one versioned palette. Renderer, core and chrome import it; none copies it.
13. A control with no action is not drawn (no "visual mirrors" without a home).
14. A change to a law value in `tokens.rs` updates UI_DIRECTION / the mockup `:root` in the same commit. A test compares `tokens.rs` with the mockup's `:root`.

## What to fix first
| # | Fix | Size |
|---|---|---|
| 1 | Decide and bundle the UI + mono fonts (Mac is official); remove the `C:/Windows/Fonts` lookup (V-B1) | S |
| 2 | Add type/space/height/icon/state tokens to `tokens.rs`; drop the aliases in `ui.rs:22-26` (V-M1, V-G2) | S |
| 3 | Reconcile the law numbers (radius 4 vs 8, seam 6 vs 12, tab capsule) with Ahmed and write them into UI_DIRECTION + mockup (V-M2) | S |
| 4 | CI grep gate for rules 1–2, with a counted allowlist that must only shrink (V-M5) | S |
| 5 | `IconButton` kit (replaces ~15 painters) incl. MUTED resting rail icons and one hover colour (V-G1, V-I6) | M |
| 6 | `NumField` with mono tabular values + label inside; reuse it in the control bar at bar size (V-I1) | M |
| 7 | `Swatch` + `Segmented` + `SectionHeader` (letter-spaced) kits (V-G1, V-I2, V-I4) | M |
| 8 | Remove the drag-ghost ease and the gradient glow; keep a flat azure outline for the drop slot (V-B2, V-I3c) | S |
| 9 | Focus ring + disabled token across the kit (V-M3, V-M4) | M |
| 10 | One palette for canvas + chrome (BG ×4, ACCENT ×2, guide colour decision, warm dot grid); delete the sandbox dummies (V-A1, V-I5, V-I8, V-G4) | M |

**What S1 must leave behind (top bar / tab strip):** tab chips and bar buttons built from `TabStrip`/`Button`/`IconButton` (no new painters), heights from tokens (not `28/46/42/36/32` literals in `chrome.rs`), no inert Share/Export/Search or burger rows, `VOID_HOVER`/`SEAM` from tokens, and the platform labels from the `tokens` helpers.

**Could not verify without a window:** actual glyph rendering and legibility on Retina (V-B1 is inferred from code + `MAC_SHELL_PORT.md:40`), icon softness from the 32px raster at 13/17pt sizes, whether font glyphs `×`/`☰` render in the egui fallback fonts, and how loud the resting rail icons look in practice.
