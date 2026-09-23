> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Study: the Varos icon library — audit of today + expansion plan (A16 / A28)

- **Date:** 2026-09-23. **Author:** Claude (study session). **Decision owner:** Ahmed.
- **Authority:** none. Level-5 document under `docs/foundation/FOUNDATION_CHARTER.md` §3. If anything here conflicts with `docs/UI_DIRECTION.md`, the law wins.
- **Why now:** Ahmed, tonight: "check the icons too, build what needs building, grow the library far beyond today — compare Illustrator's library and add what we need; many elements still need icons instead of text; designers love that. Design later is fine, but it must be in the plan." This answers `docs/PAINS_LOG.md:228` (**A16**: icons weak / unclear / wrong visual weight) and the general rule inside `docs/PAINS_LOG.md:247` (**A28**: "every new control gets a designed icon like Illustrator, not text").
- **Scope:** docs only. No code, no SVG was changed. §7 is the plan; nothing in it is started.
- **Code baseline:** `main` at `bb80641`. All `ui.rs` refs are `varos/crates/varos-app/src/ui.rs` (5,826 lines); `boxtree.rs` / `registry.rs` / `tokens.rs` are under `varos/crates/varos-app/src/shell/`.
- **Sister study:** `docs/studies/2026-09-23-CURSOR_SET_V1.md` (merged in `d30365c`). Its house rules (§3 there) are the family the icons must join.

---

## 1. الملخص (بالمصري)

1. الأيقونات النهارده **٦١ رسمة**: ٣٨ رسمة SVG (أغلبها من Lucide) + ٢٠ رسمة مرسومة بالكود على طول + ٣ حروف نص متحطوطة مكان أيقونة (× و☰ و▾).
2. المشكلة مش في العدد بس: **مفيش مكان واحد للأيقونات**. كل أيقونة جديدة محتاجة تعديل في ٥ أماكن، ومقاساتها ٦ مقاسات مختلفة (١١ لحد ١٧ بكسل) ومش مكتوبة في `tokens.rs`.
3. **سهم التحديد (V) وسهم الدايركت (A) الاتنين مفرغين** — في Illustrator والموك-أب والكيرسرز الجديدة، V مليان وA مفرّغ. دي أوضح حاجة في A16.
4. تخانة الخط مش موحّدة: الأيقونات ١.٣٣ بكسل، الباثفايندر ١.٣–١.٤، علامة ✓ ‏١.٦، والكيرسرز ١.٥. **الاقتراح: ١.٥ بكسل على الشاشة في كل المقاسات** زي الكيرسرز بالظبط (تجربة جنب بعض قبل ما نقفل).
5. لقينا **٢٢ مكان فيه نص المفروض يبقى أيقونة** (زي Clip to page، Move artwork، Snapping/Guides/Rulers، Align To، Add/Duplicate/Delete للأرت-بورد) + ٥ أماكن ناقصها زرار أصلًا (Ungroup، ترتيب الطبقات، New Layer، التنقل بين البوردات).
6. قارنّا بمكتبة Illustrator: **٢٢٢ عنصر** (أدوات وأزرار) — عندنا أيقونة لـ٤٣ بس، و**٦٠ لازمين لـv1 ومش موجودين**، و١١٩ بعد v1.
7. **القانون:** Lucide (رخصة ISC) أساس العيلة. رموز أبل (SF Symbols) ورسومات Adobe **ممنوعة** — بناخد الفكرة المتعارف عليها بس، ونرسم بإيدينا زي ما عملنا في الكيرسرز.
8. ملاحظة قانونية صغيرة: ملف `THIRD_PARTY_NOTICES.md` ناقصه نص رخصة Lucide وسطر Feather (MIT) — محتاج تصليح في شغلانة لوحدها.
9. الخطة ٤ مراحل صغيرة تتجرب بإيدك: **S1** نصلح الوزن والوضوح (M) ← **S2** نبدّل النص بأيقونات (M) ← **S3** أيقونات كل الأدوات اللي عندنا (S) ← **S4** نكبّر المكتبة مع كل نظام جديد: الاستروك، الجراديانت، النص (L).
10. قرارات مستنياك في §8: تخانة الخط، شكل الباثفايندر، والمقاسات التلاتة (13/16/20).

---

## 2. How icons are drawn today

### 2.1 Three sources, one texture path

| Source | What it is | Where | Count |
|---|---|---|---|
| **SVG strings** | `const IC_*: &str` inner-SVG path data on a 24×24 grid. Most are Lucide paths (the header comment says so, `ui.rs:26`); the align/distribute set, opacity, stroke-weight and portrait/landscape are our own | `ui.rs:27–81` (39 consts; `IC_SEARCH` is an alias of `IC_L_SEARCH`, `ui.rs:71`) | **38 glyph literals → 41 textures** (`IC_ROTATE` and `IC_EYE` are loaded twice, `ui.rs:884/915` and `886/925`) |
| **Hand-painted** | egui `painter` calls (`rect_filled`, `line_segment`, `circle_*`, `convex_polygon`) drawn every frame | pathfinder `ui.rs:4825–4864`, window caps `ui.rs:2940–2988`, etc. (full list §3.2) | **20 glyphs** |
| **Unicode text** | A character typed into a label as if it were an icon | `ui.rs:1702–1709` (`mini_btn "×"`), `boxtree.rs:496` (`"☰"`), `ui.rs:5002` (`"▾"`) | **3 glyphs** (4 call sites) |
| Logo PNG | `include_bytes!("../icon.png")` for the splash | `ui.rs:944` | 1 (not a UI icon) |

There is **no icon font** and no `include_str!` of SVG files: every UI glyph is a string literal inside `ui.rs`. (`registry.rs:95–130` also draws Unicode placeholders `⇤ ⇔ ⇥ ≡ ⋮`, but only in the unhosted sandbox; the real app renders Align/Pathfinder/Properties/Layers through the host hook, `ui.rs:1223–1253`, and the other registry panels are not in `PanelId::DOCKABLE`, `registry.rs:25`. So they never ship.)

### 2.2 The rendering path (SVG → texture)

1. `lucide(inner)` wraps the path data in `<svg viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">` (`ui.rs:1373–1378`). `lucide_filled(inner)` is the solid variant, `fill="#ffffff" stroke="none"` (`ui.rs:1382–1387`), used only for the 8 align/distribute glyphs (`ui.rs:927–936`).
2. `load_icon` / `load_icon_filled` (`ui.rs:851–871`) call `crate::cursors::render_svg(svg, ICON_RASTER, false)` — resvg 0.45 + tiny-skia on the CPU (`cursors.rs:211–236`, dependency `Cargo.toml` "resvg = 0.45") — and upload the RGBA once with `ctx.load_texture(..., TextureOptions::LINEAR)`.
3. `ICON_RASTER = 32` for **every** icon, whatever its display size or the screen's scale factor (`ui.rs:849`; the comment at `ui.rs:845–848` explains 96 px was dropped because egui-wgpu builds textures without mipmaps).
4. At draw time the white texture is **tinted** by the widget: rail cells always `WHITE` (`ui.rs:2781`), action buttons `MUTED` at rest → `WHITE`/`TEXT` on hover (`ui.rs:1753`, `1770`, `3006`, `4471`), field labels `MUTED` (`ui.rs:1559`).

**What that means on screen:** a Lucide stroke of 2 units in a 24-unit box drawn at 16 px is **1.33 logical px**. The law's mockup uses 1.5–1.6 units at 16 px = **~1.0–1.07 px** (`docs/reference/UI_VISION_MOCKUP.html:160–196`, `.ic{width:16px}` at `:19`). The approved cursor family is **1.5 px** (`CURSOR_SET_V1.md` §3). Three different weights for one app.

### 2.3 Icon sizes today — six sizes, none in `tokens.rs`

| Display size | Used by | file:line |
|---|---|---|
| 11 px | doc-tab close × | `ui.rs:3100` |
| 13 px | search pill, Layers search, status-bar Fit | `ui.rs:3055`, `4042`, `3482` |
| 14 px | number-field label icons (`Lab::Icon`), Layers eye/lock (`col_toggle`), picker pipette | `ui.rs:1556`, `3993`, `2243/2262` |
| 15 px | `icon_toggle` (link, portrait, landscape), Layers footer | `ui.rs:1751`, `4469` |
| 16 px | rail `icon_button`, `icon_btn` (flip, align, fit), shape slot | `ui.rs:2780`, `1768`, `3921` |
| 17 px | top-bar `topbtn` (magnet, plus), burger | `ui.rs:3008`, `3256` |

`tokens.rs` holds colours, radii and the seam gap only (`tokens.rs:10–44`). CLAUDE.md says tokens live in ONE place; icon sizes are the main exception left. Painted glyphs also carry their own stroke weights: pathfinder 1.3/1.4 (`ui.rs:4841`), box close ✕ 1.3 (`boxtree.rs:855–856`), tab-strip chevrons 1.7 (`boxtree.rs:414–415`), menu ✓ 1.6 (`ui.rs:3151–3152`), window caps 1.0 (`ui.rs:2955`).

### 2.4 How a new icon gets added today (5 edit sites)

1. Add `const IC_FOO: &str = r#"<path …/>"#;` in the block at `ui.rs:27–81`.
2. Add an `Option<egui::TextureHandle>` field — on `Ui` (`ui.rs:783–832`) or on one of the four holder structs `TopIcons` (`ui.rs:255`), `LayerIcons` (`ui.rs:835`), `DockIcons` (`ui.rs:3959`), `AbIcons` (`ui.rs:4868`).
3. Load it in `Ui::new` with a hand-picked unique texture name: `load_icon(&ctx, "lbl-foo", IC_FOO)` (`ui.rs:915–961`).
4. Thread the reference into the panel through the holder built each frame (`ui.rs:1102–1118`).
5. Draw it with one of six widgets, each with its own hard-coded size and tint: `icon_button` (`ui.rs:2768`), `icon_btn` (`ui.rs:1760`), `icon_toggle` (`ui.rs:1741`), `Lab::Icon` (`ui.rs:1513`), `col_toggle` (`ui.rs:3973`), `topbtn` (`ui.rs:2991`).

No test covers icons: nothing checks that an SVG parses, rasterises, or keeps the weight rule. A typo in path data silently yields `None` and an empty button (`load_icon` returns `Option`, `ui.rs:851`).

### 2.5 What is actually wrong (A16, with evidence)

| # | Finding | Evidence | Why it reads "weak / unclear" |
|---|---|---|---|
| W1 | **Selection and Direct Selection are both hollow.** `IC_SELECT` = Lucide `mouse-pointer-2`, `IC_DIRECT` = Lucide `mouse-pointer`, both run through the stroke-only `lucide()` | `ui.rs:27–28`, `880–881`, `1373–1378` | The one convention every Illustrator user reads first — **solid = Selection, hollow = Direct** — is lost. The law draws V solid (`UI_VISION_MOCKUP.html:161` `i-cursor` filled) and A hollow (`:162`); the cursor set does the same (`CURSOR_SET_V1.md` §4.1 rows 1–2). |
| W2 | **Weight drift across the app** | §2.2, §2.3 | 1.0 / 1.3 / 1.33 / 1.4 / 1.6 / 1.7 px side by side. No single weight family. |
| W3 | **Fixed 32 px raster at every scale factor** | `ui.rs:849` | At Windows 150 % a 16 px icon is 24 device px, downsampled from 32 (soft). At Mac 2× a 17 px top-bar icon is 34 device px, *up*sampled from 32 (soft). Only 16 px @ 2× lands 1:1. |
| W4 | **Pathfinder lives outside the SVG family** | `pf_btn`, `ui.rs:4825–4864` | Hand-painted, 1.3/1.4 outlines, two geometries (dock 14 px squares, bar 12 px). It was already made bolder in `d402491` (`PAINS_LOG.md:59`), but it cannot share tokens, tests or rasterisation with the rest. |
| W5 | **Flip glyphs don't say "reflect"** | `IC_FLIPH/V` = Lucide `square-centerline-dashed-horizontal/vertical` (old `flip-*` names), `ui.rs:54–55` | A dashed box split in half reads as "split", not "mirror". Illustrator's convention is two triangles mirrored across an axis; the law's mockup uses axis + arrows (`UI_VISION_MOCKUP.html:174–175`). |
| W6 | **Mixed Lucide versions** | `IC_MAGNET`, `IC_POLYGON` (hexagon), `IC_ARTBOARD` (frame), `IC_L_GROUP` (folder) don't match the paths in Lucide 1.8.0 (checked by string search against the package, §9); the rest do | Small shape-language differences (corner radii, proportions) between neighbours. |
| W7 | **Same concept, two treatments** | Opacity is an icon in Properties (`ui.rs:4594`) but the letters "Op" in the control bar (`ui.rs:3666`) | Breaks "one concept → one glyph" (the icon side of the ONE-HOME rule). |
| W8 | **Text characters used as icons** | `×` `ui.rs:1963`, `2401`; `☰` `boxtree.rs:496`; `▾` `ui.rs:5002` | Font glyphs follow font metrics, not the icon grid: wrong weight, off-centre, different per platform font (Segoe vs the Mac fallback, `ui.rs:1389–1399`). |

What is **already healthy** (keep): the filled align/distribute bars (A16.1 done, `ui.rs:56–67`, law-verbatim from `UI_VISION_MOCKUP.html:177–182`); one pipette everywhere (A16.2 done, `ui.rs:924–925`, `2233–2267`); MUTED-at-rest tint for action icons (correct per `docs/VISUAL_POLISH_PLAN.md` A16.0 — do not brighten the rest state).

---

## 3. Inventory — every icon in use today

### 3.1 SVG glyphs (38 literals, 41 textures)

"Lucide name" was verified by searching each path string in the Lucide 1.8.0 package (§9); "older Lucide" = recognisably Lucide but the path no longer matches 1.8.0.

| # | Const (`ui.rs` line) | Source | Appears in | Loaded / drawn at | Size · tint |
|---|---|---|---|---|---|
| 1 | `IC_SELECT` (27) | Lucide `mouse-pointer-2` | Rail: Selection (V) | `ui.rs:880` / `3534` | 16 · white |
| 2 | `IC_DIRECT` (28) | Lucide `mouse-pointer` | Rail: Direct Selection (A) | `881` / `3534` | 16 · white |
| 3 | `IC_PEN` (29) | Lucide `pen-tool` | Rail: Pen (P) | `883` / `3534` | 16 · white |
| 4 | `IC_RECT` (30) | Lucide `square` | Rail shape slot + flyout | `900` / `3918`, `3946` | 16 · white |
| 5 | `IC_ELLIPSE` (31) | Lucide `circle` | Shape slot + flyout | `901` | 16 · white |
| 6 | `IC_TRIANGLE` (32) | Lucide `triangle` | Shape slot + flyout | `902` | 16 · white |
| 7 | `IC_POLYGON` (78) | older Lucide `hexagon` | Shape slot + flyout | `903` | 16 · white |
| 8 | `IC_ARTBOARD` (77) | older Lucide `frame` | Rail: Artboard (Shift+O) | `882` | 16 · white |
| 9 | `IC_ROTATE` (46) | Lucide `rotate-cw` | Rail: Rotate (R) **and** rotation field label (Properties + control bar) | `884`, `915` / `3650`, `4576` | 16 white · 14 muted |
| 10 | `IC_SCALE` (48) | Lucide `move-diagonal-2` | Rail: Scale (S) | `885` | 16 · white |
| 11 | `IC_EYE` (33) | Lucide `pipette` | Rail: Eyedropper (I) **and** picker eyedropper | `886`, `925` / `2240–2264` | 16 white · 14 muted |
| 12 | `IC_L_EYE` (35) | Lucide `eye` | Layers eye column (hover hint) | `954` / `4207–4217` | 14 · muted |
| 13 | `IC_L_EYEOFF` (36) | Lucide `eye-off` | Layers eye column (hidden) | `955` | 14 · text |
| 14 | `IC_L_LOCK` (37) | Lucide `lock` | Layers lock column (locked) | `956` / `4219–4229` | 14 · text |
| 15 | `IC_L_UNLOCK` (39) | Lucide `lock-open` | Layers lock column (hover hint) | `957` | 14 · muted |
| 16 | `IC_L_GROUP` (41) | older Lucide `folder` | Layers footer: Group | `958` / `4476` | 15 · muted |
| 17 | `IC_L_TRASH` (42) | Lucide `trash-2` | Layers footer: Delete | `959` / `4479` | 15 · muted |
| 18 | `IC_L_SEARCH` (43) | Lucide `search` | Layers search field; top-bar search pill (via `IC_SEARCH` alias, `71`) | `960`, `939` / `4038`, `3054` | 13 · muted/faint |
| 19 | `IC_OPACITY` (49) | own (half-filled circle ≈ Lucide `contrast`) | Properties opacity label | `916` / `4594` | 14 · muted |
| 20 | `IC_STROKEW` (51) | own (3 lines, widths 1.3/2.4/3.8) | Properties stroke-weight label | `917` / `4611` | 14 · muted |
| 21 | `IC_LINK` (53) | Lucide `link` | Constrain W/H (Properties + Artboard panel) | `918` / `4568`, `5035` | 15 · muted / white-on-accent |
| 22 | `IC_FLIPH` (54) | Lucide `square-centerline-dashed-horizontal` | Properties: Flip horizontal | `919` / `4580` | 16 · muted |
| 23 | `IC_FLIPV` (55) | Lucide `square-centerline-dashed-vertical` | Properties: Flip vertical | `920` / `4583` | 16 · muted |
| 24–29 | `IC_AL_L/CH/R/T/M/B` (59–64) | own, **filled**, law-verbatim | Align panel (6) + control-bar mirror (L, CH, R, M) | `928–933` / `4757–4774`, `3683–3686` | 16 · muted |
| 30–31 | `IC_DIST_H/V` (66–67) | own, **filled** | Align panel: Distribute H/V centres | `934–935` / `4780–4785` | 16 · muted |
| 32 | `IC_MENU` (69) | Lucide `menu` | Top-bar burger | `938` / `3254–3257` | 17 · muted |
| 33 | `IC_PLUS` (72) | Lucide `plus` | Top-bar new tab | `940` / `3286` | 17 · muted |
| 34 | `IC_X` (73) | Lucide `x` | Doc-tab close | `941` / `3097–3104` | 11 · faint |
| 35 | `IC_MAGNET` (74) | older Lucide `magnet` | Top-bar Snapping menu | `942` / `3229` | 17 · muted |
| 36 | `IC_PORTRAIT` (79) | own (bare rect) | Artboard panel: Portrait | `921` / `5042` | 15 · muted |
| 37 | `IC_LANDSCAPE` (80) | own (bare rect) | Artboard panel: Landscape | `922` / `5045` | 15 · muted |
| 38 | `IC_FIT` (81) | Lucide `maximize` | Artboard panel, control bar (Artboard mode), status-bar Fit | `923` / `5049`, `3622`, `3479` | 16 / 13 · muted |

### 3.2 Hand-painted glyphs (20)

| # | Glyph | Where it appears | file:line |
|---|---|---|---|
| 1–4 | Pathfinder Unite / Minus Front / Intersect / Exclude (two overlapping squares, op region filled) | Pathfinder panel, Properties "Shape" mirror, control-bar mirror | `ui.rs:4843–4861` |
| 5–8 | Window caps Minimize / Maximize / Restore / Close (1 px Win11-style) | Top bar, right | `ui.rs:2958–2985` |
| 9 | ✓ check | Window menu, Snapping menu | `ui.rs:3148–3153` |
| 10 | Swap fill/stroke (double-headed arrow) | Rail fill/stroke control | `ui.rs:3866–3876` |
| 11 | Default colours (mini white/black pair) | Rail fill/stroke control | `ui.rs:3882–3899` |
| 12 | Flyout corner triangle | Rail shape slot | `ui.rs:3926–3932` |
| 13–14 | Disclosure ▸ (collapsed) / ▾ (expanded) | Layers rows | `ui.rs:4241–4248` |
| 15 | ⋯ three dots | On-canvas artboard menu button | `ui.rs:5228–5230` |
| 16 | Origin cross | Ruler corner box | `ui.rs:5381–5383` |
| 17 | ✕ close | Every box header | `boxtree.rs:852–857` |
| 18–19 | ‹ › chevrons | Tab-strip overflow scroll | `boxtree.rs:405–418` |
| 20 | Move grip bar | Box header (hover-revealed) | `boxtree.rs:390–400` |

Not counted as icons (they are widgets or state marks): the fill/stroke swatch pair (`ui.rs:3770–3904`), the 9-point reference widget (`ui.rs:1712–1738`), radio dots (`ui.rs:2033–2054`), the red "None" slash on swatches (`ui.rs:1945–1948`, `3721–3724`, `3749–3754`, `3778–3781`, `5080–5083`), the Layers drop indicator (`ui.rs:4411–4431`).

### 3.3 Unicode text glyphs (3)

| Glyph | Where | file:line |
|---|---|---|
| `×` | Properties "No paint" (`mini_btn`), Color Picker close | `ui.rs:1963`, `2401` (helper `ui.rs:1702–1709`) |
| `☰` | Box header "change this panel to" menu | `boxtree.rs:496` |
| `▾` | Artboard panel "Presets…" dropdown | `ui.rs:5002` |

**Total in use: 61 glyph drawings** (38 SVG + 20 painted + 3 text characters).

---

## 4. Where TEXT stands in for an icon

Verdicts: **→ icon** = replace the text with a designed icon (tooltip keeps the name); **+ icon** = a control or glyph is *missing* next to existing UI; **keep** = text is the right answer (Illustrator uses text here too, or the text is a value, or Ahmed asked for the name); **remove** = redundant text.

| # | Control | Location | Today | Proposed icon | Illustrator convention | Verdict |
|---|---|---|---|---|---|---|
| T1 | Control bar (Artboard mode) · clip | `ui.rs:3612` | "Clip to page" + pill switch | Page frame whose content is cut at the edge; toggle | None in AI (Varos/Figma "clip content" concept) | → icon |
| T2 | Control bar (Artboard mode) · move art | `ui.rs:3615` | "Move artwork" + pill | Artboard + small shape + arrow; toggle | AI control bar has a "Move/Copy Artwork with Artboard" icon button | → icon |
| T3 | Control bar (Artboard mode) · page colour | `ui.rs:3739` | label "Page" | Page-with-fill glyph as the field label (hex stays text) | None in AI (Varos page colour) | → icon |
| T4 | Control bar (Artboard mode) · context label | `ui.rs:3590` | "Artboard" | Artboard tool glyph (`IC_ARTBOARD`), name stays text | AI bar shows tool controls, no word | → icon |
| T5 | Control bar (Artboard mode) · orientation + new/delete | `ui.rs:3587–3624` (absent) | — (these live only in the panel, `ui.rs:5040–5047`, `5121–5130`) | Mirror portrait/landscape + new + delete | AI artboard control bar has all four as icons | + icon |
| T6 | Control bar (selection) · opacity | `ui.rs:3663–3667` | letters "Op" | `IC_OPACITY` — the Properties label already uses it (`ui.rs:4594`) | AI shows the word "Opacity" as a link | → icon |
| T7 | Control bar (idle) · tool name | `ui.rs:3693` (`main.rs:153–168`) | "Pen (P)" etc. | Tool glyph before the name | — | + icon |
| T8 | Properties · Fill/Stroke "no paint" | `ui.rs:1963` | "×" | The None glyph: white square + red slash (`NONE_RED`) | AI "None" swatch/button | → icon |
| T9 | Properties · paint row labels | `ui.rs:1921–1932` | "Fill" / "Stroke" | — | AI Properties also writes the words | keep (A18 asked for names) |
| T10 | Properties · artboard-clip release (A30) | `ui.rs:4629` | "Clip to artboard" + pill | Same glyph as T1, slashed when released | Varos concept | → icon |
| T11 | Properties (nothing selected) · Snapping | `ui.rs:4647` | "Snapping" + pill | Magnet toggle (reuse `IC_MAGNET`) | AI Properties "Snap Options" are icon buttons | → icon |
| T12 | Properties (nothing selected) · Guides | `ui.rs:4650` | "Guides" + pill | Guides glyph toggle (crossing dashed lines) | AI Properties "Guides" section = icon buttons | → icon |
| T13 | Properties (nothing selected) · Rulers | `ui.rs:4653` | "Rulers" + pill | Ruler toggle (Lucide `ruler`) | AI "Rulers & Grids" = icon buttons | → icon |
| T14 | Properties (nothing selected) · grid dots | `ui.rs:4658` | "Grid dots … On" (read-only) | Grid glyph (read-only until a toggle exists) | AI "Show Grid" icon button | → icon |
| T15 | Properties (nothing selected) · units | `ui.rs:4643` | "Units … pt" | — | AI uses a text dropdown | keep (a value) |
| T16 | Properties (nothing selected) · colour mode | `ui.rs:4659` | "Colour … RGB" | — | text | keep (a value) |
| T17 | Align · Align To | `ui.rs:4738–4751` | "Auto" · "Selection" · "Artboard" segments | Three icon segments (auto / selection bounds / artboard) | AI "Align To" is an icon dropdown (selection · key object · artboard) | → icon |
| T18 | Pathfinder caption | `ui.rs:4798` | "Unite · Minus Front · Intersect · Exclude" | — (the buttons already have icons + tooltips) | AI has no caption | remove |
| T19 | Artboard panel · presets dropdown arrow | `ui.rs:5002` | "▾" character | Drawn chevron (Lucide `chevron-down`; mockup `i-chd`, `UI_VISION_MOCKUP.html:196`) | — | → icon |
| T20 | Artboard panel · transparent page | `ui.rs:5103` | "Transparent page" + pill | Checkerboard glyph toggle | AI "Transparency grid" | → icon |
| T21 | Artboard panel · clip | `ui.rs:5113` | "Clip to page" + pill | = T1 | — | → icon |
| T22 | Artboard panel · move art | `ui.rs:5116` | "Move artwork with artboard" + pill | = T2 | as T2 | → icon |
| T23 | Artboard panel · add | `ui.rs:5122` | "+ Add" | New-artboard glyph (page + "+" in the shared badge slot) | AI Artboards panel footer icon | → icon |
| T24 | Artboard panel · duplicate | `ui.rs:5125` | "Duplicate" | Two stacked pages | AI panel menu / Properties quick action | → icon |
| T25 | Artboard panel · delete | `ui.rs:5128` | "Delete" | Trash (reuse `IC_L_TRASH`) | AI footer trash | → icon |
| T26 | Artboard panel · count field | `ui.rs:5109` | "#" letter | — | — | keep |
| T27 | Status bar · artboard i / n | `ui.rs:3491–3499` | "Artboard 1 / 3" | Keep text, add ‹ › (prev/next) | AI status bar has first/prev/next/last arrows | + icon |
| T28 | Status bar · zoom | `ui.rs:3468–3474` | "100%" | — | AI zoom field (text) | keep |
| T29 | Top bar · Window / Share / Export | `ui.rs:3235–3242` | text buttons | — | law §3.5 draws these as text buttons | keep |
| T30 | App menu rows | `ui.rs:3310–3314` | New / Open / Save / Export | — | AI menus are text | keep |
| T31 | Color Picker · close | `ui.rs:2401` | "×" | `IC_X` (already loaded, `ui.rs:941`) | — | → icon |
| T32 | Color Picker · Picker / Wheel tabs | `ui.rs:2378` | text tabs | — (decide with the A17 redesign) | AI picker has no tabs | keep |
| T33 | Color Picker · harmony rules | `ui.rs:197–206`, `2150–2171` | "Comp", "Analog", "Split", "Triad", "Tetra", "Square", "Mono", "None" | 8 harmony diagrams (dots on a ring) + tooltips | AI Color Guide uses a named list | → icon |
| T34 | Color Picker · OK / Cancel | `ui.rs:2593–2596` | text | — | text | keep |
| T35 | Box header · change panel | `boxtree.rs:496` | "☰" character | Drawn menu glyph (`IC_MENU`) | AI panel flyout glyph | → icon |
| T36 | On-canvas artboard ⋯ menu rows | `ui.rs:5237–5263` | text rows | — | AI menus are text | keep |
| T37 | Layers footer · new layer | `ui.rs:4459–4481` (absent; `ui.rs:34` comment lists "new-layer + · new-sublayer") | — | New-layer glyph (Lucide `layers-plus`) | AI Layers footer | + icon |
| T38 | Arrange (z-order) + Ungroup | keyboard only: `main.rs:200–201`, `command.rs:52` | — (no control anywhere) | Bring to front / forward / backward / send to back + Ungroup | AI Properties "Quick Actions" / Arrange buttons | + icon |

**Totals:** **22 → icon** (T1–T4, T6, T8, T10–T14, T17, T19–T25, T31, T33, T35) · **5 + icon** (T5, T7, T27, T37, T38) · **10 keep** · **1 remove** (T18).

---

## 5. Illustrator's icon vocabulary vs Varos

**Method.** Conventions come from `docs/reference/ILLUSTRATOR_TOOLS_CATALOG.md`, `docs/reference/ELEMENTS_CATALOG.md` (§1 tools, §4 stroke `:372–414`, §6 arrange `:546–618`, §7 structure `:619–693`, §8 artboard `:694–762`), `docs/history/PANELS_PRO_SPEC.md` and general knowledge of Illustrator 2024–2026. Glyphs are described **in words only**; no Adobe glyph was opened, traced or measured for this study. Items marked **(?)** are uncertain (name, shortcut or glyph).

**Status:** ✅ Varos has an icon · 🔴 needed for v1 and missing (text today, keyboard-only, or a v1 system that will need it) · ⬜ post-v1 (catalog `LATER`/`SKIP`, or a system not in the v1 sequence). "v1" follows the catalog's `CORE` tag plus the law mockup's rail (`UI_VISION_MOCKUP.html:266–271`) — Ahmed decides the final v1 line.

**Lucide base:** a name = exists in Lucide 1.8.0 (checked against the package file list, §9); "draw" = no fitting Lucide glyph, must be drawn original (§6.3).

### 5.1 Tools

| Tool (shortcut) | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Selection (V) | solid arrow | ✅ | `mouse-pointer-2`, **filled** (fix W1) | S1 |
| Direct Selection (A) | hollow arrow | ✅ | `mouse-pointer-2` outline | S1 |
| Group Selection | hollow arrow + "+" | ⬜ | draw (badge) | — |
| Magic Wand (Y) | wand with sparkle | ⬜ | `wand-sparkles` | — |
| Lasso (Q) | loop + arrow | ⬜ | `lasso-select` | — |
| Artboard (Shift+O) | frame / page corners | ✅ | `frame` | S3 |
| Pen (P) | nib | ✅ | `pen-tool` | — |
| Add Anchor Point (+) | nib + "+" | 🔴 | draw (`pen-tool` + badge) | S3 |
| Delete Anchor Point (−) | nib + "−" | 🔴 | draw (`pen-tool` + badge) | S3 |
| Anchor Point / Convert (Shift+C) | open caret "^" | 🔴 (tool exists, `editor.rs:41`, `main.rs:162`, no rail icon) | draw (matches cursor `convert.svg`) | S3 |
| Curvature (Shift+~) | nib riding a curve | ⬜ | `spline-pointer` (base) | — |
| Type (T) | "T" | 🔴 | `type` | S4 |
| Area Type | "T" inside a frame | 🔴 | draw | S4 |
| Type on a Path | "T" on a curve | 🔴 | draw | S4 |
| Vertical Type | vertical "T" | ⬜ | draw | — |
| Vertical Area Type | vertical "T" in frame | ⬜ | draw | — |
| Vertical Type on a Path | vertical "T" on curve | ⬜ | draw | — |
| Touch Type (Shift+T) | "T" with handles | ⬜ | draw | — |
| Line Segment (\\) | diagonal line | 🔴 | `slash` / draw (mockup `i-line`, `:171`) | S4 |
| Arc | quarter arc | ⬜ | draw | — |
| Spiral | spiral | ⬜ | draw | — |
| Rectangular Grid | grid | ⬜ | `grid-3x3` | — |
| Polar Grid | rings + spokes | ⬜ | draw | — |
| Rectangle (M) | square | ✅ | `square` | — |
| Rounded Rectangle | rounded square (a Rectangle mode per catalog) | ⬜ | `square-round-corner` | — |
| Ellipse (L) | circle | ✅ | `circle` | — |
| Polygon | hexagon | ✅ | `hexagon` | S1 (refresh path) |
| Triangle (Varos-only) | triangle | ✅ | `triangle` | — |
| Star | star | ⬜ | `star` | — |
| Flare | lens flare | ⬜ | draw | — |
| Paintbrush (B) | brush | ⬜ | `paintbrush` | — |
| Blob Brush (Shift+B) | brush + blob | ⬜ | draw | — |
| Shaper (Shift+N) | scribble → shape | ⬜ | draw | — |
| Pencil (N) | pencil | ⬜ | `pencil` | — |
| Smooth | pencil + wave | ⬜ | draw | — |
| Path Eraser | pencil + eraser | ⬜ | draw | — |
| Join | two ends meeting | ⬜ | draw | — |
| Eraser (Shift+E) | eraser block | 🔴 | `eraser` | S4 |
| Scissors (C) | scissors | 🔴 | `scissors` | S4 |
| Knife | blade | 🔴 | `slice` | S4 |
| Rotate (R) | curved arrow round a shape | ✅ | `rotate-cw` (redraw to convention) | S3 |
| Reflect (O) | mirrored triangles across a dashed axis | 🔴 (Flip exists as a command, `Op::Flip` `ui.rs:120`) | draw | S1 (flip) / S4 (tool) |
| Scale (S) | small + large square with diagonal arrow | ✅ | `scaling` / draw (redraw to convention) | S3 |
| Shear | slanted square | ⬜ | draw | — |
| Reshape | path + pointer | ⬜ | draw | — |
| Width (Shift+W) | stroke bulge with handles | ⬜ | draw | — |
| Warp (Shift+R) | finger smudge | ⬜ | draw | — |
| Twirl | swirl | ⬜ | draw | — |
| Pucker | pinch-in star | ⬜ | draw | — |
| Bloat | bulge-out | ⬜ | draw | — |
| Scallop | scalloped edge | ⬜ | draw | — |
| Crystallize | spiky edge | ⬜ | draw | — |
| Wrinkle | wavy edge | ⬜ | draw | — |
| Free Transform (E) | box with corner arrows | ⬜ | `vector-square` (base) | — |
| Puppet Warp | pins on a shape | ⬜ | draw | — |
| Shape Builder (Shift+M) | overlapping shapes + pointer/plus | 🔴 (catalog flagship; mockup `i-sb`, `:194`) | draw | S4 |
| Live Paint Bucket (K) | bucket | ⬜ | `paint-bucket` | — |
| Live Paint Selection (Shift+L) | arrow + bucket | ⬜ | draw | — |
| Perspective Grid (Shift+P) | two-point grid | ⬜ | draw | — |
| Perspective Selection (Shift+V) | arrow + grid | ⬜ | draw | — |
| Mesh (U) | warped grid | ⬜ | draw | — |
| Gradient (G) | gradient square + slider | 🔴 (catalog build order #15) | draw | S4 |
| Eyedropper (I) | pipette | ✅ | `pipette` | — |
| Measure | ruler + angle | ⬜ | `ruler-dimension-line` | — |
| Blend (W) | two shapes + steps | ⬜ | draw | — |
| Symbol Sprayer (Shift+S) | spray can | ⬜ | `spray-can` | — |
| Symbol Shifter | symbol + arrows | ⬜ | draw | — |
| Symbol Scruncher | symbols pulled together | ⬜ | draw | — |
| Symbol Sizer | symbol + scale | ⬜ | draw | — |
| Symbol Spinner | symbol + rotate | ⬜ | draw | — |
| Symbol Stainer | symbol + tint | ⬜ | draw | — |
| Symbol Screener | symbol + transparency | ⬜ | draw | — |
| Symbol Styler | symbol + style | ⬜ | draw | — |
| Column Graph (J) | columns | ⬜ | `chart-column` | — |
| Stacked Column Graph | stacked columns | ⬜ | draw | — |
| Bar Graph | bars | ⬜ | `chart-bar` | — |
| Stacked Bar Graph | stacked bars | ⬜ | draw | — |
| Line Graph | polyline | ⬜ | draw | — |
| Area Graph | filled polyline | ⬜ | draw | — |
| Scatter Graph | dots | ⬜ | draw | — |
| Pie Graph | pie | ⬜ | draw | — |
| Radar Graph | web | ⬜ | draw | — |
| Slice (Shift+K) | knife + frame | ⬜ | draw | — |
| Slice Selection | arrow + frame | ⬜ | draw | — |
| Hand (H / Space) | open hand | 🔴 (Space-pan exists; mockup `i-hand`, `:168`) | `hand` | S4 |
| Rotate View (Shift+H) | hand + curved arrow | ⬜ | draw | — |
| Print Tiling | page tiles | ⬜ | draw | — |
| Zoom (Z) | magnifier | 🔴 (mockup `i-zoomt`, `:169`) | `zoom-in` | S4 |
| Dimension tool (?) | dimension line | ⬜ | `ruler-dimension-line` | — |
| Intertwine (?) | two interlocked rings | ⬜ | draw | — |

### 5.2 Toolbar paint cluster (foot of the rail)

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Fill / Stroke boxes | overlapping solid square + ring | ✅ (painted, `ui.rs:3770–3864`) | — | — |
| Swap fill & stroke (Shift+X) | curved double arrow | ✅ (painted, `ui.rs:3866–3880`) | — | S1 (family weight) |
| Default colours (D) | mini white/black pair | ✅ (painted, `ui.rs:3882–3903`) | — | — |
| Color (<) | solid swatch | 🔴 (law: "the paint-mode trio at its bottom", `UI_DIRECTION.md` standard layout) | draw | S4 |
| Gradient (>) | gradient swatch | 🔴 | draw | S4 |
| None (/) | white square + red slash | 🔴 (text "×" today, T8) | draw | S2 |
| Draw Normal / Behind / Inside | square + dot variants | ⬜ (law mockup `i-ins-in/i-ins-bk`, `:183–184`, feature not built) | draw | — |

### 5.3 Pathfinder

| Op | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Unite | two squares, both filled | ✅ (painted) | `squares-unite` (outline style) | S1 |
| Minus Front | back filled, front cut away | ✅ (painted) | `squares-subtract` | S1 |
| Intersect | only the overlap filled | ✅ (painted) | `squares-intersect` | S1 |
| Exclude | both filled, overlap empty | ✅ (painted) | `squares-exclude` | S1 |
| Divide | squares split into faces | 🔴 (catalog lists "+ Divide" with the core four; engine has 4 ops only, `boolean.rs:20–25`) | draw | S4 |
| Trim | back shape trimmed | ⬜ | draw | — |
| Merge | trimmed + merged | ⬜ | draw | — |
| Crop | front as crop frame | ⬜ | draw | — |
| Outline | outlines only | ⬜ | draw | — |
| Minus Back | front minus back | ⬜ | draw | — |

### 5.4 Align & Distribute

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Align left edges | bar on left + two bars | ✅ | `align-start-vertical` (we keep the law's filled set) | S1 |
| Align horizontal centres | centre bar + two bars | ✅ | `align-center-vertical` | S1 |
| Align right edges | bar on right + two bars | ✅ | `align-end-vertical` | S1 |
| Align top edges | top bar + two bars | ✅ | `align-start-horizontal` | S1 |
| Align vertical centres | middle bar + two bars | ✅ | `align-center-horizontal` | S1 |
| Align bottom edges | bottom bar + two bars | ✅ | `align-end-horizontal` | S1 |
| Distribute vertical centres | three bars stacked | ✅ | `align-vertical-distribute-center` | S1 |
| Distribute horizontal centres | three bars side by side | ✅ | `align-horizontal-distribute-center` | S1 |
| Distribute top edges | three bars, top line | ⬜ (engine: centres only, `DistAxis`) | `align-vertical-distribute-start` | — |
| Distribute bottom edges | three bars, bottom line | ⬜ | `align-vertical-distribute-end` | — |
| Distribute left edges | three bars, left line | ⬜ | `align-horizontal-distribute-start` | — |
| Distribute right edges | three bars, right line | ⬜ | `align-horizontal-distribute-end` | — |
| Distribute vertical spacing | equal gaps, vertical | 🔴 (catalog `ELEMENTS_CATALOG.md:586` "Distribute Spacing") | `align-vertical-space-between` | S4 |
| Distribute horizontal spacing | equal gaps, horizontal | 🔴 | `align-horizontal-space-between` | S4 |
| Align To Selection | dashed bounds around objects | 🔴 (text today, T17) | draw | S2 |
| Align To Key Object | one bold object among others | 🔴 (text today, "Auto" is our smart variant) | draw | S2 |
| Align To Artboard | objects inside a page | 🔴 (text today, T17) | draw | S2 |

### 5.5 Transform

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Rotate angle field label | curved arrow | ✅ | `rotate-cw` | — |
| Flip horizontal | two triangles mirrored across a vertical axis | ✅ (glyph weak, W5) | draw (`flip-horizontal-2` as base) | S1 |
| Flip vertical | same, horizontal axis | ✅ (glyph weak, W5) | draw (`flip-vertical-2` as base) | S1 |
| Constrain proportions | chain link | ✅ | `link` | — |
| Reference point (9-dot) | 3×3 dots | ✅ (painted widget) | — | — |
| Shear angle field label | slanted square | ⬜ (`ELEMENTS_CATALOG.md:572`) | draw | — |
| Scale Strokes & Effects | checkbox in AI; a stroke + scale glyph | ⬜ | draw | — |
| Rotate 90° CW / CCW (quick action) | square + curved arrow | ⬜ | `rotate-cw-square` / `rotate-ccw-square` | — |
| Transform Again (Ctrl+D) | repeat arrow | ⬜ | draw | — |
| Transform Each | several boxes + arrows | ⬜ | draw | — |

### 5.6 Arrange & structure (Layers-panel glyphs included)

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Visibility | eye / eye-off | ✅ | `eye`, `eye-off` | — |
| Lock | lock / lock-open | ✅ | `lock`, `lock-open` | — |
| Disclosure | ▸ / ▾ triangles | ✅ (painted) | `chevron-right/down` | S1 |
| Group (Ctrl+G) | folder (Varos) / grouped squares | ✅ (`IC_L_GROUP`) | `group` | S1 (folder vs `group`: decide) |
| Delete | trash | ✅ | `trash-2` | — |
| Ungroup (Shift+Ctrl+G) | squares breaking apart | 🔴 (keyboard/command only, T38) | `ungroup` | S2 |
| Bring to Front | stack, top highlighted | 🔴 (keyboard only, `main.rs:200`) | `bring-to-front` | S2 |
| Bring Forward | stack, one up | 🔴 | draw | S2 |
| Send Backward | stack, one down | 🔴 | draw | S2 |
| Send to Back | stack, bottom highlighted | 🔴 | `send-to-back` | S2 |
| New Layer | page + "+" | 🔴 (T37) | `layers-plus` | S2 |
| New Sublayer | layer + indented "+" | ⬜ | draw | — |
| Make / Release Clipping Mask | shape clipping a square | 🔴 (masks are next in the product sequence, `PLAN_MAP_2026-09-23.md` §1 item 2) | draw | S4 |
| Target / appearance ring | hollow / filled ring | ⬜ (`ELEMENTS_CATALOG.md:678`, advanced) | `circle` / `circle-dot` | — |
| Selection-colour square | small coloured square | ⬜ | — (painted) | — |
| Locate Object | magnifier + target | ⬜ | `locate` | — |
| Collect for Export | box + arrow | ⬜ | draw | — |
| Isolation mode | object with dimmed surround | ⬜ | draw | — |
| Search | magnifier | ✅ | `search` | — |

### 5.7 Artboard

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Portrait / Landscape | tall / wide page | ✅ (2 glyphs) | `rectangle-vertical` / `rectangle-horizontal` | S1 (family weight) |
| Fit in window | four corner brackets | ✅ | `maximize` | — |
| New artboard | page + "+" | 🔴 (text today, T23) | draw (badge) | S2 |
| Duplicate artboard | two stacked pages | 🔴 (T24) | `copy` (base) | S2 |
| Delete artboard | trash | 🔴 (T25; glyph exists) | `trash-2` | S2 |
| Move/Copy artwork with artboard | page + shape + arrow | 🔴 (T2/T22) | draw | S2 |
| Clip to page (Varos) | page cutting its content | 🔴 (T1/T21) | draw | S2 |
| Transparent page (Varos) | checkerboard | 🔴 (T20) | draw | S2 |
| Page colour (Varos) | page with fill | 🔴 (T3) | draw | S2 |
| Artboard navigation prev / next | ‹ › | 🔴 (T27) | `chevron-left/right` | S2 |
| Artboard navigation first / last | ⏮ ⏭ style | ⬜ | `chevrons-left/right` (?) | — |
| Artboard Options | page + gear/slider | ⬜ | draw | — |
| Rearrange All | grid of pages | ⬜ | draw | — |
| Show Center Mark | cross in a page | ⬜ | draw | — |
| Show Cross Hairs | page midlines | ⬜ | draw | — |
| Show Video Safe Areas | nested frames | ⬜ | draw | — |
| Presets dropdown arrow | chevron | 🔴 (text "▾", T19) | `chevron-down` | S1 |

### 5.8 View, rulers, guides, grid, snapping

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Snapping (master) | magnet | ✅ (top bar) · 🔴 in Properties (T11) — counted once as ✅ | `magnet` | S2 |
| Rulers | ruler | 🔴 (T13) | `ruler` | S2 |
| Guides show/hide | dashed crossing lines | 🔴 (T12) | draw | S2 |
| Lock guides | guides + lock badge | ⬜ | draw | — |
| Grid show/hide | grid | 🔴 (T14) | `grid-3x3` | S2 |
| Snap to Grid | grid + magnet badge | ⬜ (menu text is fine today) | draw | — |
| Snap to Point | point + magnet badge | ⬜ | draw | — |
| Smart Guides (Ctrl+U) | pink guide + object (?) | ⬜ | draw | — |
| Pixel grid / Snap to pixel | pixel grid | ⬜ | draw | — |
| Zoom in / Zoom out | magnifier + / − | 🔴 (mockup rail + a status-bar zoom control) | `zoom-in` / `zoom-out` | S4 |
| Fit artboard / Fit all | corners / corners + pages | ✅ (fit) · all ⬜ — counted once as ✅ | `maximize` / `fullscreen` | — |
| Actual size 100% | "1:1" | ⬜ | draw | — |
| Outline / Preview mode (Ctrl+Y) | eye + outline square | ⬜ | draw | — |
| Rotate View | rotated page | ⬜ | draw | — |
| Screen modes | window variants | ⬜ | draw | — |

### 5.9 Stroke (the Stroke home, `UI_DIRECTION.md` ONE-HOME; spec requested in `MASTER_PLAN_V1_LAUNCH.md` §5.5)

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Stroke weight label | three lines of rising weight | ✅ (`IC_STROKEW`, deliberate weight exception) | — | — |
| Butt cap | line ending flush at the anchor | 🔴 | draw | S4 |
| Round cap | line ending in a half-circle | 🔴 | draw | S4 |
| Projecting cap | line ending in a square past the anchor | 🔴 | draw | S4 |
| Miter join | sharp corner | 🔴 | draw | S4 |
| Round join | rounded corner | 🔴 | draw | S4 |
| Bevel join | cut corner | 🔴 | draw | S4 |
| Align stroke to centre | stroke straddling the edge | 🔴 | draw | S4 |
| Align stroke inside | stroke inside the edge | 🔴 | draw | S4 |
| Align stroke outside | stroke outside the edge | 🔴 | draw | S4 |
| Dashed line toggle | dashed line | 🔴 | draw | S4 |
| Dashes: preserve exact lengths | dashes cut at corner | ⬜ | draw | — |
| Dashes: align to corners | dashes meeting at corner | ⬜ | draw | — |
| Arrowheads start / end + swap | arrow ends + swap arrows | ⬜ | `arrow-left-right` (swap) | — |
| Width profile + flip | tapered stroke | ⬜ | draw | — |
| Outline Stroke (command) | stroke becoming a filled shape | ⬜ | draw | — |

### 5.10 Type basics (Phase T)

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Align left / centre / right | ragged lines | 🔴 | `text-align-start/center/end` | S4 |
| Justify with last line left / centre / right | full lines + short last | 🔴 (3) — one row | draw | S4 |
| Justify all | full lines | 🔴 | `text-align-justify` | S4 |
| Paragraph direction LTR / RTL (Varos moat) | ¶ with arrow | 🔴 | `pilcrow` + arrow (draw) | S4 |
| Font size | small/large A | 🔴 | `a-large-small` | S4 |
| Leading | lines + vertical arrows | 🔴 | draw | S4 |
| Kerning | "VA" + arrows | 🔴 | draw | S4 |
| Tracking | spaced letters + arrows | 🔴 | draw | S4 |
| Baseline shift | letter above a baseline | 🔴 | `baseline` (base) | S4 |
| Horizontal / vertical scale | "T" + arrows | ⬜ | draw | — |
| Character rotation | rotated "T" | ⬜ | draw | — |
| All caps / small caps / super / sub / underline / strike | glyph set | ⬜ | `superscript`, `subscript`, `underline`, `strikethrough`, `case-sensitive` | — |
| Kashida / harakat controls (Arabic) | Arabic-specific; no Illustrator convention — ours to invent | ⬜ (post-v1 Arabic phase) | draw | — |

### 5.11 Color modes & swatches

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Eyedropper in picker | pipette | ✅ | `pipette` | — |
| RGB / HSB / CMYK / Grayscale mode switch | text in AI | ⬜ (we are RGB only, `ui.rs:4659`) | — | — |
| Out-of-gamut warning | triangle + ! | ⬜ | `triangle-alert` | — |
| Add swatch / delete swatch | swatch + "+" / trash | ⬜ (Swatches panel not built) | `swatch-book` + badge | — |
| Global / spot swatch marker | corner triangle / dot | ⬜ | — (painted) | — |
| Harmony rules (8) | dots on a ring per rule | 🔴 (text today, T33) — one row | draw | S4 |

### 5.12 Appearance & Effects (post-v1 per `MASTER_PLAN_V1_LAUNCH.md` §5.5)

| Control | Conventional glyph | Status | Lucide base | Stage |
|---|---|---|---|---|
| Add new fill | filled square + "+" | ⬜ | draw | — |
| Add new stroke | ring square + "+" | ⬜ | draw | — |
| Add effect (fx) | "fx" | ⬜ | draw | — |
| Clear appearance | circle-slash | ⬜ | `ban` | — |
| Duplicate item | two squares | ⬜ | `copy` | — |
| Delete item | trash | ⬜ | `trash-2` | — |
| Opacity / blend mode | half-filled circle / overlapping circles | ✅ (opacity) — counted once | `contrast` / `blend` | — |
| Graphic styles | swatch of a styled square | ⬜ | draw | — |

### 5.13 Totals

| Group | Rows | ✅ have | 🔴 v1 missing | ⬜ post-v1 |
|---|---|---|---|---|
| 5.1 Tools | 90 | 11 | 15 | 64 |
| 5.2 Paint cluster | 7 | 3 | 3 | 1 |
| 5.3 Pathfinder | 10 | 4 | 1 | 5 |
| 5.4 Align & Distribute | 17 | 8 | 5 | 4 |
| 5.5 Transform | 10 | 5 | 0 | 5 |
| 5.6 Arrange & Layers | 19 | 6 | 7 | 6 |
| 5.7 Artboard | 17 | 2 | 9 | 6 |
| 5.8 View | 15 | 2 | 4 | 9 |
| 5.9 Stroke | 16 | 1 | 10 | 5 |
| 5.10 Type | 13 | 0 | 9 | 4 |
| 5.11 Color | 6 | 1 | 1 | 4 |
| 5.12 Appearance | 8 | 1 | 0 | 7 |
| **All rows** | **228** | **44** | **64** | **120** |
| **Illustrator vocabulary only** (minus 6 Varos-specific rows: Triangle, Clip to page, Transparent page, Page colour, harmony diagrams, Kashida/harakat) | **222** | **43** | **60** | **119** |

A row is one control; a few rows stand for a small family on one control (e.g. "Justify with last line left/centre/right", "Portrait / Landscape"). Counted by script over the tables above (§9).

---

## 6. Legal + source strategy

### 6.1 What we may use

- **Lucide** — ISC licence ("Copyright (c) 2026 Lucide Icons and Contributors"); icons derived from **Feather** carry an extra MIT notice ("Copyright (c) 2013-present Cole Bemis"). Verified from the package's own `LICENSE` in the locally cached `lucide@1.8.0` (§9). 1,695 icons. ISC/MIT only require the notice to travel with the copies.
- **Our own drawings** — made with the cursor method (§6.3).
- The app already uses several Feather-derived Lucide icons: `search`, `x`, `plus`, `lock`, `trash-2` (`ui.rs:42–43`, `72–73`, `37`).

**Gap found (not fixed here — docs-only):** `varos/THIRD_PARTY_NOTICES.md:16–21` names Lucide and "ISC" but carries **no copyright line, no licence text, and no Feather MIT notice**. ISC's one condition is that the notice appears in all copies. Recommended as its own small task.

### 6.2 What we may not use

- **SF Symbols** — Apple licenses them for Apple platforms only; our icon pain cannot be solved by borrowing Compositor's glyphs (`docs/studies/2026-09-23-COMPOSITOR_UI_STUDY.md:16`, `:32`).
- **Adobe glyphs** — Illustrator's tool, panel and cursor artwork. The local Adobe dump is functional reference only and gitignored (`.gitignore:22` `**/cursors-ai/`, CLAUDE.md "Never commit proprietary reference material"). Even if some Adobe icon package carries an open licence (not verified here), Varos does not use it: an Illustrator alternative wearing Adobe's pictures is a trade-dress risk, and the cursor study already set the rule "conventions and measurements only" (`CURSOR_SET_V1.md` §5).
- **Font Awesome** — legal with attribution (CC BY 4.0; used today for the pen-nib cursor fallback, `THIRD_PARTY_NOTICES.md:5–13`), but a second family breaks the one-family rule. Not for UI icons.

### 6.3 How original glyphs are drawn (the cursor method, reused)

1. Take only the **convention** (a nib means Pen, mirrored triangles mean Reflect, a "+" badge means "add") and, where useful, a Lucide glyph as the base shape.
2. Write the SVG from primitives with coordinates chosen in the session. Never open, trace, auto-convert or "lightly modify" Adobe path data.
3. Each file carries a `<desc>` stating its origin (Lucide base name + changes, or "original").
4. Independent check before merge (Codex compared the cursor round-1 files against all 326 Adobe SVGs, `CURSOR_SET_V1.md` §5). For icons there is no local Adobe icon dump to compare against, so the check is visual + method review.

### 6.4 Coverage — which v1 glyphs Lucide already has

- **Lucide base exists (v1 🔴 items):** `mouse-pointer-2` (filled Selection), `type`, `slash`, `eraser`, `scissors`, `slice`, `hand`, `zoom-in`, `zoom-out`, `ruler`, `grid-3x3`, `layers-plus`, `ungroup`, `bring-to-front`, `send-to-back`, `chevron-down/left/right`, `copy`, `trash-2`, `align-*-space-between`, `text-align-start/center/end/justify`, `a-large-small`, `baseline`, `squares-unite/subtract/intersect/exclude` (outline style; we keep the filled convention).
- **Must be drawn original:** Add/Delete/Convert anchor (pen + badge), Reflect/Flip (mirrored triangles), Area Type, Type on a Path, Shape Builder, Gradient tool, Color/Gradient paint-mode swatches, None, Divide, Align-To ×3, Bring Forward / Send Backward, Clip mask make/release, New artboard, Move art with artboard, Clip to page, Transparent page, Page colour, Guides, stroke caps ×3 / joins ×3 / align ×3 / dashed, justify-last-line ×3, RTL/LTR direction, leading, kerning, tracking, harmony diagrams ×8.

### 6.5 One visual family — the icon grammar (proposal)

The icons must read as siblings of the approved cursors (`CURSOR_SET_V1.md` §3: 1.5 px ink, round joins/caps, 0.75 corner radius at the ink edge, no shadows, one badge slot).

1. **Master grid:** every icon authored once on a **24×24** box with a **2 px safe margin** (live area 20×20). Keylines: square 18×18, circle Ø20, so squares and circles read the same size (Lucide's own keylines).
2. **Display sizes:** three only — **13 (inline: tab close, search, field labels), 16 (default: rail, bars, panels), 20 (top bar, dialogs)**. Every size lives in `tokens.rs` (§7 note).
3. **Stroke:** **1.5 logical px on screen at every size** ("absolute stroke", like Lucide's `absoluteStrokeWidth` option). In the 24-unit master the loader sets stroke-width = 1.5 × 24 / display size: **2.77 @ 13 px, 2.25 @ 16 px, 1.8 @ 20 px** — never hand-typed per icon. Today it is 2.0 units = 1.33 px @ 16 px (`ui.rs:1376`). *Decision needed (§8 D1):* 1.5 px matches the cursors and answers A16 "weak"; the law mockup is lighter (~1.07 px); the old polish plan warned against global thickening (`VISUAL_POLISH_PLAN.md` A16.4). So S1 shows 1.33 vs 1.5 side by side before anything is locked.
4. **Caps, joins, corners:** round caps and joins; container shapes `rx = 2` on the 24 grid; small bars `rx ≤ 1`.
5. **Solid vs outline (A16's lesson):** under 20 px, **regions and bars are filled, lines are stroked**. Filled where the convention is solid: Selection arrow, align/distribute bars, pathfinder result regions, the Color paint-mode swatch. Outline everywhere else.
6. **One weight family, one documented exception:** no 1.0 / 1.3 / 1.4 / 1.6 / 1.7 strokes. Exceptions must depict weight itself (`IC_STROKEW`) or follow an OS convention (the 1 px Win11 window caps, `ui.rs:2938–2939`).
7. **Badges:** modifier marks (+ − ○ ✱ / ^ ⊘) sit in **one bottom-right slot**, same as the cursors (`CURSOR_SET_V1.md` §3 "One badge slot"). Add Anchor = pen + "+", New Layer = layers + "+", New Artboard = page + "+".
8. **One ink:** icons are white masters tinted at draw time (`MUTED` rest → `TEXT`/`WHITE` hover → white on `ACCENT` fill when active). Azure never appears *inside* a glyph (rule 4). The only coloured marks are the red None slash (`NONE_RED`) and live swatches. (Cursors are two-ink because they sit on artwork; icons sit on the warm-black chrome, so one ink is enough.)
9. **Pixel fit:** rasterise at `display_px × pixels_per_point` (fixes W3), keep straight edges on whole device pixels at 1× and 2×.
10. **One concept → one glyph** (the icon side of ONE-HOME): the bar mirror and the home draw the same icon (fixes W7). Rotate tool and rotation field may share a glyph; opacity may not be "Op" in one place and a glyph in another.

---

## 7. Plan — four hand-testable stages

**Note on `tokens.rs` (single-home rule).** Icon sizes, the on-screen stroke weight and the raster rule become tokens in `varos/crates/varos-app/src/shell/tokens.rs` (e.g. `ICON_SM = 13.0`, `ICON_MD = 16.0`, `ICON_LG = 20.0`, `ICON_STROKE_PX = 1.5`). No widget may type a raw icon size again; the six widgets in §2.4 read the token. Icon *path data* is not a token — it lives in one icon module (below). This closes the last raw-number gap listed in §2.3.

### S1 — Fix A16: weight and clarity of what exists · effort **M**

1. **One icon home.** A single `Icon` enum + one SVG file per icon under `varos/crates/varos-app/assets/icons/v1/` loaded with `include_str!` (the same plan the cursor set uses, `CURSOR_SET_V1.md` §6 step 2), one cache keyed by (icon, size, scale factor). Adding an icon becomes: one SVG file + one enum line. A headless test (resvg is CPU-only, so this obeys "no test builds a GPU `Renderer`") checks every icon parses, rasterises, and uses only the family stroke width.
2. **Rasterise per scale factor** (W3).
3. **Absolute stroke** in the loader, with a 1.33 px vs 1.5 px A/B screen for Ahmed (D1).
4. **Selection solid, Direct hollow** (W1).
5. **Pathfinder into the SVG family** — same filled convention, one glyph at two sizes instead of two painted geometries (W4, D2).
6. **Flip H/V redrawn** as mirrored triangles across an axis (W5).
7. **Refresh the four old-Lucide paths** to the pinned version (W6), record the version in `THIRD_PARTY_NOTICES.md` (plus the missing licence text, §6.1).
8. **Replace the 3 text characters** (× ☰ ▾) with drawn icons (W8) and bring the painted glyph strokes (✕ 1.3, chevrons 1.7, ✓ 1.6) to the family weight.

*Hand test:* the rail, Align, Pathfinder, control bar and Layers side by side at 100 %, 150 % (Windows) and 200 % (Mac) — screenshots before/after; V vs A readable at a glance.

### S2 — Replace text with icons where Illustrator has a standard glyph · effort **M**

The 22 "→ icon" spots and the 5 "+ icon" spots of §4, in three hand-test batches:
- **Batch a — Artboard chrome (A28):** T1–T5, T19–T25, T27. Control bar in Artboard mode shows clip, move-art, page colour, orientation, new, delete as icons; the Artboard panel footer becomes icons.
- **Batch b — Properties & Document:** T6, T8, T10–T14 (Snapping / Guides / Rulers / Grid as Illustrator-style icon toggles), T31, T35.
- **Batch c — Align & structure:** T17 (Align To ×3), T18 (drop caption), T37 (New Layer), T38 (Arrange ×4 + Ungroup — needs a home decision: Properties "Arrange" row vs Layers footer), T7.

Every icon keeps its name as a tooltip. ~20 new glyphs (the rest reuse S1 glyphs).

*Hand test:* each batch in the real window; Ahmed checks every former text control is still findable by hover.

### S3 — Full glyph set for the tools we already have · effort **S**

- **Convert** gets its first rail icon (tool exists without one, `editor.rs:41`).
- **Pen flyout**: Pen · Add Anchor · Delete Anchor · Convert, using the shape-slot flyout pattern (`ui.rs:3906–3955`) and the badge slot — matching the pen cursors one-to-one.
- **Rotate / Scale / Artboard** redrawn to their conventions (curved arrow round a shape; small + large square).
- Review Triangle / Polygon / Ellipse / Rectangle for keyline consistency.

*Hand test:* rail + flyouts at 100 % / 200 %; each rail icon matches its cursor.

### S4 — Grow the library with each upcoming system · effort **L** (each slice S–M)

Icons land **with** their system (catalog rule: "always ship a tool WITH its driving panel", `ILLUSTRATOR_TOOLS_CATALOG.md` Key relationships). Design can come later, but each slice is in the plan now:
- **Stroke home:** caps ×3, joins ×3, align ×3, dashed toggle (then arrowheads, profiles).
- **Masks** (next product step): make/release clipping mask.
- **Gradients:** Gradient tool, Color/Gradient/None paint-mode trio, linear/radial type switches.
- **Text (Phase T):** Type, Area Type, Type on a Path, paragraph align ×7, LTR/RTL, size/leading/kerning/tracking/baseline — and Arabic-specific glyphs (kashida, harakat) that no one has conventions for yet: ours to invent.
- **Navigation & drawing:** Hand, Zoom in/out, Line Segment, Shape Builder, Scissors, Knife, Eraser, Reflect tool, Divide, Distribute spacing ×2.
- **Colour:** 8 harmony diagrams for the picker (T33, pairs with the A17 redesign).

*Hand test:* per slice, inside that system's own gate.

---

## 8. Decisions for Ahmed

| # | Question | Options | Recommendation |
|---|---|---|---|
| D1 | On-screen icon stroke weight | 1.5 px (cursor family) · 1.33 px (today) · ~1.07 px (law mockup) | 1.5 px, after the S1 side-by-side |
| D2 | Pathfinder glyph style | keep hand-painted · move into the SVG family (same filled look) | move into the family |
| D3 | Icon sizes | 13 / 16 / 20 · 12 / 16 / 20 · 16 / 20 / 24 | 13 / 16 / 20 (keeps today's rail at 16, Ahmed 07-07 "the toolbar is huge") |
| D4 | Group glyph | folder (today) · Lucide `group` | `group` — a folder reads as "file", not "group" |
| D5 | Home for Arrange + Ungroup (T38) | Properties "Arrange" row · Layers footer · both (one home + a mirror) | Properties home, Layers mirror |
| D6 | Icons in text menus | none (Illustrator) · leading icons | none |

---

## 9. Evidence and method

- **Code facts** from reading `ui.rs`, `boxtree.rs`, `registry.rs`, `tokens.rs`, `cursors.rs`, `main.rs`, `editor.rs`, `boolean.rs`, `command.rs` at `bb80641`; counts by `grep -c '^const IC_'` (39) and a manual walk of every `load_icon*` call and painter glyph.
- **Lucide facts** from the locally cached package `~/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/lucide` (version 1.8.0, `"license": "ISC"` in `package.json`, `LICENSE` read in full): 1,695 icon modules in `dist/esm/icons/`; every Lucide name in this doc was checked against that file list; the source of each current `IC_*` glyph was found by searching its path string in those files.
- **Illustrator facts** from the two reference catalogs + general knowledge; items marked (?) are uncertain. No Adobe file was opened for this study.
- **Counting.** §5.13 was produced by a short script that reads every §5 table row and takes the first status mark in its Status column (228 rows: 44 ✅ · 64 🔴 · 120 ⬜). The Illustrator-only line subtracts the 6 Varos-specific rows by hand. §3 counts (38 / 20 / 3) and §4 counts (22 / 5 / 10 / 1) are hand tallies of the tables in those sections.
