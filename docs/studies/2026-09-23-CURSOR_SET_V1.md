> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Study: Varos cursor set v1 (10 original tool cursors) vs Illustrator's conventions

- **Date:** 2026-09-23
- **Author:** Claude (design session). **Decision owner:** Ahmed. He picks or rejects the look by eye. This document only lays out the proposal and the evidence.
- **Authority:** none. Level-5 document under `docs/foundation/FOUNDATION_CHARTER.md` §3. If anything here conflicts with `docs/UI_DIRECTION.md`, the law wins.
- **Deliverables (committed):** `varos/crates/varos-app/assets/cursors/v1/` has the 10 SVGs and `hotspots.json`.
- **Review page (NOT committed):** `varos/target/cursors-review/gallery.html`. It sits under `target/`, which is gitignored, because it holds copies of Adobe's cursors for side-by-side viewing.
- **Code:** unchanged. `cursors.rs` and `main.rs` were not touched. §5 lists what would change once Ahmed picks a design.

---

## 1. الملخص (بالمصري)

1. رسمنا **١٠ مؤشرات (كيرسرات) بتاعتنا من الصفر**: السهم الأسود، السهم الأبيض، البن، البن +، البن −، البن ○ (قفل المسار)، علامة الرسم (مستطيل)، التدوير، الإيد، والزووم.
2. **نفس لغة Illustrator** عشان إيدك متتلخبطش. نقطة الضغط عند سن القلم أو طرف السهم أو مركز العلامة، والعلامة الصغيرة (+ − ○) تحت على اليمين، والسهم الأسود للتحديد والأبيض للتحديد المباشر.
3. **بس الرسمة بتاعتنا:** خط رفيع زي أيقونات الشريط بتاعنا، أسود دافي `#141313`، وحوالين كل مؤشر إطار أبيض رفيع (١ بكسل) عشان يبان على الكانفس الأبيض وعلى الواجهة الغامقة. مفيش ضل.
4. البن مرسوم **مفرّغ** (جسم أبيض وحرف غامق) وليه حزام غامق من ورا. السهم الأسود بس هو اللي مليان، لأن "مليان مقابل مفرّغ" هو اللي بيفرّق بين الأداتين.
5. **ولا خط اتنقل من Adobe.** كل شكل مبني من دواير وخطوط ومستطيلات بأرقام إحنا اخترناها. من Adobe أخدنا القياسات بس (المقاس ونقطة الضغط ومكان العلامة).
6. **افتح صفحة المقارنة:** `open varos/target/cursors-review/gallery.html`. كل صف فيه Adobe جنب بتاعنا، وبتاعنا بمقاس ١× و٢× وفوق كانفس أبيض، والنقطة الزرقا هي مكان الضغط.
7. **لسه متركّبش في البرنامج.** اختار الشكل الأول، وبعدها نركّبه (الخطوات في §5).
8. **ملاحظة مهمة على الماك:** البرنامج على الماك دلوقتي بيستخدم مؤشرات النظام العادية. عشان مؤشراتنا تبان هناك حادة على شاشة Retina محتاجين خطوة صغيرة زيادة (§5.4).

---

## 2. What was measured, and from where

- **Reference:** the local, gitignored Adobe Illustrator 2026 dump at `/Volumes/My Disk/AI workspace/VAROS/Cursors/` (`_hotspot_map.txt` and `tool-cursors-svg/`). It was rendered to PNG in a session scratch folder only to **measure** it: canvas size, hotspot, glyph extent, badge position and stroke weights. No Adobe path data was opened in an editor, copied, traced or imported into our files.
- **Adobe's format:** each tool cursor is a 64×64-viewBox SVG (@2x of a 32px cursor). The hotspot is given in 1× (32px) space. Every cursor is a black glyph (≈1px at 1×) on a white keyline (≈1px, 65% opacity).
- **Varos's format** is the one `cursors.rs` already consumes (`ai_svg()` + `hcursor_svg_file()`): the SVG is rendered at `CURSOR_PX`, and the hotspot is in 32px-logical space scaled by `CURSOR_PX/32`. Our files use a **32×32 viewBox**, so no conversion is needed.
- **Sizes below** are the opaque extent in 32px space, including the keyline. They were measured with resvg 0.45.1 at 256px, then divided by 8.

## 3. The ten, and why these ten

These are the ten the brief asked for, with no swaps. Together they cover the hand's whole v1 loop: select, edit points, draw paths, draw shapes, rotate and navigate. Every one of them except Zoom already has a `CK` variant, so no new plumbing is needed beyond Zoom.

| # | Varos file (hotspot) | CK today | Adobe convention (measured) | What we keep | What we change (makes it ours) |
|---|---|---|---|---|---|
| 1 | `select.svg` (2,2) | `Select` | Solid black arrow, 13.6×18.8, hotspot 1,1 at the tip, vertical left edge, flat horizontal shelf at the back | Solid = Selection; tip = hotspot; vertical left edge; up-left pose | Symmetric V-notch back (edges at 90° and 45°, both 17 long, notch on the bisector) instead of a shelf; warm-black `#141313` body; solid 1px white keyline; soft 0.75 joins. 15.5×20.5 |
| 2 | `direct.svg` (2,2) | `Direct` | Same arrow, hollow (white body, black edge), hotspot 1,1 | Hollow = Direct Selection; same silhouette as Select | Our own silhouette (as #1); 1.5px warm-black edge. 15.5×20.5 |
| 3 | `pen.svg` (2,2) | `Pen` | Solid black nib, tip up-left, 13.2×20.9, axis ≈25° off vertical; slit and vent hole; band at the back; hotspot 1,1 at the tip | Nib = Pen; tip up-left = hotspot; slit + vent hole; separate band behind the body | **Drawn in outline** (white body, dark edge), like our Lucide rail icons; axis at 45°; convex flanks + flat back; solid dark band 1px behind; the vent is a dark dot. 17.2×17.2 |
| 4 | `pen-add.svg` (2,2) | `PenAdd` | Pen + "+" badge bottom-right (centre ≈17.5,20, arms ≈6.5); 23×25 | "+" badge, bottom-right, tip still the hotspot | Badge centred on the nib's own 45° axis (21.25,21.25); 7.5px arms; 1.5px bars placed on whole device pixels at 2×; 1px white keyline with 1px corners. 25.5×25.5 |
| 5 | `pen-delete.svg` (2,2) | `PenDel` | Pen + "−" badge; 23×22 | "−" badge, same slot | Same slot and weights as #4. 25.5×22.5 |
| 6 | `pen-close.svg` (2,2) | `PenClose` | Pen + small ring badge; 22×25 | Ring = "this click closes the path" | Ring r=3 with the same 1.5 stroke; same slot as #4. 25.5×25.5 |
| 7 | `shape-rect.svg` (11,11) | `Cross` (shape tools and the artboard create state) | Thin crosshair with an open centre and a centre dot: `CUR_PRECISE` 19×19 at hotspot 9,9, and `CUR_CROSSHAIRS` 25×25 at 12,12 (the one `cursors.rs` maps today) | Crosshair; open centre + centre dot; hotspot at the centre | Adds a small rounded-square glyph bottom-right, so the cursor names the tool (the brief's "crosshair + small glyph" convention). Ellipse, polygon and so on can follow in the same slot. Arms 6px, gap 2.75. 24.5×24.5 |
| 8 | `rotate.svg` (11,11) | `RotateNE` (+7 derived) | Curved two-headed arrow near the corner, filled triangular heads, 15×15, hotspot 7,7 (glyph centre); 8 separate files | Arc that is convex toward the corner; two heads; hotspot at the glyph centre; 8 directions | Open Lucide-style chevrons instead of filled heads; 10px-radius quarter arc. **One** base glyph: the other 7 are exact 45° turns about the hotspot (`hotspots.json` → `derive`). 18.2×18.2 |
| 9 | `hand.svg` (11,12) | `Hand` | Open white hand, black edge, 17×18, hotspot 11,11 | Open white hand = pan; hotspot near the centre | Built from four capsule fingers + a thumb capsule + a rounded palm. Even straight fingers with 1.5px dark separators. 20.8×22.5, a bit larger than Adobe's because our edge is 1.5 not 1 |
| 10 | `zoom-in.svg` (9,9) | **none**: needs a new `CK::ZoomIn` | Magnifier with a white lens and a "+" inside, 18×18, hotspot 8,8 at the lens centre | Magnifier; hotspot = lens centre; "+" inside | 1.5 lens ring; a short round-capped handle at 45°; the same "+" bars as the pen badge. 20.5×20.5 |

**House rules for the whole set** (the family, stated once):
- **Two inks only:** warm black `#141313` (UI_DIRECTION rule 7) and white. Azure `#0c8ce9` is never used in a cursor (rule 4: selection/active/focus only). It appears in the review page only as the hotspot marker.
- **Weights:** ink = 1.5px (3 device px at 2×). White keyline = 1px outside the ink on every glyph, fully opaque. No shadows (rule 2).
- **Corners:** round joins (0.75 radius at the ink edge). Keylines on bars use 1px corners, and the rect glyph uses 1.25. These are the 3px-control / 8px-box law scaled down to a 32px glyph.
- **Grid:** the whole cursor is authored on integers and shifted by +0.25. That puts every horizontal/vertical edge on a whole device pixel at 2×, and puts the hotspot's geometric point at (hx+.25, hy+.25), the centre of the hotspot device pixel at 2×. **Trade-off, stated plainly:** at 1× (a 32px bitmap) those edges fall on half pixels, so straight bars look about 1px soft. See §6.
- **Badges** always sit bottom-right on the tool's own axis, clear of the glyph by at least 3px. Tips and centres stay the hotspot.

## 4. The legal rule (one paragraph)

Illustrator's cursors are Adobe's copyrighted artwork. The local dump exists only as functional reference, and `.gitignore` (`/Cursors/`, `assets/cursors-ai/`, `**/cursors-ai/`) keeps it out of the repository. That protection is load-bearing and must never be weakened. For this set we reused **only conventions**, which are not ownable: a nib means Pen, a hollow arrow means Direct Selection, `+`/`−`/ring badges mean add/delete/close, a crosshair means precise drawing, and the hotspot sits at the tip or centre. We also took **only measurements**: canvas size, hotspot, badge position and weight ratio. Every Varos shape is written from geometric primitives with coordinates chosen in this session: lines, arcs, circles, rounded rectangles and capsules. None of it is copied, traced, auto-converted or "lightly modified" from Adobe path data, and each SVG says so in its `<desc>`. The comparison gallery holds Adobe's files, so it lives only under the gitignored `varos/target/`. It was checked with `git check-ignore -v varos/target/cursors-review/gallery.html` (→ `.gitignore:2:target/`) before committing.

## 5. How to wire the set (after Ahmed picks; nothing here is done yet)

1. **Table, in `varos/crates/varos-app/src/cursors.rs`:** add `pub fn varos_svg(ck: CK) -> Option<(&'static str, f32, f32, f32)>`, returning (stem, hx, hy, rotation°). It mirrors `ai_svg()` (lines 91–125) but points at `assets/cursors/v1/`, and its values come from `hotspots.json`. `Rotate*` all return `"rotate"` with 0/45/…/315°. `ResizeH/V/NE/NW`, `Move`, `Grab`, `Copy`, `NoDrop`, `PenNew`, `PenConnect`, `Convert` and `Eye` return `None` until v2 draws them.
2. **Loader:** `hcursor_svg_file()` (lines 272–297) reads from the dev-only `assets/cursors-ai/svg/` folder next to `CARGO_MANIFEST_DIR`. The shippable set should be **compiled into the binary** with `include_str!`, so a release build never depends on a source folder. Rotation = wrap the SVG body in `<g transform="rotate(a hx+.25 hy+.25)">` before `usvg::Tree::from_str`, which keeps one source file.
3. **Priority, in `varos/crates/varos-app/src/main.rs` ~line 604 (the `hcur` map):** `varos_svg` → built-in `svg()`. Ahmed decides whether the local-only `ai_svg` feel-test override stays as an opt-in dev switch or goes away. The dev `--dump-cursors` name list (~line 307) should gain the v1 names.
4. **macOS (the official build today):** the portable path (`cursors.rs` `mod portable`, lines 680–769) maps every `CK` to a winit **built-in** `CursorIcon`, so no custom cursor shows on the Mac at all yet. winit 0.30.13 offers `CustomCursor::from_rgba(rgba, w, h, hot_x, hot_y)` → `ActiveEventLoop::create_custom_cursor` → `Window::set_cursor`. **But** its macOS backend builds the `NSImage` with size = pixel size (`platform_impl/macos/cursor.rs:54`). So a 64px bitmap would show at double size, and a 32px bitmap would be soft on Retina. A crisp Retina cursor needs a 64px bitmap in a 32pt `NSImage`. That means either a small vendored winit patch (see `docs/VENDOR_PATCHES.md`) or building the `NSCursor` directly via `objc2-app-kit`. This is a separate, testable step.
5. **Zoom:** add `CK::ZoomIn` (and later `ZoomOut`, which is the same file with "−") when a Zoom tool or state exists. Today there is none.
6. **Tests (headless, no GPU):** each v1 SVG parses with `usvg`, renders non-empty at 32 and 64, has its hotspot inside 0..32, and every `CK` that `varos_svg` maps resolves to a real file.

## 6. Open questions for Ahmed

- **Solid or outline pen?** Adobe's nib is solid black. Ours is outline, to match our rail icons. If the hand misses the weight, a solid-body variant is a single-attribute change.
- **1× crispness vs 2×:** the set is tuned for 2× (Retina, and Windows at 200%). If Windows at 100% matters for v1, we can author a 1×-hinted twin (integer grid, no +0.25 shift).
- **Rect glyph on the crosshair:** keep it (the cursor names the tool) or use a plain crosshair for all shapes (closer to Illustrator)?
- **Hand size:** ours is about 20% larger than Adobe's. It could shrink a step if it feels big.

## 7. Review gallery

```sh
open varos/target/cursors-review/gallery.html
```

Each row shows: Adobe reference at 2× with its published hotspot | ours 1× | ours 2× | ours on a white canvas | the **real resvg bitmaps** at 32 and 64px | the 64px bitmap enlarged ×4 so pixels are visible | ours at 8× on a grid with the hotspot as an azure dot.

- **Renderer used for the PNGs:** no `resvg`, `rsvg-convert`, ImageMagick or Inkscape CLI exists on this Mac, and Python's `cairosvg` isn't installed. So a throwaway Rust binary was built in the session scratch folder, pinned to **resvg 0.45.1** through the app's own `Cargo.lock`. It uses the same crate and the same fit-viewBox-to-N-px method as `cursors.rs`, so the PNGs are what the app would actually rasterize. The page itself was assembled by a small Python script (Pillow was used only for the measurement sheets). Neither script is committed. The SVGs are the source of truth and can be edited by hand.
- The gallery is regenerated by the same scripts. If Ahmed wants that repeatable in the repo, a `tools/cursors/` builder would be a small follow-up, as long as it never writes Adobe files outside `target/`.
