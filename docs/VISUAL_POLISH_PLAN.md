> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Visual Polish Plan (A16 icons · A17 colour picker · A14 control bar)

> **Status:** DESIGN PROPOSAL (read-only session, 2026-07-09). No code changed. A future session
> executes this; Ahmed reviews visually.
> **Scope:** the three pains Ahmed flagged as "weak/unprofessional" — **A16** (icons across the app),
> **A17** (colour picker + wheel look messy), **A14** (control bar design off / not centred).
> **Law obeyed:** `docs/UI_DIRECTION.md` (7 rules + one-home), `docs/reference/UI_VISION_MOCKUP.html` (the visual
> law — every number below is checked against its `:root` + component CSS), tokens in
> `varos/crates/varos-app/src/shell/tokens.rs` (the ONLY place raw colours/sizes may live).
> All file:line refs are `varos/crates/varos-app/src/ui.rs` unless noted.

## The single biggest finding (read this first)
The mockup — **the law** — draws its **alignment glyphs FILLED** (`stroke="none" fill="currentColor"`:
a solid guide bar + two solid bars, `UI_VISION_MOCKUP.html:176–181`). The Rust app reimplemented the
SAME icons as **thin stroked outlines** (`<line>` + outline `<rect rx="1">`, `IC_AL_*` at `ui.rs:55–62`)
because the shared `lucide()` wrapper hard-codes `fill="none" stroke=#fff` (`ui.rs:1211`). Two 1.3px
outline rects + a hairline read weak and busy at 16px; two solid bars read bold and instant. **This is
the "align icons are weak" complaint, almost entirely.** The pathfinder booleans were already fixed the
same way Ahmed likes (they went hand-painted + FILLED, `pf_btn` `ui.rs:4594`) and the mockup's pathfinder
glyphs are also filled (`UI_VISION_MOCKUP.html:189–192`). Align just never got the same treatment.

---

# AREA A16 — icons feel weak / low-contrast / wrong-weight

## A16.0 — How icons are built (so fixes are grounded)
- **Two rendering paths:**
  1. **Lucide-as-texture** — `IC_*` SVG path strings → `lucide()` wraps them
     `fill=none stroke=#fff stroke-width=2` (`ui.rs:1211`) → `render_svg(..., 96, false)` rasterises at
     96 px via resvg/tiny_skia (`ui.rs:717`, `cursors.rs:193`) → drawn tinted into a 14–16 px rect.
  2. **Hand-painted** — drawn with `painter.rect_filled/line_segment/circle_*` directly (pathfinder,
     fill/stroke swatch, eyedropper pipette, swap arrow, ⋯ dots, window caption, radio dots).
- **Rest tint splits into two tiers:**
  - **Tool rail** draws icons **WHITE always** (`icon_button` `ui.rs:2582`) → strong. *Not the problem.*
  - **Action icons** (`icon_btn` `ui.rs:1586`, `icon_toggle` `ui.rs:1567`, `col_toggle` `ui.rs:3766`,
    layers footer `fbtn` `ui.rs:4235`, field-label `Lab::Icon` `ui.rs:1377`) draw **MUTED at rest**,
    WHITE/TEXT on hover → this is the dim tier Ahmed reads as "low-contrast". MUTED-at-rest is actually
    the correct Illustrator/Figma behaviour AND matches the mockup (`.cbtn{color:var(--muted)}`,
    `.ibtn{color:var(--muted)}`), so **do not brighten the rest state** — the weakness is *shape/weight*,
    not tint.

## A16 audit — which glyphs are healthy vs weak

| Set | Glyphs | Verdict |
|---|---|---|
| Tool rail | `IC_SELECT` `IC_DIRECT` `IC_PEN` `IC_ARTBOARD`(frame) `IC_ROTATE` `IC_SCALE` `IC_EYE`(=pipette) `IC_RECT` `IC_ELLIPSE` `IC_TRIANGLE` `IC_POLYGON` | ✅ real Lucide, white, crisp — **keep** |
| Layers | `IC_L_EYE` `IC_L_EYEOFF` `IC_L_LOCK` `IC_L_UNLOCK` `IC_L_GROUP`(folder) `IC_L_TRASH` `IC_L_SEARCH` | ✅ real Lucide — **keep** |
| Transform labels | `IC_LINK` `IC_FLIPH` `IC_FLIPV` `IC_ROTATE` | ✅ real Lucide — **keep** |
| App/status | `IC_MENU` `IC_PLUS` `IC_X` `IC_MAGNET` `IC_FIT` | ✅ real Lucide — **keep** |
| Pathfinder | `pf_btn` hand-painted filled squares | ✅ already fixed (`d402491`) — the reference for "strong" |
| **Align + Distribute** | `IC_AL_L/CH/R/T/M/B` `IC_DIST_H/V` (`ui.rs:55–62`) | ❌ **WEAK** — stroked outlines; law wants FILLED. **Fix A16.1** |
| Picker eyedropper | `eyedropper_btn` hand-painted pipette (`ui.rs:2058`) | ⚠️ crude + inconsistent with the real pipette the app already owns. **Fix A16.2** |
| Opacity label | `IC_OPACITY` custom half-fill circle (`ui.rs:47`) | ⚠️ acceptable (≈ Lucide "contrast"); low priority. **Fix A16.3** |
| Stroke-weight label | `IC_STROKEW` 3 lines (`ui.rs:49`) | ✅ custom but reads fine — keep |
| Portrait/Landscape | `IC_PORTRAIT`/`IC_LANDSCAPE` bare rects (`ui.rs:74–75`) | ✅ fine at label size — keep |

### A16.1 — Rebuild the align + distribute glyphs as FILLED (highest impact in A16) — **safe-mechanical**
**Problem:** `IC_AL_*` (`ui.rs:55–62`) are a hairline guide `<line>` + two **outlined** `<rect rx="1">`.
Rendered through `lucide()` (stroke 2, no fill) they become thin double outlines that wash out at the
16 px the rail/dock/bar draw them (`icon_btn` 16 px `ui.rs:1594`; align dock `ui.rs:4528`; control-bar
mirror `ui.rs:3477`). The law draws them solid (`UI_VISION_MOCKUP.html:176–181`).

**Fix (two mechanical steps):**

1. Add a fill-capable wrapper beside `lucide()` (`ui.rs:1211`):
```rust
fn lucide_filled(inner: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" \
             fill=\"#ffffff\" stroke=\"none\">{inner}</svg>"
    )
}
```
   and a sibling loader that uses it (mirror of `load_icon` `ui.rs:717`, calling `lucide_filled`).
   Load the 8 align/distribute icons (`ui.rs:778–787`) through that loader.

2. Replace the path data with the law's exact filled paths (transcribed VERBATIM from
   `UI_VISION_MOCKUP.html`, 0–24 viewBox):
```rust
// FILLED align glyphs — law-verbatim (UI_VISION_MOCKUP.html:176–181)
const IC_AL_L:  &str = r#"<path d="M4 3.5h1.6v17H4z"/><rect x="7.5" y="6.5" width="9" height="4"/><rect x="7.5" y="13.5" width="13" height="4"/>"#;
const IC_AL_CH: &str = r#"<path d="M11.2 3.5h1.6v17h-1.6z"/><rect x="7" y="6.5" width="10" height="4"/><rect x="4.5" y="13.5" width="15" height="4"/>"#;
const IC_AL_R:  &str = r#"<path d="M18.4 3.5H20v17h-1.6z"/><rect x="7.5" y="6.5" width="9" height="4"/><rect x="3.5" y="13.5" width="13" height="4"/>"#;
const IC_AL_T:  &str = r#"<path d="M3.5 4h17v1.6h-17z"/><rect x="6.5" y="7.5" width="4" height="9"/><rect x="13.5" y="7.5" width="4" height="13"/>"#;
const IC_AL_M:  &str = r#"<path d="M3.5 11.2h17v1.6h-17z"/><rect x="6.5" y="7" width="4" height="10"/><rect x="13.5" y="4.5" width="4" height="15"/>"#;
const IC_AL_B:  &str = r#"<path d="M3.5 18.4h17V20h-17z"/><rect x="6.5" y="7.5" width="4" height="9"/><rect x="13.5" y="3.5" width="4" height="13"/>"#;
// FILLED distribute glyphs — same language (three solid bars; drop the rx so they read as bars, not pills)
const IC_DIST_H: &str = r#"<rect x="3" y="6" width="3" height="12"/><rect x="10.5" y="6" width="3" height="12"/><rect x="18" y="6" width="3" height="12"/>"#;
const IC_DIST_V: &str = r#"<rect x="6" y="3" width="12" height="3"/><rect x="6" y="10.5" width="12" height="3"/><rect x="6" y="18" width="12" height="3"/>"#;
```
**Honours:** rule 5 (typography/shape is the decoration — a solid bar is unambiguous), rule 6 (Illustrator
density read clearly), and it is a *literal transcription of the law*. No token touched.

**Alt (crispest, more effort — flag for Ahmed):** hand-paint the 8 glyphs with `painter.rect_filled`
exactly like `pf_btn` (`ui.rs:4594`), skipping textures entirely → pixel-crisp at any DPI, perfectly
matching the pathfinder set Ahmed already approved. Recommend the SVG swap first (cheaper); escalate to
hand-paint only if the textured version still looks soft after A16.4.

### A16.2 — Reuse the real pipette for the picker eyedropper — **safe-mechanical (small signature change)**
**Problem:** `eyedropper_btn` (`ui.rs:2058`) hand-draws a pipette from 3 asymmetric line segments + 2
dots. It looks rougher than every Lucide glyph around it — and the app **already owns the real Lucide
pipette** as `IC_EYE` (`ui.rs:31`, the Eyedropper tool texture, loaded at `ui.rs:740`). Two different
pipettes for the same concept = unprofessional.
**Fix:** pass the existing pipette `TextureHandle` into `eyedropper_btn` and `painter().image(...)` it
(14 px, MUTED at rest / WHITE armed) instead of the hand-drawn strokes. Keep the accent-fill armed state.
**Honours:** one-home consistency, rule 4 (accent only on the armed/active state).

### A16.3 — Opacity label glyph (low priority) — **safe-mechanical**
`IC_OPACITY` (`ui.rs:47`) is a home-made half-filled circle; it's essentially Lucide "contrast" and reads
OK at label size. Optional: swap to Lucide `contrast` path for family consistency. Low impact — do last.

### A16.4 — Icon rasterisation sharpness (systemic) — **needs Ahmed's eye**
**Hypothesis:** textured icons are rasterised at **96 px** (`ui.rs:718`) then min-filtered (LINEAR,
`ui.rs:722`) into a 14–16 px rect — a ~6:1 bilinear minification with no mipmaps, which softens edges vs
the browser-crisp mockup. This (not tint) may be part of "weak". **Lever:** in `load_icon` render nearer
the display size (e.g. 40–48 px) and/or enable mipmaps in `TextureOptions`, then eyeball rail vs align vs
layers at 100% and 150% DPI. Purely a quality tweak; must be *seen*, so flag — do not batch with A16.1.
**Note on weight:** do NOT globally thicken strokes — the mockup uses stroke **1.5–1.7**, the Rust
`lucide()` already uses **2.0** (`ui.rs:1214`), so the app is if anything *heavier* than the law. The
weight problem is specifically the *outline-vs-filled* align issue (A16.1), not global stroke width.

---

# AREA A17 — colour picker + wheel look messy

Code: `build_color_modal` (`ui.rs:2157–2565`), `build_wheel` (`ui.rs:1892`). **Do NOT touch the colour
model** (HSV source of truth, channel-radio mechanic, live-preview/undo — all correct). Only layout.

## A17 — concrete problems found

| # | Problem | Where |
|---|---|---|
| P1 | **Tab-switch jump:** Picker field plane is **240×240** but the Wheel disc is **236×236** → the right column and dialog shift ~4 px when toggling Picker↔Wheel; the disc also sits 4 px narrower inside the fixed 508 px dialog, biasing it left. | plane `ui.rs:2225`; disc `ui.rs:1896`; `dw=508` `ui.rs:2165` |
| P2 | **Ragged columns / dead space:** left colour area is 240 tall; the right numeric column (preview 58 + hex + 3 HSB + 3 RGB rows ≈ 290 tall) overruns it, leaving a dead L-shaped gap under the plane and an unbalanced bottom edge. | left `ui.rs:2218–2337`; right `ui.rs:2339–2504` |
| P3 | **Three different "active" languages in one dialog:** Picker/Wheel tabs = accent **outline** (`ui.rs:2186–2188`); Fill/Stroke segments = accent **fill** (`ui.rs:2090`); harmony pills = accent **fill** (`ui.rs:1980`). Reads inconsistent, violates the single-scalpel intent of rule 4. | as noted |
| P4 | **Inconsistent control radii/sizes:** harmony pills `r=3` 52×22 (`ui.rs:1978`), harmony result chips `r=4` 30×22 (`ui.rs:2005/2008`), recent/doc swatches `r=3` 15×15 (`ui.rs:1721`), tabs 54×22, target segs 52×22. Mixed radii (3 vs 4) and widths = "shapes not aligned". | as noted |
| P5 | **Right column rhythm loose:** rows use default 8 px `item_spacing`; radio dots are 15×25 (`ui.rs:1860`) beside 76-wide/25-tall fields; hex row mixes a `#` label + 64 px edit + a 60-wide `A` field + `%`; no single baseline grid. | `ui.rs:2404–2503` |
| P6 | **Colour area ↔ numbers separation too tight:** only 8 px between the alpha rail and the numeric column — the "art" and the "controls" zones don't read as separate groups. | outer `horizontal_top` `ui.rs:2218` |

## A17 — proposed layout (structure an implementer can build; all sizes are tokens/multiples of 4)

**Shared frame:** keep `dw = 508`, `panel_frame(14)`, `item_spacing (8,8)`. Every interactive control in
the modal = **height 24, radius `R`(3)**. One active language everywhere: **accent FILL, white glyph/text**
(drop the outline-tab variant). This alone resolves P3+P4.

**Header row (unchanged skeleton, tidy the tabs):** `[ Fill | Stroke ]  ·gap 12·  [ Picker | Wheel ]
············ [pipette] [×]` — left group left-aligned, action group `right_to_left`. Make Picker/Wheel
use the same accent-fill/white active state as Fill/Stroke (edit `ui.rs:2186–2198`). Add a **1 px `LINE`
hairline** under the header (rule 2: separation by hairline) to divide header from body.

**Body = two fixed columns that BOTH stand 240 tall (fixes P1/P2/P6):**
```
┌ COLOUR AREA (fixed 240 tall) ─────────────┐   gap 16   ┌ NUMBERS (fixed width 168) ───────────┐
│  Picker tab:  plane 240×240 · spectrum 16 │           │ new│cur 104×34   [   OK   ]            │  row 34
│               · alpha 16   (gaps 8)       │           │               [ Cancel ]            │
│  Wheel  tab:  disc 240×240 · brightness16 │           │ ──────────────── hairline ───────────│
│               · alpha 16   (gaps 8)       │           │ #  [ hex ______ ]        [A __] %     │  row 24
│  (Wheel HARMONY pills move BELOW, see ↓)  │           │ (H)[__]°  (S)[__]%  (B)[__]%         │  row 24
└───────────────────────────────────────────┘           │ (R)[__]   (G)[__]   (B)[__]          │  row 24
                                                          └──────────────────────────────────────┘
RECENT    ▪ ▪ ▪ ▪ ▪ ▪            (full width, below both columns, gap 12 above)
DOCUMENT  ▪ ▪ ▪ ▪
```
Concrete changes:
1. **Disc diameter 236 → 240** (`ui.rs:1896 let d = 236.0;` → `240.0`) and centre the H×S disc in the
   exact 240×240 box the plane uses, with the brightness+alpha rails at the identical x-offsets as the
   Picker's spectrum+alpha. Now Picker↔Wheel never moves a pixel. (P1)
2. **Widen the gap between colour area and numbers from 8 → 16** (`ui.add_space(16.0)` before the right
   `vertical`, or a `vsep`+air) so the two zones read as separate groups. (P6)
3. **Right column width 176 → 168**, rows locked to height 24 with 8 px gaps; **HSB on one 3-field row,
   RGB on one 3-field row** (compact fields ~48 wide each) instead of six stacked radio rows — this drops
   the right column from ~290 to ~200 tall so it fits inside the 240 colour area with a clean shared
   bottom edge. (P2/P5) *Radio-channel selection stays* — keep the tiny `radio_dot` but move it to a
   left prefix on each field label, or (cleaner) fold channel-select into a click on the H/S/B/R/G/B
   label glyph. (Behaviour unchanged; this is the one part worth a quick Ahmed sanity-check — see below.)
4. **new/current preview: rotate to WIDE** — `104×34` above OK/Cancel instead of the current tall 44×58
   vertical split (`ui.rs:2343`), so the top of the numbers column aligns to the top of the plane and the
   split-swatch reads as "before/after" left-to-right. (P2)
5. **Unify radii:** harmony result chips `r=4` → `R`(3) (`ui.rs:2005,2008`); everything on `R`. (P4)
6. **Wheel HARMONY block** sits *below* the 240 colour area, full width, same rhythm as RECENT/DOCUMENT
   (label in `FAINT` 10.5 spaced-caps + a wrap of accent-fill pills at `R`), so Picker and Wheel share an
   identical envelope and only the colour area's contents differ. (P2/P3)

**Safe-mechanical:** #1, #2, #4, #5, #6 (pure geometry/tokens, no behaviour change).
**Needs Ahmed's eye:** #3 (collapsing six radio rows into two 3-field rows changes the *interaction
surface* of the channel radios — show him the compact version; if he wants the Photoshop six-row stack,
keep it but still lock every row to height 24 + one baseline grid, which fixes P5 without the regroup).

---

# AREA A14 — control bar design off / elements not centred

Code: `board_ctlbar` (`ui.rs:3353–3493`), frame `ui.rs:3370–3377`, `bar_sep` `ui.rs:3496`,
`ctl_chip` `ui.rs:3502`, shared `num_field` `ui.rs:1351`. A14c already fixed the rotation glyph and
confirmed `ui.horizontal` centres vertically — so the remaining pain is **"the bar's design feels tired"**
(polish/rhythm), not literal centring.

## A14 — concrete problems found

| # | Problem | Where |
|---|---|---|
| Q1 | **Uneven element heights break the centreline:** the bar mixes `num_field` 25, `ctl_chip` 17, `icon_btn` 24, `bar_sep` 16 — but the mirrored `pathfinder_row` uses `pf_btn` **34×28** (`ui.rs:4596`), the tallest thing in the bar. The frame auto-heights to 28, so the 25/24/17 px items float under-centred with loose air above/below. **This is the "not centred" read.** | frame auto-height `ui.rs:3374`; `pf_btn` `ui.rs:4596`; bar call `ui.rs:3482` |
| Q2 | **Numeric values are proportional, not tabular-mono — breaks rule 5 AND the law:** `num_field` draws its value at `FontId::proportional(13.0)` (`ui.rs:1483`). Rule 5 says "tabular mono numerals"; the law's bar/inspector fields use `font:11px var(--mono);font-variant-numeric:tabular-nums` (`UI_VISION_MOCKUP.html:73,136`). Proportional digits jitter as you scrub → the bar feels un-engineered. | `ui.rs:1483`,`1404` |
| Q3 | **Fields chunkier than the law:** bar fields are 25 tall with an 11.5 px letter label + 13 px value; the law's control-bar fields (`.mini`) are **24 tall, 9.5 px faint label, 11 px mono value** (`UI_VISION_MOCKUP.html:70–73`). Slightly oversized → "tired". | `num_field` `ui.rs:1364,1373,1483` |
| Q4 | **Bar-mirror align icons oversized:** the bar draws align via `icon_btn` at **16 px** (`ui.rs:1594`); the law's control-bar buttons use `.ic-s` = **13 px** in a 26×26 `.cbtn` (`UI_VISION_MOCKUP.html:19,76,245`). 16 px in the bar looks heavy next to the mini fields. | `ui.rs:3476–3480` |
| Q5 | **No fixed bar height / baseline:** frame is `Margin::symmetric(10,5)` auto-height (`ui.rs:3374`); the law pins the bar to **height 36, padding 0 10, align-items center** (`UI_VISION_MOCKUP.html:67–68`). Without a fixed height the bar's height shifts as content changes (Artboard vs selection vs idle), so it never settles. | `ui.rs:3370–3377` |

## A14 — proposed fixes (ordered)

1. **Size the bar's pathfinder buttons to the bar (fixes Q1 — the actual "not centred").**
   The `pf_btn` 34×28 belongs in the Pathfinder *dock*; in the *bar mirror* it must match the other bar
   controls. Give `pathfinder_row`/`pf_btn` a compact variant (or a size param) rendering **26×26**
   (glyph squares ~12 px) when used in the bar (`ui.rs:3482`), so every bar element is ≤26 tall and the
   frame settles to the law's 36. **Safe-mechanical.** Honours rule 6 (one calm beat).
2. **Pin the bar to a fixed height + centred content (fixes Q5).**
   Wrap the inner `ui.horizontal` so the bar frame is a fixed **36 px** tall with vertical padding split
   evenly, matching `.ctlbar{height:36px}` (`UI_VISION_MOCKUP.html:67`). Keep `Margin::symmetric(10, _)`
   horizontally; keep corner `RBOX`(8) + 1 px `LINE` (already correct, `ui.rs:3372–3373`).
   **Safe-mechanical.**
3. **Shrink bar-mirror align icons 16 → 13–14 px (fixes Q4).**
   In the bar's align loop (`ui.rs:3476–3480`) draw the align icons at ~13 px (law `.ic-s`), leaving the
   *dock* align set at its current size. Either a size param on `icon_btn` or a bar-local button.
   **Safe-mechanical.**
4. **Numeric values → tabular mono (fixes Q2, honours rule 5).**
   Change the `num_field` value font from `proportional(13.0)` to `FontId::monospace(12.0)` with tabular
   digits at `ui.rs:1483` (and the edit field `ui.rs:1404` to match). **App-wide (dock + bar + picker) —
   needs Ahmed's eye**, but it is a direct rule-5 correction and matches the law; it's the change that
   most makes the whole numeric UI feel "engineered". Recommend showing Ahmed a before/after.
5. **Tighten field label to the law's micro-label (Q3) — optional, needs Ahmed's eye.**
   Drop the `num_field` letter label from 11.5 → ~10 px and the box from 25 → 24 tall so bar fields read
   as the law's `.mini`. App-wide (shared with the dock, which the law sizes identically at `.fld` 25 →
   keep 24). Bundle with #4 so Ahmed judges the field restyle once.

*Note:* horizontal grouping is already correct — `bar_sep` (`ui.rs:3496`) + `item_spacing.x=6` gives the
law's group rhythm; the pill/chip order mirrors the mockup. No regrouping needed beyond the sizing above.

---

# Master priority — impact per effort

| Rank | Fix | Area | Effort | Class |
|---|---|---|---|---|
| 1 | **A16.1 — filled align + distribute glyphs** (law-verbatim) | icons | S | safe-mechanical |
| 2 | **A14.1 — shrink bar pathfinder buttons to 26 px** (kills the "not centred" read) | bar | S | safe-mechanical |
| 3 | **A14.2 — pin bar to fixed 36 px height** | bar | S | safe-mechanical |
| 4 | **A17.#1/#2/#5 — disc 240, 16 px zone gap, unify radii** (kills tab-jump + ragged look) | picker | S–M | safe-mechanical |
| 5 | **A17.#4/#6 — wide before/after swatch + shared Wheel/Picker envelope** | picker | M | safe-mechanical |
| 6 | **A16.2 — real pipette in the picker eyedropper** | icons | S | safe-mechanical (small sig change) |
| 7 | **A14.3 — bar align icons 13 px** | bar | S | safe-mechanical |
| 8 | **A14.4 — numeric values tabular-mono (rule 5)** | bar+all | S | **needs Ahmed's eye** (app-wide) |
| 9 | **A17.#3 — compact HSB/RGB rows** | picker | M | **needs Ahmed's eye** (interaction change) |
| 10 | **A16.4 — icon raster sharpness (render size/mipmaps)** | icons | M | **needs Ahmed's eye** (must be seen) |
| 11 | **A14.5 / A16.3 — micro-label restyle · opacity glyph** | bar/icons | S | polish, last |

**Batching rule:** ship 1–3 and 6–7 (all safe-mechanical, no visual judgement) as one wave Ahmed can
glance-approve; hold 8–10 for a session where Ahmed is watching, since they change app-wide type or an
interaction surface.

## Constitution check (every choice cites its rule)
- **Filled align (A16.1)** = literal transcription of the law (`UI_VISION_MOCKUP.html:176–181`); rule 5/6.
- **No shadows added anywhere; separation stays 1 px `LINE` hairlines** (rule 2) — the new header divider
  in A17 is a hairline, not a shadow.
- **Corners:** all new/edited controls stay `R`(3); boxes stay `RBOX`(8) (rule 3). A17 radius-unify
  removes the stray `r=4`.
- **Azure = scalpel** (rule 4): the A17 single active-state (accent fill) and the pipette armed state are
  the only azure; no azure added at rest.
- **Warm-black ramp only** (rule 7): every colour referenced is an existing token (`MUTED`, `TEXT`,
  `FAINT`, `ACCENT`, `LINE`, `SURFACE`, `PANEL`) — no new raw colours; tokens.rs untouched.
- **Tabular-mono numerals** (rule 5): A14.4 fixes the current proportional-digit violation.
