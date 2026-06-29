# Varos — Master Elements Catalog (the complete gathering)

> Exhaustive list of every tool/panel/system a pro vector tool (Illustrator + Affinity) has — so nothing is forgotten.
> **Status:** ✅ done · 🟡 partial · ⬜ missing · ❔ unknown.  **Importance (general, not Varos priority):** core / standard / advanced.
> Ahmed sets the real priority; this file is just the complete map. Tool detail also in `ILLUSTRATOR_TOOLS_CATALOG.md`.

> ## ⓘ v1 scope decision — 2026-06-29
> **v1 = Illustrator-class, NOT Figma-class.** Two consequences for reading this catalog:
> - **Components/Symbols + the Figma layer (Frames / Auto-Layout / resizing constraints / Dev-Mode).** Where they appear here as a one-line panel row, that is a STUB — each is really a multi-week **engine system** comparable to Color or Stroke (master + overrides + variants + nested + swap/detach). **Post-v1.** Do not mistake the panel row for the job size.
> - **Extensibility / Plugins / Scripting / single-schema (RNA)** — Varos's *stated moat* — is a **real system, deferred**, not the one-line bullet it currently is. Its enabler (the property-descriptor schema, Foundations **B6**) is built **after** Color+Stroke+Transform reveal the real property shapes (see `DETAILED_ROADMAP.md` Decision 5), not in the foundation. Give it its true weight when it lands.

## Index
1. [Tools](#1-tools)
2. [Panels (the full set) — the Window-menu panel index](#2-panels-the-full-set-the-window-menu-panel-index)
3. [Color system](#3-color-system)
4. [Stroke system](#4-stroke-system)
5. [Text / Type system](#5-text-type-system)
6. [Arrange systems](#6-arrange-systems)
7. [Structure systems](#7-structure-systems)
8. [Artboard + Document system](#8-artboard-document-system)
9. [Snapping / guides / grid / rulers](#9-snapping-guides-grid-rulers)
10. [Canvas interaction & navigation](#10-canvas-interaction-navigation)
11. [Save / File System](#11-save-file-system)
12. [Export system](#12-export-system)
13. [Comments / Collaboration / Review](#13-comments-collaboration-review)
14. [Welcome / Home / New-Document](#14-welcome-home-new-document)
15. [Menu command map](#15-menu-command-map)
16. [Preferences + Effects/Appearance](#16-preferences-effects-appearance)

## 1. Tools
*Every toolbar tool, hidden sub-tool, and toolbar-bottom control a pro vector editor needs (grounded in Illustrator + Affinity Designer), with keyboard letters and Shift/Alt/Space modifiers.*  
(75 items — ✅11 · 🟡8 · ⬜56 · ❔0)

- ✅ **Selection / black arrow (V)** · _core_ — Select whole objects/groups; bounding box = move/scale/rotate.
  V · default tool · click=select · Shift=add/toggle to selection · marquee-drag=rubber-band · Alt-drag=duplicate · arrow keys=nudge (Shift=10x) · double-click=enter group/isolation · drag bbox handle (Shift=constrain, Alt=from-center) · corner-out=rotate · modeless flip with A
- ✅ **Direct Selection / white arrow (A)** · _core_ — Select & edit individual anchors, segments, and bezier handles.
  A · click anchor/segment · Shift=multi-anchor · marquee anchors · drag segment to reshape · drag handle · Alt-click=isolate path · the universal edit layer under every path tool · pairs with V (modeless flip)
- ✅ **Pen (P)** · _core_ — Core bezier path tool: place anchors, drag for curves, build paths.
  P · click=corner anchor · drag=smooth anchor (symmetric handles) · Alt-drag=break handle while placing · Shift=constrain 45° · click first anchor=close path · hover segment=Add-anchor hint · hover anchor=Delete hint · Ctrl=temporary Direct-Select · contextual cursor (new/draw/add/delete/close/connect)
- ✅ **Add Anchor Point (+)** · _core_ — Adds an anchor on a segment without changing the path shape.
  + · click on a path segment · Pen already does this on hover — just expose as its own tool · nested in Pen flyout
- ✅ **Delete Anchor Point (−)** · _core_ — Removes an anchor and heals the path between neighbors.
  − · click an existing anchor · Pen does this on hover · nested in Pen flyout
- ✅ **Anchor Point / Convert (Shift+C)** · _core_ — Convert corner↔smooth; pull out, retract, or break handles.
  Shift+C · click anchor=corner (retract handles) · drag anchor=pull symmetric handles (smooth) · drag one handle=break into independent (cusp) · the third leg of bezier editing · in Varos bound to Pen+Alt
- ⬜ **Line Segment (\)** · _core_ — Draws a single straight line segment.
  backslash · drag=line · Shift=constrain 45° · Alt=draw from center · Space=reposition while drawing · click canvas=exact length/angle dialog
- ✅ **Rectangle (M)** · _core_ — Draws a rectangle/square; a Live Shape (editable W/H + corner radius).
  M · drag=rectangle · Shift=square · Alt=from center · Space=reposition while drawing · arrow keys=live corner-radius · click canvas=exact W/H dialog · Live-Shape widget (corner handles, radius)
- ✅ **Ellipse (L)** · _core_ — Draws an ellipse/circle; Live Shape (pie/arc angles).
  L · drag=ellipse · Shift=circle · Alt=from center · Space=reposition · click=exact W/H dialog · Live-Shape pie/arc start-end angle handles
- ✅ **Polygon** · _core_ — Draws a regular polygon from center; live side count.
  drag=polygon · arrow up/down=add/remove sides · Shift=constrain rotation · Space=reposition · click=options (radius, sides) · triangle = 3 sides (Varos has a dedicated Triangle too)
- ⬜ **Type (T)** · _core_ — Context-smart text: empty canvas=point text, in shape=area text, on path=path text.
  T · click=point text · drag=area text box · click in shape=area type · click on path=type-on-path · one smart tool covers most cases · pairs with Character/Paragraph panels
- ⬜ **Area Type** · _core_ — Flows/wraps text inside a closed shape (body paragraphs).
  nested in Type flyout · click a path/shape to convert it to a text container · overflow port (+) handles · options (rows/columns, inset, first-baseline)
- ⬜ **Type on a Path** · _core_ — Text that rides along a path outline (badges, logos).
  nested in Type flyout · click a path · drag brackets to set start/end/flip · options (Rainbow/Skew/3D-ribbon/Stair/Gravity effect, alignment to path, spacing)
- ✅ **Fill / Stroke swatches + swap + default** · _core_ — Toolbar-bottom paint hub: active Fill & Stroke wells everything writes to.
  X=toggle focus fill/stroke · Shift+X=swap fill↔stroke · D=default (white fill / black stroke) · double-click=open color picker · the always-on paint target
- 🟡 **Color / Gradient / None toggle (bottom)** · _core_ — Three small buttons under swatches setting the active paint type.
  < = solid Color (comma) · > = Gradient (period) · / = None · applies to whichever of fill/stroke has focus
- ✅ **Eyedropper (I)** · _core_ — Copies fill/stroke/appearance/type attributes from one object to another.
  I · click source to pick & apply to selection · Shift-click=pick only one color · Alt-click=apply current attributes to target · double-click=choose which attributes to pick/apply · can sample on-screen color
- 🟡 **Shape Builder (Shift+M)** · _core_ — Interactively merge/subtract overlapping shapes by dragging across regions.
  Shift+M · drag across regions=unite · Alt-drag=delete/subtract region · hover highlights region · Shift-marquee=combine multiple · same Clipper2 boolean engine as Pathfinder · gap-detection options · flagship
- 🟡 **Rotate (R)** · _core_ — Rotates selection around a clickable pivot point.
  R · click=set pivot then drag to rotate · Shift=constrain 45° · Alt-drag=rotate a copy · Enter/double-click=exact-angle dialog (+copy)
- 🟡 **Scale (S)** · _core_ — Scales selection around a clickable pivot.
  S · click pivot, drag to scale · Shift=uniform/constrain · Alt-drag=scale a copy · double-click=exact % dialog (uniform/non-uniform, scale strokes & effects option)
- ⬜ **Scissors (C)** · _core_ — Splits a path at a clicked point into two endpoints.
  C · click on a segment or anchor=cut there · creates two coincident endpoints (open the path) · works on closed shapes to open them
- ⬜ **Knife** · _core_ — Freehand cut through filled shapes, producing closed pieces.
  nested with Scissors · drag a freeform cut line · Alt=straight cut · Alt+Shift=45° cut · cuts all unlocked objects it crosses into separate closed paths
- ⬜ **Eraser (Shift+E)** · _core_ — Freehand removes area from vector shapes (reshapes/splits them).
  Shift+E · drag to erase · [ ] resize · Shift=constrain · Alt-drag=rectangular-marquee erase · double-click=brush angle/roundness/size options · only affects selected art if a selection exists
- ⬜ **Artboard (Shift+O)** · _core_ — Create/resize/reorder artboards (pages/frames).
  Shift+O · drag=new artboard · drag handles=resize · Alt-drag=duplicate · arrange/rename in Artboards panel · presets · = Figma frames
- 🟡 **Hand (H / hold Space)** · _core_ — Pans the canvas view.
  H · or hold Spacebar from any tool to temporarily pan · double-click tool=fit artboard in window · drag=pan
- 🟡 **Zoom (Z)** · _core_ — Magnifies / reduces the view.
  Z · click=zoom in, Alt-click=zoom out · drag=marquee-zoom to region · double-click tool=100% · Ctrl++ / Ctrl+- · Ctrl+0=fit, Ctrl+1=100% · Ctrl+wheel/pinch=zoom · scrubby-zoom drag
- 🟡 **Group Selection** · _standard_ — Progressively selects object → its group → outer group with repeated clicks.
  nested in A flyout · click-again expands selection up the group hierarchy · usually delivered via smart double-click instead of a separate tool
- ⬜ **Magic Wand (Y)** · _standard_ — Selects all objects sharing an appearance (same fill/stroke/opacity/blend).
  Y · tolerance options per attribute (fill color, stroke color, stroke weight, opacity, blend mode) · double-click tool = options dialog · overlaps a 'Select > Same' menu
- ⬜ **Curvature (Shift+`)** · _standard_ — Handle-free smooth-curve drawing; rubber-band preview between points.
  Shift+grave · click=add point with auto-smooth curve · double-click=corner point · drag existing point to reshape · Esc/Enter=end · beginner-friendly Pen alternative
- ⬜ **Pencil (N)** · _standard_ — Freehand path drawing that auto-fits a bezier to the stroke.
  N · drag to draw freeform path · Alt=close on release / straight segment · redraw over existing path to reshape · double-click=fidelity/smoothness + 'keep selected' + 'edit selected paths' options · great with a tablet
- ⬜ **Smooth** · _standard_ — Smooths/simplifies a rough path by brushing along it.
  nested with Pencil · drag along a path to reduce jitter/anchors · double-click=smoothness amount · pairs with Pencil/Paintbrush
- ⬜ **Join tool** · _standard_ — Drag across two open path ends to join/trim them at a crossing.
  nested with Pencil · scrub between two endpoints to connect; trims overlap at corners · plain Join command (Ctrl+J) is the more core counterpart
- 🟡 **Rounded Rectangle** · _standard_ — Rectangle with rounded corners (a corner-radius mode, not really a separate tool).
  nested in Rect flyout · arrow up/down=radius while dragging · Left/Right=min/max radius · best as a corner-radius MODE of Rectangle (matches single-parametric-shape plan)
- ⬜ **Star** · _standard_ — Draws a star burst with adjustable points and inner radius.
  drag=star · arrow up/down=points · Ctrl-drag=hold inner radius (spoke ratio) · Alt=fix shoulder angles (straight sides) · Shift=constrain · click=options (radius 1/2, points)
- ⬜ **Touch Type (Shift+T)** · _standard_ — Move/scale/rotate/kern individual characters while staying editable text.
  Shift+T · select a glyph in a text object · drag corner=scale, top-circle=rotate, body=move/baseline-shift · per-character transforms preserved as live text
- ⬜ **Drawing modes (bottom)** · _standard_ — Toolbar-bottom switch: Draw Normal / Draw Behind / Draw Inside.
  Shift+D cycles · Draw Normal=stack on top · Draw Behind=place under selection · Draw Inside=auto-clip new art into selected shape (sets up clip mask)
- ⬜ **Gradient (G)** · _standard_ — Applies and edits gradients by dragging direction/length on the artwork.
  G · drag on object=set gradient axis/length/angle · on-canvas gradient annotator (stops, midpoints, radius, aspect) · linear/radial/freeform · pairs with Gradient panel · double-click stop=color
- ⬜ **Paintbrush (B)** · _standard_ — Draws calligraphic/art/pattern/bristle/scatter brush strokes along a path.
  B · drag=brushed path · uses active Brush from Brushes panel · Alt=close · double-click=fidelity/'keep selected' options · pairs with Brushes panel
- ⬜ **Blob Brush (Shift+B)** · _standard_ — Paints filled, merged vector shapes (single compound fill, no stroke outline).
  Shift+B · drag=filled blob; overlapping same-color blobs auto-merge · [ ] resize brush · double-click=size/roundness/angle + keep-selected/merge options
- ⬜ **Width (Shift+W)** · _standard_ — Sculpts variable stroke width along a path (width points).
  Shift+W · drag on stroke=add width point & widen · Alt-drag=asymmetric side · drag existing point · Delete=remove point · saved as a reusable Width Profile
- ⬜ **Reflect (O)** · _standard_ — Mirrors selection across a clickable axis.
  O · click to set axis point/angle, drag to flip · Alt-click=axis dialog +copy · Shift=constrain axis · classic symmetry workflow
- ⬜ **Shear** · _standard_ — Slants/skews selection around a pivot and axis.
  nested with Scale · click pivot, drag to shear · Shift=constrain shear axis · Alt=shear a copy · double-click=exact angle/axis dialog
- ⬜ **Free Transform (E)** · _standard_ — One on-canvas widget for move/scale/rotate/shear/distort/perspective.
  E · drag handles=scale/rotate · with touch-widget: Constrain, Free Distort, Perspective Distort modes · Ctrl-drag corner=distort · Ctrl+Alt+Shift=perspective
- ⬜ **Rotate View (Shift+H)** · _standard_ — Temporarily tilts the canvas view (not the artwork) for comfortable drawing.
  Shift+H · drag to rotate view · Shift=snap 15° · Esc/reset to 0° · view-only, artwork coordinates unchanged
- ⬜ **Edit Toolbar / Customize ('...' drawer)** · _standard_ — Manage which tools show in the rail; access the full tool drawer.
  bottom '...' button opens all tools · drag tools in/out of the rail · Basic vs Advanced toolbar presets · group flyouts (tear-off in some tools)
- ⬜ **Lasso (Q)** · _advanced_ — Freehand-loop selection of anchor points / path segments.
  Q · draw a freeform loop to select anchors · Shift=add · Alt=subtract · niche (rectangular marquee covers most cases)
- ⬜ **Path Eraser** · _advanced_ — Erases (trims) along a path line by dragging over it.
  nested with Pencil · drag over an open/closed path to remove that stretch · overlaps select-segment-and-delete
- ⬜ **Arc** · _advanced_ — Draws an open or closed arc segment.
  nested in Line flyout · drag=arc · F=flip · arrow keys=change concavity/slope · X=convex/concave · C=open/closed · Shift=constrain · click=options dialog (length, type, base-along axis)
- ⬜ **Spiral** · _advanced_ — Draws a spiral with adjustable decay and winds.
  nested in Line flyout · drag=spiral · R=reverse direction · arrows up/down=add/remove winds · Ctrl-drag=decay · click=options (radius, decay, segments, style)
- ⬜ **Rectangular Grid** · _advanced_ — Draws a rectangular grid of horizontal/vertical dividers.
  nested in Line flyout · drag=grid · arrow keys=add/remove rows & columns · F/V=row skew · click=options (size, dividers, skew); convenience generator (Pen/shapes cover output)
- ⬜ **Polar Grid** · _advanced_ — Draws a concentric/radial (polar) grid.
  nested in Line flyout · drag=grid · arrows=concentric & radial dividers · X/D=skew · click=options dialog · convenience generator
- ⬜ **Flare** · _advanced_ — Draws a lens-flare effect (center, rays, halo, rings).
  nested in shape flyout · drag center+rays then drag halo/rings · dated effect · skip-tier in pro use
- ⬜ **Vertical Type** · _advanced_ — Point text laid out top-to-bottom (CJK vertical).
  nested in Type flyout · vertical writing direction · narrow Western need
- ⬜ **Vertical Area Type** · _advanced_ — Vertical text flowed inside a shape.
  nested in Type flyout · CJK vertical area text
- ⬜ **Vertical Type on a Path** · _advanced_ — Vertical text riding a path.
  nested in Type flyout · CJK path text
- ⬜ **Screen mode (bottom)** · _advanced_ — Toolbar-bottom cycle of canvas chrome: normal / full-with-menu / full screen.
  F cycles · presentation/Esc to exit · Affinity/IL bottom-of-toolbar control
- ⬜ **Mesh (U)** · _advanced_ — Adds a gradient mesh with per-node color for freeform shading.
  U · click shape=add mesh point/line · Shift-click=add without recoloring · drag node=warp · Alt-click=remove node · photorealistic shading; heavy
- ⬜ **Live Paint Bucket (K)** · _advanced_ — Fills overlapping line-art regions like a coloring book (Live Paint group).
  K · click a bounded region=fill · drag across regions · arrow keys=cycle swatches · gap-detection options · requires making a Live Paint group first
- ⬜ **Live Paint Selection (Shift+L)** · _advanced_ — Selects faces/edges within a Live Paint group.
  Shift+L · click face/edge · Shift-click=multi · then recolor selected regions/edges
- ⬜ **Reshape** · _advanced_ — Adjusts a path region by dragging selected anchors as a smooth group.
  nested with Scale · select anchors then drag to reshape while keeping detail · niche legacy tool
- ⬜ **Puppet Warp** · _advanced_ — Pin-and-bend mesh distortion of artwork.
  place pins; drag a pin to deform; auto-generated mesh; rotate around a selected pin (hover ring) · Alt-drag=rotate · used to repose vector art
- ⬜ **Warp (Shift+R)** · _advanced_ — Push/smear artwork with a brush (liquify-style).
  Shift+R · drag brush to warp · [ ] brush size · double-click=brush options (width/height/angle/intensity) · head of the liquify family
- ⬜ **Twirl** · _advanced_ — Creates swirling distortions under the brush.
  nested with Warp · hold=continuous twirl · options=twirl rate; liquify family
- ⬜ **Pucker** · _advanced_ — Pulls anchors toward the cursor (deflate).
  nested with Warp · brush-based deflate distortion
- ⬜ **Bloat** · _advanced_ — Pushes anchors away from the cursor (inflate).
  nested with Warp · brush-based inflate distortion
- ⬜ **Scallop** · _advanced_ — Adds random curved scallop details toward the cursor.
  nested with Warp · liquify family detail brush · complexity/detail options
- ⬜ **Crystallize** · _advanced_ — Adds random spiked details away from the cursor.
  nested with Warp · liquify family spike brush
- ⬜ **Wrinkle** · _advanced_ — Adds wrinkle-like horizontal/vertical details.
  nested with Warp · liquify family · horizontal/vertical amount options
- ⬜ **Symbol Sprayer (Shift+S)** · _advanced_ — Sprays multiple instances of a symbol as a symbol set.
  Shift+S · drag=spray instances · Alt=remove · [ ] brush size · pairs with Symbols panel · legacy scatter system
- ⬜ **Symbol Shifter / Scruncher / Sizer / Spinner / Stainer / Screener / Styler** · _advanced_ — Seven modifier tools that adjust sprayed symbol instances.
  nested with Sprayer · Shifter=move · Scruncher=density · Sizer=scale · Spinner=rotate · Stainer=tint · Screener=opacity · Styler=apply graphic style · legacy symbolism family
- ⬜ **Column / Bar / Stacked / Line / Area / Scatter / Pie / Radar Graph** · _advanced_ — Nine legacy data-graph generators (chart types).
  J for Column · flyout holds Column/Stacked-Column/Bar/Stacked-Bar/Line/Area/Scatter/Pie/Radar · enter data in a spreadsheet dialog · legacy; usually a plugin
- ⬜ **Slice (Shift+K)** · _advanced_ — Defines web-export slices on the artwork (legacy).
  Shift+K · drag=slice region · obsolete web image-slicing; superseded by asset/artboard export
- ⬜ **Slice Selection** · _advanced_ — Selects and adjusts existing slices.
  nested with Slice · click/resize slice boundaries · legacy
- ⬜ **Perspective Grid (Shift+P)** · _advanced_ — Sets up a 1/2/3-point perspective grid to draw in perspective.
  Shift+P · drag grid widgets to define planes/horizon/vanishing points · plane-switching widget (1/2/3) · draw onto active plane · complex, niche
- ⬜ **Perspective Selection (Shift+V)** · _advanced_ — Moves/scales objects within the perspective grid.
  Shift+V · drag art onto a perspective plane · 1/2/3 to pick plane · 5=move perpendicular · brings flat art into perspective
- ⬜ **Print Tiling** · _advanced_ — Adjusts the printable page tiling origin on the artboard.
  nested with Hand · drag to position page tiles · legacy print workflow

## 2. Panels (the full set) — the Window-menu panel index
*Every dockable panel a pro vector tool exposes under Window, each with its purpose (deep contents covered by other agents).*  
(46 items — ✅3 · 🟡6 · ⬜37 · ❔0)

- 🟡 **Properties (contextual)** · _core_ — Adaptive single panel showing settings for the current selection/tool
  context-sensitive · shows transform, appearance, align, quick-actions for nothing-selected/object/text/multiple · Illustrator default workspace · Affinity 'Context Toolbar' equivalent
- ⬜ **Transform** · _core_ — Numeric position, size, rotation, shear of selection
  X/Y · W/H · reference-point 9-grid · rotate angle · shear · constrain-proportions lock · flip H/V · 'scale strokes & effects' · 'align to pixel grid' · rectangle/corner properties
- ✅ **Align** · _core_ — Align and distribute objects relative to each other/artboard/key object
  align-left/center/right + top/middle/bottom · distribute-spacing · distribute-objects · align-to: selection/key-object/artboard · distribute-spacing value
- ✅ **Pathfinder** · _core_ — Boolean shape combination and division operations
  shape modes: unite/minus-front/intersect/exclude · pathfinders: divide/trim/merge/crop/outline/minus-back · 'make compound shape' · expand · trap (options)
- 🟡 **Layers** · _core_ — Stacking/hierarchy tree of all objects, groups, layers
  layer/sublayer/object rows · visibility (eye) · lock · target/appearance dot · selection color · drag-reorder · template/outline-view · locate-object · release-to-layers · merge/flatten · collect
- 🟡 **Color** · _core_ — Mix the active fill/stroke color by sliders/values
  color model: RGB/CMYK/HSB/Grayscale/Web-safe · sliders + hex/values · spectrum ramp · none/white/black swatches · out-of-gamut & non-web warnings · invert/complement
- ⬜ **Swatches** · _core_ — Library of saved colors, gradients, patterns, color groups
  add/delete swatch · spot vs process · global swatches · color groups/folders · gradient & pattern swatches · list/thumbnail view · swatch-libraries menu (Pantone etc) · sort
- ⬜ **Gradient** · _core_ — Create and edit linear/radial/freeform gradient fills/strokes
  type linear/radial/freeform · gradient slider with stops (color+location+opacity+midpoint) · angle · aspect-ratio · reverse · stroke-gradient mode · apply to fill/stroke
- 🟡 **Stroke** · _core_ — Stroke weight and all stroke styling parameters
  weight · cap (butt/round/projecting) · join (miter/round/bevel) · miter-limit · align stroke (center/inside/outside) · dashes & gaps + dash-cap · arrowheads + scale · profile (width)
- ⬜ **Character** · _core_ — Per-character/run typography controls
  font family/style · size · leading · kerning (auto/optical/metrics) · tracking · vertical/horizontal scale · baseline shift · rotation · caps/superscript · language · anti-alias
- ⬜ **Paragraph** · _core_ — Paragraph-level text formatting
  alignment (L/C/R/justify variants) · indents (L/R/first-line) · space before/after · hyphenation · justification options · drop-cap · tabs link · roman/CJK · direction (LTR/RTL)
- ⬜ **Transparency** · _core_ — Opacity, blend modes, and opacity masks
  opacity % · blend mode (multiply/screen/overlay/…) · make/release opacity-mask · clip & invert-mask toggles · isolate-blending · knockout-group
- ✅ **Tools / Toolbar (Basic & Advanced)** · _core_ — The tool palette itself, dockable as a panel
  basic vs advanced toolbar · customize/drag tools in/out · hidden tool flyouts · fill/stroke proxy · drawing-mode + screen-mode controls
- ⬜ **Appearance** · _standard_ — Stacked list of fills, strokes, effects, opacity on selection
  multiple fills/strokes · per-attribute opacity & blend · live-effects list (reorderable) · add-new-fill/stroke · clear/reduce-to-basic · duplicate · graphic-style link
- ⬜ **Brushes** · _standard_ — Apply and manage brush definitions to strokes
  brush types: calligraphic/scatter/art/bristle/pattern · apply to path · remove brush stroke · new/options · brush-libraries · 'keep selected' · options-of-selected-object
- ⬜ **Symbols** · _standard_ — Reusable master instances placed many times (sprites)
  symbol library · place instance · break link · duplicate/edit · dynamic vs static symbol · 9-slice · registration point · symbolism-tool set · symbol-libraries
- ⬜ **Character Styles** · _standard_ — Named, reusable sets of character-level formatting
  create from selection · apply · edit/redefine · override+ indicator · clear overrides · based-on hierarchy · load styles
- ⬜ **Paragraph Styles** · _standard_ — Named, reusable sets of paragraph formatting
  create/apply/redefine · override indicator + clear · based-on / next-style · normal-paragraph-style · load styles
- ⬜ **Glyphs** · _standard_ — Browse and insert any glyph from the current font
  full glyph grid · show: entire-font/alternates-for-selection/ligatures/etc · recently-used · zoom slider · double-click to insert · alternate-glyph popup
- ⬜ **Tabs** · _standard_ — Set tab stops and indents for selected text
  L/C/R/decimal tab markers · ruler aligned to text · tab position value · leader (dots) · alignment char · snap-to-unit · magnet button
- ⬜ **Artboards** · _standard_ — Manage the document's multiple artboards/pages
  list/reorder artboards · add/delete/duplicate · rename · rearrange-all (grid) · per-artboard size/orientation · move with artwork · options (display, video rulers)
- ⬜ **Asset Export** · _standard_ — Stage objects/artboards for multi-format/scale export
  drag assets in · export-for-screens · multiple scales (1x/2x/3x) + suffix · formats PNG/JPG/SVG/PDF · per-asset settings · artboards vs assets tabs
- ⬜ **Links** · _standard_ — Manage placed/linked external images
  linked vs embedded list · relink/relink-from-CC · edit-original · update-modified · embed/unembed · go-to-link · link info (resolution, path, scale, status)
- ⬜ **Image Trace** · _standard_ — Convert raster images to editable vector paths
  presets (sketch/silhouette/line-art/photo/etc) · mode color/grayscale/B&W · threshold · paths/corners/noise · palette · ignore white · preview · expand-to-paths
- ⬜ **Navigator** · _standard_ — Thumbnail overview for panning and zooming
  document thumbnail · red view-box (drag to pan) · zoom slider + % field · view-all-artboards · proxy preview area
- ⬜ **Libraries (CC / Assets)** · _standard_ — Cloud/shared store of reusable colors, type, components, graphics
  create/switch libraries · color/char-style/graphic/component groups · drag-to-place · linked assets · share/collaborate · search · (Affinity 'Assets' panel equiv)
- 🟡 **History / Histories** · _standard_ — Visual list of undo states to step back/forward
  chronological step list · click to revert · Affinity 'History' with persistent (saved-with-doc) history + scrubber slider · Illustrator (limited) · clear
- ⬜ **Graphic Styles** · _standard_ — Saved named appearance recipes (fills+strokes+effects) applied in one click
  library grid · apply/add-style · merge styles · break-link · graphic-style-libraries · default style · ties into Appearance panel
- 🟡 **Control bar (contextual toolbar)** · _standard_ — Top context strip of common options for current selection/tool
  quick fill/stroke/weight · align · transform · type controls · 'opens-panel' links · Affinity persistent context toolbar
- ⬜ **Stroke profiles / Width (Affinity)** · _standard_ — Variable-width stroke profile editor
  width-profile presets · add/edit width points · pressure profile · save profile (Illustrator exposes via Stroke; Affinity stroke panel)
- ⬜ **Glyph Browser / Special chars** · _standard_ — Insert special characters and symbols (Affinity)
  category browser · recently used · search by name/unicode · insert · overlaps Illustrator Glyphs
- ⬜ **Constraints / Snapping Manager** · _standard_ — Configure snapping candidates and constraints (Affinity)
  snap to geometry/grid/guides/key-objects · snapping presets · candidate radius · force-pixel-alignment
- ⬜ **Effects / FX (Affinity)** · _standard_ — Live non-destructive layer effects (shadow, glow, blur, etc.)
  outer/inner shadow · outer/inner glow · outline · 3D/bevel · gaussian blur · color overlay · gradient overlay · per-effect params (Illustrator via Appearance+Effect menu)
- ⬜ **Color Guide** · _advanced_ — Suggests harmonious color variations from a base color
  harmony rules (complementary/analogous/triad/etc) · tints-shades / warm-cool / vivid-muted columns · base color · edit-colors / save-to-swatches · limit to library
- ⬜ **OpenType** · _advanced_ — Toggle OpenType font features per selection
  ligatures (standard/discretionary) · contextual alternates · swashes · stylistic alternates/sets · titling · ordinals · fractions · figure style (lining/oldstyle, tabular/proportional) · position
- ⬜ **Actions** · _advanced_ — Record/play macros to batch repetitive tasks
  action sets · record/stop/play · insert menu-item/stop · toggle dialog · button-mode · batch processing · default action set
- ⬜ **Document Info** · _advanced_ — Read-only summary of the document and selection
  objects/spot-colors/patterns/gradients/fonts/linked-&-embedded-images/font-details lists · document settings · ruler units · selection-only toggle · save as text
- ⬜ **Comments** · _advanced_ — Review threads/annotations pinned on the canvas
  add pin/comment · reply thread · resolve/reopen · assign/mention · filter · navigate between comments · (collaboration/cloud doc context)
- ⬜ **Attributes** · _advanced_ — Misc object metadata: overprint, fill rule, image map, URL
  overprint fill/stroke · show center point · reverse-path-direction · even-odd vs non-zero winding · image map + URL · output resolution
- ⬜ **Variables / Data Merge** · _advanced_ — Bind object properties to data for templated batch output
  variable types (visibility/text-string/linked-file/object) · bind to object · capture data set · cycle data sets · import XML/data · template
- ⬜ **Pattern Options (pattern edit)** · _advanced_ — Edit a seamless repeating pattern swatch in isolation
  tile type (grid/brick/hex) · brick offset · width/height · overlap · copies preview · tile-edge color · dim-copies
- ⬜ **Separations Preview** · _advanced_ — Preview print color separations and overprints
  toggle individual plates (C/M/Y/K + spots) · overprint preview · CMYK simulation · detects rich-black/overprint issues
- ⬜ **Flattener Preview** · _advanced_ — Preview which artwork is affected by transparency flattening
  highlight rasterized/affected regions · preset · resolution · refresh · for legacy print output
- ⬜ **SVG Interactivity** · _advanced_ — Add JavaScript event handlers to objects for SVG export
  event (onclick/onmouseover/…) + action/script · list of events · for interactive SVG output
- ⬜ **Magic Wand** · _advanced_ — Tune the tolerance used by the magic-wand selection tool
  select-by: fill-color/stroke-color/stroke-weight/opacity/blend-mode · tolerance per attribute
- ⬜ **Adjustment / Live Filters (Affinity)** · _advanced_ — Non-destructive raster adjustments inside a vector doc
  brightness/contrast · HSL · curves · recolor · blur/sharpen live filters · for embedded raster in vector persona

## 3. Color system
*Full color pipeline: pickers/models, swatches & libraries, gradients, harmony/recolor tools, fill/stroke controls, and eyedropper sampling — everything a pro vector tool exposes for defining and managing color.*  
(42 items — ✅4 · 🟡3 · ⬜35 · ❔0)

- ⬜ **Color picker dialog (full)** · _core_ — Modal large picker for choosing an exact color
  Square SV field + vertical hue bar (default) · radio-switch axis (H/S/B/R/G/B drives the bar vs field) · live old/new swatch compare · numeric fields per model · hex field · 'Color Swatches' toggle (pick from swatch list) · out-of-gamut & non-websafe warning triangles with click-to-correct · 'Only Web Colors' restrict
- ⬜ **Color sliders + value fields (Color panel)** · _core_ — Docked panel with per-channel sliders and numeric entry
  Sliders tinted to show result · numeric box per channel · % vs 0-255 vs 0-100 depending on model · tab between fields · spectrum/ramp strip at bottom for quick click-pick · panel menu to switch model · None/registration shortcuts
- 🟡 **RGB color model** · _core_ — Screen/additive color in red-green-blue
  0-255 (or 0-100%) per channel · sRGB working space · used for web/screen output · default for screen docs
- 🟡 **Hex (HTML) input** · _core_ — Web hex string entry for a color
  #RRGGBB · 3-digit shorthand · with/without # · paste support · 8-digit #RRGGBBAA (alpha, in some tools) · copy-as-hex
- ✅ **Fill control (swatch)** · _core_ — Active fill-color well in toolbar/inspector
  Front fill square · double-click opens picker · shows solid/gradient/pattern/none · drag-drop color onto it · accepts swatch drops
- ✅ **Stroke control (swatch)** · _core_ — Active stroke-color well
  Back outlined square · double-click picker · solid/gradient/none · paired with stroke-weight & style elsewhere
- ✅ **Swap / Default / Fill-Stroke proxy** · _core_ — The classic fill-over-stroke toggle cluster
  Swap fill↔stroke (Shift+X) · Default black-fill/no... wait: default = black stroke + ? (X cycles focus) · reset to default (D) · X toggles active attribute · None button
- 🟡 **None / Color / Gradient mode buttons** · _core_ — Three quick-set buttons under fill/stroke proxy
  [Color] last solid · [Gradient] last gradient · [None] removes paint · '/' key = None · keyboard '<' '>' '.' cycle in AI
- ⬜ **Swatches panel** · _core_ — Library of saved/named colors for the document
  Grid/list view · add/delete/duplicate · rename · sort by name/kind · show-by-kind filter (color/gradient/pattern/group) · select-all-unused · merge swatches · thumbnail size · drag to reorder
- ⬜ **Gradient — types** · _core_ — Multi-color blends as fill/stroke
  Linear · Radial · (Affinity also Conical/Bitmap fill) · Illustrator Freeform (points & lines mesh-like) · apply to fill OR stroke
- ⬜ **Gradient stops** · _core_ — Color anchors along the gradient ramp
  Add (click ramp) / delete (drag off) · per-stop color via picker/swatch · drag to reposition · numeric location % · pick stop color from swatch or model
- ⬜ **Gradient geometry controls** · _core_ — On-canvas annotator to set direction/extent
  Gradient tool drag = angle+length · start/end handles · radial: aspect-ratio & center offset (origin/focal) · angle numeric field · scale/skew radial to ellipse · reverse-gradient button
- ✅ **Eyedropper — basic sampling** · _core_ — Pick a color from any object/pixel onto active attr
  Click object → copy its fill/stroke to selection · sample from placed images · sample anywhere on screen (hold + drag off-canvas) · I shortcut
- ⬜ **Color wheel picker** · _standard_ — Round hue ring with inner saturation/value area
  Hue ring (outer) · inner triangle OR square SV box · drag handles · Affinity uses wheel-by-default · toggle between wheel and box/sliders modes · click-anywhere to set
- ⬜ **HSB / HSL model** · _standard_ — Perceptual hue-saturation-brightness(or lightness) entry
  Hue 0-360° · Saturation 0-100% · Brightness/Value 0-100% (HSB) or Lightness (HSL) · easier for designers to dial harmonies/tints
- ⬜ **CMYK color model** · _standard_ — Print/subtractive cyan-magenta-yellow-black
  0-100% per channel · for print docs · ICC profile aware · total-ink warning · process color definition basis
- ⬜ **Grayscale model** · _standard_ — Single black-percentage channel
  0-100% K (or 0-255 gray) · for 1-color/grayscale work · convert-to-grayscale path
- ⬜ **Global swatches** · _standard_ — Linked color that updates everywhere when edited
  Global flag (corner triangle) · edit one → all instances update · tint percentage of a global · core to brand/recolor workflows
- ⬜ **Tint / shade controls** · _standard_ — Percentage tints derived from a base swatch
  Tint slider 0-100% for global/spot · auto-generate tint set · keeps link to parent global
- ⬜ **Swatch groups / folders** · _standard_ — Organize swatches into named collections
  Color group folder · group from selection · expand/collapse · used as Recolor harmony source · drag swatches in/out
- ⬜ **Swatch libraries (built-in)** · _standard_ — Pre-made palette collections to open
  Pantone (Solid/Process/Pastels/Metallics) · TOYO/HKS/DIC/ANPA · Web/Material · skintones, metals, harmonies · open as floating panel · persistent vs add-to-doc
- ⬜ **Save / load custom palettes (.ase / .aco)** · _standard_ — Import/export swatch sets between files & apps
  Export .ase (Adobe Swatch Exchange) · .aco · Affinity .afpalette · import from GIMP/CSS/image · 'Add Used Colors' · create app/system/document-scoped palette
- ⬜ **Palette from image / document** · _standard_ — Auto-extract a palette from artwork or photo
  Generate palette from selected image · 'Add all colors in document' · k-means/dominant extraction · Affinity 'Create Palette From Image/Document'
- ⬜ **Gradient midpoints** · _standard_ — Diamond control of blend balance between two stops
  Diamond marker on ramp · drag 0-100% to skew transition · numeric midpoint field
- ⬜ **Gradient opacity stops** · _standard_ — Per-stop alpha for fade gradients
  Opacity % per stop · enables transparent gradients · independent of color stops
- ⬜ **Color Guide / harmony rules** · _standard_ — Suggest harmonious colors from a base
  Harmony rules dropdown (complementary, analogous, triad, tetrad, monochrome, shades, etc.) · variation grid (tints/shades, warm/cool, vivid/muted) · base-color link · 'Edit Colors' jump · save group to Swatches
- ⬜ **Eyedropper — appearance vs color options** · _standard_ — Choose how much of the look the eyedropper copies
  Double-click eyedropper dialog · pick/apply: fill color, stroke, opacity, full appearance (effects), character/paragraph type attrs · checkboxes for each · 'Appearance' vs just-color modes
- ⬜ **Eyedropper — sample size / raster** · _standard_ — Control averaging when sampling pixels/images
  Point sample vs 3x3 / 5x5 average · sample from current layer vs all layers · apply sampled color back with Alt-click (reverse apply)
- ⬜ **Recent colors strip** · _standard_ — Quick row of last-used colors
  Auto-tracked recents · click to reapply · clear-recents · sometimes a dedicated panel/footer ramp
- ⬜ **Color spectrum / ramp bar** · _standard_ — Continuous strip for fast approximate picking
  Full-hue ramp at Color-panel bottom · click to sample · CMYK/RGB tintable ramp · None & white/black quick chips at ends
- ⬜ **Drag-and-drop / copy-paste color** · _standard_ — Move colors between wells, swatches, objects
  Drag fill well onto object/swatches · drag swatch onto object · copy hex/color, paste onto selection · drag between fill and stroke wells
- ⬜ **Lab color model** · _advanced_ — Device-independent perceptual L*a*b* entry
  L 0-100 · a/b -128..127 · widest gamut · used for precise/spot-color matching · Affinity & AI expose it
- ⬜ **Color model / working-space management** · _advanced_ — Document-level color mode and ICC profiles
  Document RGB vs CMYK mode · assign/convert ICC profile · 32-bit/HDR (Affinity) · soft-proofing of output profile · rendering intent (perceptual/relative/etc.)
- ⬜ **Gamut / web-safe warnings** · _advanced_ — Alerts when a color is unprintable or non-web
  Out-of-CMYK-gamut triangle · non-web-safe cube · click warning to snap to nearest in-gamut/web color
- ⬜ **Registration color** · _advanced_ — Special swatch that prints on all plates
  For crop/trim marks · 100% of every plate · distinct from rich black · non-deletable swatch
- ⬜ **Spot vs Process color** · _advanced_ — Distinguish named ink (spot) from CMYK-built (process)
  Spot = single named ink (dot icon) · process = built from CMYK/RGB · spot tint % · spot→process conversion · used for Pantone/special inks
- ⬜ **Freeform gradient (mesh-lite)** · _advanced_ — Place color points freely for organic blends
  Points mode (color dots) · Lines mode (path of dots) · spread/area per point · Illustrator-specific · no mesh editing needed
- ⬜ **Gradient mesh** · _advanced_ — Editable grid of color points across an object
  Mesh tool adds rows/cols · per-node color + bezier mesh handles · most flexible photoreal vector shading · AI-only (Affinity lacks true mesh)
- ⬜ **Pattern fills / swatches** · _advanced_ — Tiled artwork used as a fill
  Pattern swatch · tile size/offset/overlap · pattern-editing mode · seamless preview · scale/rotate independent of object
- ⬜ **Recolor Artwork** · _advanced_ — Remap all colors in a selection at once
  Color wheel of current colors · link/unlink hue-shift · reduce colors / # of colors · assign source→output mapping · presets/harmony rules · randomize hue/brightness · preserve spots/black · recolor from library · live preview
- ⬜ **Overprint / trapping attributes** · _advanced_ — Print-output color behavior per object
  Overprint fill/stroke checkboxes (Attributes panel) · overprint preview · trap/knockout for spot separations
- ⬜ **Color blindness / proof preview** · _advanced_ — Simulate how colors read for accessibility/print
  Color-blind soft proof (protanopia/deuteranopia) · overprint preview · separations preview · contrast check

## 4. Stroke system
*The complete stroke/outline system for a pro vector tool: weight, caps, joins, alignment, dashes, arrowheads, variable-width profiles, pressure, stroke-vs-fill order, scaling, and the brush-vs-stroke distinction — grounded in Adobe Illustrator + Affinity Designer conventions.*  
(19 items — ✅0 · 🟡4 · ⬜15 · ❔0)

- 🟡 **Stroke weight / width** · _core_ — The thickness of the outline, the most fundamental stroke property.
  Numeric field (pt/px/mm/in, document-unit aware) · stepper +/- · scrub/drag-to-set · presets dropdown (0.25/0.5/1/2/3pt…) · 0 = no visible stroke (but stroke still present) · hairline concept · per-segment weight (AI variable-width) is separate · keyboard nudge
- 🟡 **Stroke color / paint type** · _core_ — What the stroke is painted with — solid, gradient, pattern, or none.
  Solid color · gradient stroke (along path / across path / within stroke — AI) · pattern stroke · none/transparent · stroke opacity (separate from object opacity) · stroke blend mode (advanced) · applies via fill/stroke swatch with stroke target active
- ⬜ **Cap style (line ends)** · _core_ — How the two ends of an open path (and dash ends) are terminated.
  Butt cap (flush, no overhang) · Round cap (semicircle, extends by ½ weight) · Projecting/Square cap (square, extends by ½ weight) · 3-button toggle in stroke panel · affects open-path ends AND every dash end · default = butt
- ⬜ **Join style (corners)** · _core_ — How two stroke segments meet at an anchor/corner.
  Miter join (sharp point) · Round join (rounded corner) · Bevel join (flattened/chamfered corner) · 3-button toggle · applies at every corner vertex · default = miter
- ⬜ **Stroke alignment / position** · _core_ — Where the stroke sits relative to the path geometry.
  Align Center (½ in/½ out — default) · Align Inside (stroke fully inside path) · Align Outside (stroke fully outside) · 3-button toggle · inside/outside only valid on closed paths (AI greys out on open) · changes object visual bounds · interacts with align/snap (use geometric vs visual bounds)
- ⬜ **Miter limit** · _standard_ — Threshold that converts a too-sharp miter join into a bevel to prevent runaway spikes.
  Numeric ratio (miter length ÷ stroke width), AI default 4, range ~1–500 · angle below which miter flips to bevel · only relevant when join=miter · 'x' unit label · prevents long spikes on acute angles
- 🟡 **Dashed line pattern** · _standard_ — Renders the stroke as a repeating dash/gap sequence instead of solid.
  Dashes toggle on/off · up to 3 dash + 3 gap value pairs (AI: dash1/gap1/dash2/gap2/dash3/gap3) · each value editable in document units · pattern repeats · interacts with cap style (round caps = dotted look on 0-length dash) · dash phase/offset (advanced)
- ⬜ **Dash corner/end alignment** · _standard_ — Controls how the dash pattern is distributed so dashes land cleanly on corners and ends.
  AI two modes: 'preserve exact dash & gap lengths' vs 'align dashes to corners & path ends, adjusting lengths to fit' · prevents ugly partial dashes at corners · auto-stretches dashes evenly · 2-button toggle next to dash fields
- ⬜ **Arrowheads (start & end)** · _standard_ — Decorative caps/markers placed at the start and/or end of an open path.
  Start-marker picker + End-marker picker (independent dropdowns) · arrowhead library (arrows, circles, squares, bars, feathers… AI has ~39) · per-end scale % · independent start/end scale · flip/swap start↔end button · align arrowhead tip: to path end vs beyond path end (2-button) · custom arrowheads from symbols (advanced)
- ⬜ **Stroke order (above/below fill)** · _standard_ — Whether the stroke renders on top of the fill or behind it.
  Default: stroke above fill (half the weight covers fill edge) · Affinity 'Stroke behind fill' toggle (full weight visible, fill on top) · also stroke-above-text vs below · affects perceived weight & color blending at edges
- ⬜ **Scale stroke & effects with object** · _standard_ — Whether stroke weight (and dashes/effects) scale when the object is resized.
  Global preference + per-transform toggle ('Scale Strokes & Effects') · ON: 2pt stroke → 4pt at 200% · OFF: stroke weight stays constant · also governs dash/corner-effect scaling · matters for non-uniform scale · UI in transform panel / prefs
- 🟡 **Stroke on text / shared stroke** · _standard_ — Applying strokes to live text and other non-path objects, and stroke targeting.
  Stroke on live editable text (outline letters) · stroke on groups/symbols via Appearance · fill/stroke target toggle (which one swatches affect) · stroke inherits at group vs object level · stroke 'X'/none default key (D for default fill+stroke, X to swap)
- ⬜ **Outline stroke / expand** · _standard_ — Convert a stroked path into a filled compound shape (the stroke becomes real geometry).
  Object > Path > Outline Stroke · turns weight/caps/joins/dashes/arrowheads/width-profile into editable filled vector outlines · needed for boolean ops on strokes, export fidelity, non-scaling guarantees · irreversible (bakes appearance)
- ⬜ **Variable width / width profiles** · _advanced_ — Stroke whose weight varies along the path (thick-to-thin, tapered, bulging).
  Width Tool — drag any point on stroke to add a width point & set side handles · per-point width (symmetric or asymmetric in/out) · add/move/delete/duplicate width points · width profile dropdown (Uniform + 6 built-in AI profiles) · save custom profile to list · flip profile along/across · reset to uniform
- ⬜ **Pressure / tablet input** · _advanced_ — Stroke width (and opacity) driven by stylus pressure when drawing.
  Pressure-sensitive width on pencil/paintbrush/blob brush · tilt & bearing input (advanced) · velocity-based width (Affinity) · pressure curve editor · min/max width mapping · stabilization/smoothing · falls back to width-profile data on the resulting path
- ⬜ **Multiple strokes (stroke stack)** · _advanced_ — Stacking several strokes on one object via the Appearance panel for layered outlines.
  Add stroke / duplicate stroke · per-stroke weight/color/align/dash/opacity/blend · reorder strokes in stack · offset each via transform effect · toggle visibility per stroke · enables double-outline, inline, neon effects
- ⬜ **Brush vs stroke distinction** · _advanced_ — Brushes replace the plain stroke with art/pattern/scatter/bristle marks along the path.
  Brush types: Calligraphic, Scatter, Art, Bristle, Pattern (AI) + Image/textured brushes (Affinity) · brush applied as the path's stroke (still editable vector path) · brush library/panel · per-brush options (angle, roundness, scatter, spacing) · 'Remove Brush Stroke' reverts to plain stroke · brush scales with weight
- ⬜ **Corner / dash rounding controls** · _advanced_ — Fine controls for how strokes negotiate sharp corners and short segments.
  Round-join radius interplay with corner-widget radius · dash gap on very tight corners · hairline minimum render (1px floor at any zoom) · zoom-independent stroke preview option · pixel-grid stroke alignment (pixel preview)
- ⬜ **Stroke presets / graphic styles** · _advanced_ — Saving and reapplying a full stroke configuration as a reusable style.
  Save stroke (weight+color+caps+joins+dash+arrowheads+profile) as Graphic Style · apply to other objects · stroke swatch presets · eyedropper picks up full stroke appearance · update style propagates to all users

## 5. Text / Type system
*Every text object kind, the Character/Paragraph/OpenType/Glyphs panels, styles, font management, and text conversion that a pro vector tool (Illustrator + Affinity Designer) exposes.*  
(63 items — ✅0 · 🟡1 · ⬜62 · ❔0)

- ⬜ **Point text (artistic text)** · _core_ — Click-and-type text anchored at a single point that grows freely without wrapping
  Click to set insertion point · type freely (no auto-wrap, line breaks only on Enter) · resizing the bounding box scales the type (Illustrator) · Affinity calls it 'Artistic Text' · single baseline anchor
- ⬜ **Area text (paragraph/frame text)** · _core_ — Text flowed inside a closed shape/frame that wraps to the container width
  Drag a rectangle or convert any closed path into a text frame · auto-wraps to frame · resize frame reflows (not scales) text · 'Frame Text' in Affinity · place text into any vector shape
- ⬜ **Text/insertion cursor & selection editing** · _core_ — In-place caret editing with character/word selection
  Blinking caret · click to place · drag/shift to select range · double-click word · triple-click line/paragraph · Ctrl/Cmd+A select all · keyboard navigation (arrows, home/end, word-jump) · cut/copy/paste (plain & with formatting)
- ⬜ **Font family selector** · _core_ — Choose the typeface for selected text
  Searchable dropdown · live preview in list · favorites/starred · recently used · filter by classification (serif/sans/script/mono) · 'similar fonts' · variable-font detection · system + document + (cloud) fonts
- ⬜ **Font style / weight selector** · _core_ — Pick the named instance within a family (Regular, Bold, Italic, etc.)
  Style dropdown (Thin…Black, Italic, Condensed) · only valid styles for chosen family · faux bold / faux italic fallback when style absent
- ⬜ **Font size** · _core_ — Set type size
  Numeric field + stepper · unit (pt/px/mm) · preset size list · increase/decrease shortcut (e.g. Ctrl+Shift+>/<) · auto-size for area text option
- ⬜ **Leading (line spacing)** · _core_ — Vertical distance between baselines
  Numeric value · 'Auto' leading (% of size, default ~120%) · multiple/exact modes · leading applies to line or paragraph · keyboard nudge
- ⬜ **Tracking (letter-spacing)** · _core_ — Uniform spacing across a range of characters
  Numeric value (1/1000 em) · applies to selection · keyboard nudge (Alt+arrows) · distinct from kerning
- ⬜ **Underline** · _core_ — Line beneath text
  Toggle · (advanced) underline weight/offset/color/gap-over-descenders in some tools
- ⬜ **Per-character fill & stroke** · _core_ — Apply color/stroke to selected text runs
  Fill color & stroke color/weight on text selection · gradient/pattern fill on type · stroke alignment · independent of object-level style
- ⬜ **Paragraph alignment** · _core_ — Horizontal alignment of lines within a paragraph
  Left · Center · Right · Justify (last line left/center/right) · Justify all lines (full) · align toward/away from spine for RTL
- ⬜ **Text-to-outlines (Create Outlines)** · _core_ — Convert live text into editable vector paths
  Each glyph -> compound path · destructive (loses editability) · keeps fill/stroke/appearance · per-selection · used for handoff/effects/missing-font safety
- ⬜ **Area-text overflow / overset indicator** · _standard_ — Visual flag (red plus / overflow port) when text exceeds the frame
  Red '+' out-port marker · overset text hidden until frame grown or threaded · click port to load cursor for threading
- ⬜ **Text threading / linked frames** · _standard_ — Flow a single text story across multiple linked frames
  In-port / out-port on each frame · click out-port then click next frame (or new drag) to link · thread arrows shown · release/break thread · reflow propagates across chain · multi-page/multi-column stories
- ⬜ **Area-text columns & rows** · _standard_ — Subdivide a text frame into multiple columns (and rows) with gutter
  Number of columns / rows · gutter spacing · column width · 'Area Type Options' (Illustrator) · flow direction (by rows / by columns) · text-flow order
- ⬜ **Text inset / frame margins (padding)** · _standard_ — Inner padding between frame edge and text
  Inset spacing (uniform or per-side top/left/bottom/right) · first-baseline offset option (ascent/cap-height/leading/x-height/fixed) · min first-baseline value
- ⬜ **Vertical alignment in frame** · _standard_ — Align text block vertically within an area frame
  Top · center · bottom · justify (distribute lines) · independent of horizontal paragraph alignment
- ⬜ **Type on a path** · _standard_ — Text that flows along the outline of any open or closed path
  Click a path with type-on-path tool · start/end/center brackets to slide & flip text along path · flip to other side of path · path-type options: effect (Rainbow/Skew/3D Ribbon/Stair Step/Gravity), align-to-path (ascender/descender/center/baseline), spacing around curves
- ⬜ **Kerning** · _standard_ — Space between a specific pair of characters
  Auto / Metrics (font's kern table) · Optical (visually computed) · Manual value (between two glyphs at caret) · units of 1/1000 em
- ⬜ **Horizontal scale** · _standard_ — Stretch/condense glyph width without changing height
  Percent value (100% = normal) · distorts glyphs (not true condensed) · reset to 100%
- ⬜ **Vertical scale** · _standard_ — Stretch/condense glyph height without changing width
  Percent value (100% = normal) · independent of horizontal scale · reset
- ⬜ **Baseline shift** · _standard_ — Raise/lower characters relative to the baseline
  Positive (up) / negative (down) value in points · for sub/superscript tweaks, on-path lift, mixed-size alignment · keyboard nudge (Alt+Shift+arrows)
- ⬜ **Case transform (caps)** · _standard_ — Change letter casing as a non-destructive attribute
  All Caps · Small Caps (true OT small caps vs synthesized) · sentence/lowercase/uppercase/title-case commands (Change Case) · preserves underlying characters
- ⬜ **Superscript / subscript** · _standard_ — Raised/lowered smaller text
  Superscript & subscript toggles · uses OT feature if present else synthesized (scaled + baseline-shifted) · ordinals
- ⬜ **Strikethrough** · _standard_ — Line through text
  Toggle · single line through x-height · (advanced) weight/offset controls
- ⬜ **Language assignment** · _standard_ — Tag text run with a language for hyphenation & spell-check
  Per-character/paragraph language · drives hyphenation dictionary · spell-check · locale-specific OT behavior
- ⬜ **Indents** · _standard_ — Paragraph edge offsets
  Left indent · right indent · first-line indent (positive or hanging/negative) · per-paragraph
- ⬜ **Space before / after paragraph** · _standard_ — Vertical gap between paragraphs
  Space-before value · space-after value · in points · distinct from leading · 'space between same-style paragraphs' (advanced)
- ⬜ **Hyphenation** · _standard_ — Auto-break long words at line ends
  On/off · words longer than N letters · min letters before/after break · max consecutive hyphens · hyphenation zone · capitalized-words toggle · language dictionary · discretionary/manual hyphens
- ⬜ **OpenType features panel** · _standard_ — Toggle typographic features baked into the font
  Standard/discretionary ligatures · contextual alternates · swashes · stylistic alternates · titling alts · ordinals · fractions · stylistic sets (ss01–ss20) · figure style (lining/oldstyle, proportional/tabular) · superscript/subscript/numerator/denominator · slashed zero · positional (init/medial/final/isolated) forms
- ⬜ **Ligatures** · _standard_ — Combine character pairs into single glyphs
  Standard ligatures (fi, fl) on/off · discretionary ligatures (st, ct) · controlled via OT panel or character menu
- ⬜ **Glyphs panel** · _standard_ — Browse and insert every glyph in a font
  Full glyph grid · filter by category (punctuation/symbols/arrows) · access alternates for selected glyph · recently used · search by name/Unicode · insert at caret · pick alternate via in-context flyout under selected glyph
- ⬜ **Special characters / insert glyph** · _standard_ — Insert symbols, dashes, spaces, marks
  Em/en dash · em/en/thin/hair/non-breaking spaces · curly quotes · ellipsis · bullet · copyright/trademark · section/paragraph marks · soft/hard returns · discretionary hyphen
- ⬜ **Smart punctuation / typographer's quotes** · _standard_ — Auto-convert straight quotes & dashes to typographic forms
  Straight->curly quotes · -- -> en/em dash · ... -> ellipsis · (c)->© · per-language quote style · 'Smart Punctuation' batch command
- ⬜ **Character styles** · _standard_ — Reusable named sets of character-level formatting
  Define from selection · apply to runs · override indicator & clear-override · based-on/parent style · redefine from current · attributes: font/size/leading/kern/track/case/color etc.
- ⬜ **Paragraph styles** · _standard_ — Reusable named sets of paragraph + character formatting
  Whole-paragraph style · next-style chaining · based-on hierarchy · override clearing · includes alignment/indents/spacing/hyphenation + base character attrs · default '[Normal]' style
- ⬜ **Find/Replace font** · _standard_ — Locate and swap fonts document-wide
  List all fonts used · show missing fonts · replace one font with another across document · filter by font type · replace all instances
- ⬜ **Missing-font handling / substitution** · _standard_ — Resolve fonts not installed when opening a file
  Highlight missing-font text (pink) · substitute font · resolve via cloud/activate · keep original reference · per-style remap dialog
- ⬜ **Find / Replace text & spell check** · _standard_ — Search and edit text content across the document
  Find/change text · case-sensitive/whole-word/regex (GREP-style advanced) · find next/change/change-all · spell check with dictionary · auto-correct · custom dictionary
- ⬜ **Convert point<->area text** · _standard_ — Switch a text object between point and area modes
  Point-text -> area (and back) · auto-size area text · widget/double-click on frame edge to convert (Illustrator)
- ⬜ **Auto-size / fit options** · _standard_ — Frame or type that resizes to content
  Auto-size area text height/width · 'fit text to frame' / shrink-to-fit · grow-with-text · superscript-safe
- ⬜ **Placeholder / fill with text** · _standard_ — Insert dummy text quickly for layout
  Fill with lorem ipsum / placeholder · 'Fill With Placeholder Text' command · auto-on-create option
- ⬜ **Show hidden characters** · _standard_ — Reveal non-printing marks while editing
  Spaces · tabs · paragraph/line returns · end-of-story · em/en space markers · toggle view
- 🟡 **Font size / preview & sampling shortcuts** · _standard_ — Convenience controls around type
  Eyedropper to copy type attributes between text objects · 'sample text' formatting copy · font-preview size setting in menu
- ⬜ **Text on export / PDF text preservation** · _standard_ — Keep type editable/searchable in exports
  Embed/subset fonts on PDF/SVG export · live text vs outlined-on-export option · SVG <text> vs <path> · selectable PDF text
- ⬜ **Vertical text / writing direction** · _advanced_ — Type set top-to-bottom (CJK) and overall text-direction control
  Vertical type tool & vertical area type · tate-chu-yoko (horizontal-in-vertical) · LTR vs RTL paragraph direction · per-story writing mode
- ⬜ **RTL / bidirectional text & Arabic shaping** · _advanced_ — Right-to-left scripts with correct contextual letter joining
  RTL paragraph direction · bidi mixing LTR+RTL runs · Arabic/Hebrew · contextual glyph shaping (init/medial/final/isolated) · kashida/justification elongation · (flagged as Varos moat, currently broken in Penpot era)
- ⬜ **Character rotation** · _advanced_ — Rotate individual selected characters
  Per-character rotation angle (degrees) · useful in vertical text · distinct from rotating the whole text object
- ⬜ **Anti-aliasing / hinting mode** · _advanced_ — How text edges render
  None / Sharp / Crisp / Strong (Illustrator) · pixel-grid snapping · per-text-object setting · affects raster export
- ⬜ **Justification settings** · _advanced_ — Fine control of spacing when text is justified
  Min/desired/max word spacing · letter spacing · glyph scaling ranges · auto-leading % · single-line vs every-line (paragraph) composer
- ⬜ **Composer (line-breaking engine)** · _advanced_ — Algorithm choosing line breaks across a paragraph
  Single-line composer (greedy) vs Every-line/paragraph composer (Knuth-Plass-style optimal) · affects rag & spacing evenness
- ⬜ **Tab stops & ruler** · _advanced_ — Align text at defined tab positions
  Left/center/right/decimal tabs · tab leader (dots) · tab ruler UI · per-paragraph tab list · indent markers on ruler
- ⬜ **Bullets & numbered lists** · _advanced_ — List formatting for paragraphs
  Bullet glyph/char · numbered (1,2,3 / a,b,c / i,ii) · list level/indent · marker-to-text spacing · restart/continue numbering
- ⬜ **Drop caps** · _advanced_ — Enlarged initial letter spanning N lines
  Number of lines tall · number of characters · drop-cap style override
- ⬜ **Hanging punctuation / optical margin alignment** · _advanced_ — Push punctuation past the margin for clean edges
  Roman hanging punctuation · optical margin alignment (point-size-based) · per-story toggle
- ⬜ **No-break / keep options** · _advanced_ — Prevent breaks within runs and control paragraph splitting
  No-break attribute on selection · keep-with-next lines · keep-lines-together · widow/orphan control · start-paragraph options
- ⬜ **Fractions & figure styles** · _advanced_ — Numeric typesetting variants
  Diagonal fractions · lining vs oldstyle figures · proportional vs tabular figures · numerator/denominator · ordinals
- ⬜ **Variable font axes** · _advanced_ — Continuous sliders for variable-font design axes
  Weight (wght) · width (wdth) · slant (slnt) · optical size (opsz) · italic (ital) · custom/registered axes · named instances · live interpolation
- ⬜ **Style import / sync from library** · _advanced_ — Bring text styles across documents/shared library
  Load styles from another document · shared/team library text styles · conflict resolution on import
- ⬜ **Touch Type tool** · _advanced_ — Directly manipulate individual characters while text stays live
  Select one glyph in a live string · move/scale/rotate it via on-screen handles · adjusts baseline-shift/kern/scale/rotation under the hood · text remains editable
- ⬜ **Text wrap around objects** · _advanced_ — Flow area text around overlapping shapes/images
  Wrap on/off per object · offset/inset distance · wrap shape (bounding box / object shape / alpha) · invert wrap · jump/jump-to-next-column options
- ⬜ **Import / place text files** · _advanced_ — Bring external text into the document
  Place .txt/.rtf/.docx · with/without formatting · encoding choice · paste plain vs paste with styling · import options (clean breaks/quotes)
- ⬜ **Baseline grid / snapping** · _advanced_ — Snap text baselines to a document grid
  Document baseline grid (start offset + increment) · align-to-grid first-line/all-lines per paragraph · show/hide grid

## 6. Arrange systems
*All systems for precisely arranging, positioning, transforming, aligning, distributing, and combining objects: numeric Transform, Align & Distribute, Pathfinder, and the per-object/repeat/path-derivation transform commands.*  
(34 items — ✅9 · 🟡7 · ⬜18 · ❔0)

- ⬜ **Transform panel — position (X / Y)** · _core_ — Numerically read/set the selected object's location on the canvas/artboard.
  X & Y fields type-or-scrub · origin measured from the active 9-point reference point · respects document ruler units (px/pt/mm/in/etc.) · supports math in field (e.g. 100+20) · negative values · relative move via reference-point offset · per-artboard vs global coordinate origin
- ⬜ **Transform panel — dimensions (W / H)** · _core_ — Numerically read/set the object's bounding-box width and height.
  W & H fields type-or-scrub · % or absolute units · uses geometric vs visual (stroke/effect-inclusive) bounds · math in field · enter to commit · paired with constrain-proportions lock
- 🟡 **Constrain proportions (aspect lock)** · _core_ — Lock W:H so resizing one dimension scales the other proportionally.
  Chain-link toggle between W & H · also engaged by Shift while dragging a corner handle · remembers current aspect ratio · applies to typed values and handle drags
- 🟡 **Rotate (angle field)** · _core_ — Rotate the selection by an exact angle around the reference point.
  Angle entry (degrees, CCW positive in AI) · rotates about active 9-point reference point · Shift-drag handle = 45° increments · separate Rotate tool with click-to-set-pivot (Alt-click opens dialog) · negative angles · cumulative vs absolute
- ✅ **Align objects — 6-way** · _core_ — Align selected objects along a common horizontal or vertical edge/center.
  Align Left / Center (horizontal) / Right · Align Top / Middle (vertical) / Bottom · operates on bounding boxes · single-click buttons · respects Align To target
- ✅ **Distribute objects (centers/edges)** · _core_ — Even out spacing of object reference edges along an axis.
  Vertical Distribute: Top / Center / Bottom · Horizontal Distribute: Left / Center / Right · equalizes chosen edge gaps across 3+ objects · works on bounding boxes
- ✅ **Pathfinder — Shape Mode: Unite** · _core_ — Merge selected shapes into one combined outline.
  Union of all overlapping/adjacent shapes · adopts front object's appearance · Alt-click = live/non-destructive Compound Shape (re-editable) vs plain click = expanded path
- ✅ **Pathfinder — Shape Mode: Minus Front** · _core_ — Subtract front (upper) shapes from the backmost shape.
  Backmost object minus all in front · keeps back object's fill · Alt = live compound shape · order-dependent (z-order matters)
- ✅ **Pathfinder — Shape Mode: Intersect** · _core_ — Keep only the overlapping region of all selected shapes.
  Result = common area of all shapes · removes non-overlapping parts · Alt = live compound shape · needs actual overlap
- ✅ **Z-order arrange (front/back)** · _core_ — Change stacking order of objects within their group/layer.
  Bring to Front · Bring Forward · Send Backward · Send to Back · Paste in Front/Back · shortcuts Ctrl+] / Ctrl+[ etc. · operates within layer context
- ✅ **Group / Ungroup (nested)** · _core_ — Combine objects into a single selectable unit; reverse it.
  Group (Ctrl+G) · Ungroup (Ctrl+Shift+G) · nested groups · isolation-mode enter/exit for editing inside · group stays in arrange/align as one bbox
- ⬜ **Shear / Skew (angle field)** · _standard_ — Slant the object along an axis by an exact angle.
  Shear angle field in Transform panel · separate Shear tool with axis & angle dialog · horizontal/vertical/angled shear axis · about reference point · combines with rotation
- ⬜ **9-point reference point (origin/anchor)** · _standard_ — Choose which of 9 bbox points all transforms pivot/measure from.
  3x3 anchor grid widget (corners/edges/center) · sets origin for X/Y readout, rotate pivot, scale anchor, shear axis · default = top-left or center per tool · persists per session
- ⬜ **Scale Strokes & Effects** · _standard_ — Toggle whether stroke weight and effects scale with the object.
  Checkbox in Transform panel flyout + Preferences · ON = 2x object doubles stroke weight; OFF = stroke stays fixed · affects effects (shadows, etc.) and corner radii · global default in prefs, per-action override
- ⬜ **Transform panel flyout options** · _standard_ — Panel menu of transform-related toggles and quick commands.
  Scale Strokes & Effects · Scale Corners (live rectangle radius) · Align to Pixel Grid · Transform Object Only / Pattern Only / Both (for filled patterns) · Flip Horizontal / Flip Vertical commands
- ⬜ **Flip Horizontal / Vertical (Reflect)** · _standard_ — Mirror the selection across a vertical or horizontal axis.
  Quick Flip H / Flip V (about reference point) · full Reflect tool with arbitrary axis angle + click-to-set axis · Alt-click = reflect+copy · Object > Transform > Reflect dialog with preview & Copy
- ⬜ **Move dialog (precise offset)** · _standard_ — Move selection by exact horizontal/vertical distance or distance+angle.
  Object > Transform > Move (Enter on a transform tool) · Horizontal/Vertical OR Distance+Angle inputs · Preview · Copy button (move-and-duplicate) · respects units
- ⬜ **Scale dialog (uniform / non-uniform)** · _standard_ — Scale by exact percentage uniformly or per-axis.
  Uniform % OR Horizontal/Vertical % · Scale Strokes & Effects checkbox · Scale Corners · Transform Objects / Patterns · Preview · Copy · separate Scale tool with click-to-set-origin
- ⬜ **Distribute Spacing (exact gap value)** · _standard_ — Set equal gaps between objects, optionally to an exact entered value.
  Vertical & Horizontal Distribute Spacing buttons · numeric spacing field for precise inter-object gap (e.g. 20px) · requires a key object when using exact value · measures clear space between bounds
- ⬜ **Align To: Selection / Key Object / Artboard** · _standard_ — Choose the reference frame everything aligns/distributes against.
  Align to Selection (group bbox) · Align to Key Object (click one object to anchor; outlined; it stays put) · Align to Artboard (current artboard bounds) · Cancel Key Object · dropdown in Align panel/control bar
- ✅ **Pathfinder — Shape Mode: Exclude (Exclude Overlap)** · _standard_ — Remove overlapping regions, keeping non-overlapping areas (even-odd).
  Even-odd fill rule on combined shape · overlaps punch holes · Alt = live compound shape · useful for donut/cutout effects
- 🟡 **Pathfinder — Divide** · _standard_ — Cut all shapes into separate non-overlapping faces along every intersection.
  Splits into discrete closed regions grouped together · each face independently selectable · keeps fills · removes strokes by default · key for slicing artwork
- 🟡 **Pathfinder — Trim** · _standard_ — Remove hidden (overlapped) parts of back shapes; keep fills, drop strokes.
  Front object stays whole, back objects clipped by what's in front · no merging of same-color · strokes removed · grouped result
- 🟡 **Pathfinder — Merge** · _standard_ — Trim hidden areas AND merge adjacent same-color shapes.
  Like Trim but unites abutting shapes sharing identical fill · differing colors stay separate (clipped) · strokes removed · grouped
- 🟡 **Pathfinder — Crop** · _standard_ — Use the frontmost shape as a mask, keeping only what's inside it.
  Topmost object = crop boundary; everything outside discarded · removes strokes · flattens to clipped fills · non-mask alternative to clipping mask
- ✅ **Pathfinder — Minus Back** · _standard_ — Subtract all back (lower) shapes from the frontmost shape.
  Frontmost object minus everything behind it · keeps front appearance · inverse of Minus Front · order-dependent
- ⬜ **Transform Again** · _standard_ — Repeat the last transform (incl. duplicate) once more.
  Cmd/Ctrl+D · re-applies last move/scale/rotate/reflect/transform-each · with a move+copy creates evenly spaced duplicate arrays (step-and-repeat) · repeatable for patterns
- ⬜ **Offset Path** · _standard_ — Create a concentric copy of a path inset/outset by an exact distance.
  Object > Path > Offset Path · Offset distance (+ outward / - inward) · Joins: Miter / Round / Bevel · Miter limit · Preview · works on open & closed paths · creates new path (non-destructive to original)
- ⬜ **Outline Stroke** · _standard_ — Convert a stroked line into a filled shape matching the stroke's width.
  Object > Path > Outline Stroke · turns weight+caps+joins+dashes into a closed fillable outline · enables independent fill/stroke on the outline · irreversible (destructive) · respects variable/profile widths
- 🟡 **Free Transform tool / widget** · _standard_ — Interactive handle-based combined scale/rotate/shear/distort.
  Bounding-box handles for scale & rotate · modifier keys for shear · Perspective Distort & Free Distort sub-modes (drag corners independently) · touch widget in AI · constrain with Shift
- ⬜ **Pathfinder — Outline** · _advanced_ — Convert shape edges into separate open stroked path segments.
  Divides outlines at intersections into line segments · fills become 0-width strokes inheriting object color · useful for line art from regions
- ⬜ **Pathfinder options / expand** · _advanced_ — Settings governing pathfinder precision and compound-shape behavior.
  Precision (path approximation) · Remove Redundant Points · Divide & Outline Will Remove Unpainted Artwork · Expand button (commit a live compound shape to a real path) · Release compound shape
- ⬜ **Transform Each** · _advanced_ — Apply scale/move/rotate to each selected object about its OWN origin.
  Dialog: Scale H/V% · Move H/V · Rotate angle · per-object reference point (9-pt) · Random checkbox (scatter/jitter values) · Reflect X/Y · Preview · Copy · vs normal transform which uses one shared origin
- ⬜ **Reset bounding box** · _advanced_ — Re-orient a rotated object's bounding box back to axis-aligned.
  Object > Transform > Reset Bounding Box · realigns handles to 0° after rotation so W/H read true · doesn't change appearance

## 7. Structure systems
*Document hierarchy and organization: the Layers panel and its full feature set, object naming, grouping, isolation, masking, z-order stacking, lock/hide visibility, and selection-propagation (select same/similar).*  
(35 items — ✅6 · 🟡9 · ⬜20 · ❔0)

- ⬜ **Layers panel (panel shell)** · _core_ — Dockable panel listing the document's stacking hierarchy as the canonical structure UI.
  scrollable list · panel menu (flyout) · row height/small-list toggle · search/filter rows (AI/Affinity) · panel options dialog (row size, thumbnail size, show only layers vs all objects)
- ⬜ **Layers (top-level)** · _core_ — Top-level containers that hold objects and sublayers; the primary organizational unit.
  create new layer · delete layer · duplicate layer · layer color assignment · double-click to open Layer Options (name/color/template/lock/show/print/preview/dim images)
- ⬜ **Sublayers & nested object rows** · _core_ — Nested layers and individual object/group rows under a parent layer, forming a tree.
  expand/collapse disclosure triangle · new sublayer · object rows auto-appear under their layer · groups as expandable rows · arbitrary nesting depth · indent guides
- 🟡 **Per-row visibility toggle (eye)** · _core_ — Show/hide an individual layer, sublayer, group, or object.
  eye icon column · click toggles · Alt/Opt-click = isolate (hide all others) · drag down the eye column to toggle many · cascades to children · hidden items skip render+hit-test
- 🟡 **Per-row lock toggle** · _core_ — Lock a row so its contents can't be selected or edited on canvas.
  lock/padlock column (empty until set) · click toggles · drag down column to lock many · cascades to children · locked items not click-selectable · stays in z-order
- 🟡 **Reorder by drag (restack)** · _core_ — Drag rows up/down/into to change stacking order and re-parent.
  drag within a layer = restack z-order · drag onto another layer = move objects there · drop-line indicator · drag into a group/layer to nest · multi-row drag · z-order is reflected on canvas
- ✅ **Group** · _core_ — Combine selected objects into a single nestable container that moves/transforms as one.
  Object > Group (Ctrl/Cmd-G) · grouping groups = nesting one level · group becomes a row in Layers · group shares a bounding box · groups can carry shared appearance/opacity
- ✅ **Ungroup** · _core_ — Dissolve a group one level, returning children to the parent context.
  Object > Ungroup (Shift-Ctrl/Cmd-G) · peels exactly one nesting level · repeat to fully ungroup · children rise to parent layer/group · preserves relative z-order
- ✅ **Z-order: Bring to Front** · _core_ — Move selection to the very top of the stacking order.
  Object > Arrange > Bring to Front (Shift-Ctrl/Cmd-]) · top of layer/group context · multi-select keeps relative order
- ✅ **Z-order: Bring Forward** · _core_ — Move selection up one step in the stacking order.
  Object > Arrange > Bring Forward (Ctrl/Cmd-]) · single-step raise · within current container
- ✅ **Z-order: Send Backward** · _core_ — Move selection down one step in the stacking order.
  Object > Arrange > Send Backward (Ctrl/Cmd-[) · single-step lower · within current container
- ✅ **Z-order: Send to Back** · _core_ — Move selection to the very bottom of the stacking order.
  Object > Arrange > Send to Back (Shift-Ctrl/Cmd-[) · bottom of layer/group context · relative order preserved
- 🟡 **Select All / Deselect / Inverse / re-select** · _core_ — Global selection commands operating across the active artboard or document.
  Select All (Ctrl/Cmd-A) · All on Active Artboard · Deselect (Shift-Ctrl/Cmd-A) · Reselect (Ctrl/Cmd-6) · Inverse · Next/Previous Object Above/Below (Alt-Ctrl/Cmd-]/[)
- ⬜ **Selection-color indicator (square)** · _standard_ — Colored square at row's right edge marking on-canvas selection and that layer's selection-bbox color.
  appears when a row's object is selected · color = the layer's assigned color (also tints bounding box/anchors/paths on canvas) · click-drag the square to move selection between layers
- ⬜ **Row thumbnails** · _standard_ — Small live preview swatch of each layer/object's contents in its row.
  rendered mini-preview · size adjustable in panel options · can be turned off for performance · updates live with edits
- ⬜ **Locate object in panel** · _standard_ — Jump the Layers panel to highlight/scroll-to the row of the currently selected canvas object.
  panel-menu 'Locate Object' · auto-expands ancestor rows/groups · scrolls into view · optional 'Locate Layer for ...' setting on selection
- ⬜ **Flatten Artwork / Merge layers** · _standard_ — Collapse multiple layers into one (flatten) or merge selected layers.
  Flatten Artwork (all → one) · Merge Selected (chosen → topmost selected) · preserves visual stacking · destructive to layer structure
- 🟡 **Object naming / inline rename** · _standard_ — Give layers, groups, and objects custom names for organization and export.
  double-click row label to rename · default labels by type (<Path>, <Group>, <Rectangle>, <Compound Path>) · names drive Asset Export slices/SVG ids · auto-name vs custom-name distinction
- 🟡 **Add to / select within group** · _standard_ — Place new objects into the active group and select group members directly.
  draw-inside active group · Group Selection tool (AI) adds one nesting level per click · double-click to enter group (isolation) · group members editable without ungrouping
- ⬜ **Isolation mode** · _standard_ — Temporarily isolate a group/sublayer/object so only it is editable, dimming everything else.
  double-click group to enter · isolation breadcrumb bar at top · everything outside dimmed + locked · Esc / click empty / breadcrumb to exit · nested entry (drill deeper) · new art added to isolated group
- ⬜ **Clipping mask (clip path)** · _standard_ — Use the topmost object's shape to crop the visibility of objects beneath it.
  Object > Clipping Mask > Make (Ctrl/Cmd-7) · topmost vector = clip path (becomes no-fill/stroke) · Release · Edit Contents vs Edit Mask · clip set/group container · layer-level clip mask · masked content still editable
- ⬜ **Paste in Front / in Back** · _standard_ — Paste copied art directly above or below the current selection in z-order (and same position).
  Paste in Front (Ctrl/Cmd-F) · Paste in Back (Ctrl/Cmd-B) · keeps exact x/y position · interacts with Paste Remembers Layers · also Paste in Place / on All Artboards
- 🟡 **Lock Selection / Unlock All** · _standard_ — Lock the selected objects from canvas editing and release all locks.
  Object > Lock > Selection (Ctrl/Cmd-2) · Lock All Artwork Above · Lock Other Layers · Unlock All (Alt-Ctrl/Cmd-2) · complements per-row lock column
- 🟡 **Hide Selection / Show All** · _standard_ — Hide the selected objects from view and reveal all hidden objects.
  Object > Hide > Selection (Ctrl/Cmd-3) · Hide All Artwork Above · Hide Other Layers · Show All (Alt-Ctrl/Cmd-3) · complements per-row eye column
- ⬜ **Select Same** · _standard_ — Select all objects in the document sharing an attribute with the current selection.
  Select > Same > Fill Color / Stroke Color / Stroke Weight / Opacity / Blending Mode / Fill & Stroke / Appearance / Graphic Style / Shape / Symbol Instance · right-click contextual menu · eyedropper-driven matching
- 🟡 **Compound path / compound shape (structure)** · _standard_ — Multiple subpaths treated as one object with even-odd holes; a structural container in the tree.
  Object > Compound Path > Make (Ctrl/Cmd-8) / Release · holes via even-odd or non-zero winding · shows as single <Compound Path> row · distinct from boolean compound shapes (live Pathfinder)
- ⬜ **Move selection between layers (drag dot)** · _standard_ — Relocate selected objects to a different layer/group without changing canvas position.
  drag the selection-color square in Layers to a target row · or cut + Paste in Place with Paste Remembers Layers off · preserves position, changes parent
- ⬜ **Target / appearance column (meatball)** · _advanced_ — Target indicator (hollow/filled ring) showing what's selected and where appearance is applied.
  double-ring icon = has appearance attributes · hollow vs filled = targeted/selected state · click to target a row for the Appearance panel · shaded ring = contains targeted children (AI-specific)
- ⬜ **Collect in New Layer** · _advanced_ — Gather the selected rows into a single newly-created layer.
  panel-menu command · works on multi-selected layers/objects · preserves relative stacking · creates parent layer
- ⬜ **Release to Layers (sequence/build)** · _advanced_ — Distribute a group's/layer's children into separate layers, optionally cumulatively (for animation builds).
  Release to Layers (Sequence) · Release to Layers (Build) · one object per layer · cumulative-stack option · used for frame/animation export
- ⬜ **Paste Remembers Layers** · _advanced_ — Toggle so pasted objects return to their source layer instead of the active layer.
  panel-menu checkbox (global pref) · on = paste into originating layer (creates it if missing) · off = paste into active layer · interacts with Paste in Place/in Front/in Back
- ⬜ **Opacity mask (luminosity mask)** · _advanced_ — Use a grayscale object's luminosity to control the transparency of the masked content.
  Transparency panel > Make Mask · black=hidden, white=visible, gray=partial · Clip checkbox · Invert Mask · Link/unlink mask-to-art · click mask thumbnail to edit · release mask
- ⬜ **Select Object / Select Similar (by kind)** · _advanced_ — Select all objects of a category regardless of attributes.
  Select > Object > All on Same Layers / Direction Handles / Bristle Brush Strokes / Brush Strokes / Clipping Masks / Stray Points / Text Objects / All Text Objects · Affinity 'Select Similar'
- ⬜ **Save / load selections** · _advanced_ — Store a named selection set to recall later.
  Select > Save Selection (names the current set) · Edit Selection (rename/delete) · recallable from the Select menu
- ⬜ **Template / guide / print layer flags** · _advanced_ — Special layer states: template (dimmed, locked, non-printing), non-printing, and dimmed images.
  Layer Options: Template (lock+dim, ignored on export) · Print on/off · Preview vs Outline per-layer view · Dim Images to % · used for tracing references

## 8. Artboard + Document system
*Everything for defining the canvas/page surface(s): multiple artboards and their tools/panel, presets & sizes, document setup (units/DPI/color/bleed/profiles), navigation-to-artboard, artboard visual options, and per-artboard export.*  
(32 items — ✅0 · 🟡1 · ⬜31 · ❔0)

- ⬜ **New Document dialog** · _core_ — Entry point that creates a document with an initial artboard and base settings.
  Profile/intent tabs (Print · Web · Mobile · Film&Video · Art&Illustration) · recent/saved presets · name field · width/height · units · orientation · number of artboards + arrange/spacing/rows · bleed · color mode (RGB/CMYK) · raster effects PPI · 'Create' / 'More Settings'
- ⬜ **Document Setup dialog** · _core_ — Edit document-wide settings after creation (File ▸ Document Setup).
  Units · bleed (all-sides / link-sides) · edit-artboards shortcut · transparency grid (show/colors/size) · simulate colored paper · type options (units, highlight) · point/pica size · 'Edit Artboards' button
- ⬜ **Units setting** · _core_ — Document/ruler measurement unit and per-field overrides.
  Global ruler unit (px · pt · pc/picas · in · mm · cm · ha) · separate type unit · stroke unit · per-field unit override by typing suffix (e.g. '10mm') · general/keyboard-increment unit
- ⬜ **Document dimensions & presets** · _core_ — Overall canvas/page size and reusable size presets.
  Width × height fields · link/lock aspect ratio · standard print (A-series A0–A6, B-series, US Letter/Legal/Tabloid/Ledger) · screen (1920×1080, common breakpoints) · mobile device presets (iPhone/Android/iPad) · social media sizes · custom saved preset
- ⬜ **Orientation** · _core_ — Portrait vs landscape toggle for document/artboard.
  Portrait · landscape · swaps width↔height · per-artboard orientation independent of document default
- ⬜ **Create artboard** · _core_ — Add a new artboard/page to the document.
  Artboard tool drag-to-draw · 'New Artboard' button in panel · duplicate-on-create · default-size new artboard · drag with content to clone-area · numeric entry of size on creation
- ⬜ **Delete artboard** · _core_ — Remove an artboard from the document.
  Delete key / panel trash · option to delete artboard only (keep art) vs delete with contents · cannot delete last artboard · renumber remaining artboards
- ⬜ **Resize artboard** · _core_ — Change a single artboard's dimensions.
  Drag handles with Artboard tool · W/H fields in control bar/Options · preset dropdown · 'Fit to Artwork Bounds' · 'Fit to Selected Art' · constrain proportions · reference-point anchor for resize
- ⬜ **Move artboard (with / without content)** · _core_ — Reposition an artboard on the canvas, optionally moving its art.
  Drag with Artboard tool · X/Y position fields · 'Move/Copy Artwork with Artboard' toggle (decides if contained objects move too) · arrow-key nudge · snap to other artboards
- ⬜ **Artboard tool** · _core_ — Dedicated tool (Shift+O) to create/edit/move/resize artboards.
  Enter/exit artboard-edit mode · draw new · drag/resize handles · control-bar options (presets, name, orientation, move-with-art, delete) · Esc to exit · interacts with rulers/guides
- ⬜ **Artboards panel** · _core_ — List manager for all artboards (add/delete/reorder/rename/navigate).
  Numbered list rows · double-click row = fit-in-window to that artboard · new/delete buttons · panel menu (duplicate, rearrange, options) · drag to reorder · select-multiple
- ⬜ **Fit-to-artboard navigation** · _core_ — Zoom/center the view on a specific artboard or all artboards.
  Fit Artboard in Window (Ctrl/Cmd+0) · Fit All in Window (Ctrl/Cmd+Alt+0) · double-click panel row to fit · next/previous artboard (PageUp/PageDown style) · 'Actual Size' 100% · artboard navigator/dropdown in status bar
- ⬜ **DPI / PPI (raster effects resolution)** · _standard_ — Resolution for rasterizing effects and export of pixel output.
  72 (screen) · 150 (medium) · 300 (high/print) · custom value · separate document raster-effects PPI vs export DPI · 'Use document raster resolution' option
- ⬜ **Bleed** · _standard_ — Extra printable margin beyond trim for print safety.
  Top/bottom/left/right values · link-all toggle · separate per-side values · bleed guide shown around artboard (red line) · included/excluded in export · per-artboard vs document-wide
- ⬜ **Color mode / format** · _standard_ — Document color model governing fills, strokes, swatches.
  RGB vs CMYK document mode · grayscale · changing mode converts existing colors · affects color picker channels · Web/screen→RGB, print→CMYK default · spot color support
- ⬜ **Transparency grid** · _standard_ — Checkerboard backdrop showing document transparency.
  Show/hide transparency grid · light/medium/dark presets · custom two colors · grid size (small/medium/large) · simulate colored paper option · per-document background
- ⬜ **Duplicate artboard** · _standard_ — Copy an artboard with or without its contents.
  Alt/Opt-drag with Artboard tool · 'Duplicate' in panel menu · move artboard with artwork toggle (copies contained objects) · offsets the copy
- ⬜ **Reorder / renumber artboards** · _standard_ — Change artboard order which drives export/print sequence.
  Drag rows in Artboards panel · 'Move Up/Down' · renumber sequentially · affects export filename ordering and PDF page order · 'Rearrange' re-sorts by order
- ⬜ **Rename artboard** · _standard_ — Give each artboard a custom label.
  Double-click name in panel · Name field in Artboard Options / control bar · name used as export filename suffix · unique-name handling
- ⬜ **Auto-rearrange artboards** · _standard_ — Tidy all artboards into an even grid/row layout.
  'Rearrange All Artboards' dialog · layout: grid by row / grid by column / single row / single column · columns count · spacing between · move artwork toggle · left-to-right vs right-to-left order
- ⬜ **Artboard presets & standard sizes** · _standard_ — Quick-pick library of named artboard dimensions.
  Print (A4, Letter, etc.) · Web/screen resolutions · mobile/device frames · video (HD/4K, 16:9) · social formats · 'Fit to Artwork/Selection' presets · custom user presets
- ⬜ **Artboard Options dialog** · _standard_ — Per-artboard visual aids and metadata.
  Name · preset/size · X/Y/W/H · orientation · show center mark · show crosshairs · show video safe areas (action/title-safe) · ruler pixel-aspect · fade region outside artboard · 'Update while dragging'
- ⬜ **Active / current artboard concept** · _standard_ — The currently-targeted artboard for paste, new-object, and export defaults.
  Highlighted active artboard · 'Paste in Place / Paste on All Artboards' · new objects land on active artboard · status-bar artboard indicator · click artboard to make active
- ⬜ **Per-artboard export** · _standard_ — Export each artboard as its own file/page.
  'Use Artboards' checkbox (range or all) in Save/Export · one file per artboard with name suffix · multi-page PDF from artboards · Export for Screens (each artboard as asset, scales/formats) · range selection (e.g. 1-3,5)
- ⬜ **Artboard-relative coordinates / rulers** · _standard_ — Ruler origin and coordinates measured per active artboard vs global.
  Global ruler vs artboard ruler toggle · per-artboard 0,0 origin · X/Y in Transform relative to active artboard · video ruler pixel-aspect
- 🟡 **Artboard background / canvas vs page distinction** · _standard_ — Separation between the infinite canvas and bounded printable artboard area.
  Artboard = printable/exportable bounds · canvas/scratch area outside (art allowed but not exported unless on artboard) · fade/dim outside-artboard region · artboard outline shown
- ⬜ **ICC color profile / color management** · _advanced_ — Assigned working profile defining how colors are interpreted/displayed.
  Assign profile (sRGB, Adobe RGB, Display P3, US Web Coated SWOP, Coated FOGRA39) · embed profile on save/export · soft-proof / proof setup · rendering intent · 'Don't color-manage' option
- ⬜ **Facing pages / spreads** · _advanced_ — Two-page spread layout for print/book/brochure work.
  Facing-pages toggle · left/right page pairing · spine/gutter · master page concepts (more publishing-tier) · spread vs single-page export — niche in pure vector tools (DTP-leaning)
- ⬜ **Show center mark** · _advanced_ — Visual center indicator on the artboard.
  Toggle in Artboard Options · centered cross/dot · alignment reference · non-printing guide
- ⬜ **Show crosshairs** · _advanced_ — Edge midpoint crosshair guides on the artboard.
  Toggle in Artboard Options · crosshairs at each side midpoint · alignment aid · non-printing
- ⬜ **Show video / safe areas** · _advanced_ — Action- and title-safe overlay guides for video artboards.
  Toggle in Artboard Options · action-safe + title-safe rectangles · pixel-aspect-ratio correction · for film/video intent
- ⬜ **Convert selection to artboard / artboard from artwork** · _advanced_ — Generate an artboard sized to selected objects.
  'Artboard from Artwork bounds' · convert selected object bounds into a new artboard · object-to-artboard with margin · used to crop/frame existing art

## 9. Snapping / guides / grid / rulers
*The full precision-alignment substrate: rulers, draggable/object/smart guides, document & pixel grids, and every snapping target/option a pro vector tool offers to place geometry exactly.*  
(35 items — ✅0 · 🟡0 · ⬜35 · ❔0)

- ⬜ **Rulers (show/hide)** · _core_ — Horizontal + vertical measurement rulers along canvas edges.
  Toggle View > Rulers (Ctrl/Cmd+R) · top + left ruler bars · live tick marks track cursor position · scale follows zoom level
- ⬜ **Ruler units** · _core_ — Measurement unit shown on rulers & all numeric fields.
  px · pt · pc (picas) · in · mm · cm · ft · meters · right-click ruler to switch · per-document default unit · type-size unit separate (pt/px)
- ⬜ **Ruler guides (drag from ruler)** · _core_ — Drag a guide line out of a ruler onto the canvas.
  drag from top ruler = horizontal guide · from left ruler = vertical · double-click ruler to drop a guide at exact value · drag with modifier to convert H↔V guide
- ⬜ **Hide / show guides** · _core_ — Toggle visibility of all guides without deleting them.
  View > Guides > Hide Guides (Ctrl/Cmd+;) · guides still active for snapping per option · non-printing by default
- ⬜ **Snap to guides** · _core_ — Objects/anchors snap to guide lines while dragging.
  View > Snap to Guides toggle · snaps bbox edges/center + anchors · works with locked guides
- ⬜ **Smart Guides (master toggle)** · _core_ — On-the-fly contextual alignment guides shown while editing.
  View > Smart Guides (Ctrl/Cmd+U) · appear only during drag/draw/transform · magenta/green construction lines · drives most modern snapping UX
- ⬜ **Object alignment guides** · _core_ — Dynamic lines showing edge/center alignment with other objects.
  snap to other objects' edges, centers, vertical/horizontal axes · highlight when aligned · 'align to pixel' variant
- ⬜ **Anchor / point snapping** · _core_ — Snap cursor to existing anchor points & path nodes.
  snap to anchors, path segments, handle ends · 'snap to point' tolerance in px · highlights node with marker (e.g. 'anchor', 'path', 'intersect')
- ⬜ **Snap to point** · _core_ — Master toggle making the cursor snap to nearby points.
  AI View > Snap to Point · cursor turns white/hollow when engaged · works with anchors, guide intersections, ruler origin
- ⬜ **Zero / ruler origin point** · _standard_ — Sets the 0,0 datum for coordinates & measurements.
  drag from top-left ruler corner to reposition origin · double-click corner to reset · per-artboard origin (AI: rulers relative to active artboard) · global vs artboard ruler mode
- ⬜ **Guide at exact coordinate** · _standard_ — Create/position a guide via numeric entry, not just dragging.
  dialog or field to type X/Y value · nudge selected guide by arrows · panel listing guide positions (Affinity Guides Manager)
- ⬜ **Make guides from objects** · _standard_ — Convert selected vector objects into guide lines.
  AI: Object > Guides > Make Guides (Ctrl/Cmd+5) · Release Guides (Alt+Ctrl+5) turns them back to paths · shape outline becomes non-printing guide
- ⬜ **Lock / unlock guides** · _standard_ — Prevent guides from being moved/selected accidentally.
  View > Guides > Lock Guides (Alt+Ctrl+;) · locked guides ignore clicks · separate from object lock
- ⬜ **Clear / delete guides** · _standard_ — Remove all guides at once or individually.
  View > Guides > Clear Guides · select a single guide + Delete · delete only on active artboard vs all
- ⬜ **Guide color & style** · _standard_ — Customize guide line color and dash style.
  Preferences > Guides & Grid · color swatch · line vs dashes style · separate guide vs smart-guide vs grid colors
- ⬜ **Path / segment snapping** · _standard_ — Snap to any point along a path edge, not only anchors.
  snap to nearest point on segment · snap to path intersections · snap to midpoint of segment · edge-of-shape snapping
- ⬜ **Snapping text labels (measurement)** · _standard_ — Live dimension/offset readouts shown during snap.
  X/Y delta, width/height, angle labels follow cursor · distance to snapped object in current unit · AI Smart Guides 'Measurement Labels' option
- ⬜ **Spacing / equal-distribution guides** · _standard_ — Guides showing equal gaps between objects while dragging.
  pink spacing indicators (Figma/Affinity-style) · snap to match existing gap · equal-distribution hints · 'distribute spacing' snap
- ⬜ **Construction angles / angle snapping** · _standard_ — Smart Guides snap drawing/rotation to preset angles.
  default 0/45/90/135° · custom angle set (AI: up to 6 custom angles) · Shift constrains to 45° increments · angle readout during draw
- ⬜ **Snapping tolerance / radius** · _standard_ — Pixel distance at which snap engages.
  Preferences > Smart Guides snapping tolerance (e.g. 4 px) · separate point-snap radius · larger = grabbier · per-snap-type tolerance
- ⬜ **Document grid (show/hide)** · _standard_ — Regular background grid across the whole document.
  View > Show Grid (Ctrl/Cmd+") · grid sits behind or in front of artwork toggle · non-printing
- ⬜ **Snap to grid** · _standard_ — Geometry snaps to grid intersections while editing.
  View > Snap to Grid (Shift+Ctrl+") · overrides smart guides when on · snaps move/scale/draw · disabled in Pixel Preview (becomes snap-to-pixel)
- ⬜ **Gridline spacing & subdivisions** · _standard_ — Control grid cell size and minor divisions.
  'Gridline every' value + unit · 'Subdivisions' count · major vs minor line styling · Preferences > Guides & Grid
- ⬜ **Pixel grid** · _standard_ — 1px grid visible at high zoom for pixel-perfect work.
  View > Show Pixel Grid (appears >600% zoom) · aligns to device pixels · toggle in Preferences
- ⬜ **Snap to pixel / pixel-perfect mode** · _standard_ — Force all geometry onto whole-pixel boundaries.
  AI 'Align New Objects to Pixel Grid' (doc-level + per-object) · Affinity 'Force Pixel Alignment' · snaps anchors to integer px · critical for UI/web export
- ⬜ **Snap to geometry (bounding box)** · _standard_ — Snap object bounding-box handles to other geometry while dragging.
  bbox edges/center/corners snap · snap to other bbox · snap during scale/rotate · Affinity 'Snap to object geometry' / 'bounding boxes'
- ⬜ **Snap to artboard / page edges & margins** · _standard_ — Snap to artboard bounds, center, and margin/bleed lines.
  snap to artboard edges + center · snap to bleed/margin guides · column/row layout guides (Affinity)
- ⬜ **Transform / measurement readout (HUD)** · _standard_ — On-canvas tooltip of live X/Y/W/H/angle during edits.
  follows cursor during move/scale/rotate/draw · shows delta + absolute · respects current unit · ties into smart-guide measurement labels
- ⬜ **Artboard vs global rulers** · _advanced_ — Whether ruler 0,0 is per-artboard or whole-canvas.
  AI: 'Change to Global Rulers' / 'Change to Artboard Rulers' · origin jumps to active artboard's corner · affects exported coordinate references
- ⬜ **Video / print ruler presets** · _advanced_ — Specialized ruler tick subdivisions for video/print work.
  AI 'Video Rulers' option · ruler ticks in video pixels / safe-area units · niche, mostly motion-graphics workflows
- ⬜ **Smart Guide option toggles** · _advanced_ — Granular enable/disable of each Smart-Guide behavior.
  checkboxes: Alignment Guides · Anchor/Path Labels · Measurement Labels · Object Highlighting · Transform Tools · Construction Guides
- ⬜ **Grid style & color** · _advanced_ — Appearance of grid lines.
  lines vs dots style · grid color swatch · opacity · separate major/minor colors
- ⬜ **Snapping manager / candidate options (Affinity)** · _advanced_ — Central panel toggling every snap source independently.
  Affinity Snapping toolbar: snap to grid · guides · object geometry · bounding boxes · key points · midpoints · spread/page · candidate-aware vs single-axis · presets (UI design / page layout)
- ⬜ **Column / layout grid guides** · _advanced_ — Configurable column/row guides for layout work.
  AI Make Guides via Split Into Grid · Affinity Columns guides (count, gutter, margin) · InDesign-style layout grid · objects snap to columns
- ⬜ **Temporary snap suppression / cycling** · _advanced_ — Modifier keys to disable or cycle snapping mid-drag.
  hold key to ignore snapping temporarily · cycle through stacked snap candidates · toggle without leaving the drag · arrow-key nudge ignores snap

## 10. Canvas interaction & navigation
*Everything for moving around the canvas (zoom/pan/rotate-view) and the direct-manipulation feel of grabbing, dragging, selecting, transforming, snapping, measuring, and nudging objects with live cursor/modifier feedback.*  
(56 items — ✅17 · 🟡14 · ⬜25 · ❔0)

- 🟡 **Zoom in / out (incremental)** · _core_ — Step the canvas magnification up/down around a focal point.
  Ctrl++ / Ctrl+- · keeps a focal point (cursor or selection center) stable · stepped multiplier (e.g. 1.5×) · clamped min/max (Varos 0.05×–40×)
- 🟡 **Fit artboard / all in window** · _core_ — Frame the whole document (or active artboard) to the viewport.
  Ctrl+0 fit-in-window · fit-all vs fit-active-artboard distinction · double-click Hand tool = fit · Varos Ctrl+0 resets view to identity (not a true content-fit)
- ✅ **Actual size / 100%** · _core_ — Reset to 1:1 pixel magnification.
  Ctrl+1 · double-click Zoom tool = 100% · 'Actual Size' menu item · DONE in Varos (Ctrl+1 sets zoom=1.0)
- 🟡 **Ctrl/Alt + scroll-wheel zoom** · _core_ — Wheel zooms toward the cursor.
  zoom-to-cursor (pan compensates so point under cursor stays put) · configurable wheel direction · DONE in Varos via Alt+scroll (zoom anchored to cursor)
- 🟡 **Pan — Hand tool** · _core_ — Drag the canvas to reposition the view.
  H tool · click-drag moves view · double-click Hand = fit-in-window · DONE (Varos pans via Space/middle-mouse drag)
- ✅ **Pan — spacebar-hold (temporary Hand)** · _core_ — Hold Space to pan from any tool, release to revert.
  hold Space → grab/closed-hand cursor → drag · returns to prior tool on release · DONE in Varos (space_down → Hand/Grab cursor + pan)
- ✅ **Pan — scroll-wheel / two-finger scroll** · _core_ — Wheel scrolls the view vertically; modifier for horizontal.
  vertical scroll · Shift+scroll = horizontal · two-finger trackpad pan in any direction · DONE in Varos (vertical, shift-horizontal, dx from trackpad)
- ✅ **Grab/drag an anchor point** · _core_ — Direct-select tool picks up and moves a single anchor with realism.
  white-arrow click-drag on a vertex · live path reshape · Shift constrains to 45° (snap45) · DONE in Varos (Direct tool anchor drag + shift constrain)
- ✅ **Grab/drag a bezier handle** · _core_ — Pull a control handle to reshape curvature live.
  drag handle endpoint · coupled mirror handle moves opposite (smooth) · Shift = 45° constrain · live curve preview · DONE in Varos (Drag::Handle with couple + opp_len)
- ✅ **Break a single handle (Alt)** · _core_ — Alt while dragging one handle decouples it from its twin (creates a corner).
  Alt-drag handle → independent in/out tangents (cusp) · DONE in Varos (Alt sets broken=true on handle drag)
- ✅ **Marquee select objects** · _core_ — Drag an empty-area rectangle to select all enclosed/touched objects.
  black-arrow rubber-band · touch vs fully-enclosed policy · group caught as whole · adds to selection across multiple objects · DONE in Varos (Drag::ObjMarquee, group-aware)
- ✅ **Marquee select anchors/handles** · _core_ — Direct-select rubber-band grabs multiple vertices at once.
  white-arrow marquee selects anchors within rect · then move them together · DONE in Varos (Drag::Marquee over anchors)
- ✅ **Click select + Shift add/toggle** · _core_ — Single click selects; Shift-click adds/removes one object.
  click empty = deselect · Shift-click toggles membership · click-through to object under others · DONE in Varos (object tool selection)
- ✅ **Bounding-box move (drag body)** · _core_ — Drag inside the selection bbox to translate object(s).
  grab fill/interior to move · Shift constrains to H/V/45° axis (snap45) · DONE in Varos (Drag::Object + shift constrain)
- ✅ **Bounding-box scale handles (8)** · _core_ — Corner + edge-mid handles resize the selection.
  4 corners + 4 edge mids · corner = 2D, edge = 1D · works in object's local/rotated space · DONE in Varos (8 handles, Drag::Scale local-space)
- 🟡 **Scale modifiers (Shift / Alt)** · _core_ — Constrain proportions or scale from center while resizing.
  Shift = uniform/aspect-lock · Alt = scale from center (pivot=center) · Shift+Alt = both · DONE: Varos Alt→pivot center; Shift uniform-scale present
- ✅ **Rotate just outside a corner** · _core_ — Hover beyond a corner handle to get the rotate cursor and spin.
  rotate ring/zone outside each corner · curved rotate cursor per corner (rotation-aware) · drag rotates about pivot · DONE in Varos (TfHit::Rotate ring + per-corner rotate cursor)
- ⬜ **Live snapping to objects (smart guides)** · _core_ — Edges/centers/anchors of other objects snap and show alignment lines while dragging.
  snap to anchor, path, center, edge, intersection · pink smart-guide lines + labels · equal-spacing hints · snap tolerance · missing in Varos
- ✅ **Nudge with arrow keys** · _core_ — Arrow keys move selection by a fixed step.
  1px (or keyboard-increment pref) · Shift = ×10 · works on objects, anchors, handles · DONE in Varos (1 / Shift-10 nudge)
- ✅ **Tactile / contextual cursor feedback** · _core_ — Cursor changes to reflect the exact action under the pointer.
  resize double-arrows (rotation-aware), rotate arc per corner, move, copy (Alt), pen states, hand/grab, eyedropper · DONE in Varos (rich Illustrator cursor set, state-driven)
- ⬜ **Right-click context menu (canvas)** · _core_ — Contextual actions at the pointer (arrange, group, transform, etc.).
  object ops · transform · arrange/z-order · isolate · lock/hide · paste-here · missing in Varos
- 🟡 **Select-all / deselect / inverse / reselect** · _core_ — Bulk selection commands.
  Ctrl+A select all · Ctrl+Shift+A deselect · invert selection · reselect-last · select-same (fill/stroke/etc.) · partial in Varos (basic select/deselect; no inverse/same)
- ✅ **Constrain-axis move (Shift)** · _core_ — Shift while moving locks to horizontal/vertical/45° axis.
  Shift+drag body → nearest 45° axis (snap45) · applies to objects and anchors · DONE in Varos
- ✅ **Alt-drag duplicate** · _core_ — Hold Alt while dragging to leave a copy behind.
  Alt+move = duplicate · copy cursor shown · group duplicates as a group · DONE in Varos (DupPending + copy cursor, group-aware)
- ⬜ **Zoom presets / level list** · _standard_ — Jump to named magnification levels.
  preset steps (6.25/12.5/25/50/100/150/200/400/800/1600%…) · editable zoom % field in status/zoom widget · dropdown of levels · Varos has a zoom HUD but no preset menu/editable field
- ⬜ **Fit selection in window** · _standard_ — Zoom/pan so the current selection fills the view.
  Zoom-to-selection shortcut · frames bbox of selected objects with margin · missing in Varos
- ⬜ **Marquee / drag-zoom (Zoom tool)** · _standard_ — Drag a rectangle to zoom into that exact region.
  Z tool · drag box → fills viewport with that area · Alt-click = zoom out · click = step zoom at point · Varos lacks a dedicated Zoom tool with drag-rect
- 🟡 **Scrubby / continuous zoom** · _standard_ — Press-drag left-right for smooth analog zoom.
  Zoom tool press+hold then drag horizontally (Illustrator GPU scrubby) · live continuous scale · Varos has Space+Ctrl+click stepped zoom only (no scrubby drag)
- ⬜ **Pinch-to-zoom (trackpad/touch)** · _standard_ — Two-finger pinch gesture zooms the canvas.
  trackpad pinch · touchscreen pinch · momentum/smooth scaling · missing in Varos
- ✅ **Pan — middle-mouse drag** · _standard_ — Press mouse wheel button and drag to pan.
  MMB-drag pan (CAD/3D convention many pros expect) · DONE in Varos
- ⬜ **Scrollbars** · _standard_ — Edge scrollbars indicate/scroll canvas position.
  horizontal + vertical bars · draggable thumbs · auto show/hide · proportional to content extent · missing in Varos (full-bleed board, no bars)
- ⬜ **Navigator / overview panel** · _standard_ — Thumbnail of the whole document with a draggable view-rectangle.
  mini-map · drag red proxy box to pan · zoom slider · click-to-jump · missing in Varos
- 🟡 **Zoom level indicator / readout** · _standard_ — Persistent display of current magnification.
  % readout in status bar / zoom widget · click to type exact % · DONE-ish: Varos shows a zoom HUD (read-only, not editable)
- 🟡 **Add/subtract to selection while marqueeing** · _standard_ — Shift extends, Alt removes during/after a marquee.
  Shift+marquee = union with existing · base selection preserved (Varos keeps `base` set) · partial: Shift-extend present, Alt-subtract on marquee less clear
- 🟡 **Rotate constrain (Shift = 15°)** · _standard_ — Shift snaps rotation to fixed increments.
  Shift → 15°/45° steps while rotating · live angle feedback · partial in Varos (rotation present; Shift-15° increment not confirmed)
- ⬜ **Movable transform pivot/reference point** · _standard_ — Relocate the rotation/scale origin.
  drag the center reference point off-center · 9-point reference-point selector in transform panel · rotate/scale about it · missing in Varos (pivot fixed to opposite handle / center)
- ⬜ **Live dimension / measurement readout while dragging** · _standard_ — Floating tooltip shows W/H, X/Y, angle, distance as you drag.
  size label on resize · dx/dy + distance on move · angle on rotate · radius/sides on shape draw · missing in Varos (a key tactile-feel gap)
- ⬜ **Snap to grid** · _standard_ — Positions lock to a configurable grid while moving/drawing.
  snap-to-grid toggle · grid spacing/subdivisions · pixel grid (snap-to-pixel for crisp export) · missing in Varos
- ⬜ **Snap to guides / ruler guides** · _standard_ — Drag-out guides from rulers that objects snap to.
  pull guides from H/V rulers · lock/clear guides · snap objects to guides · guide color · missing in Varos
- ⬜ **Snap toggle / enable-disable (Ctrl hold)** · _standard_ — Momentarily suppress or force snapping during a drag.
  hold key to disable snap mid-drag · global snap on/off toggle · snap-strength setting · missing in Varos
- ⬜ **Space-to-reposition while drawing/dragging** · _standard_ — Hold Space mid-creation to move the whole shape before releasing.
  drag a shape, hold Space → reposition origin, release Space → continue sizing · also for moving a marquee while dragging · missing in Varos
- 🟡 **Hover / pre-selection highlight** · _standard_ — Outline an object or anchor under the cursor before clicking.
  hover outline on object · anchor/segment highlight on path hover · 'hot' handle emphasis · partial in Varos (anchor glyphs exist; full hover pre-highlight unconfirmed)
- 🟡 **Double-click to enter (isolation / edit)** · _standard_ — Double-click drills into a group or starts path/anchor editing.
  dbl-click group = isolation mode · dbl-click object with Direct/Object tool = edit path · escape exits up one level · DONE-ish in Varos (double_click handled for Object/Direct)
- ⬜ **Rulers** · _standard_ — Edge rulers showing document coordinates.
  H/V rulers · unit display · 0,0 origin draggable · ruler toggle (Ctrl+R) · source for pulling guides · missing in Varos
- ⬜ **Grid display** · _standard_ — Visible reference grid behind artwork.
  show/hide grid · grid spacing & subdivisions · color · grid-in-back toggle · independent of snap · missing in Varos
- ⬜ **Cursor coordinate / info readout** · _standard_ — Live X/Y (and delta) of the pointer in document units.
  status-bar or Info-panel X/Y · current unit · selection W/H · distance/angle of last drag · missing in Varos
- ⬜ **Click-through / select-behind** · _standard_ — Reach an object stacked under another at the same point.
  Alt/Ctrl-click cycles overlapping objects · select-behind shortcut · right-click 'select next below' · missing in Varos
- ⬜ **Auto-scroll at viewport edge** · _standard_ — Dragging an object/marquee near the edge scrolls the canvas.
  edge-push auto-pan during drag · speed ramps with proximity · lets you drag beyond visible area · missing in Varos
- 🟡 **Escape / Enter during interaction** · _standard_ — Cancel or commit an in-progress drag/transform.
  Esc cancels current marquee/transform/path · Enter commits · returns to clean state · partial in Varos (Esc handling exists for some tools, not comprehensive)
- ⬜ **View modes (outline / preview / pixel)** · _standard_ — Switch how the canvas renders artwork.
  Preview (Ctrl+Y toggle Outline) · Pixel-preview · Overprint preview · GPU vs CPU preview · missing in Varos
- ⬜ **Rotate canvas view** · _advanced_ — Temporarily rotate the whole viewport (artwork data unchanged).
  Rotate-View tool (Affinity/Illustrator) · drag to spin · numeric angle field · snap to 15° · reset-rotation command · missing in Varos
- ⬜ **Flip canvas view** · _advanced_ — Mirror the viewport horizontally to check artwork balance.
  flip-view-horizontal toggle (does not flip the data) · used to spot composition errors · missing in Varos
- ⬜ **Shear / skew via bbox** · _advanced_ — Drag an edge handle with a modifier to slant the selection.
  Ctrl/Cmd-drag edge handle = shear (Affinity) · or dedicated Shear tool · missing in Varos
- ⬜ **Lasso / freehand selection** · _advanced_ — Draw a freeform region to select anchors/objects.
  Lasso tool (Illustrator selects anchors) · freehand path vs rectangular marquee · missing in Varos
- 🟡 **Pan/zoom inertia & smoothing** · _advanced_ — Momentum and eased animation for navigation feel.
  flick-pan inertia · animated zoom transitions · 60fps GPU smoothness (the 'realism' Ahmed wants) · partial in Varos (GPU redraw; no inertia/easing layer)
- ⬜ **Bird's-eye / quick-zoom-out peek** · _advanced_ — Hold a key to momentarily zoom out to overview, release to return.
  press-hold bird's-eye-view (Affinity) → drag to a region → release zooms there · fast navigation of large canvases · missing in Varos

## 11. Save / File System
*Everything for persisting, opening, importing, linking, recovering, and packaging documents — native .varos format, Save/Save As/Save a Copy/Template, autosave + crash recovery, version history, recent files, Place/Import, linked-vs-embedded assets + Links panel, package/collect, and file size/compression.*  
(31 items — ✅0 · 🟡2 · ⬜29 · ❔0)

- ⬜ **Native document format (.varos)** · _core_ — The proprietary file that round-trips a document with zero loss.
  Single-file container · stores full vector data (paths/anchors/handles), layers/groups, artboards/pages, fill/stroke/effects, text + embedded/linked fonts, embedded raster, document color profile, guides/grid, metadata · versioned schema + format-version stamp · forward/backward-compat handling · magic-number/header · ideally one schema = file+AI+plugin+inspector (Blender-RNA approach)
- ⬜ **Save (Ctrl/Cmd+S)** · _core_ — Write current document to its existing file path.
  Overwrites same .varos file · first-ever Save falls through to Save As (no path yet) · clears dirty/modified flag · updates title-bar asterisk/dot · atomic write (temp file + rename) to avoid corruption on crash mid-save · keep-prior-version option
- ⬜ **Save As (Ctrl/Cmd+Shift+S)** · _core_ — Write to a NEW file path/name and switch the document to it.
  OS save dialog · choose folder + filename + format · document now 'is' the new file (further Saves go there) · format dropdown (.varos + legacy/compat targets) · overwrite-confirm prompt · in AI: Illustrator/PDF/EPS/SVG/template version options
- ⬜ **New (Ctrl/Cmd+N) + New Document dialog** · _core_ — Create a blank document with chosen presets.
  Preset categories (Print/Web/Mobile/Film&Video/Art) · width/height + units · orientation · # of artboards + spacing/grid · bleed · color mode (RGB/CMYK) · raster effects/PPI · recently-used presets · 'Create' vs 'More Settings'
- ⬜ **Open (Ctrl/Cmd+O)** · _core_ — Open an existing file into a new document tab/window.
  OS open dialog · file-type filter (.varos + importable: AI/PDF/SVG/EPS/raster) · multi-select open · open-as-copy option · drag-file-onto-app-window to open · cloud/recent locations sidebar
- 🟡 **Save-vs-Export distinction** · _core_ — Conceptual split: Save persists the editable native doc; Export emits a flattened deliverable.
  Save = lossless round-trippable .varos · Export = PNG/JPG/SVG/PDF for hand-off (lossy/flattened, NOT reopened as working file) · separate menu sections & dialogs · 'Export As' vs 'Export for Screens' vs Save · prevents users from 'saving' a PNG and losing layers
- ⬜ **Dirty/modified state + close prompts** · _core_ — Track unsaved changes and guard against data loss.
  Title-bar modified indicator (asterisk/dot) · 'Save changes before closing?' on close/quit · per-tab unsaved badge · Save/Don't Save/Cancel · quit-with-multiple-unsaved batched prompt · revert blocked-without-save
- ⬜ **Autosave / auto-backup** · _core_ — Periodically persist work in the background to survive crashes.
  Interval setting (e.g. every N min) · save-to-original vs separate backup copy · per-document temp/recovery files · keep-N-backups · pause-during-active-drag · pref toggle + location · low-overhead incremental write · indicator/timestamp of last autosave
- ⬜ **Crash / document recovery** · _core_ — Restore unsaved documents after a crash, power loss, or kill.
  On relaunch: detect orphaned recovery files → 'Recover documents?' dialog · list recoverable docs w/ thumbnails · recovered doc opens as unsaved (must Save As) · clean-up stale recovery files · session restore (reopen previously open docs/tabs) · corruption fallback to last good autosave
- ⬜ **Place / Import — vector (SVG/PDF/AI/EPS)** · _core_ — Bring external vector files into the current document.
  File ▸ Place · link vs embed checkbox · place-gun cursor (click to drop / drag to size) · multi-place loaded cursor (cycle files, Esc to drop) · PDF page selector + crop-to (art/trim/bleed/media box) · AI/EPS parsing to native paths · SVG import (paths, groups, gradients, text, clip/mask) · replace-on-place vs new
- ⬜ **Place / Import — raster (PNG/JPG/TIFF/PSD/WebP/GIF)** · _core_ — Bring bitmap images into the document.
  Link vs embed · PSD layer/options dialog (flatten vs layers, comps) · color-profile handling · DPI/scale on import · transparency/alpha preserved · multi-image place · drag-drop from OS/Finder · clipboard paste image
- 🟡 **Export (deliverable output) — adjacent system** · _core_ — Emit flattened/standard files for delivery (distinct from Save, listed for completeness).
  Export As (PNG/JPG/SVG/PDF/TIFF/WebP) · Export for Screens (multi-artboard, multi-scale @1x/2x/3x, suffixes) · Save for Web (legacy) · per-format options dialogs · asset-export presets · partially prototyped in old export modal · belongs to its own Export category but is the twin of Save
- ⬜ **Save a Copy (Ctrl/Cmd+Alt+S)** · _standard_ — Write a duplicate to a new path WITHOUT changing the active document's path.
  Active doc stays pointed at original file · used for snapshots/branches/back-ups · does NOT clear dirty flag of the working doc · distinct from Save As (that re-points)
- ⬜ **Save as Template** · _standard_ — Save document as a reusable starting point that opens as 'Untitled' copies.
  Template file type (.vit-style) · opening a template spawns a new unsaved doc (never overwrites the template) · ships stock templates + user templates folder · stores artboard sizes, swatches, styles, placeholder content
- ⬜ **New from Template / New from Selection** · _standard_ — Start a new document seeded from a template or current selection.
  Template gallery/browser · category filter + search · thumbnails · 'New from template…' file picker · New from current selection (copies styles/swatches into fresh doc)
- ⬜ **Open Recent / Recent files list** · _standard_ — Quick re-open of recently used documents.
  File ▸ Open Recent submenu (last N) · home/start screen recent grid with thumbnails · pin/favorite · clear-recent-list · missing/moved-file greyed out · stored in user prefs · jump-list/dock integration
- ⬜ **Home / Start screen** · _standard_ — Landing surface for New, Open, Recent, Templates on launch.
  Recent files grid + thumbnails · New-doc preset tiles · template browser · learn/tutorials · 'no document open' state · toggle in prefs
- ⬜ **Revert (F12 / Revert to Saved)** · _standard_ — Discard all changes since last save and reload from disk.
  Re-reads file from disk · confirmation prompt (destructive) · resets undo history (or marks revert as undoable) · disabled when never saved / no changes
- ⬜ **Import via clipboard / drag-drop / paste-special** · _standard_ — Get content in without the Place dialog.
  Paste from other apps (SVG/vector, bitmap, AI clipboard format/PDF) · drag from browser/Finder onto canvas · 'Paste in place' / paste-remembers-layers · paste as raster vs vector option · cross-app fidelity (PDF/SVG on clipboard)
- ⬜ **Linked vs embedded assets** · _standard_ — Choose whether placed files reference an external file or are baked into the doc.
  Linked = external file referenced (small doc, edits propagate, can go missing) · Embedded = stored inside .varos (portable, larger) · default link/embed pref · embed-link / unembed(extract-to-file) actions · per-asset toggle · auto-embed-on-save option
- ⬜ **Links panel** · _standard_ — Manage every placed/linked asset in the document.
  List w/ thumbnails + name + status (linked/embedded/modified/missing) · Relink (to new file / to folder / from CC libs) · Go-to-link (select on canvas) · Update-link (reload modified) · Edit Original (open in external editor) · Embed/Unembed · link info (path, size, dims, PPI, transform, color space) · filter/sort · missing-link & modified-link warning badges
- ⬜ **File size / compression options** · _standard_ — Controls that govern how big the saved file is.
  Zip/deflate the container · 'Create PDF Compatible File' toggle (AI) — bloats size · 'Use Compression' toggle · embed-ICC-profiles toggle · raster down-sampling on save · subset embedded fonts · option to include/exclude full undo history & thumbnails · estimated-size readout
- ⬜ **Document thumbnail / preview generation** · _standard_ — Embed a preview image so OS/file-browser and Recents can show the doc.
  Embedded PNG thumbnail on save · OS shell/Finder/Explorer preview + Quick Look · used in Recent/Home grids · 'include thumbnail' pref · regenerate on save
- ⬜ **Save location targets (local / cloud / project)** · _standard_ — Where files can live and how the app reaches them.
  Local filesystem (offline-first, the Varos default) · cloud document option · recent locations · per-OS default folder · network/drive paths · save-back to original location for opened cloud files
- ⬜ **Save Selected / Save Slices (export-ish saves)** · _advanced_ — Save only selected artwork/artboards to separate files.
  Save each artboard as its own file · 'Save selected slices' · per-artboard filename templating · overlaps with Export but writes native-ish copies
- ⬜ **Version history / snapshots** · _advanced_ — Keep and browse prior saved states of a document.
  Named/auto versions · timeline list w/ timestamps + thumbnails · preview-then-restore (non-destructive) · restore-as-copy · diff/compare · local-history store vs cloud · prune/retention policy · 'mark this version'
- ⬜ **Edit Original / external round-trip** · _advanced_ — Open a linked asset in its native editor and re-sync on save.
  Launch default app for asset type · watch file for changes · auto/prompt update on return · works for raster (Photoshop) and vector (linked AI/SVG)
- ⬜ **Package / Collect for output** · _advanced_ — Gather the document + all linked assets + fonts into one folder.
  File ▸ Package · copies linked images into /Links · copies fonts into /Fonts (license caveat) · generates report/summary (fonts, links, colors, spot colors) · relink-to-packaged-folder option · creates folder structure + optional .idml-style manifest · for hand-off/archive
- ⬜ **Document metadata / file info** · _advanced_ — Author, description, keywords, and history stored with the file.
  File Info dialog (title/author/description/keywords/copyright) · XMP metadata · creation/modified timestamps · app + version stamp · ruler/units, color profile, fonts-used summary · document-level color settings
- ⬜ **Legacy / version-down save & format compatibility** · _advanced_ — Write files openable by older app versions or other tools.
  'Save as legacy version' dropdown · warning when features won't survive down-save · open files from newer version (best-effort) · import competitor formats (.ai/.afdesign-style/.sketch/.fig where feasible)
- ⬜ **File locking / multi-open guard** · _advanced_ — Prevent two sessions clobbering the same file.
  Lock/in-use marker when a doc is open · 'file in use / open read-only?' prompt · stale-lock cleanup after crash · read-only flag honored

## 12. Export system
*Every way to get artwork out of the tool — single-object/artboard exports, batch screen/asset export, format-specific option dialogs (SVG/PDF/raster), Save for Web, slices, presets, and clipboard copy.*  
(39 items — ✅0 · 🟡0 · ⬜39 · ❔0)

- ⬜ **Export As… (single-file export dialog)** · _core_ — Primary one-shot export of selection/document to a chosen format
  format dropdown (SVG/PNG/JPG/PDF/EPS/TIFF/WEBP/PSD/AVIF) · 'Use Artboards' checkbox + range (All/1-3) · per-format options sub-dialog · destination folder + filename · single vs separate files
- ⬜ **PNG export options** · _core_ — Raster PNG output settings
  resolution/scale (72/150/300 ppi or 1x–3x) · background (transparent/white/artboard color) · anti-alias (none/art-optimized/type-optimized) · interlaced · bit depth (8/24/32) · clip to artboard
- ⬜ **JPG/JPEG export options** · _core_ — Lossy raster output settings
  quality slider 0–100 / low–maximum · compression method (baseline/optimized/progressive) · resolution/scale · background matte color · ICC profile embed
- ⬜ **SVG export options** · _core_ — Vector SVG output with fine control
  styling (presentation attributes / inline CSS / internal CSS / style elements) · font (SVG/convert to outline) · images (preserve/embed/link) · object IDs (layer names/minimal/unique) · decimal precision (1–7) · minify · responsive (drop width/height) · CSS class vs inline · 'Show Code' preview
- ⬜ **Per-artboard export** · _core_ — Export each artboard as its own file
  export only selected artboards · range syntax (1,3-5) · use artboard name as filename · include/exclude artboard bounds · overlapping-object clipping
- ⬜ **Resolution / scale presets** · _core_ — Reusable output size multipliers and ppi values
  ppi presets (72/144/150/300/600) · scale presets (0.5x/1x/1.5x/2x/3x) · absolute width/height target (px) · 'fit to' dimension · maintain aspect ratio
- ⬜ **Background / transparency handling** · _core_ — Control matte/alpha on raster export
  transparent vs solid background · matte color for edge blending · include/exclude artboard background fill · flatten transparency
- ⬜ **Export selection vs document vs artboards scope** · _core_ — Choose what the export covers
  current selection only · whole document/canvas · specific artboard(s) · visible layers only · include hidden/locked toggle
- ⬜ **PDF export options** · _standard_ — Print/vector PDF output
  PDF preset (Press/Print/Smallest/X-1a/X-3/X-4) · compatibility version · marks & bleeds (crop/registration/color bars, bleed values) · compression/downsampling · color conversion + output intent · preserve editing capabilities · multi-page from artboards · fonts embed/subset
- ⬜ **TIFF export options** · _standard_ — High-quality lossless raster (print)
  color model (RGB/CMYK/Grayscale) · resolution · LZW/ZIP/none compression · byte order (IBM/Mac) · embed ICC profile · anti-alias
- ⬜ **WEBP export options** · _standard_ — Modern web raster (lossy+lossless)
  lossy/lossless toggle · quality slider · alpha transparency · scale/resolution · metadata strip
- ⬜ **Export for Screens** · _standard_ — Batch export many artboards + assets at multiple scales/formats in one pass
  two tabs (Artboards / Assets) · artboard range + selection · per-format scale rows (0.5x–3x, custom) · suffix per scale · format per row · prefix · open location after export · create sub-folders per format/scale
- ⬜ **Asset Export panel** · _standard_ — Persistent panel: drag objects in for repeatable multi-format export
  drag-and-drop objects to register as assets · per-asset format+scale+suffix rows · add scale row (+) · 'Export Selected'/'Export All' · auto-collect / generate on save · thumbnail list · rename assets
- ⬜ **Per-asset scale rows (@1x/@2x/@3x)** · _standard_ — Multiple density outputs per asset for responsive/retina
  scale multipliers (1x/2x/3x/0.5x/custom %) · ppi target option · iOS/Android/Web presets that auto-add the standard scale set
- ⬜ **Per-asset suffix & naming** · _standard_ — Filename token appended per scale/format
  suffix string (@2x, _dark) · prefix · base name from object/layer name · token rules · collision handling
- ⬜ **Format presets (platform sets)** · _standard_ — One-click bundles of scale+format for a target platform
  iOS (1x/2x/3x PNG) · Android (mdpi–xxxhdpi) · Web (1x/2x) · custom saved presets · editable preset manager
- ⬜ **Clipping to artboard vs bounding box** · _standard_ — Define export extents
  clip to artboard bounds · trim to artwork bounding box · include bleed/margin padding · 'use artboards' vs whole-canvas · per-object tight bounds
- ⬜ **Copy as SVG to clipboard** · _standard_ — Copy selection as SVG markup for paste into code/other apps
  copies vector SVG string · respects SVG styling prefs · paste into editor/browser/other vector tool
- ⬜ **Copy as PNG / raster to clipboard** · _standard_ — Copy selection as a bitmap to the OS clipboard
  chosen scale/ppi · transparent background · paste into raster apps / chat / docs
- ⬜ **Export preview & file-size readout** · _standard_ — Preview the output and estimated size before committing
  live thumbnail/zoomable preview · estimated file size per setting · before/after compare · pixel-grid preview at scale
- ⬜ **Color profile / color space on export** · _standard_ — Manage color fidelity in exported files
  RGB vs CMYK vs Grayscale · embed ICC profile · convert to sRGB for web · output intent (print) · gamut warning
- ⬜ **Anti-aliasing options** · _standard_ — Edge smoothing control on raster output
  none (crisp pixel) · art-optimized (supersampling) · type-optimized (hinting) · per-export override
- ⬜ **Output destination & file management** · _standard_ — Where and how files land
  choose folder · auto sub-folders per format/scale/artboard · filename template/tokens · overwrite vs increment · 'open folder after export' · remember last location
- ⬜ **Batch / multi-format single pass** · _standard_ — Produce several formats+scales from one export action
  queue multiple format rows · progress indicator · cancel/abort · summary of files written · error report on failures
- ⬜ **Rasterization & flatten on export** · _standard_ — Convert vector/transparency/effects to pixels for compatible output
  rasterize live effects (blur/shadow) · flatten transparency preset · expand blends/meshes · outline strokes/text · resolution for raster effects
- ⬜ **Vector decimal precision / minification** · _standard_ — Trim coordinate precision and whitespace to shrink vector files
  decimal places slider · round coordinates · collapse transforms · remove unused defs/metadata · minify/whitespace strip · gzip-friendly output
- ⬜ **Quick Export / single-keystroke re-export** · _standard_ — Repeat last export with same settings instantly
  remember last format+location+scale · one-click/hotkey re-export · 'Export Again' · per-document remembered settings
- ⬜ **Export progress, queue & error handling** · _standard_ — Feedback and resilience during large batch exports
  progress bar + count · cancel · skip-on-error vs abort · post-export summary/log · notification on completion
- ⬜ **EPS export options** · _advanced_ — Legacy PostScript vector interchange
  version/compatibility · preview (none/TIFF) · transparency flattening preset · embed fonts · include CMYK PostScript · include linked files
- ⬜ **AVIF / next-gen format export** · _advanced_ — Newer high-compression web image
  quality · lossless toggle · bit depth (8/10/12) · alpha · scale — long-tail web format
- ⬜ **PSD / layered export** · _advanced_ — Photoshop interchange preserving layers
  write layers (flat vs editable) · resolution · color model · anti-alias · max editability · maximum compatibility — rasterizes vectors
- ⬜ **Save for Web (legacy)** · _advanced_ — Web-optimized raster with live preview + file-size readout
  2-up/4-up compare panes · format (GIF/JPEG/PNG-8/PNG-24) · quality/dither/colors · file size + download-time estimate · image size resample · color table · transparency/matte
- ⬜ **Slices / Slice tool** · _advanced_ — Divide artwork into named regions exported independently
  manual slice tool + slice-from-guides/selection · slice select tool · slice options (name, URL, alt, type image/no-image) · per-slice format · auto-slices for gaps
- ⬜ **Copy as PDF / vector to clipboard** · _advanced_ — Copy vector data for paste into print/vector apps
  PDF/EPS clipboard flavor · preserves vector + CMYK · cross-app paste into InDesign/office
- ⬜ **Drag-out export (drag asset to OS)** · _advanced_ — Drag an object/asset from canvas or panel directly to desktop/Finder
  drag selection → file on disk · uses default/asset-defined format · OS drag-and-drop integration
- ⬜ **Metadata in exported files** · _advanced_ — Embed/strip descriptive data
  strip metadata for size (web) · include author/copyright/XMP · geolocation strip · keywords · preserve vs remove EXIF
- ⬜ **SVG interactivity / link & ID preservation** · _advanced_ — Keep names, IDs, hyperlinks usable in code
  preserve layer/object names as IDs · keep hyperlinks/anchors · ARIA/title/desc for accessibility · data-* attribute retention
- ⬜ **Print output / print dialog** · _advanced_ — Send artwork to a physical/PDF printer (related export path)
  printer + page setup · tiling for oversized art · marks & bleed · scale to fit/custom · color management · print presets
- ⬜ **Animated/multi-frame export (GIF/APNG/sprite)** · _advanced_ — Export sequences or sprite sheets
  animated GIF/APNG from artboards/states · sprite sheet packing · frame delay/loop · per-frame artboard mapping — niche for icon/animation work

## 13. Comments / Collaboration / Review
*The full commenting, review, presence and sharing layer (Figma-style) adapted for an offline-first desktop vector tool — pins, threads, mentions, reactions, resolve, comment panel, share/review links, presence, notifications — noting which parts work locally vs require a backend/sync server.*  
(28 items — ✅0 · 🟡0 · ⬜28 · ❔0)

- ⬜ **Author identity / avatars** · _core_ — Attribution of comments and presence to people
  Name + avatar + color per user · local profile for offline single-user · account/auth for multi-user · guest/anonymous commenters · avatar fallback initials · requires account system for real multi-user identity
- ⬜ **Comment tool / mode** · _standard_ — Dedicated tool/mode to drop and read comments on the canvas
  Toolbar entry + shortcut (Figma 'C') · click-to-place pin · cursor changes to comment cursor · toggles a comment overlay layer separate from design objects · esc to exit · works offline (pins stored in the .varos file)
- ⬜ **Comment pin (canvas anchor)** · _standard_ — A placed marker anchoring a comment thread to a canvas location
  Numbered/avatar bubble · click-place at point · drag to reposition · pinned in document/canvas coordinates so it tracks zoom/pan · color/state styling (open vs resolved) · hover preview · offline-capable
- ⬜ **Comment thread + replies** · _standard_ — Threaded conversation under a single pin
  Root comment + nested/flat replies · chronological order · author + timestamp per message · 'view thread' popover · collapse/expand · reply box with send button · keyboard send (Enter / Cmd+Enter) · offline-capable, syncs when online
- ⬜ **Compose / rich text in comments** · _standard_ — Input box for writing comment text
  Plain + light markdown (bold/italic/code/links) · multiline · paste images/attachments · emoji insert · link auto-detect · draft persistence · char limit · placeholder hint
- ⬜ **@mentions** · _standard_ — Tag a collaborator inside a comment to notify them
  @-trigger autocomplete of people in file/team · highlighted mention token · resolves to user identity · fires a notification to the mentioned person · needs an identity/account system + backend to deliver; local @ tokens can render offline but delivery needs sync
- ⬜ **Resolve / reopen thread** · _standard_ — Mark a thread done (and undo)
  Resolve checkmark hides pin from default view · reopen restores it · resolved state + who/when recorded · 'show resolved' toggle · resolving collapses the thread · works offline (state in file)
- ⬜ **Edit / delete comment** · _standard_ — Modify or remove your own messages
  Edit own comment (shows 'edited' tag) · delete own comment/reply · delete whole thread (author/owner) · confirm on destructive delete · permission-gated (author vs admin)
- ⬜ **Comments list / panel** · _standard_ — Side panel listing all threads in the file
  Scrollable list of threads · each row = avatar + snippet + location + status · click row → jump/zoom to pin on canvas · sort (newest/oldest/most-recent-activity) · unread indicators · count badge · two-way highlight with canvas pin
- ⬜ **Show/hide comments overlay** · _standard_ — Global toggle for comment pin visibility on canvas
  Show/hide all pins · hide resolved · independent of editing tools · per-view (does not delete data) · keyboard toggle · pins excluded from export/render
- ⬜ **Share link / invite** · _standard_ — Give others access to view/comment/edit a file
  Generate share URL · roles: viewer / commenter / editor · email invite · link scope (anyone-with-link vs restricted) · password/expiry · copy-link button · REQUIRES hosting/backend; offline alt = export/send the .varos file
- ⬜ **Permissions / roles** · _standard_ — Control who can do what
  Owner / editor / commenter / viewer tiers · per-file or per-project · can-comment vs can-edit gating · revoke access · transfer ownership · needs account/permissions backend
- ⬜ **Notifications** · _standard_ — Alert users to new comments, replies, mentions, resolves
  In-app inbox/bell · per-event types (reply, @mention, resolve, assigned) · email/push digests · mark-read · mute thread/file · notification settings · REQUIRES backend for delivery; in-app local notifications possible single-user only
- ⬜ **Pin anchoring targets** · _advanced_ — What a pin can attach to (point vs object vs region)
  Free point on canvas · anchored to a specific object/layer (follows when object moves) · anchored to an artboard/frame · region/area selection · text-range/in-context anchor · pin re-flows if anchored element transforms
- ⬜ **Reactions / emoji on comments** · _advanced_ — Quick emoji acknowledgement on a comment or reply
  Emoji picker · per-message reaction chips with counts · who-reacted tooltip · toggle on/off · common quick set (👍❤️🎉) · counts aggregate across users (needs sync to merge)
- ⬜ **Filter / search comments** · _advanced_ — Narrow the comment list
  Filters: open / resolved / all · mine / mentions-me / by-author · by artboard/page · text search within comments · date filters · 'only unread' · combine filters
- ⬜ **Read / unread tracking** · _advanced_ — Track which comments a user has seen
  Unread dot per thread/reply · 'mark all as read' · unread count in panel + file thumbnail · per-user read state (needs per-identity store; cross-device read state needs backend)
- ⬜ **Live presence / multiplayer cursors** · _advanced_ — See other users' cursors and selections live in the file
  Named colored cursors · live selection highlight · 'X is here' avatars in top bar · viewport-follow/spotlight · click-to-follow a user · REQUIRES real-time backend (CRDT/OT + websockets) — not an offline feature
- ⬜ **Avatar stack / active collaborators** · _advanced_ — Top-bar list of who is currently in the file
  Stacked avatars of online users · overflow '+N' · click to follow/locate · online/idle status dot · needs presence backend
- ⬜ **Real-time co-editing sync** · _advanced_ — Multiple people editing the same document simultaneously
  Conflict-free merge (CRDT/OT) · operational sync of edits · last-write/merge semantics · the substrate the whole live-collab layer needs · REQUIRES backend + sync server; offline tool can do async file-share instead
- ⬜ **Review / present mode** · _advanced_ — A read-focused mode for stakeholders to review and comment
  Distraction-free view · comment-only interaction · click-through artboards/prototype · fit-to-screen · separate from edit mode · can be local (present from desktop) but sharing it to remote reviewers needs backend
- ⬜ **Version sharing / version history** · _advanced_ — Share or review a specific saved version/snapshot
  Named versions/snapshots · restore/revert · compare versions · comment tied to a version · 'shared for review' link of a version · local snapshots work offline (file history); cross-user version sharing needs backend
- ⬜ **Comment assignment / tasks** · _advanced_ — Turn a comment into an actionable assigned task
  Assign thread to a person · due/status · 'assigned to me' filter · checkbox/todo conversion · ties into notifications · needs identity + (for cross-user) backend
- ⬜ **Attachments in comments** · _advanced_ — Attach files/images/screenshots to a comment
  Drag-drop image/file · inline thumbnail preview · paste from clipboard · download attachment · size limits · stored in file (offline) or uploaded (needs storage backend)
- ⬜ **Deep links to comments** · _advanced_ — A URL/anchor that opens the file at a specific comment
  Copy-link-to-comment · opens app + zooms to pin + opens thread · used in notification emails · needs URL scheme / hosting for remote; local deep link possible via app URI
- ⬜ **Activity log / audit feed** · _advanced_ — Chronological feed of comment + edit activity in the file
  Who did what when · comment/resolve/edit events · filter by type/person · per-file activity panel · export · needs event store; richer cross-user feed needs backend
- ⬜ **Comment export / report** · _advanced_ — Export all comments for handoff/records
  Export threads to CSV/PDF/markdown · include location + status + author · annotated-PDF review export · works offline from file data
- ⬜ **Block / mute / report** · _advanced_ — Moderation controls for shared/public files
  Mute a thread/file's notifications · block a user · report abuse on public links · spam controls · relevant only with multi-user backend + public sharing

## 14. Welcome / Home / New-Document
*The first-run/home/start experience and the New Document creation flow: home screen (recent files, open, learn, what's-new), the New Document dialog (preset categories, sizes, units, color mode, DPI, artboards, bleed, orientation, templates), recent presets, and sample/onboarding content.*  
(28 items — ✅0 · 🟡0 · ⬜28 · ❔0)

- ⬜ **Home / Start screen (landing view)** · _core_ — Full-window launch screen shown when no document is open
  Appears on app start & when all docs closed · large logo/branding · big 'New file' + 'Open' buttons · search bar for recent files · left nav rail (Home / Learn / Recent / Files) · empty-state when no recents
- ⬜ **Recent files grid / list** · _core_ — Browsable list of recently opened documents for quick reopen
  Thumbnail previews · file name + path + last-modified date + size/dimensions · grid vs list toggle · sort (name/date/kind) · filter/search · right-click (open / open containing folder / remove from recent / copy path / pin) · pin-to-top favorites · clear recent list · missing-file (greyed/relink) state · cloud vs local badge
- ⬜ **Open file (from home)** · _core_ — Entry points to open existing documents from the start screen
  'Open...' button → OS file picker · drag-and-drop file onto window/home · open recent shortcut · supported-formats filter in picker (.varos native + .svg/.ai/.pdf/.eps import) · 'Open recent' submenu · recently-opened-folders
- ⬜ **New file (from home) entry** · _core_ — Primary action that launches document creation
  'New file' button on home · keyboard shortcut (Ctrl+N) · 'New from template' variant · 'New from clipboard' (auto-size to pasted content) · quick-create with last-used preset (skip dialog)
- ⬜ **New Document dialog (overall)** · _core_ — Modal for configuring and creating a new canvas/document
  Left: preset category tabs · center: preset thumbnail gallery · right: detail/settings panel · live preview of page · 'Create' / 'Cancel' buttons · remembers last settings · resizable/scrollable · escape to cancel
- ⬜ **Preset categories (tabs)** · _core_ — Grouping of document presets by intended medium
  Recent · Saved (custom) · Print · Web · Mobile/Devices · Film & Video · Art & Illustration · (Affinity adds: Photo, Architectural; AI adds: Tablet) · category icons · per-category default unit & color mode
- ⬜ **Built-in size presets (per category)** · _core_ — Common ready-made document dimensions
  Print: A4/A3/A5/A6, Letter/Legal/Tabloid, business card, postcard · Web: 1920×1080, 1366×768, common screen, social (IG post/story, FB, Twitter/X, YouTube thumb) · Mobile: iPhone (various), Android, iPad/tablet · Film: HDTV 1080p, 4K UHD, DCI, NTSC/PAL, 720p · Art: square canvases, print-at-DPI · shows W×H + unit + orientation on each card
- ⬜ **Custom width / height** · _core_ — Manual entry of document dimensions
  W & H numeric fields · per-field unit suffix · link/lock aspect-ratio toggle · math in field (e.g. 1920/2) · swap W↔H · min/max clamping · live thumbnail update
- ⬜ **Units selector** · _core_ — Measurement unit for the new document
  Pixels · Points · Picas · Inches · Millimeters · Centimeters · (Affinity: feet/meters for large format) · per-document default · converts existing values on change · ruler unit follows
- ⬜ **Orientation** · _core_ — Portrait vs landscape page setup
  Portrait / Landscape toggle buttons · swaps W and H · greyed when square · affects all artboards/preview
- ⬜ **Color mode** · _standard_ — Document working color model
  RGB (web/screen) · CMYK (print) · (Grayscale, LAB in some tools) · default per category (web→RGB, print→CMYK) · sets default swatches/profile · warning that changing mode later shifts colors
- ⬜ **Raster / rasterization resolution (DPI/PPI)** · _standard_ — Pixel density for raster effects & placed images
  High (300ppi) / Medium (150) / Screen (72) presets + custom value · affects rasterize effects, blur, exports · 'Raster Effects Resolution' in AI · default by category (print 300, web 72)
- ⬜ **Number of artboards / pages** · _standard_ — Create multiple artboards in one document at creation time
  Count field · arrangement (grid-by-row / grid-by-column / by-row / by-column / arrange flow) · columns count · spacing/gutter between artboards · auto-layout preview · (Affinity uses single canvas; AI multi-artboard)
- ⬜ **Bleed** · _standard_ — Extra printable margin beyond trim edge
  Top/Bottom/Left/Right values · link-all toggle · unit-aware · shown as red guide in preview · print-category default (e.g. 3mm/0.125in) · 0 for web
- ⬜ **Document name field** · _standard_ — Name assigned to the new document
  Defaults 'Untitled-1' (auto-increment) · editable before create · becomes window title & save filename suggestion
- ⬜ **Recent presets (in dialog)** · _standard_ — Quick access to recently used document setups
  Top row/tab of last-N created configs · one-click recreate · shows dimensions+unit · auto-populated
- ⬜ **Live document preview (in dialog)** · _standard_ — Visual representation of the configured page
  Shows page proportions · bleed (red) + margin (guide) overlays · multi-artboard layout preview · orientation reflected · approximate scale label
- ⬜ **First-run / onboarding experience** · _standard_ — Guided introduction the first time the app launches
  Welcome splash · what's-this product blurb · quick tour / coachmarks of UI · theme/light-dark choice · UI density/scale · workspace/role pick · 'skip tour' · sign-in/account prompt (optional, can be offline) · sample document offer
- ⬜ **Quick-start / blank-by-default fast path** · _standard_ — Skip the dialog and jump straight into a canvas
  Affinity/AI option: open last preset instantly · 'Create New' with Enter · pref to bypass home screen on launch · 'Always start with New Document' setting
- ⬜ **Color profile / bit depth** · _advanced_ — ICC profile and per-channel precision
  RGB profile (sRGB / Display P3 / Adobe RGB) · CMYK profile (US Web Coated etc.) · 8-bit vs 16-bit per channel · transparent vs white background · advanced/collapsible section
- ⬜ **Margins / safe area** · _advanced_ — Non-bleed inner guides for content safe zone
  Top/Bottom/Left/Right margin values · link toggle · used for guides on canvas · 'Title/action safe' for film presets
- ⬜ **Save / manage custom presets** · _advanced_ — Persist user-defined document configurations
  'Save preset' (save icon) from current settings · name it · appears under Saved category · delete/rename custom presets · export/import preset files
- ⬜ **Templates (built-in & online)** · _advanced_ — Pre-designed starter files with content, not just blank size
  Template gallery tab/section · categories (flyer, card, social, presentation, logo) · thumbnail browse + search · local bundled + downloadable/online templates · 'Find more templates' link · preview before use · opens as editable copy · 'Blank document' option alongside
- ⬜ **What's New panel** · _advanced_ — Release-notes / feature highlights for current version
  Shown on first launch after update · version number · feature cards with images/gifs · 'learn more' links · dismiss / 'don't show again' · accessible later from Help menu
- ⬜ **Learn / tutorials section** · _advanced_ — In-home access to educational content
  Tutorial cards/thumbnails · getting-started guides · video links · 'hands-on' practice files · external docs/community links · search · (Illustrator 'Discover' panel) · offline-bundled vs online
- ⬜ **Sample / example files** · _advanced_ — Ready-made artwork to explore the tool's capabilities
  'Sample files' or 'Open sample' on home · demonstrate features (gradients, type, effects) · open as copy (don't overwrite) · bundled offline · good first-run safety net
- ⬜ **Cloud / account area (home)** · _advanced_ — Sign-in and cloud document access from start screen
  Account avatar/sign-in · cloud documents tab · sync status · storage usage · (optional for offline-first tool — can be omitted/stubbed) · 'work offline' mode
- ⬜ **Home screen preferences / behavior settings** · _advanced_ — User control over the welcome experience
  Toggle: show home screen on launch · number of recents to keep · default new-doc preset · disable 'what's new' · reset onboarding · theme on home

## 15. Menu command map
*Exhaustive top-menu command catalog (File, Edit, Object, Type, Select, Effect, View, Window, Help) grounded in Adobe Illustrator + Affinity Designer, with sub-menus, key options/modifiers, importance and Varos status.*  
(144 items — ✅6 · 🟡12 · ⬜126 · ❔0)

- ⬜ **File ▸ New** · _core_ — Create a new document
  Preset categories (Print/Web/Mobile/Film&Video/Art&Illustration) · size/units/orientation · number of artboards + grid layout · bleed (top/bottom/left/right) · color mode RGB/CMYK · raster effects PPI · templates
- ⬜ **File ▸ Open** · _core_ — Open an existing document
  Open native + interop formats (.ai/.afdesign/.svg/.pdf/.eps) · file dialog · format filter
- ⬜ **File ▸ Open Recent Files** · _core_ — Re-open recently used documents
  MRU submenu list · 'Clear Recent' entry · count configurable in prefs
- ⬜ **File ▸ Close** · _core_ — Close current document
  Prompt to save if dirty · 'Close All' variant · 'Close Other'
- ⬜ **File ▸ Save** · _core_ — Persist document to native format
  Save to .varos/native · no dialog if already saved · dirty-flag aware
- ⬜ **File ▸ Save As** · _core_ — Save under new name/location/format
  Native + format options · creates copy as active doc
- ⬜ **File ▸ Place / Import** · _core_ — Bring external content into doc
  Place raster/vector/PDF/SVG · Link vs Embed toggle · Template option · replace-on-reselect · loaded-cursor multi-place · Affinity 'Place' tool
- ⬜ **File ▸ Export ▸ Export As** · _core_ — Export to a single output format
  PNG/JPG/SVG/PDF/EPS/TIFF/GIF/WEBP · resolution/scale · color space · per-format options dialog
- ⬜ **File ▸ Document Setup** · _core_ — Edit doc-wide settings after creation
  Units · bleed · transparency grid · type/language options · edit artboards button · raster PPI
- ⬜ **File ▸ Exit / Quit** · _core_ — Close the application
  Prompts to save all dirty docs · app-level quit
- ✅ **Edit ▸ Undo** · _core_ — Reverse last action
  Multi-level history · Ctrl+Z · labeled with last op
- ✅ **Edit ▸ Redo** · _core_ — Re-apply undone action
  Ctrl+Shift+Z / Ctrl+Y · labeled with op
- ⬜ **Edit ▸ Cut** · _core_ — Remove selection to clipboard
  Ctrl+X · works on objects/text/anchors
- ⬜ **Edit ▸ Copy** · _core_ — Copy selection to clipboard
  Ctrl+C · vector data + render to OS clipboard
- ⬜ **Edit ▸ Paste** · _core_ — Paste clipboard at center/cursor
  Ctrl+V · pastes into active artboard/layer
- ⬜ **Edit ▸ Clear / Delete** · _core_ — Delete selection without clipboard
  Del key · removes objects/anchors
- ⬜ **Edit ▸ Preferences** · _core_ — App-wide settings panels
  General · Selection&Anchor · Type · Units · Guides&Grid · Smart Guides · Slices · Dictionary/Hyphenation · Plug-ins/Scratch · User Interface · Performance/GPU · File Handling&Clipboard · Appearance of Black
- 🟡 **Object ▸ Transform** · _core_ — Geometric transform submenu
  Move (dialog) · Rotate · Reflect · Scale · Shear · Transform Each · Transform Again (Ctrl+D) · Reset Bounding Box · copy-during-transform
- ✅ **Object ▸ Arrange** · _core_ — Z-order commands
  Bring to Front/Forward · Send to Back/Backward · Send to Current Layer
- ✅ **Object ▸ Align (menu)** · _core_ — Align/distribute via menu
  Mirrors Align panel · align-to selection/artboard/key object
- ✅ **Object ▸ Group / Ungroup** · _core_ — Group management
  Ctrl+G / Shift+Ctrl+G · nested groups
- 🟡 **Object ▸ Path** · _core_ — Path operations submenu
  Join · Average · Outline Stroke · Offset Path · Simplify · Add/Remove Anchor Points · Divide Objects Below · Split Into Grid · Clean Up · Reverse Path Direction
- ⬜ **Object ▸ Clipping Mask** · _core_ — Mask content to top shape
  Make (Ctrl+7) · Release · Edit Contents · Affinity: mask by nesting/child
- 🟡 **Object ▸ Compound Path** · _core_ — Combine paths into one with holes
  Make (Ctrl+8) · Release · even-odd/non-zero fill
- ✅ **Object ▸ Geometry (Affinity: Add/Subtract/Intersect/Divide/Combine)** · _core_ — Boolean operations menu
  Add · Subtract · Intersect · Divide · Combine (XOR) · non-destructive compound option
- 🟡 **Type ▸ Font / More from Adobe Fonts** · _core_ — Font family chooser
  Family/style submenu · recently used · activate cloud fonts · favorites
- 🟡 **Type ▸ Size** · _core_ — Font size submenu
  Preset sizes + 'Other…' · point/px units
- ⬜ **Type ▸ Create Outlines** · _core_ — Convert text to editable paths
  Shift+Ctrl+O · glyphs→compound paths · irreversible (lose editability)
- ⬜ **Select ▸ All / All on Active Artboard** · _core_ — Select everything
  Ctrl+A · all-artboard scope variant
- 🟡 **Select ▸ Deselect** · _core_ — Clear selection
  Shift+Ctrl+A
- 🟡 **View ▸ Zoom In / Out / Fit / 100%** · _core_ — Zoom commands
  Ctrl+ +/- · Fit Artboard in Window (Ctrl+0) · Fit All · Actual Size 100% (Ctrl+1) · Zoom to Selection
- ⬜ **View ▸ Smart Guides** · _core_ — Toggle dynamic alignment hints
  Ctrl+U · object/anchor snap cues · measurement/spacing labels · alignment lines
- ⬜ **View ▸ Snap to Point / Pixel / Grid / Guides** · _core_ — Snapping toggles
  Snap to Point · Snap to Pixel · Snap to Grid · Snap to Glyph · Affinity Snapping Manager
- 🟡 **Window ▸ Panels list (core)** · _core_ — Toggle dockable panels
  Layers · Properties · Appearance · Color · Color Guide · Swatches · Gradient · Stroke · Transparency · Align · Pathfinder · Transform
- ⬜ **Help ▸ About** · _core_ — Version & license info
  Build/version · license · credits · system info
- ⬜ **File ▸ New from Template** · _standard_ — Start from a .ait template file
  Opens template chooser · seeds document with reusable content/styles
- ⬜ **File ▸ Save a Copy** · _standard_ — Write a copy without switching active doc
  Keeps current doc open/active · for snapshots/backups
- ⬜ **File ▸ Revert** · _standard_ — Discard changes, reload last saved
  Restores from disk version · confirm prompt
- ⬜ **File ▸ Export ▸ Export for Screens** · _standard_ — Batch export artboards/assets at multiple scales
  Artboards vs Assets tabs · multi-scale (@1x/2x/3x) · format presets · suffix/prefix naming · output folder
- ⬜ **File ▸ Export ▸ Asset Export / Export Persona** · _standard_ — Slice-driven continuous asset export
  Affinity Export Persona · drag-to-create slices · per-slice formats/scales · 'Create Slices from layers/selection'
- ⬜ **File ▸ Export Selection** · _standard_ — Export only current selection
  Bounds-cropped export · same format options as Export As
- ⬜ **File ▸ Document Color Mode** · _standard_ — Switch RGB ⇄ CMYK for whole doc
  RGB Color / CMYK Color toggle · affects swatches & export
- ⬜ **File ▸ Print** · _standard_ — Print / generate print output
  Printer + PPD · marks & bleed · output color · color management · separations · scaling/tiling · print preview
- ⬜ **File ▸ Share / Export PDF** · _standard_ — Quick share or PDF publish
  PDF presets (press/print/screen/smallest) · share link · cloud (AI)
- ⬜ **Edit ▸ Paste in Front / in Back** · _standard_ — Paste preserving position + z-order
  Ctrl+F (front) / Ctrl+B (back) · same X/Y as source
- ⬜ **Edit ▸ Paste in Place / on All Artboards** · _standard_ — Paste at original coordinates
  Shift+Ctrl+V (in place) · Alt+Shift+Ctrl+V (all artboards)
- ⬜ **Edit ▸ Find & Replace** · _standard_ — Search/replace text in document
  Match case/word · whole-word · wildcards · replace all · search across text objects
- ⬜ **Edit ▸ Edit Colors** · _standard_ — Recolor/adjust color submenu
  Recolor Artwork · Adjust Color Balance · Blend Front→Back · Convert to RGB/CMYK/Grayscale · Invert/Saturate
- ⬜ **Edit ▸ Color Settings** · _standard_ — Document color management config
  RGB/CMYK working spaces · ICC profiles · rendering intent · conversion engine · assign/convert profile
- ⬜ **Edit ▸ Keyboard Shortcuts** · _standard_ — Customize/keymap editor
  Per-command rebinding · menu vs tool shortcuts · save/load sets · conflict warnings · export list
- ⬜ **Object ▸ Lock / Unlock All** · _standard_ — Prevent/restore editability
  Lock Selection · Lock All Above/Other Layers · Unlock All
- ⬜ **Object ▸ Hide / Show All** · _standard_ — Toggle object visibility
  Hide Selection · Hide Above/Other · Show All
- ⬜ **Object ▸ Expand** · _standard_ — Convert appearance to editable paths
  Expand fill/stroke/object · expand gradient to mesh/N-objects
- ⬜ **Object ▸ Expand Appearance** · _standard_ — Bake effects/brushes into geometry
  Flattens live effects to real paths
- ⬜ **Object ▸ Rasterize** · _standard_ — Convert vector to pixels
  Resolution/PPI · background transparent/white · anti-alias · add bleed
- 🟡 **Object ▸ Shape (Live Shapes)** · _standard_ — Convert/edit live shapes
  Convert to Shape · Expand Shape · live corner radius retention
- ⬜ **Object ▸ Repeat (Radial/Grid/Mirror)** · _standard_ — Live repeat clones
  Radial count/radius · grid spacing · mirror axis · edit-master-updates-all
- ⬜ **Object ▸ Blend** · _standard_ — Interpolated blend between objects
  Make/Release · Blend Options (steps/smooth/distance) · Replace Spine · Reverse Spine · Reverse Front to Back · Expand
- ⬜ **Object ▸ Distort & Transform / Warp / Envelope Distort** · _standard_ — Envelope & warp deformation
  Make with Warp (arc/arch/flag/wave…) · Make with Mesh (rows/cols) · Make with Top Object · Edit/Release/Expand Envelope · envelope options
- ⬜ **Object ▸ Image Trace** · _standard_ — Raster→vector tracing
  Make · Make and Expand · Release · presets (B/W, sketch, photo) · threshold/paths/corners
- ⬜ **Object ▸ Text Wrap** · _standard_ — Wrap text around object
  Make/Release · Text Wrap Options (offset)
- ⬜ **Object ▸ Artboards** · _standard_ — Artboard management
  Convert to Artboards · Rearrange All · Fit to Selected/Artwork Bounds · Add · Insert/Move · Artboard Options (size/name/marks)
- ⬜ **Type ▸ Recent Fonts** · _standard_ — Quick re-pick recent fonts
  MRU font list
- ⬜ **Type ▸ Glyphs** · _standard_ — Open Glyphs panel
  All glyphs · alternates · ligatures/swashes · OpenType features · insert by Unicode
- ⬜ **Type ▸ Area Type Options** · _standard_ — Configure text-box flow
  Rows/columns count + gutter · inset spacing · first baseline · text-flow order · auto-size
- ⬜ **Type ▸ Type on a Path** · _standard_ — Text along a path options
  Effect (rainbow/skew/3D/stair/gravity) · align to path · flip · spacing
- ⬜ **Type ▸ Threaded Text** · _standard_ — Link overflowing text boxes
  Create · Release Selection · Remove Threading · overflow port indicator
- ⬜ **Type ▸ Find Font** · _standard_ — List/replace fonts used
  Document fonts list · replace with system/doc font · missing-font fixes
- ⬜ **Type ▸ Change Case** · _standard_ — Recase selected text
  UPPER/lower/Title/Sentence case
- ⬜ **Type ▸ Convert to Area/Point Type** · _standard_ — Toggle text container model
  Point↔Area type conversion
- ⬜ **Select ▸ Inverse** · _standard_ — Invert current selection
  Selects all unselected objects
- ⬜ **Select ▸ Same** · _standard_ — Select objects sharing an attribute
  Fill Color · Stroke Color · Stroke Weight · Opacity · Blending Mode · Fill&Stroke · Graphic Style · Symbol Instance · Font family/size
- ⬜ **Select ▸ Object** · _standard_ — Select by object kind
  All on Same Layers · Direction Handles · Bristle/Brush Strokes · Clipping Masks · Stray Points · Text Objects · Point/Area Type · Flyaway/Not Aligned
- ⬜ **Effect ▸ Apply/Last Effect** · _standard_ — Reapply most recent effect
  Shift+Ctrl+E (apply) · Alt+Shift+Ctrl+E (with dialog)
- ⬜ **Effect ▸ Document Raster Effects Settings** · _standard_ — Global raster-effect resolution
  PPI (72/150/300) · background · anti-alias · clipping mask
- ⬜ **Effect ▸ Convert to Shape** · _standard_ — Live shape substitution effect
  Rectangle/Rounded Rect/Ellipse · absolute/relative size
- ⬜ **Effect ▸ Distort & Transform (vector)** · _standard_ — Vector distort live effects
  Free Distort · Pucker & Bloat · Roughen · Transform · Tweak · Twist · Zig Zag
- ⬜ **Effect ▸ Path** · _standard_ — Path-based live effects
  Offset Path · Outline Object · Outline Stroke
- 🟡 **Effect ▸ Pathfinder (live)** · _standard_ — Non-destructive boolean effects
  Add/Subtract/Intersect/Exclude · Merge/Trim/Crop/Outline · applied to groups
- ⬜ **Effect ▸ Stylize (vector)** · _standard_ — Common vector stylize effects
  Drop Shadow · Inner/Outer Glow · Feather · Round Corners · Scribble
- ⬜ **Effect ▸ Warp** · _standard_ — Live warp envelope effects
  Arc/Arc Lower-Upper/Arch/Bulge/Shell/Flag/Wave/Fish/Rise/Fisheye/Inflate/Squeeze/Twist · bend/distortion sliders
- ⬜ **Effect ▸ Photoshop ▸ Blur** · _standard_ — Raster blur effects
  Gaussian Blur · Radial Blur · Smart Blur
- ⬜ **View ▸ Outline / Preview / GPU Preview** · _standard_ — Switch rendering mode
  Outline (Ctrl+Y) wireframe · Preview (rendered) · GPU/CPU preview · Pixel Preview · Overprint Preview
- ⬜ **View ▸ Rulers** · _standard_ — Toggle/configure rulers
  Show/Hide Rulers (Ctrl+R) · Video Rulers · Change to Global/Artboard rulers · units context
- ⬜ **View ▸ Guides** · _standard_ — Guide management
  Hide/Show · Lock · Make Guides (from objects) · Release Guides · Clear Guides · drag-from-ruler
- ⬜ **View ▸ Grid** · _standard_ — Document grid
  Show/Hide Grid · Snap to Grid · grid spacing/subdivisions (in prefs)
- ⬜ **View ▸ Pixel Preview** · _standard_ — Preview rasterized pixel result
  Anti-aliased pixel render at export PPI
- 🟡 **View ▸ Show/Hide Bounding Box** · _standard_ — Toggle transform handles
  Shift+Ctrl+B · selection bbox visibility
- ⬜ **View ▸ Show/Hide Edges** · _standard_ — Toggle selection path highlight
  Ctrl+H · hides anchor/path overlay
- ⬜ **View ▸ Show Artboards / Print Tiling / Slices** · _standard_ — Toggle structural overlays
  Artboard rulers/edges · print page tiling · slice boundaries · text threads · gradient annotator
- ⬜ **Window ▸ Workspace** · _standard_ — Switch/save UI layouts
  Essentials/Painting/Typography/Layout presets · New Workspace · Manage/Reset
- ⬜ **Window ▸ Panels list (type/symbols)** · _standard_ — Type & asset panels
  Character · Paragraph · Character/Paragraph Styles · Glyphs · Tabs · OpenType · Symbols · Brushes · Graphic Styles
- 🟡 **Window ▸ Toolbars / Control / Tools** · _standard_ — Toggle tool & control bars
  Basic/Advanced toolbar · Control (contextual) bar · customize tools
- ⬜ **Window ▸ Open Documents list** · _standard_ — Switch between open docs
  Active document checklist at menu bottom
- ⬜ **Help ▸ Documentation / User Guide** · _standard_ — Open help docs
  Online manual · in-app help home
- ⬜ **Help ▸ Search / Discover (in-app)** · _standard_ — Searchable command & help finder
  Find commands/tutorials · contextual search
- ⬜ **Help ▸ Keyboard Shortcuts / Welcome / Tutorials** · _standard_ — Learning entry points
  Shortcut reference · welcome screen · hands-on tutorials
- ⬜ **Help ▸ Check for Updates** · _standard_ — Update the app
  Version check · download/install updates
- ⬜ **File ▸ Browse in Bridge / Browse Templates** · _advanced_ — Visual asset/file browser entry
  Adobe Bridge launch (AI) · thumbnail browsing of templates/stock
- ⬜ **File ▸ Save as Template** · _advanced_ — Save current as reusable template
  Writes .ait-style template · strips instance content
- ⬜ **File ▸ Save Selected Slices / Save for Web (legacy)** · _advanced_ — Web-optimized export of slices
  Per-slice PNG/JPG/GIF · legacy export-for-web dialog
- ⬜ **File ▸ Package** · _advanced_ — Collect doc + fonts + links into a folder
  Copies linked images · copies/embeds fonts · generates report · for handoff
- ⬜ **File ▸ Scripts** · _advanced_ — Run automation scripts
  Bundled scripts list · 'Other Script…' to browse · JS/ExtendScript host
- ⬜ **File ▸ File Info / Document Properties** · _advanced_ — Edit metadata
  Title/author/keywords/copyright · XMP metadata · description
- ⬜ **Edit ▸ Paste without Formatting / Special** · _advanced_ — Paste stripped or as chosen type
  Plain content · choose embed/link/raster on paste
- ⬜ **Edit ▸ Find Next** · _advanced_ — Jump to next match
  Continues last Find query
- ⬜ **Edit ▸ Check Spelling** · _advanced_ — Spell-check document text
  Per-language dictionary · ignore/learn/change · auto spell flagging
- ⬜ **Edit ▸ Edit Custom Dictionary** · _advanced_ — Manage added words
  Add/remove user dictionary entries
- ⬜ **Edit ▸ Edit Original** · _advanced_ — Open linked file in source app
  Round-trip edit of placed/linked asset
- ⬜ **Edit ▸ Transparency Flattener Presets** · _advanced_ — Manage flattening settings
  Low/medium/high presets · raster/vector balance for print
- ⬜ **Edit ▸ Print Presets / PDF Presets / SWF Presets** · _advanced_ — Manage export/print preset sets
  Create/load/export named presets
- ⬜ **Edit ▸ Assign Profile** · _advanced_ — Attach/replace color profile
  Don't manage / working space / specific profile
- ⬜ **Edit ▸ My Settings / Sync Settings** · _advanced_ — Backup & sync app preferences
  Export/import settings bundle · cloud sync
- ⬜ **Object ▸ Crop Marks / Create Trim Marks** · _advanced_ — Add print crop marks
  Around selection bounds
- ⬜ **Object ▸ Slice** · _advanced_ — Web slice management
  Make/Release · Slice Options · Create from Guides/Selection
- ⬜ **Object ▸ Pattern ▸ Make/Edit** · _advanced_ — Create/edit tiling patterns
  Pattern editing mode · tile type (grid/brick/hex) · spacing/overlap · copies preview
- ⬜ **Object ▸ Perspective** · _advanced_ — Perspective grid attach/edit
  Attach to Active Plane · Release · Move Plane to Match
- ⬜ **Object ▸ Live Paint** · _advanced_ — Live Paint group ops
  Make · Merge · Release · Gap Options · Expand
- ⬜ **Object ▸ Graph** · _advanced_ — Data-driven graph objects
  Type/Data/Design/Column/Marker · edit graph data
- ⬜ **Type ▸ Fit Headline** · _advanced_ — Stretch type to fill width
  Justifies single-line point/area text to column width
- ⬜ **Type ▸ Smart Punctuation** · _advanced_ — Convert to typographer's marks
  Smart quotes/dashes/ligatures/fractions/ellipsis
- ⬜ **Type ▸ Insert Special Character / Whitespace / Break** · _advanced_ — Insert glyphs & spacing marks
  Symbols/dashes · em/en/hair/thin space · line/column/page break
- ⬜ **Type ▸ Show Hidden Characters** · _advanced_ — Toggle invisibles
  Spaces/tabs/returns/break markers
- ⬜ **Type ▸ Type Orientation** · _advanced_ — Horizontal/vertical text
  Switch writing direction · for CJK/decorative
- ⬜ **Select ▸ Reselect** · _advanced_ — Reapply last Select query
  Ctrl+6 · repeats Same/Object filter
- ⬜ **Select ▸ Next / Previous Object Above-Below** · _advanced_ — Step selection through z-order
  Alt+Ctrl+] / [ · cycle stacked objects
- ⬜ **Select ▸ Start/Stop Global Edit** · _advanced_ — Edit all similar objects at once
  Match by appearance/size · live multi-edit
- ⬜ **Select ▸ Save Selection** · _advanced_ — Name & store current selection
  Recall later from Select menu · appears at bottom of menu
- ⬜ **Select ▸ Edit Selection** · _advanced_ — Rename/delete saved selections
  Manage stored named selections
- ⬜ **Effect ▸ 3D and Materials** · _advanced_ — Live 3D extrude/revolve/inflate
  Extrude & Bevel · Revolve · Rotate · materials/lighting · map artwork to surface
- ⬜ **Effect ▸ Crop Marks** · _advanced_ — Live crop-mark effect
  Non-destructive trim marks around bounds
- ⬜ **Effect ▸ Rasterize (live)** · _advanced_ — Live raster conversion effect
  Reversible rasterization in appearance stack
- ⬜ **Effect ▸ SVG Filters** · _advanced_ — Apply/import SVG filter effects
  Built-in SVG filters · import custom .svg filter
- ⬜ **Effect ▸ Photoshop Effects ▸ Effect Gallery** · _advanced_ — Raster filter gallery
  Browse/stack PS-style filters with preview
- ⬜ **Effect ▸ Photoshop ▸ Brush Strokes / Distort / Sketch / Stylize / Texture / Artistic / Video / Pixelate** · _advanced_ — Raster artistic filter families
  Accented edges · glass/ocean ripple · halftone · glowing edges · grain/mosaic/craquelure · NTSC colors
- ⬜ **View ▸ Rotate View** · _advanced_ — Rotate the canvas view
  Non-destructive canvas rotation · reset to 0°
- ⬜ **View ▸ Perspective Grid** · _advanced_ — Show/define perspective grid
  1/2/3-point presets · define grid · snap to grid · lock station point
- ⬜ **View ▸ Show/Hide Transparency Grid** · _advanced_ — Toggle checkerboard backdrop
  Shift+Ctrl+D · visualize transparency
- ⬜ **View ▸ Trim View** · _advanced_ — Hide everything past artboard edge
  Clean preview without bleed/overflow
- ⬜ **View ▸ New View / Edit Views** · _advanced_ — Save named zoom/scroll states
  Store viewport bookmarks · recall quickly
- ⬜ **View ▸ Presentation / Full Screen Mode** · _advanced_ — Distraction-free preview
  Hide all UI chrome · artboard-only view
- ⬜ **Window ▸ Panels list (advanced)** · _advanced_ — Power/management panels
  Asset Export · Artboards · Links · Navigator · Info · Document Info · Actions · Variables · Magic Wand · Image Trace · Separations Preview · Flattener Preview · Attributes · History (Affinity) · Libraries/CC Libraries
- ⬜ **Window ▸ Application Frame / Arrange Documents** · _advanced_ — Window & multi-doc layout
  Tile/cascade · float tabs · consolidate all · new window of same doc
- ⬜ **Help ▸ System Info / Diagnostics** · _advanced_ — Environment report for support
  GPU/OS/memory · copy diagnostics · report a bug

## 16. Preferences + Effects/Appearance
*Every app-wide preference/setting pane plus the non-destructive Appearance/Effects/Graphic-Styles system (multiple fills/strokes, live effects, save-and-apply styles, expand).*  
(48 items — ✅0 · 🟡4 · ⬜44 · ❔0)

- ⬜ **General preferences** · _core_ — Core app-wide behavior knobs
  Units (per type) shortcut here · Keyboard Increment (arrow-key nudge distance) · Constrain Angle (global rotation baseline for shapes/move) · Corner Radius default for rounded-rect · Disable auto-add/delete anchors · Use precise cursors · Show/disable tooltips · Anti-aliased artwork · Select same tint % · Append [Converted] to filenames · Double-click to isolate · Use Japanese crop marks · Transform pattern tiles · Scale corners/strokes & effects · Reset all warning dialogs
- ⬜ **Units preferences** · _core_ — Measurement units for each context
  General units (px/pt/pica/inch/mm/cm/ha) · Stroke units · Type units · Asian type units · Identify objects by Object Name vs XML ID · per-document override
- ⬜ **Appearance panel** · _core_ — Non-destructive stack of all attributes per object
  Lists every fill, stroke, effect, opacity for selected object/group/layer · Reorder by drag (stacking order = render order) · Show/hide each attribute (eye) · Duplicate/delete attribute · Target indicator · Clear appearance / reduce to basic appearance · 'New art has basic appearance' toggle · Edit attribute by clicking its value
- ⬜ **Drop Shadow effect** · _core_ — Live cast shadow behind object
  Mode (blend) · Opacity % · X/Y offset · Blur radius · Color vs Darkness % · live & editable · stacks in Appearance
- ⬜ **Effect re-edit / stacking / order** · _core_ — Manage applied live effects
  Double-click in Appearance to re-open dialog · reorder effects (order changes result) · effect on fill vs stroke vs object · delete effect · move between attributes
- 🟡 **Selection & Anchor Display prefs** · _standard_ — How selections/anchors/handles look & behave
  Selection tolerance (px) · Object selection by path only · Snap to point + distance · Command/Ctrl-click to select behind · Anchor point size (3 sizes) · Handle size (3 styles) · Highlight anchors on hover · Show handles when multiple anchors selected · Enable rubber-band for Pen/Curvature · Hide corner widget below N px
- ⬜ **Type preferences** · _standard_ — Text-engine defaults & behavior
  Size/leading increment · Tracking increment · Baseline shift increment · Greek type below N px · Show font names in English · Number of recent fonts · Enable in-menu font preview + preview size · Highlight substituted/alternate glyphs · Enable missing-glyph protection · Auto-size new area type · Fill new type objects with placeholder/lorem text · Default direction (LTR/RTL)
- ⬜ **Guides & Grid preferences** · _standard_ — Appearance of guides and the document grid
  Guide color + style (lines/dots) · Grid color + style · Gridline every (spacing) · Subdivisions count · Grids in back · Show pixel grid (above zoom %)
- ⬜ **Smart Guides preferences** · _standard_ — Toggle/configure live alignment hints
  Color · Alignment guides on/off · Anchor/path labels · Object highlighting · Measurement labels · Transform tools snapping · Construction guides angle set (preset angles) · Snapping tolerance (px)
- 🟡 **User Interface preferences** · _standard_ — Theme & chrome appearance
  Brightness/theme (Dark/Medium-Dark/Light/Medium-Light) · Canvas color (match UI brightness vs white) · UI Scaling slider (small↔large) · Scale cursor proportionally · Auto-collapse iconic panels · Open documents as tabs · Large/small tabs · Show panel labels
- ⬜ **File Handling & Clipboard prefs** · _standard_ — Saving, recovery, links, copy/paste format
  Data recovery / auto-save interval + on/off · Number of recent files · Use low-res proxy for linked images · Auto-relink missing links · Clipboard copy as: PDF / AICB (preserve paths vs appearance) / SVG Code · Background export/save · Enable Version History
- 🟡 **Performance / GPU preferences** · _standard_ — Rendering acceleration & smoothness toggles
  GPU Performance on/off · Animated Zoom (smooth vs stepped) · Real-time drawing & editing · Show GPU/CPU mode indicator · History states count (undo depth) · Rotate-view GPU · Retina/HiDPI handling
- ⬜ **Cursor / pointer preferences** · _standard_ — Tool-cursor style & precision
  Standard vs Precise (crosshair) cursors · Caps-Lock toggles precise · Brush-size circle cursor · Snap-cursor feedback · custom cursor set per tool
- ⬜ **Keyboard shortcuts editor** · _standard_ — View/remap all shortcuts & make sets
  Menu commands vs Tools tabs · Assign/clear keys · Conflict warnings · Save/load named shortcut sets · Export shortcut list · import from other apps
- ⬜ **Color & appearance app settings** · _standard_ — Default color model & syncing prefs
  Default document color mode (RGB/CMYK) · Color management/sync · Black appearance (rich vs 100%K) · Color picker mode (HSB/RGB/Hex/wheel)
- ⬜ **Reset & preferences storage** · _standard_ — Restore defaults / portable settings
  Reset all preferences on launch (modifier) · Reset warning dialogs · Export/import preferences · Sync settings to cloud account
- ⬜ **Multiple fills per object** · _standard_ — Stack several fills on one path
  Add Fill button · each fill own color/gradient/pattern + opacity + blend mode · stack order matters · per-fill live effect (e.g. offset+color) · enables complex single-path graphics
- ⬜ **Multiple strokes per object** · _standard_ — Stack several strokes on one path
  Add Stroke button · each own weight/color/dash/align/opacity/blend · offset via Transform effect · parallel-line & railroad effects from one path
- ⬜ **Per-attribute & object-level effects** · _standard_ — Apply live effect to a single fill/stroke or whole object
  Effect attached under specific fill/stroke vs at top (whole object) · drag effect between attributes · double-click effect to re-edit params · effects re-render on any edit
- ⬜ **Inner / Outer Glow effect** · _standard_ — Soft luminous halo inside or outside edges
  Blend mode · Color swatch · Opacity · Blur · Inner: center vs edge source
- ⬜ **Blur effects (Gaussian)** · _standard_ — Soften object non-destructively
  Gaussian Blur radius · (legacy Radial/Smart blur in raster-effects set) · resolution tied to raster effects settings
- ⬜ **Feather effect** · _standard_ — Fade object edges to transparent
  Feather radius (px) · soft vignette edge · vector-based
- ⬜ **Distort & Transform: Transform effect** · _standard_ — Live move/scale/rotate/reflect + copies
  Scale H/V · Move H/V · Rotate angle · Reflect X/Y · Copies count · Random · Reflect-about anchor (9-pt) · non-destructive, re-editable
- ⬜ **Distort & Transform: Roughen** · _standard_ — Add jagged/rough variation to path
  Size (% or absolute) · Detail (points per inch) · Smooth vs Corner points · live preview
- ⬜ **Distort & Transform: Zig Zag** · _standard_ — Convert path to waves or spikes
  Size · Ridges per segment · Points: Smooth (wave) vs Corner (zigzag) · relative/absolute
- ⬜ **Warp effects** · _standard_ — 15 preset envelope-style bends
  Styles: Arc, Arc Lower/Upper, Arch, Bulge, Shell Lower/Upper, Flag, Wave, Fish, Rise, Fisheye, Inflate, Squeeze, Twist · Horizontal/Vertical orientation · Bend % · Distortion H/V % · live & re-editable
- ⬜ **Convert to Shape effect** · _standard_ — Live retarget path to rectangle/rounded-rect/ellipse
  Shape choice · Absolute size or Relative (extra W/H) · corner radius · non-destructive (e.g. auto-resizing button backgrounds)
- ⬜ **Stylize: Round Corners effect** · _standard_ — Live-round all corner anchors
  Radius value · applies to whole path non-destructively
- ⬜ **Stylize: Scribble/Glow/Shadow grouping** · _standard_ — Stylize submenu container
  Drop Shadow, Inner/Outer Glow, Round Corners, Scribble, Feather all live under Stylize · vector-effect family
- ⬜ **Path effects (Offset Path / Outline Object / Outline Stroke)** · _standard_ — Live path-geometry effects
  Offset Path: distance + joins (miter/round/bevel) + miter limit · Outline Object (live) · Outline Stroke (live) · re-editable vs destructive menu versions
- ⬜ **Graphic Styles panel** · _standard_ — Save & reuse a whole appearance as one style
  Save current appearance as style · Apply to selection (one click) · thumbnail library · default styles · merge styles (combine multiple) · break link to style · additive apply (Alt/Opt) · rename/delete/duplicate · libraries (.ai style libs)
- ⬜ **Expand Appearance** · _standard_ — Bake live appearance into editable vector geometry
  Converts effects/multiple fills-strokes/styles into real paths & groups · destructive (no longer live) · needed for export/handoff · vs 'Expand' (fills/strokes/gradient to mesh)
- ⬜ **Blend modes & opacity in Appearance** · _standard_ — Per-attribute transparency & compositing
  Object opacity + per-fill/per-stroke opacity & blend mode · Normal/Multiply/Screen/Overlay/etc. · Isolate Blending · Knockout Group · Opacity mask (lives in Transparency panel, surfaces in Appearance)
- ⬜ **Slices preferences** · _advanced_ — Web-slicing display options
  Show slice numbers · Line color for slice boundaries · (legacy web-export feature)
- ⬜ **Hyphenation / Dictionary prefs** · _advanced_ — Language dictionary & hyphenation control
  Default language · Hyphenation exceptions (add/remove words) · New word case-sensitivity · spell-check dictionary selection
- ⬜ **Plug-ins & Scratch Disks prefs** · _advanced_ — Extension folder & temp/swap disk config
  Additional plug-ins folder (choose path) · Primary scratch disk · Secondary scratch disk (for virtual memory when RAM full)
- ⬜ **Distort & Transform: Pucker & Bloat** · _advanced_ — Curve segments inward/outward from anchors
  Slider -100 (pucker/spiky) ↔ +100 (bloat/rounded) · live
- ⬜ **Distort & Transform: Tweak** · _advanced_ — Randomly distort anchors & handles
  Horizontal/Vertical amount (%/abs) · Anchor points · 'In'/'Out' control points toggles · random jitter
- ⬜ **Distort & Transform: Twist** · _advanced_ — Rotate inner more than outer
  Angle (degrees, +/-) · spirals the path · live
- ⬜ **Distort & Transform: Free Distort** · _advanced_ — Drag 4 corners to perspective-warp
  4-handle bounding box dialog · non-destructive envelope-lite
- ⬜ **Scribble effect** · _advanced_ — Make fills look hand-sketched
  Presets (Childlike/Dense/etc.) · Angle · Path overlap +/- · Variation · Stroke width · Curviness · Spacing · Variation sliders
- ⬜ **3D effects (Extrude & Bevel / Revolve / Rotate / Inflate)** · _advanced_ — Pseudo-3D from 2D art
  Extrude depth + caps + bevels · Revolve angle + offset · Rotate in 3D space (X/Y/Z) · Inflate · Lighting (light sources, shading color, ambient, highlight, blend steps) · Map art onto surfaces · Perspective · (classic vs new 3D-and-Materials w/ ray-tracing, materials, shadows)
- 🟡 **Pathfinder effects (live)** · _advanced_ — Boolean ops applied live to groups
  Add/Intersect/Exclude/Subtract/Minus Back/Divide/Trim/Merge/Crop/Outline/Hard-Soft Mix/Trap as live effect on a group (vs one-shot Pathfinder panel)
- ⬜ **Raster / Photoshop effects** · _advanced_ — Pixel-based filter effects on vector art
  Effect Gallery, Blur, Brush Strokes, Distort, Pixelate, Sketch, Texture, Artistic, Video · rasterizes at Document Raster Effects resolution
- ⬜ **Rasterize effect & Document Raster Effects Settings** · _advanced_ — Control resolution/quality of raster effects
  Resolution (72/150/300/dpi) · Background white/transparent · Anti-alias · Clipping mask · Add space around object · Preserve spot colors · affects all live raster effects
- ⬜ **SVG Filters effect** · _advanced_ — Apply/import SVG filter primitives
  Built-in SVG filter presets · Import SVG Filter · Apply SVG Filter dialog · feGaussianBlur/feColorMatrix etc. · export-faithful
- ⬜ **Graphic Style updates & links** · _advanced_ — Live-linked styles propagate edits
  Edit a style → all objects using it update · override per-object then re-apply · style applied to layer cascades to children
- ⬜ **Appearance targeting (object/group/layer)** · _advanced_ — Apply appearance at different hierarchy levels
  Target ring in Layers panel · effect on group vs each child · layer-level appearance applies to all contents · drag/duplicate appearance between targets

## ⚠️ Possibly missing (completeness critic — to review)
**Whole categories:** Selection & Hit-testing systems (as a first-class category — select-behind/cycle, marquee touch-vs-enclose policy, lasso, select-by-attribute matching engine, isolation hit model) · Brushes / Brush definition system (a full category: calligraphic/art/scatter/bristle/pattern/image brushes, brush libraries, brush options, brush-to-stroke binding) — only referenced piecemeal under Stroke · Symbols / Components / Instances system (master-instance model, overrides, nested instances, 9-slice scaling, dynamic symbols, swap-instance, detach) — the Figma-component backbone, only a panel stub exists · Image / Raster handling & Image Trace (place-link-embed pixel images, crop, mask, resolution, PSD layers, Image Trace vectorization, raster effects resolution) as its own pipeline · Effects / Live Effects / Appearance pipeline at category level beyond #16 — Blend (object blends/spine), Envelope Distort, Warp, 3D & Materials, Pattern-making, Live Paint, Gradient Mesh as a coherent generative-art system · Plugins / Extensibility / Scripting / API (the stated Varos MOAT — extension manifest, plugin panels, scripting host, the single-schema RNA, plugin marketplace, sandboxing) — almost entirely absent from the catalog · Accessibility (a11y) — keyboard-only operation, screen-reader/ARIA, focus order, high-contrast UI, color-blind-safe UI, reduced-motion, hit-target sizing, alt-text on exported SVG · Preferences/Settings infrastructure beyond the panes listed — settings storage/portability, per-document vs app scope, telemetry/privacy opt-in, crash reporting, GPU/driver fallback settings, language/locale of the UI itself · Internationalization / Localization of the APPLICATION (UI language, RTL UI mirroring, locale number/unit formatting) — distinct from RTL text content · Window / Workspace / Multi-document management (tabs, tear-off windows, tile/cascade, second window of same doc, saved workspaces, panel docking/floating/iconize, multi-monitor) — only scattered menu items · Status bar / Info readout system (persistent bottom bar: zoom field, artboard selector, current-tool hint, cursor coords, selection count, units, document color profile indicator) · Onboarding / Help / In-app guidance beyond Home (command palette / search-actions, contextual tooltips, interactive tutorials, what's-this, learn panel) — partially in #14/#15 but no command-palette/quick-search category · Performance / Rendering engine controls & large-document handling (GPU vs CPU, zoom-dependent render quality, occlusion, draft mode, memory/scratch, object-count limits, tiling) as a user-facing concern · Measurement / Annotation / Dimensioning tools (Measure tool, dimension lines, redline/spec annotations, distance+angle readout tool) — Measure tool is entirely absent · Data-driven / Variables / Templating (data merge, variables panel, CSV/JSON binding, dynamic content) — only a panel stub; no category · Auto Layout / Constraints / Responsive layout (Figma auto-layout, resizing constraints pin/stretch, layout grids as constraints) — a core modern-tool system entirely missing · Prototyping / Interaction / Animation (frames-as-screens, links/hotspots, transitions, micro-interactions, motion/Affinity-isn't-but-Figma-is) — absent; relevant given Figma framing · Print / Production / Prepress system (separations, overprint, trapping, crop/registration marks, ICC output intent, flattener, bleed-at-output, PDF/X) — scattered, no unified category · Document templates & asset libraries / shared styles ecosystem (CC-Libraries-style cross-document shared colors/type/components, library publishing & linking, updates propagation) · Clipboard / Interop / Copy-paste-as fidelity (copy as SVG/PDF/AICB/PNG, paste cross-app, paste-in-place semantics, drag-in/drag-out) — partly in Export/Save but no unified interop category · Update / Licensing / Distribution (auto-update channel, version/build, license/activation, EULA, telemetry consent, installer/portable, OSS license attributions — relevant for a free/OSS Windows app) · Touch / Pen / Tablet / Input-device support (Wacom/stylus pressure-tilt across tools, touch gestures, on-screen touch controls, Surface Pen, configurable input/shortcuts per device)

- **1. Tools:** Measure tool (ruler/measure — distance, angle, dimensions between two points; samples width/height/delta) — a classic Illustrator tool entirely absent · Reshape tool is listed but Anchor-point conversion via marquee + the 'Shaper' tool (draw rough shape → recognized as clean shape; Shaper gesture mode) is missing · Crop Image / Crop tool for placed raster (Illustrator's image-crop widget) · Color picker / 'Apply last-used color' and live fill-while-drawing · Selection: 'Select Behind' (Ctrl/right-click cycle) as a tool behavior · Node/Corner tool (Affinity) — dedicated corner-rounding tool distinct from Convert · Vector Crop / clipping via tool (Affinity Vector Crop) · Pen 'Rubber-band preview' toggle and Pen+Shift+click straight-segment chaining as explicit modifier note · Place-gun / loaded-cursor multi-place as a tool-level interaction
- **2. Panels (the full set):** Info panel (live X/Y, W/H, distance, angle, cursor RGB/CMYK readout) — distinct from Transform, a core panel that's missing · Assets panel (Affinity) — reusable saved graphics/components store (separate from Symbols/Libraries) · Stock / content panel (Adobe Stock, Affinity Stock) — placeable stock imagery · Snapshot panel (Affinity persistent histories/snapshots) · Scope / Histogram panels (Affinity — for raster/adjustment work) · CMYK/Separations soft-proof toggle panel · Notes / annotation panel · Version history panel (cloud/local snapshots) · Layer FX / Effects panel as its own (Affinity 'fx') distinct from Appearance · Constraints panel (Figma-style resizing constraints) · Component properties / variants panel (modern component systems)
- **3. Color system:** Color blending modes between fills (the per-fill blend already noted, but document-level isolate/knockout for color groups) · HDR / 32-bit color & unbounded values (Affinity) · Color sampler / multi-point persistent samplers (vs transient eyedropper) · Document swatches vs global/application swatches scope distinction · 'Add Selected/Used Colors to Swatches' batch command · Color contrast / WCAG checker (accessibility-driven, increasingly standard) · Average / Blend colors between objects (Edit > Edit Colors > Blend Front to Back / Horizontally / Vertically) · Invert / Saturate / Adjust Color Balance commands · Tint adjustment for placed images · Color theme extraction / Adobe-Color-style harmony generation from photo
- **4. Stroke system:** Dash phase / offset (start the dash pattern at an offset) — noted as advanced sub-bullet but not its own item · Stroke join 'corner' interplay with live corner radius (sharp vs rounded vs other corner styles per-corner) · Pattern-along-path / dashes that follow corners cleanly (already partial) plus 'fit dashes to length' · Stroke opacity & blend mode independent of object (noted under paint but deserves explicit item) · Variable-width on text strokes / brush on text · Pressure-curve editor & stabilization/smoothing for freehand strokes (input-side) · Non-scaling stroke flag (SVG vector-effect: non-scaling-stroke) for UI/icon export
- **5. Text / Type system:** Spell-check / grammar with live underline (noted under Find but deserves explicit live-spelling item) · Kashida / justification-elongation control (Arabic) as explicit item (only mentioned inside RTL bullet — this is the stated former moat) · Diacritics / harakat positioning controls (Arabic vowel marks) — Varos-relevant · Font activation / cloud-font & Google-font runtime loading (memory says this exists in old engine; missing here) · Font embedding & subsetting on save (separate from export) · Text styles import/sync from library (listed) + 'Redefine style from selection' · Noise/optical kerning preview & manual kern HUD · Number/figure spacing (tabular vs proportional) — listed under fractions but also a standalone need · Leading 'auto' percentage default setting · Text decoration color/style (underline/strikethrough weight, offset, color) as explicit · Bullet/number list — listed; but 'multi-level list' & list continuation across frames · Tate-chu-yoko & CJK composition (listed in vertical) plus mojikumi/burasagari (CJK spacing) — long tail · Glyph fallback chain / missing-glyph notdef handling
- **6. Arrange systems:** Align to pixel grid (snap bounds to whole pixels) as an arrange action · Distribute by ABSOLUTE spacing across artboard (not just between objects) · Tidy-up / auto-arrange into grid (Figma 'Tidy up') · Make same width/height (equalize dimensions across selection) — common modern command · Group into frame / wrap-in-container · 'Paste in place / paste at same position' as arrange-relevant · Boolean: 'Combine/XOR' as live non-destructive compound (Affinity keeps boolean live & re-editable — distinct from one-shot Pathfinder; the live-editability is a key Affinity advantage worth flagging) · Repeat/step-and-repeat grid & radial array as an arrange command (also a tool/effect) · Snap-to-key-object during distribute (exact-gap requires key object — noted, but the key-object UX as arrange concept)
- **7. Structure systems:** Frames / Boards as containers (Figma frame = clipping + auto-layout container; distinct from artboards and groups) — the modern board model · Layer/object opacity & blend at the LAYERS-panel level (per-layer, not just per-object) · Search/filter the layers tree by name/type (modern large-doc need) · Color-label / tag objects & layers (organizational labels beyond selection color) · Layer comps / view states (saved visibility+position snapshots) · 'Quick mask' / mask thumbnail editing UX · Nesting depth indicator / breadcrumb navigation for deep hierarchies · Smart-object / linked-component instance row representation · Export-flag per object (mark object as exportable asset from layers)
- **8. Artboard + Document system:** Frames vs Artboards distinction (Figma-style frames that clip + nest, separate from print artboards) · Infinite canvas vs fixed-page mode toggle · Artboard auto-grow / auto-size to content as live behavior (not just one-shot fit) · Background color / fill per artboard (distinct from transparency grid) · Artboard clipping toggle (clip content to artboard bounds, like Figma frame clip) · Document grid/layout-grid attached PER artboard (columns/rows overlay per frame) · Pasteboard/scratch area size limits & 'show only active artboard' · Pages vs artboards (multi-page document model for print/book) · Document-level default styles (default fill/stroke/text for new objects) · Spell-check language at document level · Document fonts list / font report (Document Info overlaps) · Recently-used artboard presets
- **9. Snapping / guides / grid / rulers:** Measurement/dimension annotation that persists (not just transient HUD) · Snap to glyph / text baseline & cap-height snapping (Illustrator 'Snap to Glyph') · Distance-to-edges + equal-spacing 'smart distribute' snapping (Figma red spacing) — partly noted, make explicit standalone · Snap rotation to other objects' angles · Guides from object center / margins automatically · Isometric / SSR (axonometric) guide presets (separate from full perspective grid) · 'Lock guides to artboard' (guides move with artboard) · Per-artboard layout grid (columns/rows/modular) as snap target · Baseline grid snapping (listed in Type but also a guides-system item) · Dynamic guides showing equal dimensions ('same size' match hints)
- **10. Canvas interaction & navigation:** Command palette / quick-action search (Ctrl+/ or Cmd+K) — modern must-have, missing entirely · Contextual right-click menu (noted missing) — also submenu richness (select-same, isolate, export selection, copy-as) · Quick-measure (hold Alt to see distance to other objects — Figma)  · Tab/Shift-Tab to cycle selection through objects · Enter to enter group / Esc to exit isolation (navigation semantics) · Zoom-to-fit-selection on key (noted) + zoom to pointer-region 'Z-drag' · Multi-touch rotate gesture (canvas) for tablets · Pen barrel-button / eraser-end mapping · Snap-suppression key during nav (Ctrl) — listed; also 'hold to measure' · Keyboard panning (no scrollbars) accessibility path · Live alignment to artboard center while dragging
- **11. Save / File System:** Cloud documents / sync & offline conflict resolution (relevant even for offline-first when collab added) · Open/import competitor formats explicitly: .ai, .pdf, .svg, .eps, .sketch, .fig, .afdesign, .cdr — as first-class import matrix · Incremental/differential save & save performance for large docs · Save WebP/AVIF metadata, font subsetting choices in native save · Backup rotation & 'save history with document' option · 'Open as untitled copy' / read-only open · Auto-save scope (to original vs sidecar) — listed; also encrypted/locked documents · Document recovery thumbnail + preview (listed) plus 'restore previous session tabs' · File association / OS shell integration (double-click .varos opens app, Quick Look thumbnail provider)
- **12. Export system:** ICO / favicon export (multi-size icon container) — common web need · Lottie / animated-vector / SVG-animation export (modern motion handoff) · PDF/X & print-ready PDF presets (overlaps print, but export-side) · Export with layers preserved to SVG (groups/ids/named layers) — partly noted; make explicit 'layered SVG' · CSS / code export (export styles as CSS, design tokens, or measurements for dev handoff — Figma 'Inspect/Dev Mode') · Export presets save/load & per-document remembered export settings · Trim/crop to content vs artboard toggle (listed) plus padding/margin on export · Honor pixel-grid / pixel-snapping on raster export for crisp icons · Background export (non-blocking) & export queue · Export naming tokens/templates (layer name, scale, format placeholders) · Sprite-sheet / texture-atlas export with JSON map (game/icon workflow)
- **13. Comments / Collaboration / Review:** Dev mode / handoff inspect (measurements, CSS, asset download for developers) — major Figma surface, missing · Branching / merge of design files (version branches) · Cursor chat / quick ephemeral messages · Voice/huddle or screen-follow (observation mode) — listed as follow; expand · Conflict resolution UI for offline-edited then synced docs · Public share / embed (read-only web view of a design) · Watermark / view-only protection on shared links · Comment on prototype/flow (not just static canvas)
- **14. Welcome / Home / New-Document:** Command palette accessible from home (search actions/files/help) · 'Continue where you left off' / restore last session · Cloud vs local document tabs on home · Plugin / extension discovery entry on home (relevant to Varos extensibility moat) · Keyboard-shortcut cheat-sheet / interactive shortcut trainer · Theme & UI-scale chooser surfaced at first run (listed in onboarding; make explicit) · Import-from-other-tool wizard (open .fig/.sketch/.ai) on home · Account/offline-mode toggle (listed) — plus license/activation state
- **15. Menu command map:** Edit ▸ Duplicate (Ctrl+D as duplicate-in-place in many tools, distinct from Transform Again) · Object ▸ Convert to Frame / Wrap in Frame (modern container) · Object ▸ Make/Edit Auto Layout (responsive container) · Type ▸ Spelling submenu (check, auto, dictionary) · View ▸ Show/Hide Comments & View ▸ Dev/Inspect Mode · View ▸ Lock/Unlock Guides at menu level · Window ▸ New Window (second view of same doc) & Window ▸ float/dock panels · Help ▸ Report a Bug / Send Feedback (relevant for OSS) · Help ▸ Plugins/Scripts manager entry · File ▸ Import (distinct entry from Place in some tools) & File ▸ Export Dev Resources · Object ▸ Collect for Export / mark-for-export · Edit ▸ Redo / Undo History (open history panel) · File ▸ Generate / AI-assisted commands (modern tools add AI menu — relevant given 'AI-native' product framing)
- **16. Preferences + Effects/Appearance:** Keyboard shortcut sets import/export (listed) + per-tool vs per-menu conflict resolver UI · UI language / localization of the app preference · Telemetry / privacy / data-collection consent settings · Auto-update channel & beta opt-in settings · Default new-object appearance & default styles preference · Tablet/stylus pressure & input-device calibration prefs · Reduced-motion / animation-disable accessibility pref · High-contrast / color-blind-safe UI theme pref · Scratch disk / memory / undo-depth (listed) plus cache-clear action · Effect: Round Corners vs live corner-radius widget distinction · Effect: Pattern-along-path / brush-as-effect · Effect: Drop-shadow on groups/text with knockout · Graphic Styles libraries (shared/exported) & cross-document sync · Appearance: 'New art has basic appearance' & default-fill-stroke pref (listed) — also expand-on-export automation

Scope: I treated the catalog as a gathering exercise and hunted the long tail of Illustrator + Affinity Designer + Figma (the latter matters because Varos's own framing per memory is "Figma + Illustrator + Affinity," with components/frames/auto-layout/dev-mode being Figma table-stakes the current catalog under-weights).

Biggest whole-category gaps (highest value to add):
1. EXTENSIBILITY / PLUGINS / SCRIPTING / single-schema API — this is explicitly Varos's stated MOAT ("free/open/EXTENSIBLE Illustrator," Blender-RNA single schema = file+AI+plugins+inspector) yet there is no category for it. This is the most important omission relative to the product's own thesis.
2. COMPONENTS / SYMBOLS / INSTANCES (master + overrides + variants + nested + swap + detach) — only a panel stub exists; it's the Figma backbone and an Illustrator/Affinity symbol system. Deserves a full category.
3. AUTO LAYOUT / CONSTRAINTS / RESPONSIVE and FRAMES-AS-CONTAINERS — entirely absent; core to modern (Figma) vector/UI work.
4. BRUSHES as a system, IMAGE/RASTER+IMAGE-TRACE pipeline, and the GENERATIVE/EFFECTS family (Blend, Envelope/Warp, Mesh, Live Paint, Pattern-making, 3D) — these appear only as scattered tool/panel rows, not coherent systems.
5. Cross-cutting concerns with NO home: Accessibility (a11y), App Internationalization/Localization (separate from RTL text content), Window/Workspace/multi-doc management, Status bar/Info readout, Command palette / quick-action search, Measurement/Annotation/Dimensioning, Print/Prepress, Data/Variables, Prototyping/Interaction, Dev-mode/handoff, Update/Licensing/Distribution, and Touch/Pen/Tablet input.

Within-category highlights worth not forgetting: the Measure tool (tool category), the Info panel (panel category), command palette (navigation), Kashida/harakat Arabic controls as explicit items (the former moat, currently buried inside one RTL bullet), live/re-editable booleans (Affinity's key advantage over one-shot Pathfinder), non-scaling-stroke flag and pixel-snapping on export (icon/UI workflows), CSS/design-token/dev-handoff export, and Dev Mode / Inspect for collaboration.

Note: no project files were relevant to read for this taxonomy task — it is a pure domain-completeness pass grounded in Illustrator/Affinity/Figma conventions plus the Varos product memory. The 'core/standard/advanced' importance tags and varosStatus for each new item are intentionally left for the catalog owner to apply per the task's existing tagging scheme, though many of the cross-cutting categories above are 'standard' in modern pro tools and 'missing' in Varos.

