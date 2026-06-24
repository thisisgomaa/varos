# Varos — Shape Tools (build notes)

Shapes are the next tool after the pen. **Core principle: a shape IS a path** — the exact same anchor model as pen-drawn paths — so every shape is immediately editable with the white arrow (Direct Selection), Illustrator-style. There is no special "shape object"; a shape is just a pre-made path.

## The shapes (build in this order)
1. **Rectangle (M)** = 4 **corner** anchors, closed path. (Rounded rectangle later = corners replaced by quarter-arc handles.)
2. **Ellipse / Circle (L)** = 4 **smooth** anchors at top / right / bottom / left, each with two handles. **Use the standard 4-point bezier circle:** handle length = radius × **0.5523** (kappa). This is what makes it look *truly round* instead of a bulgy diamond — get this number right.
3. **Polygon / Triangle** = N **corner** anchors spaced evenly around a circle (triangle = 3, hexagon = 6 …), closed.
4. **Star** = 2N anchors alternating between an outer and an inner radius.
5. **Line (\\)** = 2 anchors, open path.

## How they draw (Illustrator behavior)
- **Drag to draw:** press at one corner, drag to the opposite corner; the shape sizes to the drag's bounding box. (NOT click-to-place a fixed default size.)
- **Shift** = constrain to a perfect square / circle / equal proportions.
- **Alt** = draw from the center instead of from a corner.
- **Live preview** while dragging — the shape forms under the cursor until release.
- **One undo step** per shape created.
- *(Optional, later)* a plain click with no drag opens a small dialog for exact width/height — Illustrator does this; skip for now.

## After creation
- Because the shape is a path, switching to the **white arrow (A)** lets you grab and edit its anchors **immediately** — drag a rectangle's corner, pull a circle's handle, etc. No mode, no double-click (identical to the pen work already shipped).

## ★ This is the "add-a-tool pattern" moment (important)
While building the FIRST shape tool, set up ONE clean, repeatable way to define a tool: its name + shortcut + toolbar button + pointer handlers + how it writes anchors into the shared path model. Make the **Rectangle tool the template**, so Ellipse / Polygon / Star / Line are each a quick copy of that template. **"Easy to add a tool" is Varos's core strength — this is where it becomes real.** Keep each new tool small and let Ahmed verify it in his browser before the next.
