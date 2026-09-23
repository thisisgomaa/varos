> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Study: Varos cursor set v1 — original tool cursors vs Illustrator's conventions

- **Date:** 2026-09-23 (round 1: the first 10 cursors; round 2, same day: the full set covering every `CK` state).
- **Author:** Claude (design session). **Decision owner:** Ahmed. After round 1, the coordinator relayed that Ahmed approved the round-1 drawings as the Varos cursor style. Round 2 finishes the set in the same style. The set is still **not wired**: the code is unchanged.
- **Authority:** none. Level-5 document under `docs/foundation/FOUNDATION_CHARTER.md` §3. If anything here conflicts with `docs/UI_DIRECTION.md`, the law wins.
- **Deliverables (committed):** `varos/crates/varos-app/assets/cursors/v1/` holds **30 SVGs** and `hotspots.json`. `hotspots.json` maps all 28 `CK` variants plus 3 proposed ones to a file and a hotspot.
- **Review page (NOT committed):** `varos/target/cursors-review/gallery.html`. It sits under `target/`, which is gitignored, because it holds copies of Adobe's cursors for side-by-side viewing.
- **Code:** `cursors.rs` and `main.rs` are untouched. §6 lists what changes once the set is wired.

---

## 1. الملخص (بالمصري)

1. الطقم كله بقى **جاهز على الورق**: ٣٠ مؤشر بتغطي كل حالة في البرنامج (الـ٢٨ حالة اللي في الكود، وزيادة عليهم الزووم + والزووم − وأداة الآرتبورد).
2. **نفس الروح بتاعة أول ١٠** اللي عجبوك: خط ١.٥، أسود دافي على أبيض، إطار أبيض ١ بكسل، زوايا ناعمة، من غير ضل ومن غير أزرق.
3. **العلامات الصغيرة (+ − ○ ✱ / ^ ⊘ #) ليها مكان واحد ومقاس واحد** تحت على اليمين في كل المؤشرات.
4. الإيد المقفولة (وانت بتسحب الشاشة) ليها **نفس كف ونفس نقطة ضغط** الإيد المفتوحة، فمش هتلاقي المؤشر بينط لما تدوس.
5. أسهم التدوير الـ٨ كلها **رسمة واحدة** متلفّة كل مرة ٤٥ درجة. وأسهم التكبير الـ٤ كلها برضه رسمة واحدة متلفّة.
6. **"التحريك"** بيستخدم السهم الأسود العادي، زي Illustrator بالظبط. و**"النسخ"** سهم أسود جنبه +، زي ما طلبت.
7. صلّحنا ملاحظة Codex: **كل الخطوط بقت ١.٥ بالظبط**. شق سن البن كان ١.٢٥ وبقى ١.٥، ويد الزووم كانت ٢.٥ وبقت ١.٥.
8. **ولا خط اتنقل من Adobe.** اتفحصت أول ١٠ رسومات قصاد كل ملفات Adobe وطلعت أصلية.
9. افتح صفحة المقارنة: `open varos/target/cursors-review/gallery.html`. أول الصفحة فيه صف بالعيلة كلها، وبعده كل مؤشر جنب اللي بيقابله عند Adobe.
10. لسه **متركّبش في البرنامج**. الخطوة الجاية تركيبه (§6)، وعلى الماك محتاج خطوة زيادة عشان Retina.

---

## 2. What was measured, and from where

- **Reference.** This is the local, gitignored Adobe Illustrator 2026 dump, used only to **measure** canvas size, hotspot, glyph extent, badge position and stroke weights.
  - Round 1 used the external-disk copy at `/Volumes/My Disk/AI workspace/VAROS/Cursors/`, which includes `_hotspot_map.txt`.
  - That disk was unmounted during round 2. So round 2 used the identical local copy in the main checkout at `varos/crates/varos-app/assets/cursors-ai/svg/` (326 SVGs, gitignored by `assets/cursors-ai/` / `**/cursors-ai/`). Its hotspots came from the round-1 reading of `_hotspot_map.txt` and from `cursors.rs` `ai_svg()`.
  - Adobe files were only rasterised in a session scratch folder and looked at. No Adobe path data was opened in an editor, copied, traced or imported.
- **Adobe's format.** Each file is a 64×64-viewBox SVG (@2x of a 32px cursor), with its hotspot in 1× (32px) space. The glyph is black ink (≈1px at 1×) on a white keyline (≈1px, 65% opacity).
- **Varos's format** is the one `cursors.rs` already consumes (`ai_svg()` + `hcursor_svg_file()`): the SVG is rendered at `CURSOR_PX`, and the hotspot is in 32px-logical space, scaled by `CURSOR_PX/32`. Our files use a **32×32 viewBox**, so no conversion is needed.
- **Sizes** are the opaque extent in 32px space, including the keyline. They were measured with resvg 0.45.1 at 256px and divided by 8.

## 3. House rules (the family, stated once)

- **Two inks only:** warm black `#141313` (UI_DIRECTION rule 7) and white `#ffffff`.
  - Azure `#0c8ce9` is never used in a cursor (rule 4). It appears in the review page only as the hotspot marker.
  - No shadows (rule 2).
- **Ink weights — one rule, no silent exceptions.** Every stroked ink line in every file is **1.5px** (3 device px at 2×). After the round-2 review fix, the files contain exactly these `stroke-width` values:
  - **1.5** — all ink strokes: outlines, badge lines, crosshair arms, chevrons, the arc, the pen slit and the zoom handle. The slit was 1.25 and the handle 2.5 in round 1; both were normalised in round 2.
  - **3.5** — the white keyline under a 1.5 ink stroke (1.5 + 1px on each side).
  - **2, 3, 5, 7** — used only in `hand.svg` and `grab.svg`, as a **layered-outline construction**:
    - The silhouette is a union of capsules (fingers, thumb) and a rounded palm. It is painted in three passes:
      - (a) white keyline: capsules stroked 7 = 2 + 2×1.5 + 2, palm stroked 5;
      - (b) warm-black: capsules stroked 5 = 2 + 2×1.5, palm filled and stroked 3 = 2×1.5;
      - (c) white interior: capsules stroked 2, palm filled.
    - Pass (c) hides every inner edge. What stays **visible is a 1.5px ink border and a 1px keyline**, the same as everywhere else. The 1.5px dark gaps between fingers fall out of the 1.5 spacing.
    - So the visible weight rule holds. Those numbers are construction widths, not different line weights.
- **Filled marks (not strokes):**
  - the pen's vent dot (circle r=1.35);
  - the crosshair centre dot (1.5×1.5 square);
  - the solid pen band, eyedropper collar and eyedropper bulb. These are filled rounded rects that also carry the 1.5 edge.
- **Corners:**
  - Round joins everywhere, and round caps on every free line end (0.75 radius at the ink edge).
  - **Cap exception — butt caps on bars that sit on a white field.** Straight bars drawn over a white rounded-rect keyline, or inside the white lens, use `stroke-linecap="butt"`, so the bar ends stop square: the `+` badge in `pen-add` and `copy`; the `−` badge in `pen-delete`; the crosshair arms in `shape-rect` and `artboard`; the `#` badge bars in `artboard`; and the `+` / `−` inside the lens of `zoom-in` / `zoom-out`.
  - Keylines on straight bars use 1px corners. The rect badge uses 1.25, and bands use 0.75.
  - These are the 3px-control / 8px-box law scaled down to a 32px glyph.
- **Grid:**
  - Coordinates are authored on a **0.25px grid** (quarter and half values are common, e.g. `resize-h` uses 2.75 / 5.5 / 7.25 / 12.75 / 14.5 / 17.25), and every file is then shifted by +0.25.
  - Straight horizontal/vertical ink bars (in the un-rotated files) are centred on whole or half coordinates. With the 1.5 ink, the 3.5 keyline and the +0.25 shift, their edges land on whole device pixels at 2×. Quarter values are used for line ends, diagonals, arcs and keyline rects, where that rule does not apply.
  - The hotspot's geometric point sits at (hx+.25, hy+.25), the centre of the hotspot device pixel at 2×.
  - **Trade-off:** at 1× (a 32px bitmap) those edges fall on half pixels, so straight bars are about 1px soft. See §7.
- **One badge slot:**
  - Every badge shares one anchor: centred at (21.25, 21.25), bottom-right, on the tool's diagonal, with its ink at least 5px clear of the main glyph.
  - Optical size is 7.5–9px per glyph, not one fixed size. Measured ink boxes:
    - 7.5×7.5 — `+` (pen-add, copy), ring (pen-close), rect (shape-rect), `#` (artboard);
    - 7.5×1.5 — `−` (pen-delete);
    - 7.56×8.5 — asterisk (pen-new);
    - 8×8 — slash (pen-connect) and ⊘ (no-drop);
    - 9×5.25 — caret (convert).
  - The same slot is used on the pen, the arrow and the crosshair: `+ − ○ ✱ / ^ ⊘ □ #`.
  - Tips and centres stay the hotspot.
- **Derived glyphs are exact turns of one drawing.**
  - `rotate-*` = `rotate-ne` turned about its hotspot (11,11) by 45° steps.
  - `resize-v/nw/ne` = `resize-h` turned about (10,10) by 90°/45°/−45°.
  - Each file carries its turn as a plain `rotate(a hx hy)` transform. Every turned glyph fits the canvas without shifting, so its hotspot stays unchanged.

## 4. The set

### 4.1 Round 1 — the ten (approved style)

| # | Varos file (hotspot) | CK | Adobe convention (measured) | What we keep | What we change (makes it ours) |
|---|---|---|---|---|---|
| 1 | `select.svg` (2,2) | `Select`, `Move` | Solid black arrow, 13.6×18.8, hotspot 1,1 at the tip, vertical left edge, flat shelf at the back | Solid = Selection; tip = hotspot; vertical left edge | Symmetric V-notch back (edges at 90°/45°, both 17 long); warm-black body; solid 1px keyline; soft joins. 15.5×20.5 |
| 2 | `direct.svg` (2,2) | `Direct` | Same arrow, hollow | Hollow = Direct Selection | Our silhouette; 1.5 warm-black edge. 15.5×20.5 |
| 3 | `pen.svg` (2,2) | `Pen` | Solid black nib, tip up-left, 13.2×20.9, slit, vent hole, back band | Nib = Pen; tip = hotspot; slit + vent; band behind | Drawn in **outline** like our rail icons; 45° axis; convex flanks; solid band 1px behind. 17.2×17.2 |
| 4 | `pen-add.svg` (2,2) | `PenAdd` | Pen + "+" (centre ≈17.5,20, arms ≈6.5); 23×25 | "+" bottom-right | Shared badge slot; 7.5 arms; bars on whole device pixels at 2×. 25.5×25.5 |
| 5 | `pen-delete.svg` (2,2) | `PenDel` | Pen + "−"; 23×22 | "−" | Shared slot. 25.5×22.5 |
| 6 | `pen-close.svg` (2,2) | `PenClose` | Pen + ring; 22×25 | Ring = close path | Ring r=3. 25.5×25.5 |
| 7 | `shape-rect.svg` (11,11) | `Cross` | Crosshair, open centre + dot: `CUR_PRECISE` 19×19 @9,9; `CUR_CROSSHAIRS` 25×25 @12,12 | Crosshair; open centre; centre dot | Adds a small rounded-square badge so the cursor names the tool. 24.5×24.5 |
| 8 | `rotate-ne.svg` (11,11) (was `rotate.svg`) | `RotateNE` | Curved two-headed arrow, filled heads, 15×15 @7,7, 8 files | Arc convex toward the corner; hotspot at glyph centre | Open chevrons; 10px-radius quarter arc; the base glyph of the 8. 18.2×18.2 |
| 9 | `hand.svg` (11,12) | `Hand` | Open white hand, 17×18 @11,11 | Open white hand; central hotspot | Capsule fingers + rounded palm (layered construction, §3). 20.8×22.5 |
| 10 | `zoom-in.svg` (9,9) | *proposed* `ZoomIn` | Magnifier, "+" inside, 18×18 @8,8 | Lens centre = hotspot | 1.5 ring **and 1.5 handle** (round 2 fix); same "+" bars as the badge. 20.5×20.5 |

### 4.2 Round 2 — completing every CK state

Scope was every `CK` variant in `cursors.rs` (lines 10–42; `ALL_CURSORS` = 28) that had no v1 drawing, plus the three states Ahmed named that have no `CK` yet (zoom-out, artboard; zoom-in was drawn in round 1). The coordinator's list missed **`PenNew` and `PenConnect`**, but both are real `CK` states, so both are drawn.

| Varos file (hotspot) | CK | Adobe convention (measured) | What we keep | What we change |
|---|---|---|---|---|
| `pen-new.svg` (2,2) | `PenNew` | `CUR_PENNEW`: pen + asterisk badge; 22.6×25.1 @1,1 | Asterisk = "new path" | Six-spoke asterisk of three 1.5 lines, radius 3.5, in the shared slot. 25.6×26.0 |
| `pen-connect.svg` (2,2) | `PenConnect` | `CUR_PENCONTINUE`: pen + slanted "/" bar; 23.9×25 @1,1 | "/" = continue an open path | Round-capped 1.5 slash at 45°, 9.2 long. 25.8×25.8 |
| `convert.svg` (2,2) | `Convert` | `CUR_PENCORNER`: pen + thin lopsided caret; 21.2×24 @1,1 | Caret "^" = convert anchor, **on the pen** (not a bare caret like today's built-in) | Symmetric 90° caret. 26.2×24.2 |
| *(reuses `select.svg`)* (2,2) | `Move` | `CUR_MOVE`: the same plain black arrow as Select | Illustrator shows the plain arrow while moving | No separate file: a copy would be byte-identical to `select.svg` |
| `copy.svg` (2,2) | `Copy` | `CUR_MOVECOPY`: black arrow over an offset white arrow (double arrow); 15.6×16.8 @1,1 | Arrow tip = hotspot | Per the brief: solid arrow + the shared "+" badge (the OS "copy" convention). **Open question §7:** Illustrator users know the double arrow. 25.5×25.5 |
| `no-drop.svg` (2,2) | `NoDrop` | `CUR_NOMOVE`: arrow + ⊘ badge; 20.8×26.2 @1,1 | Arrow + "no" circle | Ring r=3.25 + diagonal bar, both 1.5, in the shared slot. 25.8×25.8 |
| `grab.svg` (11,12) | `Grab` | `CUR_FIST`: small closed hand 11.9×11.7 @11,11 | Closed hand while panning | **Same palm, construction and hotspot as `hand.svg`**, so Space-hand → drag never jumps; fingers fold to a knuckle row; thumb tucked. 18.5×17.5 |
| `eyedropper.svg` (3,20) | `Eye` | `CUR_EYEDROPPER`: pipette at 45°, dark bulb/collar, light tube; 18×18 @2,17 | Pipette at 45°, tip bottom-left = hotspot | Hollow pointed tube → 1px gap → solid collar → 1px gap → solid capsule bulb (the pen band's gap rule). 20.4×20.4 |
| `resize-h.svg` (10,10) | `ResizeH` | `CUR_SCALEHORIZONTAL`: filled heads + a centre tick; 19×9 @9,9 | Two-headed straight arrow; centre hotspot | Open chevron heads (same 2.75 arms as rotate); no centre tick. 18.0×9.0 |
| `resize-v.svg` (10,10) | `ResizeV` | `CUR_SCALEVERTICAL` @9,9 | Vertical two-headed arrow | `resize-h` turned 90°. 9.0×18.0 |
| `resize-nw.svg` (10,10) | `ResizeNW` (↖↘) | `CUR_SCALETLBR`: 15×15 @9,9 | Diagonal two-headed arrow | `resize-h` turned 45°. 13.8×13.8 |
| `resize-ne.svg` (10,10) | `ResizeNE` (↗↙) | `CUR_SCALETRBL` @9,9 | Diagonal two-headed arrow | `resize-h` turned −45°. 13.8×13.8 |
| `rotate-e.svg` (11,11) | `RotateE` | `CUR_ROTATEFROMRIGHT`: 7.2×15 @7,7 | Arc convex to the right | `rotate-ne` turned 45°. 8.0×20.5 |
| `rotate-se.svg` (11,11) | `RotateSE` | `CUR_ROTATEBOTTOMRIGHTCORNER` @7,7 | Convex to bottom-right | Turned 90°. 18.2×18.2 |
| `rotate-s.svg` (11,11) | `RotateS` | `CUR_ROTATEFROMBOTTOM` @7,7 | Convex downward | Turned 135°. 20.5×8.0 |
| `rotate-sw.svg` (11,11) | `RotateSW` | `CUR_ROTATEBOTTOMLEFTCORNER` @7,7 | Convex to bottom-left | Turned 180°. 18.2×18.2 |
| `rotate-w.svg` (11,11) | `RotateW` | `CUR_ROTATEFROMLEFT` @7,7 | Convex to the left | Turned 225°. 8.0×20.5 |
| `rotate-nw.svg` (11,11) | `RotateNW` | `CUR_ROTATETOPLEFTCORNER` @7,7 | Convex to top-left | Turned 270°. 18.2×18.2 |
| `rotate-n.svg` (11,11) | `RotateN` | `CUR_ROTATEFROMTOP` @7,7 | Convex upward | Turned 315°. 20.5×8.0 |
| `artboard.svg` (11,11) | *proposed* `Artboard` | `CUR_ARTBOARD`: crosshair + a large page corner; 20×20 @8,8 | Crosshair = "drag out a new board" | Our shape crosshair + a mini `#` frame badge (echoes our Lucide "frame" artboard rail icon). 24.5×24.5 |
| `zoom-out.svg` (9,9) | *proposed* `ZoomOut` | `CUR_ZOOMOUT` @8,8 | Magnifier with "−" | `zoom-in` without the vertical bar. 20.5×20.5 |

**Direction check for the derived rotates.** `rotate_ck()` in `main.rs` (lines 127–150) numbers directions by screen angle: E=0, SE=45 … NE=315. The base glyph is NE, so each file is NE turned by (dir − 315) mod 360. That gives E +45, SE +90, S +135, SW +180, W +225, NW +270 and N +315. Each result was checked visually: every arc bulges toward its named side, with the hotspot on the concave side.

## 5. The legal rule (one paragraph)

Illustrator's cursors are Adobe's copyrighted artwork. The local dump exists only as functional reference, and `.gitignore` (`/Cursors/`, `assets/cursors-ai/`, `**/cursors-ai/`) keeps it out of the repository. That protection is load-bearing and must never be weakened.

Both rounds reused **only conventions**, which are not ownable:
- a nib means Pen, a hollow arrow means Direct Selection;
- the `+`/`−`/ring/asterisk/slash/caret badges mean pen states, and ⊘ means not allowed;
- a pipette means sampling, a fist means grabbing;
- two-headed arrows mean resize, and a curved one means rotate;
- the hotspot sits at the tip or the centre.

They also used **only measurements**: canvas size, hotspot, badge position and weight ratio. Every Varos shape is written from geometric primitives with coordinates chosen in these sessions. None of it is copied, traced, auto-converted or "lightly modified" from Adobe path data. Each SVG says so in its `<desc>`, and Codex's review of round 1 confirmed this (see below).

The comparison gallery holds Adobe's files, so it lives only under the gitignored `varos/target/`. That was checked with `git check-ignore -v varos/target/cursors-review/gallery.html` (→ `.gitignore:2:target/`) before each commit.

**Independent check (round 1):** as relayed by the coordinator, Codex compared the 10 round-1 files against all 326 Adobe SVGs (2,039 paths, normalised). It found no identical or near-identical path data and judged every file Original. Its one nit, inconsistent ink weights, is fixed in round 2 (§3). Round 2 was built with the same method.

**Independent check (round 2):** Codex compared the 20 round-2 files against the same 326 Adobe SVGs and judged all 20 Original — **APPROVE WITH NITS**. Both nits were doc-only (badge sizes and caps/grid described inaccurately in §3) and are fixed in §3 above. The set merged to `main` on 2026-09-23.

## 6. How to wire the set (after sign-off; nothing here is done yet)

1. **Table, in `varos/crates/varos-app/src/cursors.rs`:**
   - Add `pub fn varos_svg(ck: CK) -> (&'static str, f32, f32)`, returning (stem, hx, hy) for **all 28 `CK` variants**. It mirrors `ai_svg()` (lines 91–125) but points at `assets/cursors/v1/`, and its values come from `hotspots.json` → `"ck"`.
   - The code consumes **one file per CK** (`ai_svg` maps each `Rotate*`/`Resize*` to its own file, and `hcursor_svg_file` has no rotation parameter). So the 8 rotate and 4 resize files exist as separate files, and no runtime rotation is needed.
   - `CK::Move` → `"select"`.
2. **Loader:**
   - `hcursor_svg_file()` (lines 272–297) reads from the dev-only `assets/cursors-ai/svg/` folder next to `CARGO_MANIFEST_DIR`. The shippable set should be **compiled into the binary** (`include_str!`), so a release build never depends on a source folder.
   - The built-in `svg()`/`FA_NIB`/`PIPETTE` fallbacks (lines 131–189), including the Font Awesome nib, can then be retired. That also removes the Font Awesome NOTICE dependency for cursors.
3. **Priority, in `varos/crates/varos-app/src/main.rs` ~line 604 (the `hcur` map):**
   - Use `varos_svg` first.
   - Ahmed decides whether the local-only `ai_svg` feel-test override survives as an opt-in dev switch or is deleted.
   - The `--dump-cursors` name list (~line 307) should switch to the v1 names.
4. **New CK variants to add when their state exists in the app:**
   - **`CK::ZoomIn` / `CK::ZoomOut`** — there is no zoom tool or zoom state yet. Add them with the Zoom tool (Alt flips in → out).
   - **`CK::Artboard`** — the Artboard tool's empty-board create state uses `CK::Cross` today (`main.rs` `desired_ck`, the `ToolKind::Artboard` arm, and the `AbDrag::Create` early return). Adding it is optional polish: until then, artboard creation shows the shape crosshair.
   - Each new variant must also be added to `ALL_CURSORS` and to the portable `icon()` match (`cursors.rs` lines 705–735).
5. **macOS (the official build today):**
   - The portable path (`cursors.rs` `mod portable`, lines 680–769) maps every `CK` to a winit **built-in** `CursorIcon`, so no custom cursor shows on the Mac yet.
   - winit 0.30.13 offers `CustomCursor::from_rgba(rgba, w, h, hot_x, hot_y)` → `ActiveEventLoop::create_custom_cursor` → `Window::set_cursor`. **But** its macOS backend builds the `NSImage` with size = pixel size (`platform_impl/macos/cursor.rs:54`). A 64px bitmap would therefore show at double size, and a 32px one would be soft on Retina.
   - A crisp Retina cursor needs a 64px bitmap in a 32pt `NSImage`. That means either a small vendored winit patch (see `docs/VENDOR_PATCHES.md`) or building the `NSCursor` directly via `objc2-app-kit`. This is a separate, testable step.
6. **Tests (headless, no GPU):**
   - Every v1 SVG parses with `usvg` and renders non-empty at 32 and 64.
   - Every hotspot lies inside 0..32 and on an opaque-or-adjacent pixel.
   - `varos_svg` covers every entry of `ALL_CURSORS`, and every stem resolves to a file.
   - A lint that the only `stroke-width` values are {1.5, 3.5} plus the hand/grab construction set {2, 3, 5, 7}, so weight drift can't come back silently.

## 7. Open questions for Ahmed

- **Copy cursor:** keep the arrow + "+" (the brief; the OS convention), or switch to Illustrator's black-over-white double arrow, which Illustrator users recognise on Alt-drag? Either is a single small file.
- **Solid or outline pen?** Adobe's nib is solid. Ours is outline, to match the rail icons. This was approved in round 1; it is listed only so it stays a conscious choice.
- **1× crispness vs 2×:** the set is tuned for 2× (Retina, and Windows at 200%). If Windows at 100% matters for v1, we can author a 1×-hinted twin (integer grid, no +0.25 shift).
- **Badge glyphs on crosshairs** (□ rectangle, # artboard): keep them, or use a plain crosshair for every shape tool (closer to Illustrator)?
- **Resize heads:** open chevrons read lighter than Adobe's filled heads. If they feel weak on a busy canvas, filled chevrons are a small change.

## 8. Review gallery

```sh
open varos/target/cursors-review/gallery.html
```

- **Top of the page:** the whole family, all 30 files as real 64px bitmaps, on the dark UI and on a white canvas.
- **Then one row per file** (31 rows: every file, plus `CK::Move` shown as its own row against `CUR_MOVE`). Each row shows:
  - Adobe's counterpart(s) at 2× with its published hotspot;
  - ours at 1×, 2×, and on a white canvas;
  - the **real resvg bitmaps** at 32 and 64px;
  - the 64px bitmap enlarged ×4;
  - ours at 8× on a grid with the hotspot as an azure dot.
- Every one of the 32 Adobe counterparts was found. The page loads with no broken images (checked in a browser).
- **Renderer:** no `resvg`, `rsvg-convert`, ImageMagick or Inkscape CLI exists on this Mac, and Python's `cairosvg` isn't installed. So the PNGs come from a throwaway Rust binary in the session scratch folder, pinned to **resvg 0.45.1** through the app's own `Cargo.lock`. It uses the same crate and the same fit-viewBox-to-N-px method as `cursors.rs`, so the PNGs are what the app would rasterise.
- **Tooling:** the page, the family sheet and the SVG generator are small Python scripts (Pillow is used only for measurement and contact sheets). None of them is committed; the SVGs are the source of truth and can be edited by hand. If Ahmed wants this repeatable in the repo, a `tools/cursors/` builder is a small follow-up, as long as it never writes Adobe files outside `target/`.
