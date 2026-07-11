> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Illustrator Tools Catalog (reference)

Full analysis of Adobe Illustrator's tools + essential panels, grouped, with Varos build-priority.
**Priority key:** `CORE` = build (the ~20% that does ~80%) · `LATER` = build eventually · `SKIP` = legacy/niche, defer or skip.
**Already shipped:** Pen (P) + Direct Selection / white arrow (A) — the irreducible heart of a vector editor.

---

## 1. Selection tools
- **Selection / black arrow (V)** — `CORE` (shipped) — selects whole objects/groups; bounding box = move/scale/rotate. The default tool. ↔ pairs with A in a modeless flip (the core IL feel).
- **Direct Selection / white arrow (A)** — `CORE` (shipped) — selects/edits anchors, segments, handles inside an object. The universal edit layer under EVERY path tool.
- **Group Selection** — `LATER` — progressively selects object→group→outer. Better delivered via smart double-click than a separate tool.
- **Magic Wand (Y)** — `LATER` — selects objects by shared appearance (same fill/stroke). A "Select > Same" menu covers most of its value first.
- **Lasso (Q)** — `SKIP` — freehand-loop select of anchors; niche (rectangular marquee handles most cases).

## 2. Pen & Path tools
- **Pen (P)** — `CORE` (shipped) — the bezier path tool; click=corner, drag=smooth, close on first anchor.
- **Add Anchor (+)** — `CORE` — add a point on a segment without changing shape (Pen does it on hover — just expose).
- **Delete Anchor (−)** — `CORE` — remove a point and heal the path.
- **Convert / Anchor Point (Shift+C)** — `CORE` — corner↔smooth; drag to pull/break handles. The third leg of bezier editing.
- **Curvature (Shift+`)** — `LATER` — handle-free smooth-curve drawing; beginner-friendly Pen alternative.
- **Line Segment (\\)** — `CORE` — single straight line; drag, or click for exact length/angle.
- **Arc / Spiral / Rectangular Grid / Polar Grid** — `SKIP` — convenience generators; Pen/Curvature cover their output.
- **Pencil (N)** — `LATER` — freehand path drawing (great with a tablet).
- **Smooth** — `LATER` — smooths a rough path; pairs with Pencil.
- **Path Eraser** — `SKIP` — trims a path line; overlaps select-and-delete-segment.
- **Join tool** — `LATER` — drag-to-join two path ends (the plain Join command Ctrl+J is more core).

## 3. Shape tools
- **Rectangle (M)** — `CORE` (in progress) — drag to draw; Shift=square, Alt=from-center, Spacebar=reposition, click=dialog. Live Shape (corner-radius editable).
- **Rounded Rectangle** — `LATER` — NOT a separate tool → a corner-radius MODE of Rectangle.
- **Ellipse (L)** — `CORE` (in progress) — circle/ellipse; Alt-from-center heavily used. Live Shape (pie/arc).
- **Polygon** — `CORE` (in progress) — regular polygon from center; arrow keys change side count live (triangle = 3 sides).
- **Star** — `LATER` — star burst; arrows = points, Ctrl-drag = inner radius. Not a Live Shape.
- **Flare** — `SKIP` — dated lens-flare effect.
- *Architecture note: make ONE shape primitive with parameters (sides, radius, corner-radius) — matches Varos's single-schema plan, far simpler than six tools.*

## 4. Type tools  *(the hard part is the text engine, not the buttons)*
- **Type (T)** — `CORE` — context-smart: empty=point text, inside a shape=area text, on a path=path text. One smart tool covers most.
- **Area Type** — `CORE` — flow/wrap text inside a shape (paragraphs/body).
- **Type on a Path** — `CORE` — text riding a path outline (badges/logos); pairs with the pen.
- **Vertical Type / Vertical Area / Vertical on Path** — `LATER`/`SKIP` — CJK vertical layout; narrow need.
- **Touch Type (Shift+T)** — `LATER` — per-character move/scale/rotate while staying editable text.

## 5. Paint & Color tools  *(model: every object = Fill + Stroke; each can be color / gradient / pattern)*
- **Fill / Stroke swatches** (X=focus, Shift+X=swap, D=default, /=none) — `CORE` — the always-on paint hub everything writes to.
- **Color panel** — `CORE` — mix color (hex/RGB/HSB).
- **Stroke panel** — `CORE` — weight, caps, joins, alignment, dashes, arrowheads.
- **Eyedropper (I)** — `CORE` — copy fill/stroke/style between objects (high value, low cost).
- **Gradient tool (G) + Gradient panel** — `CORE`/`LATER` — linear/radial blends; drag on canvas to set direction.
- **Swatches panel** — `LATER` — saved/global reusable colors (design tokens).
- **Mesh (U)** — `LATER` — freeform per-point color (photorealistic shading); heavy.
- **Paintbrush (B) / Blob Brush (Shift+B) + Brushes panel** — `LATER` — artistic strokes / filled marker shapes.
- **Live Paint Bucket (K) + Live Paint Selection (Shift+L)** — `LATER` — color overlapping line-art regions like a coloring book.
- **Recolor Artwork** — `LATER` — remap all colors of an illustration at once.

## 6. Transform & Reshape tools  *(move/scale/rotate are behaviors layered on V/A, plus dedicated tools)*
- **Transform panel (Shift+F8)** — `CORE` — exact X/Y, W/H, rotate, shear; 9-point reference point.
- **Transform Again (Ctrl+D)** — `CORE` — repeat the last transform → step-and-repeat arrays.
- **Rotate (R) / Scale (S) / Reflect (O) / Shear** — `CORE`/`LATER` — transform around a clicked pivot; Alt = transform-a-copy.
- **Free Transform (E)** — `LATER` — combined move/scale/rotate/distort/perspective via one widget.
- **Width (Shift+W)** — `LATER` — sculpt variable stroke width along a path.
- **Warp / Twirl / Pucker / Bloat / Scallop / Crystallize / Wrinkle (Shift+R …)** — `LATER`/`SKIP` — liquify-style brush distortions; niche.
- **Puppet Warp** — `LATER` — pin-and-bend mesh distortion.

## 7. Cutting, Building & Special tools
- **Shape Builder (Shift+M)** — `CORE` — drag to merge / Alt-drag to subtract overlapping shapes. **Flagship feature — build first among these.**
- **Scissors (C)** — `CORE` — split a path at a clicked point.
- **Knife** — `CORE` — freehand cut through filled shapes into closed pieces.
- **Eraser (Shift+E)** — `CORE` — freehand remove area from vector shapes.
- **Perspective Grid (Shift+P) + Perspective Selection (Shift+V)** — `LATER` — perspective drawing; complex, niche.
- **Symbolism family** (Sprayer Shift+S + 7 modifiers) — `SKIP` — legacy scatter/instancing; replace with a modern component system later.
- **Graph family** (9 chart tools) — `SKIP` — legacy data-viz; skip or add as a plugin later.
- **Slice + Slice Selection** — `SKIP` — obsolete web image-slicing; use asset/artboard export instead.

## 8. Navigation, Artboard & Essential Panels
- **Hand (H / hold Spacebar)** — `CORE` — pan the view.
- **Zoom (Z / Ctrl+wheel / Ctrl+0 fit / Ctrl+1 = 100%)** — `CORE` — magnify the view.
- **Rotate View (Shift+H)** — `LATER` — tilt the view (not the artwork).
- **Print Tiling** — `SKIP` — legacy paper-tiling.
- **Artboard (Shift+O)** — `CORE` — create/resize "pages" (= Figma frames).
- **Pathfinder panel** — `CORE` — booleans: Unite, Minus Front, Intersect, Exclude (+ Divide). Maps onto Clipper2; same engine as Shape Builder.
- **Layers panel (F7)** — `CORE` — object/stacking tree: reorder, hide, lock, rename, group. The source of truth for z-order.
- **Align & Distribute (Shift+F7)** — `CORE` — align edges/centers, distribute spacing, align-to-key-object / align-to-artboard.
- **Appearance panel (Shift+F6)** — `LATER` — stacked multiple fills/strokes/effects, non-destructive (IL's deepest idea). Ship single-fill/single-stroke first.

---

## ▶ Recommended build order (after Pen + Direct Select, shipped)
1. **Shapes** — Rectangle (M) + Ellipse (L) + Polygon as ONE Live-Shape primitive (W/H, corner-radius, sides, rotation) + Shift/Alt/Spacebar/click-dialog. *(in progress)*
2. **Fill & Stroke** — toolbar swatch pair + Color panel (hex/RGB) + Stroke panel (weight/caps/joins/dashes). Makes shapes render.
3. **Transform panel** — exact X/Y, W/H, rotate + the Selection bounding-box wired to the same numbers.
4. **Navigation** — Zoom (Ctrl+wheel, Ctrl+0, Ctrl+1) + Hand (Spacebar-hold).
5. **Layers panel** — reorder / hide / lock / rename / group.
6. **Align & Distribute.**
7. **Finish the Pen's anchor trio** — Add (+) / Delete (−) / Convert (Shift+C). Already on hover — just expose.
8. **Pathfinder** — Unite / Minus Front / Intersect / Exclude / Divide (on Clipper2).
9. **Shape Builder (Shift+M)** — interactive boolean; same engine as Pathfinder. Flagship.
10. **Eyedropper (I).**
11. **Line Segment tool (\\).**
12. **Artboard (Shift+O)** — multiple pages.
13. **Cutting trio** — Scissors (C) / Knife / Eraser (Shift+E).
14. **Smart Type tool (T)** — auto-detect point/area/path.
15. **Gradient (G)** — linear + radial.

## ▶ Key relationships (protect these)
- **Selecting ≠ doing:** V grabs objects, A grabs anchors; move/scale/duplicate are behaviors on top of V/A, NOT separate tools. Protect the modeless V↔A flip.
- **Direct Selection (A) is the universal edit layer** under every path-creating tool (Pen, shapes, Curvature, type-on-path).
- **Pathfinder panel and Shape Builder are the SAME boolean engine** (Clipper2) with two front-ends — build the engine once.
- **Live Shapes are parametric** — one shape primitive with parameters, not six tools (matches the single-schema plan).
- **Always ship a tool WITH its driving panel** — a tool without its panel is half-built. Must-have panels: Pathfinder, Layers, Align, Transform, Stroke, Color.

*(Full per-tool detail + relationships preserved in the source analysis; this file is the condensed working reference.)*

---

## 9. Additions from Affinity Designer + Illustrator review (2026-06-24)
Comparing the catalog against real Affinity Designer + Illustrator screenshots surfaced these missing pieces. **Most slot into EXISTING phases (they enrich Arrange / Text) — they do NOT add new phases, so the plan and v1 finish line still hold.**

- **Opacity + Blend modes** (Transparency) — `CORE` — per-object opacity + blend (Normal/Multiply/Screen…). → enriches Paint/Arrange.
- **Group / Ungroup** (Ctrl+G / Shift+Ctrl+G) — `CORE` — fundamental organization. → Arrange (Layers).
- **Arrange / z-order** (bring to front / send to back, Ctrl+] / Ctrl+[) — `CORE`. → Arrange (Layers).
- **Clipping mask** (clip artwork to a shape, Ctrl+7) — `CORE` — very common. → Arrange.
- **Rulers / Guides / Grid / Snapping** — `CORE` — canvas aids expected from minute one. → foundation/navigation.
- **Character + Paragraph panels** (font, size, leading, tracking, alignment) — `CORE` (with Type) — real text needs these. → Text (Phase 5).
- **Effects / fx** (drop shadow, blur, distort…) — `LATER` — live non-destructive effects. Post-v1.
- **Properties panel** (contextual — surfaces the right controls for the current selection) — `LATER` — modern convenience.
- **Export panel** (multi-format / multi-size, "export for screens") — `LATER` — v1 has basic SVG/PNG; richer export later.
- **Image Trace** (raster → vector) — `LATER` — needs raster support; defer.
- **Symbols / Components** (reusable instances, Figma-style) — `LATER` — aligns with the extensibility / "build tools on it" vision; post-v1.
- **Global Edit** (edit all similar objects at once) — `LATER`.
- **OUT OF SCOPE — Pixel / raster persona** (Affinity's Pixel mode / Photoshop-style raster editing): Varos is a **vector** tool (Illustrator competitor), not Photoshop. **Skip.**

Net: the v1 "Arrange" phase grows (group, z-order, opacity, clipping mask, guides) and "Text" grows (Character/Paragraph panels). Everything else is post-v1.

