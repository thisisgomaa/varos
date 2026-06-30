# Varos — Snap/Transform: web-grounded feature matrix (sourced)
> Companion to SNAP_TRANSFORM_SPEC.md — verified against official docs (Adobe/Serif/Figma) + a survey (Sketch/CorelDRAW/Inkscape/Penpot).

Based on the four research briefs, here is the consolidated feature matrix.

# Varos Snap / Transform / Guides-Grid-Rulers — Web-Grounded Feature Matrix

Consolidated from official documentation for Adobe Illustrator (helpx.adobe.com), Affinity Designer (Serif official help), Figma (help.figma.com), and a survey of Sketch, CorelDRAW, Inkscape, and Penpot. Every row is sourced; the source URL list is at the end. Per the standing Varos rule, **all shortcuts are bound to Illustrator's exact Windows keys** even where a behavior is adopted from another tool.

Recommendation legend: **v1** = ship in first snapping/transform pass · **Stage 2** = next milestone · **Stage 3** = later/advanced · **skip** = out of scope (mostly text/print/raster).

---

## 1. Feature Matrix

### 1a. Snapping — targets & options

| Feature | Illustrator | Affinity | Figma | Others (Sketch/Corel/Inkscape/Penpot) | Recommend for Varos |
|---|---|---|---|---|---|
| Master snap on/off gate | No single gate (split across Smart Guides / Snap to Grid / Snap to Point) | **Yes** — "Enable snapping" master toggle + magnet button | "Snap to objects" pref + hold `Ctrl` to suspend | Inkscape `%` master toggle; Penpot toggle | **v1** — one master toggle; `Ctrl`-held = temporarily suspend (Figma convention, additive to IL) |
| Snap to anchor / vertex (point) | **Snap to Point** (2px radius) | "Snap to object geometry" (vertices) | "Snap to geometry" (vector-edit mode only) | Inkscape/Corel: node snap | **v1** |
| Snap to path edge (anywhere on outline) | No | "Snap to object geometry" partial | No | Inkscape "snap to paths"; Corel "Edge" | **v1** |
| Snap to bbox edges/corners | via Smart Guides | "Snap to object bounding boxes" | Yes (bbox edges + centers) | Inkscape bbox toggles; all | **v1** |
| Snap to bbox / object **mid-points** (center) | Weak (Smart Guides only) | "Include bounding box mid points" (dependent child toggle) | Center snap | Inkscape "object midpoints"; Corel "Center" | **v1** — dependent child toggle pattern |
| Snap to segment **midpoint** (between two nodes) | No | partial (key points) | No | **Inkscape** "midpoints of segments"; **Corel** "Midpoint" (triangle marker) | **v1** — cheap, used constantly |
| Snap to **path intersection** | No | No | No | **Inkscape** + **Corel** "Intersection" (diamond marker) | **v1** — pro must-have, conspicuously absent in big three |
| Snap to **shape key points** (corner-radius tangents) | No | "Snap to shape key points" | No | Corel quadrant/key-point family | **Stage 2** |
| Snap to ellipse **quadrants** (N/S/E/W extremes) | No | partial | No | **Corel** "Quadrant"; Inkscape smooth-node | **Stage 2** — needs ellipse-aware geometry |
| **Tangent / perpendicular** snap while drawing | No | No | No | **Corel** "Tangent"/"Perpendicular"; **Inkscape** | **Stage 3** — math-heavy, pen-time differentiator |
| Snap to grid | **Snap to Grid** | "Snap to grid" | Snap to pixel grid (separate) | all | **v1** |
| Snap to single grid **line** (one axis) vs intersection only | intersection-ish | grid | pixel | **Inkscape** + **Corel** allow line-snap | **v1** — small high-value refinement |
| Snap to guides | implicit | "Snap to guides" | Yes | all; **Penpot** has dedicated toggle | **v1** |
| Snap to artboard/page **edge** | via Smart Guides | "Snap to spread" | frame snapping | **Inkscape** page-boundary toggle; all | **v1** |
| Snap to artboard/page **center / mid-points** | Weak | "Include spread mid points" (child toggle) | center | Inkscape; Corel page-center align | **v1** — cheap, very useful |
| Snap to margins (+ margin mid-points) | No | "Snap to margin" / "Include margin mid points" | No | Corel | **skip** (until print margins) |
| **Snap to gaps & sizes** (equal-gap + equal-size, on-canvas arrows) | weak Smart-Guides hint | **"Snap to gaps and sizes"** — flagship; arrows for matched gaps & matched W/H | equal-spacing snap (red, on the snap) | Sketch/Penpot live distances | **v1** — single highest-leverage snap to steal |
| "Only snap to **visible** objects" | inconsistent | "Only snap to visible objects" | n/a | n/a | **v1** — correctness win |
| Snap to **text baseline** | No | No | No | **Corel** "Text Baseline" | **skip** (until text engine) |
| Snap to glyph outlines | **Snap to Glyph** (5th system) | No | No | No | **skip** |

### 1b. Snapping — engine model & tolerance

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| **Independent snap-source vs snap-target** point sets | No (opaque) | partial | No | **Inkscape** ("snap from bbox" vs "snap from nodes"); **Corel** | **v1 (architecture)** — model both sets from day one; UI may expose a subset |
| **Candidate model** (scoped, tunable snap-target set) | No (opaque "nearby") | **Yes** — Candidate List / Immediate layers / All layers; **Maximum** (FIFO eviction); hover/create designates candidate; "purple halo" feedback | No | No | **v1 (architecture)** — Affinity's keystone idea: O(candidates) not O(document), curbs jitter. Default: Candidate List, Max ~5–8, hover-add, halo |
| Snapping **tolerance** exposed to user | hidden ~2px | **"Screen tolerance"** slider — measured in **screen px** (zoom-invariant) | fixed | varies | **v1** — apply tolerance in screen space after projecting candidates; default ~8px screen |
| Snapping **presets** (task-scoped bundles) | No | **5 presets**: Page layouts / +objects / Object creation / Curve drawing / UI design; user-savable | No | No | **Stage 2** — high value/low cost; ship a few named, savable presets |
| Suspend-snap modifier | per-mode | magnet toggle | **hold `Ctrl`** | varies | **v1** — `Ctrl` held = temporarily off |

### 1c. Smart guides + measurement (the "feel")

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Smart Guides alignment lines | **Smart Guides** (`Ctrl+U`); align to anchors/edges/centers | alignment guides | **red guide line** on snap | Sketch/Penpot dynamic-alignment | **v1** |
| Live **ΔX/ΔY + position** readout during drag | Yes (Measurement Labels) | yes | **red distance numbers** | Sketch/Penpot | **v1** — *Varos can beat Figma by showing the gap continuously, not just on the snap* |
| **Equal-spacing detection** as a snap target | weak | "snap to gaps" arrows | red equal-spacing snap | Sketch/Penpot | **v1** |
| **Quick measure** — select one, hover another, read both-axis gap | No | n/a | **`Alt`-hover** (red line, dual-axis numbers); nested = `Ctrl+Alt` | Inkscape Measure tool (M) | **v1** — additive to IL; bind `Alt`-hover |
| Construction/**angle guides** (preset/custom angles, lock-to-angle) | "Construction Guides" pref (up to 6 angles) | n/a | No | **Corel "Dynamic Guides"** (angle+distance, intersections, line-extensions) | **Stage 2** (orthogonal w/ distance) → **Stage 3** (angled/intersecting/extension) |
| Readability "pill" for on-canvas numbers | plain | plain | plain red | **Sketch** rounded pill | **v1** — copy Sketch's pill |
| Disciplined 2-color language (align/measure vs arrange) | n/a | purple halo = candidates | **red = align/measure, pink = arrange (Smart Selection)** | n/a | **v1 (lock the color system now)** |
| Show snapping **candidates** highlight | No | "purple halo" | No | No | **v1** |

### 1d. Grid types

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Rectangular doc grid (lines or **dots**, spacing + subdivisions) | Yes — Color, Style Lines/Dots, "Gridline every", Subdivisions | line grid | layout grid | Corel/Inkscape dots-or-lines | **v1** (Varos already on a dot grid) |
| **Pixel grid** + snap to whole pixels | Pixel Preview `Alt+Ctrl+Y`; Snap to Pixel; Align to Pixel Grid | "Force Pixel Alignment" + "Move By Whole Pixels" | Pixel grid (≥400% zoom); snap to pixel grid | Sketch pixel-fit; **Penpot** snap+subpixel toggle | **v1** (pixel-snap toggle, document-aware) · **Stage 2** (fit-to-pixel command) |
| Baseline grid | No | Publisher only | No | **Corel** (14pt default) | **skip** (until text/layout) |
| **Isometric / axonometric** grid (Angle X/Z) | No | No | No | **Inkscape** axonometric grid | **Stage 3** — design grid abstraction so it slots in |

### 1e. Guides & rulers

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Drag guides from ruler | Yes | Yes | Yes | all | **v1** |
| Show/Hide guides; Lock guides | `Ctrl+;` / `Alt+Ctrl+;` | yes | yes | all | **v1** (IL keys) |
| Make/Release guides (object↔guide) | `Ctrl+5` / `Alt+Ctrl+5` | yes | n/a | Inkscape | **Stage 2** |
| Guides snap to objects/nodes/intersections | partial | yes | n/a | **Inkscape** | **v1** (if ruler guides ship) |
| Numeric / **angled** guides; guide presets (cols/margins) | numeric drag | yes | n/a | **Inkscape** (any angle, `Ctrl`+click rotate, numeric dialog); **Corel** presets | **Stage 2** (numeric) · **Stage 3** (angled + presets) |
| Dedicated "snap to guides" toggle shortcut | no exact equiv | yes | n/a | **Penpot** `Shift+Ctrl+´` | **v1** — free-to-bind |
| Rulers show/hide | `Ctrl+R` | yes | yes | all | **v1** |
| Ruler origin: **drag-from-corner to set, double-click to reset** | Yes | yes | n/a | Inkscape | **v1** — clean discoverable interaction |
| **Per-artboard vs global** ruler origin | Yes (View > Rulers) | spread-relative | per-frame | all | **v1** — pick one convention, never deviate (top-left origin, Y-down) |

### 1f. Transform

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Transform panel: X/Y, W/H, angle, shear | Yes | Transform panel + Link toggle | position/size/rotate fields | all | **v1** |
| **Math + units in every field** (`100mm/2`, `+10`) | Yes (`+ - * /` + unit suffix) | yes | partial | Inkscape | **v1** — high-impact, cheap |
| **Reference-point locator** (3×3); governs **typed** transforms only | Yes (drag/tool use center/pointer) | origin handle | n/a | n/a | **v1** — decide this rule explicitly |
| Constrain proportions (Shift) / from-center (Alt) on resize | Yes | Link toggle; Shift/Alt | **Shift=ratio, Alt=center, Shift+Alt=both** | Penpot | **v1** — identical across apps, bind verbatim |
| **Rotation snap to 15°** with modifier | `Shift` constrain | Shift = 15° | `Shift` drag = 15° | Penpot `Ctrl`=45° | **v1** — bind **`Shift`** (IL), verify IL Constrain-Angle |
| Free Transform (perspective / free distort) | **`E`** + touch widget | n/a | n/a | n/a | **Stage 2** |
| Individual tools: Rotate/Scale/Reflect/Shear | `R` / `S` / `O` / (none) | context | n/a | Inkscape | **v1** (Rotate/Scale/Reflect) · **Stage 2** (Shear) |
| **Movable transform origin** (drag pivot, even outside object) | Rotate tool `Alt`-click | "Show Rotation Center", drag, dbl-click reset | n/a | **Inkscape** (persists per object, is flip axis) | **v1** |
| **Origin obeys snapping** (snap pivot to geometry/center/key pts) | partial | **Yes** (origin snaps like anything else) | n/a | Inkscape | **v1** — makes radial power-duplicate work |
| Rotation-center as a **snap target** | No | n/a | n/a | **Inkscape** (separate from object center) | **Stage 2** |
| Move… dialog (numeric move) | `Shift+Ctrl+M` | yes | n/a | all | **v1** |
| **Nudge / big-nudge** (arrow / Shift+arrow), configurable | Keyboard Increment + Shift=10× | yes | small=1, big=10, **two independent configurable values** | Penpot | **v1** — adopt Figma's two-field config, default to IL behavior; arrow/Shift+arrow bind verbatim |

### 1g. Power-duplicate / step-and-repeat

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| **Transform Again / step-and-repeat** (remember delta, re-apply+accumulate) | **`Ctrl+D`** | Power Duplicate (`Ctrl+J` loop) | Linear repeat (count+spacing) | Inkscape stamp (Space) | **v1** — adopt behavior, bind **`Ctrl+D`** (IL) |
| **Transform Each** (per-object pivot + Random + Copy) | `Alt+Shift+Ctrl+D` | n/a | n/a | n/a | **Stage 2** — scatter/array superpower |
| Stamp-a-copy during live drag/rotate | via `Ctrl+D` family | Power Duplicate | n/a | **Inkscape** Space-to-stamp | **Stage 3** |
| **Scale Strokes & Effects** toggle | Yes (off by default — classic gotcha) | yes | n/a | n/a | **v1** — explicit toggle the moment strokes scale |

### 1h. Align / distribute

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Align L/C/R, T/C/B | Yes (`Shift+F7` panel) | yes | yes | all | **v1** |
| Distribute (edges / centers) | Yes | yes | yes | all | **v1** |
| **Distribute Spacing by literal value** (type exact gap) | Yes (needs key object) | gaps | equal-spacing | Sketch Smart Distribute | **v1** |
| Align relative to **selection / artboard(page)** | Align To dropdown | yes | yes | **Corel** (selection/page/grid) | **v1** (selection + page) |
| **Align to Key Object** (click-again → blue outline) | Yes | yes | n/a | Corel key-object | **Stage 2** |
| Align to **grid point** | No | grid | n/a | **Corel** "nearest grid point" | **Stage 3** |
| **Use Preview Bounds** (align by visible stroke edge) | Yes (flyout) | yes | n/a | n/a | **Stage 2** |
| **Smart Distribute** drag-handle (set spacing by dragging; swap+reflow) | No | n/a | **Smart Selection** (pink handles) | **Sketch** Smart Distribute | **Stage 3** |
| **Tidy** — one-click regularize a messy cluster | No | n/a | Tidy up (auto-layout) | **Sketch** "Tidy" | **Stage 3** |

### 1i. Pixel snapping (UI/icon work)

| Feature | Illustrator | Affinity | Figma | Others | Recommend for Varos |
|---|---|---|---|---|---|
| Snap geometry to whole pixels | Snap to Pixel / Align to Pixel Grid (per-object) | **Force Pixel Alignment** (objects, nodes, handles) | snap to pixel grid | Penpot snap; Sketch fit | **v1** — document-aware (only when doc units = px) |
| **Move by integer-pixel deltas preserving sub-pixel phase** | No | **"Move By Whole Pixels"** (requires Force Pixel Alignment) | no | n/a | **Stage 2** — replicate faithfully; common half-implementation otherwise |
| Pixel grid visible at high zoom | Pixel Preview ≥600% | yes | ≥400% | Corel/Sketch | **v1** — copy ~400% threshold |
| One-shot **fit selection to pixel bounds** | No | no | no | **Sketch** `Ctrl+Cmd+X`; Corel | **Stage 2** |

---

## 2. Notable feature names + exact shortcuts (Illustrator = canonical for Varos)

Bind these Illustrator Windows keys verbatim:

| Action | Illustrator (Windows) — adopt for Varos |
|---|---|
| Smart Guides | `Ctrl + U` |
| Snap to Point | `Alt + Ctrl + '` |
| Snap to Grid | `Shift + Ctrl + '` |
| Show/Hide Grid | `Ctrl + '` |
| Pixel Preview | `Alt + Ctrl + Y` |
| Show/Hide Rulers | `Ctrl + R` |
| Show/Hide Guides | `Ctrl + ;` |
| Lock Guides | `Alt + Ctrl + ;` |
| Make Guides | `Ctrl + 5` |
| Release Guides | `Alt + Ctrl + 5` |
| **Transform Again (step-and-repeat)** | `Ctrl + D` |
| **Transform Each** | `Alt + Shift + Ctrl + D` |
| Move… (numeric dialog) | `Shift + Ctrl + M` |
| Free Transform | `E` |
| Rotate / Scale / Reflect tools | `R` / `S` / `O` |
| Align panel | `Shift + F7` |
| Group / Ungroup | `Ctrl + G` / `Shift + Ctrl + G` |

**Modifiers (consistent IL ⟷ Figma ⟷ Affinity — bind verbatim):**
- `Shift` + resize = constrain proportions · `Alt` + resize = from center · `Shift+Alt` = both.
- `Shift` while rotating = constrain to 15° increments.
- Hold `Ctrl` = temporarily suspend snapping (Figma convention; additive — IL has no conflicting binding).

**Additive bindings (no IL equivalent — safe to introduce, named per source tool):**
- **Quick measure** (Figma): `Alt`-hover a second object → dual-axis distance; nested = `Ctrl+Alt`-hover.
- **Snap-to-guides toggle** (Penpot ships `Shift+Ctrl+´`): no IL equivalent → free to bind.
- **Nudge config** (Figma two-field small/big): default to IL's Keyboard Increment + Shift=10× behavior; `Arrow` / `Shift+Arrow` identical in both.

**Named features worth keeping the source name/mental model:**
- Affinity: **"Snap to gaps and sizes"**, **Snapping Presets** (Page layouts / Object creation / **Curve drawing** / **UI design**), **Candidate List + Maximum**, **Force Pixel Alignment** / **Move By Whole Pixels**, **Power Duplicate**.
- Figma: **Snap to objects**, **Snap to geometry**, **Smart Selection**, **small/big nudge**.
- Illustrator: **Transform Again**, **Transform Each**, **Align to Key Object**, **Scale Strokes & Effects**, **Use Preview Bounds**, **Reference point locator**.
- Corel: **Dynamic Guides**, snap markers **Intersection / Midpoint / Quadrant / Tangent / Perpendicular / Edge**.
- Inkscape: independent **snap-from-bbox vs snap-from-nodes**, **axonometric grid**, **Measure tool (M)**.

---

## 3. Gaps the big three miss but pros expect (from the survey)

1. **Per-point snap source/target model** (Inkscape, CorelDRAW). The big three mostly snap *bounding box → bounding box*. Pros expect to choose *which point of the moving object leads* (a specific node, bbox edge-midpoint, center, rotation center) **independently** of *which targets it lands on*. **This is the single largest gap** and the thing that makes a vector tool feel "precise" vs "approximate." Build the engine around it even if v1's UI exposes a subset.
2. **Snap to path intersection** (Inkscape/Corel "Intersection," diamond marker) — including the intersection of two guide projections. Conspicuously weak/absent in Figma; a professional must-have.
3. **Snap to segment midpoint** between any two nodes (Corel "Midpoint"). Cheap to compute, used constantly, missing from the big three.
4. **Snap to anywhere on a path edge** (slide along a contour), not just to nodes (Inkscape/Corel "Edge").
5. **Quadrant / tangent / perpendicular snapping** (Corel) — geometrically meaningful points/constraints with no actual node; the CAD-grade tier the big three skip.
6. **Dynamic / construction guides** (CorelDRAW) — temporary guides emanating from a snap point at preset *or custom* angles, with live distance + angle readout, stepped-distance "ticks," placing an object at the **intersection of two dynamic guides**, and **line-extension** guides. The "didn't-know-I-needed-it" standout; generalizes smart-guides to arbitrary angles and to drawing-from-a-point.
7. **Movable, persistent, snappable transform origin** (Inkscape) that survives per object and doubles as the flip axis — foundational for hand-built radial arrays; the big three under-serve it.
8. **Isometric / axonometric grid** (Inkscape) with snapping along angled axes — beloved by icon/game/technical artists; absent from the big three.
9. **Move-by-whole-pixels preserving sub-pixel phase** (Affinity) vs merely snapping to the pixel grid — the distinction icon designers need and a common half-implementation.
10. **Live measurement readout in a readable pill while dragging** (Sketch/Penpot) — the most-felt feature; the pill-rendering detail matters for legibility.
11. **One-shot "fit selection to pixel bounds"** (Sketch) — round an existing selection's geometry to integer pixels in a single command.
12. **Snap to grid line (single axis)** not only full intersections (Inkscape/Corel) — small refinement, high value.

---

## 4. Source URLs

**Adobe Illustrator**
- https://helpx.adobe.com/illustrator/using/default-keyboard-shortcuts.html
- https://helpx.adobe.com/illustrator/desktop/measure-and-align/grids-and-guides/work-with-smart-guides.html
- https://helpx.adobe.com/illustrator/desktop/measure-and-align/grids-and-guides/smart-guides-options.html
- https://helpx.adobe.com/illustrator/using/pixel-perfect.html
- https://helpx.adobe.com/illustrator/using/snap-to-glyph.html
- https://helpx.adobe.com/illustrator/desktop/manage-objects/reshape-transform-objects/transform-panel-overview.html
- https://helpx.adobe.com/illustrator/desktop/manage-objects/reshape-transform-objects/transform-objects.html
- https://helpx.adobe.com/illustrator/using/tool-techniques/free-transform-tool.html
- https://helpx.adobe.com/illustrator/using/moving-aligning-distributing-objects.html
- https://helpx.adobe.com/illustrator/desktop/measure-and-align/grids-and-guides/use-rulers.html
- https://helpx.adobe.com/nz/illustrator/using/rulers-grids-guides-crop-marks.html
- https://helpx.adobe.com/illustrator/desktop/measure-and-align/grids-and-guides/align-graphic-objects-with-guides.html
- https://helpx.adobe.com/illustrator/desktop/measure-and-align/grids-and-guides/align-graphic-objects-with-grids.html

**Affinity Designer**
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/DesignAids/snapping.html
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/DesignAids/pixelAlign.html
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/ObjectControl/duplicate.html
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/ObjectControl/transform.html
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/ObjectControl/rotateShear.html
- https://s3-eu-west-1.amazonaws.com/affinity-docs/help/designer/en-US.lproj/pages/Panels/transformPanel.html
- https://webdesign.tutsplus.com/articles/how-to-power-duplicate-in-affinity-designer--cms-25010
- https://forum.affinity.serif.com/index.php?/topic/188453-affinity-designer-v1-snapping-max-candidates-and-tolerance-sliders/

**Figma**
- https://help.figma.com/hc/en-us/articles/360039956914-Adjust-alignment-rotation-position-and-dimensions
- https://help.figma.com/hc/en-us/articles/360039956974-Measure-distances-between-layers
- https://help.figma.com/hc/en-us/articles/20774752502935-Add-measurements-and-annotate-designs
- https://help.figma.com/hc/en-us/articles/4404575206295-Set-small-and-big-nudge-values
- https://help.figma.com/hc/en-us/articles/360041065034-Adjust-your-zoom-and-view-options
- https://help.figma.com/hc/en-us/articles/360040450233-Arrange-layers-with-Smart-selection
- https://www.figma.com/blog/introducing-smart-selection/
- https://help.figma.com/hc/en-us/articles/360040328653-Use-Figma-products-with-a-keyboard
- https://forum.figma.com/t/seeing-red-distance-measurements-between-objects/4781
- https://help.figma.com/hc/en-us/articles/31440427042839-Create-patterns-with-transforms

**Survey — Sketch / CorelDRAW / Inkscape / Penpot**
- http://tavmjong.free.fr/INKSCAPE/MANUAL/html/Snapping.html
- https://www.inkscapefriends.com/tutorials/introduction-to-snapping-part-1-bounding-boxes/
- https://www.inkscapefriends.com/tutorials/introduction-to-snapping-part-2-path-nodes-and-grids/
- https://www.tutorviacomputer.com/inkscape/inkscape-snapping/
- http://inkscape.gitlab.io/inkscape/doxygen/actions-canvas-snapping_8cpp_source.html
- https://bugs.launchpad.net/bugs/1260551
- https://www.educba.com/inkscape-grid/
- http://tavmjong.free.fr/INKSCAPE/MANUAL/html/IsometricProjection.html
- https://inkscapetutorials.wordpress.com/2014/04/21/inkscape-0-91-feature-measurement-tool/
- https://inkscape.gitlab.io/inkscape/doxygen/measure-tool_8cpp_source.html
- https://www.tutorviacomputer.com/inkscape/rotate-flip-objects/
- https://inkscape.org/doc/tutorials/tips/tutorial-tips.html
- https://daviesmediadesign.com/project/how-to-duplicate-objects-around-a-circle-in-inkscape-rotate-copies-lpe/
- https://product.corel.com/help/CorelDRAW/540223850/Main/EN/Documentation/CorelDRAW-Snapping-objects.html
- https://product.corel.com/help/CorelDRAW/Documentation-Windows/CorelDRAW-en/CorelDRAW-Dynamic-guides.html
- http://product.corel.com/help/CorelDRAW/540240626/Main/EN/Doc/CorelDRAW-Using-dynamic-guides.html
- http://product.corel.com/help/CorelDRAW/540111130/Main/EN/Documentation/CorelDRAW-Setting-up-the-grid.html
- https://product.corel.com/help/CorelDRAW/540240626/Main/EN/Doc/CorelDRAW-Setting-up-baseline-grid.html
- https://product.corel.com/help/CorelDRAW/540111148/CorelDRAW-en/CorelDRAW-Set-up-guidelines.html
- http://product.corel.com/help/CorelDRAW/540240626/Main/EN/Doc/CorelDRAW-Aligning-and-distributing-objectsa.html
- https://www.sketch.com/docs/interface-and-settings/the-mac-app-interface/the-canvas/
- https://www.sketch.com/docs/designing/layer-basics/aligning-layers/
- https://www.sketch.com/blog/2019/08/15/speed-up-your-workflow-with-smart-distribute/
- https://help.penpot.app/user-guide/designing/workspace-basics/
- https://help.penpot.app/user-guide/designing/layers/
- https://github.com/penpot/penpot/issues/6307
- https://designmodo.com/transform-duplicate-objects-illustrator/
- https://help.figma.com/hc/en-us/articles/31440427042839-Create-patterns-with-transforms
