> **Status:** reference — gap inventory (tools, paint, layers, application), researcher read of the code, 2026-10-09. Not a decision and not a work order. Varos = `main` @ `b1de865`. Companion to [EXTERNAL_PRIOR_ART_SURVEY](../EXTERNAL_PRIOR_ART_SURVEY.md) (licence rules, top-10 lifts) and [APPEARANCE_MASKS_STUDY](../APPEARANCE_MASKS_STUDY.md) (appearance, opacity, blend, masks — **not redone here**).

# GAP 2 — Tools, paint, layers and application vs Illustrator

**Path keys.** Varos (`varos/crates/`): `c/`=`varos-core/src/`, `a/`=`varos-app/src/`, `b/`=`varos-bridge/src/`, `r/`=`varos-render-wgpu/src/`, `p/`=`varos-pdf/src/`, `cli/`=`varos-cli/src/`.
Prior art: `VC/`=`reference/artcraft/vectorcraft/crates/` @ `a469568`; `VCe/`=`VC/engine/src/cmd/`; `PC/`=`reference/artcraft/photocraft/crates/` @ `4cb7cf3`. Numbers in parentheses = `wc -l`.
**Status words:** HAVE (usable in the app today; owner-seen or not is in STATUS) · PARTIAL (core/model or one path only) · MISSING (0 grep hits unless noted).
**Liftable:** COPY (file-level with attribution) · ADAPT (port the algorithm/data onto our model) · IDEA (shape/checklist only) · BUILD (nothing worth taking). **Size:** S ≤1 d · M 1–3 d · L 1–2 wk · XL >2 wk (one implementer, our tests included, no owner/review time).

**Facts that shape every row** (verified):
1. VC's own path model is close to ours: `VC/geom/src/path.rs:33` `Anchor{p,in,out,kind}` with absolute handles inside `SubPath`/`PathData` (`:185`, `:354`) vs our `c/model.rs` `Anchor{p,hin,hout,smooth}` — an adapter is small. Their *geometry* sits on `kurbo`; all their booleans/planar maps sit on **linesweeper** (`VC/pathops/Cargo.toml`, `VC/pathops/src/boolean.rs:1,11`). Varos uses `flo_curves` + `i_overlay` (`c/boolean.rs:1-14`). Anything planar = ADAPT-ALGORITHM onto i_overlay unless an ADR adds linesweeper.
2. VC tools are thin: a tool emits a command id + JSON (`VC/tools/src/draw2/family.rs:73-99`); the real work is in `VCe/*` (634 `cmd!(` + 117 `query` entries). Their UI (`VC/ui-egui`, eframe) and CPU renderer are not portable (survey fact 2–3).
3. Varos has 12 tools: `ToolKind` = Object, Direct, Pen, Rect, Ellipse, Triangle, Polygon, Convert, Eyedropper, Artboard, Rotate, Scale (`c/editor.rs:39-52`); 7 rail buttons + a 4-shape flyout (`a/ui.rs:213-231`); tool keys V A P M L R S I ⇧O (`a/main.rs:322-335`). Illustrator ships ~85 tools (`VC/tools/src/catalog.rs:20-140` is a faithful list).
4. Path appearance today: one `fill`, one `stroke` (`Paint = None | Solid`, `c/model.rs:136-140`), `stroke_width`, `holes`, `opacity` (`c/model.rs:193-215`). Groups carry no paint; `Xform` is rotation-only (`c/model.rs:46-51`).
5. Every new editor capability also needs an `EditCommand` (`c/command.rs:15-278`) and, to stay agent-native, a Bridge verb (`b/dto.rs:161`).

## A. Selection & transform

| Capability | Varos today | Illustrator | Prior-art source | Lift | Depends on | Size |
|---|---|---|---|---|---|---|
| Selection tool (V) | HAVE — `c/tools/object.rs`; bbox scale/rotate hit `c/editor.rs:817-856`; key `a/main.rs:323` | click/marquee, bbox scale/rotate | `VC/tools/src/select.rs` (822) | — | — | — |
| Direct Selection (A) | HAVE — `c/tools/direct.rs`; segment drag `Drag::Segment` `c/editor.rs:86`; Ctrl = Direct `:3779` | anchors/handles/segments | `VC/tools/src/direct.rs` (1057) | IDEA | — | — |
| Group Selection (climb groups per click) | MISSING — not in `ToolKind` | Direct+Alt; each click adds next group up | `VC/tools/src/lib.rs:570` (`DirectSelectionTool::new(true)`) | ADAPT | groups tree | S |
| Lasso (Q) | MISSING | freehand anchor/object select | `VC/tools/src/xform/wand.rs:49,67` (`point_in_polygon`, `LassoTool`) | ADAPT | — | S |
| Magic Wand (Y) | MISSING | select by fill/stroke/opacity tolerance | `VC/tools/src/xform/wand.rs:19`; `VCe/wand.rs:47` | ADAPT | Select Same matchers | M |
| Select ▸ Same (fill, stroke, weight, opacity, appearance…) | MISSING — no Select menu; Edit has only Select All/Deselect `a/chrome.rs:586-587` | Select ▸ Same ▸ 10+ attributes | `VCe/select.rs:59-145` (fill/stroke/weight/blend/appearance/style) | ADAPT (pure matching on our `Paint`) | — | S |
| Select ▸ Inverse / Next Above-Below / Reselect / All on Artboard | MISSING (Select All `c/editor.rs:4487` only) | Select menu | `VCe/select.rs:22-27` | ADAPT | — | S |
| Free Transform (E): shear / distort / perspective | MISSING — bbox handles scale+rotate only (`TfHit` `c/editor.rs:111-113`) | widget with free distort & perspective | `VC/tools/src/xform/free.rs:18,91,121` (461) | ADAPT shear; IDEA distort | distort needs Envelope (live node) | M |
| Rotate (R) / Scale (S) tools, pivot click, Alt-copy | HAVE — `c/tools/rotate.rs:1-31`; `a/main.rs:328-329` | tool + pivot + Alt | — | — | — | — |
| Rotate/Scale/Reflect/Shear **dialogs** (Alt-click, Enter) | MISSING | numeric dialog, Copy, preview | `VC/tools/src/xform/transform.rs:168` emits `Action::Dialog` | IDEA (our kit sheet) | K3 fields | S |
| Reflect tool (O) | PARTIAL — Flip H/V buttons `a/ui/panels/properties.rs:99-102` → `c/editor.rs:1704`; no tool (doc comment `c/tools/rotate.rs:1` claims "Reflect (O)" but no `ToolKind`, no key) | axis by click-drag | `VC/tools/src/xform/transform.rs:91` `reflect_matrix`; `VCe/object.rs:58` | ADAPT | — | S |
| Shear tool + dialog | MISSING (0 hits) | shear angle/axis | `VC/tools/src/xform/transform.rs:24` (`TransformKind::Shear`), `VCe/object.rs:67` | ADAPT | baked geometry, or `Xform`→affine (format bump) | S–M |
| Transform Each | MISSING | per-object scale/move/rotate/random | `VCe/menucmds.rs:74` (+ validation `:297`) | ADAPT | — | S |
| Transform Again (⌘D) | HAVE — `c/editor.rs:1992`; menu `a/chrome.rs:593` | ⌘D | — | — | — | — |
| Transform panel X/Y/W/H, angle, 9-pt reference, link | HAVE — `a/ui/panels/properties.rs:36-78`; `SetObjectBounds` `c/command.rs:46-60`. No shear field, no "scale strokes" option | + shear, scale strokes & effects | `VC/ui-egui/src/panels/transform.rs` | IDEA | stroke engine (scale strokes) | S |
| Apply live transform (Expand) | PARTIAL — `c/editor.rs:969` `expand_transform`, 0 callers in app/Bridge | Object ▸ Expand | — | BUILD (wire it) | — | S |
| Align 6 + to Selection/Artboard | HAVE — `c/editor.rs:183-213,1496`; `a/ui/panels/align.rs:33-55` | + Align to Key Object | `VCe/select.rs:40` (`select.key`) | ADAPT key object | — | S |
| Distribute objects (6 modes) | PARTIAL — H/V centres only `a/ui/panels/align.rs:58-64`, `c/editor.rs:1614` | left/centre/right/top/middle/bottom | `VCe/object.rs:167` | ADAPT | — | S |
| Distribute Spacing (fixed gap) | PARTIAL — `c/editor.rs:1662` exists, 0 callers in app/Bridge | spacing value + key object | `VCe/object.rs:176` | BUILD (wire) | key object | S |
| Arrange (front/forward/backward/back) | HAVE — `c/editor.rs:1381`; `a/chrome.rs:596-602`. Send to Current Layer MISSING | + Send to Current Layer | `VCe/object.rs:85-101` | ADAPT | — | S |
| Group / Ungroup | HAVE — `c/editor.rs:1915,1951`; `a/chrome.rs:605-606` | ⌘G / ⇧⌘G | — | — | — | — |
| Isolation mode | MISSING (0 hits) | double-click group; breadcrumb; others dimmed | `VCe/object.rs:146-147`; `VC/engine/src/lib.rs:86,121` (`isolation: Option<NodeId>`) | ADAPT | Layers tree; double-click today = Direct path select `c/editor.rs:4738-4741` | M |
| Lock / Hide selection (⌘2/⌘3), Unlock All, Show All, lock/hide others | PARTIAL — per-row toggles `c/editor.rs:4951-5018`, `a/ui/panels/layers.rs:386-420`; no menu items/shortcuts (`a/main.rs:270-351`) | Object ▸ Lock/Hide family | `VCe/object.rs:104-106`, `VCe/menucmds.rs:38-65` | ADAPT | — | S |
| Smart guides (alignment, geometry, spacing, ⌘U) | HAVE — `SnapGuide` `c/editor.rs:157-163`, `snap_move` `:2730`, `snap_xy` `:2845`; View toggles `a/chrome.rs:620-626` | + angle/construction guides, measurement labels | `VC/tools/src/guides.rs` (1254) | IDEA | — | — |

## B. Drawing tools and path operations

| Capability | Varos today | Illustrator | Prior-art source | Lift | Depends on | Size |
|---|---|---|---|---|---|---|
| Rectangle (M), Ellipse (L) | HAVE — `c/model.rs:1067-1103`; Triangle is a Varos extra | + Alt from centre, Shift square | `VC/geom/src/shapes.rs:14,72` | — | — | — |
| Rounded Rectangle | PARTIAL — Bridge only, radius baked into anchors `b/design.rs:330-338,590`; no tool/UI | tool + ↑↓ radius | `VC/geom/src/shapes.rs:20,51-67` | ADAPT | Live Corners for editability | S |
| Polygon (sides, ↑↓) | PARTIAL — fixed 6 sides `c/model.rs:1086` | sides 3–1000 | `VC/geom/src/shapes.rs:90` | COPY | — | S |
| Star | MISSING | points, r1/r2 | `VC/geom/src/shapes.rs:103,117` | COPY | — | S |
| Line Segment (\\) | MISSING (Pen open path only) | angle/length dialog | `VC/geom/src/shapes.rs:123` | COPY | — | S |
| Arc, Spiral, Rectangular Grid, Polar Grid | MISSING | four tools + dialogs | `VC/geom/src/shapes.rs:128-186`; tool `VC/tools/src/draw2/family.rs:73-99` | COPY math, tool ours | — | M |
| Flare | MISSING | lens flare object | `VC/tools/src/extra.rs:25` | IDEA (low value) | gradients | M |
| Live Corners widget (round/inverted/chamfer) | MISSING | corner widgets on Direct | `VC/geom/src/corners.rs:134,149` (283); `VC/tools/src/corners.rs` (616) | ADAPT | per-corner params stored or bake-on-edit decision | M |
| Pen (P): draw, close, continue, connect, Alt convert, contextual cursor | HAVE — `c/tools/pen.rs`; `PenHint` `c/editor.rs:56-63`; Alt→Convert `:3775-3786` | Pen | `VC/tools/src/pen.rs` (745) | IDEA | — | — |
| Add / Delete Anchor tools (+ / −) | PARTIAL — Pen on segment adds `c/editor.rs:3628`, on anchor deletes (P12) `:3547`; no own tools/keys | separate tools, + / − keys | `VC/tools/src/draw2/anchor.rs` (281) | ADAPT | — | S |
| Anchor Point / Convert (⇧C) | PARTIAL — `c/tools/convert.rs` reachable only via Alt in Pen (`c/editor.rs:3781`); not on rail (`a/ui.rs:213-221`), no key | own tool | `VC/tools/src/draw2/anchor.rs:39` | BUILD (expose) | — | S |
| Curvature (⇧~) | MISSING | through-points curves | `VC/tools/src/draw2/curvature.rs` (255) | ADAPT | kurbo adapter | M |
| Pencil (N), Smooth, Path Eraser, Join tool | MISSING | freehand + fidelity | `VC/tools/src/draw2/gesture.rs:43-49` (389); fitter `VC/pathops/src/fit.rs:30-97`; `edit.rs:137` smooth; `VCe/draw2.rs:112,121` | ADAPT | curve fitter | M–L |
| Scissors (C) | MISSING | cut at point | `VC/tools/src/draw2/anchor.rs:40` | ADAPT | — | S |
| Knife, Eraser (⇧E) | MISSING | cut/erase fills | `VC/tools/src/draw2/gesture.rs:45-46`; `VC/tools/src/cut.rs` (303); `VC/pathops/src/planar.rs:359` `cut_out` | ADAPT-ALGORITHM | i_overlay booleans | M |
| Shape Builder (⇧M) | MISSING | merge/delete regions by drag | `VC/pathops/src/planar.rs:321` (1136); `VC/tools/src/builder.rs:770` (1024) — linesweeper | ADAPT-ALGORITHM | planar faces on i_overlay | L |
| Live Paint + Bucket (K) | MISSING | live paint groups | `VC/pathops/src/planar.rs:292`; `builder.rs:771`; `VCe/buildcmds.rs:74-83` | IDEA → ADAPT | planar faces + live-object node | XL |
| Width tool (⇧W) | MISSING | variable width points | `VC/tools/src/distort/width.rs` (702); `VC/effects/src/stroke/width.rs` (367, polygon output); `VCe/distortcmds.rs:23-50` | ADAPT | stroke engine, outline stroke | L |
| Warp, Twirl, Pucker, Bloat, Scallop, Crystallize, Wrinkle | MISSING | liquify brushes | `VC/tools/src/distort/liquify.rs:25` (911) | ADAPT | resample/refit helpers | L |
| Blend tool (W) + Object ▸ Blend | MISSING | live blend, spine | `VC/doc/src/blend.rs:35-380` (1158); `VCe/live.rs:27-117` | ADAPT later | live-object node (ADR-0013 era) | L |
| Perspective Grid | MISSING | 1/2/3-point grid + attach | `VC/geom/src/projective.rs` (186); `VC/tools/src/distort/perspective.rs` (1513); `VCe/perspgrid.rs` | COPY homography; IDEA rest | — | XL |
| Pathfinder shape modes (Unite, Minus Front, Intersect, Exclude) | HAVE — `c/boolean.rs:20-30`; `c/editor.rs:1321`; panel `a/ui/panels/align.rs:87-95` | 4 shape modes (+Alt = live compound shape) | test oracles `VC/pathops/tests/prop_pathops.rs` (survey #1) | ADAPT tests | — | — |
| Pathfinder Divide, Trim, Merge, Crop, Outline, Minus Back | MISSING | 6 pathfinders | `VC/pathops/src/pathfinder.rs:29-54,87` (505) + planar | ADAPT-ALGORITHM (Minus Back = op swap) | planar faces on i_overlay | M–L |
| Compound path Make / Release (⌘8) | PARTIAL — `Path.holes` `c/model.rs:204`, only produced by booleans; no command | ⌘8 / ⌥⇧⌘8 | `VCe/object.rs:108-109` | ADAPT | even-odd per prim (have) | S |
| Offset Path | MISSING | delta, joins, miter | `VC/pathops/src/offset.rs:79` (kurbo + boolean union) | ADAPT | kurbo note, i_overlay cleanup | M |
| Simplify | MISSING | curve precision, corner angle | `VC/pathops/src/edit.rs:12-47` + `fit.rs` | ADAPT | curve fitter | S–M |
| Outline Stroke | MISSING | stroke → fill | `VC/pathops/src/offset.rs:45`; `VC/effects/src/stroke/outline.rs` (121) | ADAPT | stroke model (caps/joins/align) | M |
| Expand (fill/stroke/appearance) | PARTIAL — Expand Transform only (`c/editor.rs:969`, unwired) | Object ▸ Expand | `VCe/expand.rs:36` | ADAPT | outline stroke, appearance | M |
| Reverse path direction | PARTIAL — `c/editor.rs:3535`, used internally by Pen join | Attributes ▸ Reverse | `VCe/path.rs:53` | BUILD (wire) | — | S |
| Join (⌘J) / Average (⌥⌘J) | PARTIAL — Pen connect `c/editor.rs:3696`; no commands | Object ▸ Path | `VC/pathops/src/edit.rs:311-346`; `VCe/path.rs:62,71` | ADAPT | — | S |
| Add Anchor Points | MISSING | midpoint per segment | `VC/pathops/src/edit.rs:299` | COPY | — | S |
| Divide Objects Below | MISSING | cutter path | `VCe/pathops.rs:139` | ADAPT | booleans (have) | M |
| Clean Up (stray points, unpainted) | MISSING | Object ▸ Path ▸ Clean Up | `VCe/pathops.rs:157` | ADAPT | — | S |

## C. Paint

Opacity, blend modes, isolate/knockout, appearance stack and masks: **see [APPEARANCE_MASKS_STUDY](../APPEARANCE_MASKS_STUDY.md) §2 (model) and §6 (eight slices S1–S8)**; rows below only point there.

| Capability | Varos today | Illustrator | Prior-art source | Lift | Depends on | Size |
|---|---|---|---|---|---|---|
| Fill/stroke model (each = colour/gradient/pattern; multiple via Appearance) | PARTIAL — one fill + one stroke, `Paint = None\|Solid` `c/model.rs:136-140,198-201`; swap/default/none keys `a/main.rs:336-344` | Fill/Stroke + Appearance | `VC/doc/src/appearance.rs:346-376` `StrokeLayer` | IDEA (field list) | study §2 `StackItem` | L (study S2–S4) |
| Stroke weight | HAVE — `c/model.rs:201`; `a/ui/panels/properties.rs:133-139`; `SetStrokeWidth` `c/command.rs:65` | weight | — | — | — | — |
| Caps, joins, miter limit | MISSING — renderer round-only `r/tess.rs:131-149`; PDF hard-codes round `p/write.rs:211,395` | butt/round/square; miter/round/bevel | geometry `VC/effects/src/stroke.rs` (141); GPU-friendly polygon stroking `PC/vector/src/stroke.rs:101` | ADAPT | `StrokeStyle` (study S3) + format v5 | M |
| Dashes (+ align to corners) | MISSING | dash/gap ×3, preserve vs align | `VC/effects/src/stroke/dash.rs` (332); `PC/vector/src/stroke.rs:293` | ADAPT | caps/joins | M |
| Arrowheads | MISSING | arrowhead library, scale, tip/extend | `VC/effects/src/stroke/arrow.rs` (407) | COPY-WITH-ATTRIBUTION | caps/joins | M |
| Stroke align (centre/inside/outside) | MISSING | 3 aligns (closed paths) | `StrokeAlign` field `VC/doc/src/appearance.rs:357` | ADAPT | Offset Path or clip | M |
| Width profiles | MISSING | 7 profiles + custom | `VC/effects/src/stroke/width.rs` (367) | ADAPT | caps/joins, outline stroke | L |
| Linear / radial gradient + Gradient tool (G) on canvas | MISSING — picker tab disabled `a/ui/picker/panel.rs:59` | gradient panel + annotator | model `VC/color/src/gradient.rs:12,75` (794); annotator `VC/tools/src/xform/gradient.rs:68-231` (928); LUT WGSL `PC/gpu/src/compose.wgsl:632-680` | ADAPT | `Paint::Gradient` (study S2), WGSL, PDF shading | XL |
| Gradient on stroke (within/along/across) | MISSING | 3 modes | `VC/effects/src/stroke/gradient.rs` (432) | ADAPT later | gradients, outline stroke | M |
| Freeform gradient | MISSING | points / lines | `VC/color/src/freeform.rs` (575) | ADAPT later | gradients | L |
| Gradient mesh (U) | MISSING | Coons mesh | `VCe/live.rs:227-272`; `VC/doc/src/live.rs` | IDEA | live-object node | XL |
| Patterns (make/edit/tile, pattern fill) | MISSING | Pattern Options | `VCe/patterncmds.rs:27-83`; `VC/doc/src/pattern.rs` (751) | IDEA | tiled render pass, `Paint` variant | XL |
| Swatches (document, global, spot, groups, libraries, .ase/.gpl) | MISSING — `PanelId::Swatches` is a dummy, not dockable `a/shell/registry.rs:16,22-25`; picker drawer only Recent/Board/Document `a/ui/picker/drawer.rs:44-59` | Swatches panel | `VC/color/src/swatch.rs:9-45`; `palette_io.rs` (324); `libraries.rs` (534); `VCe/swatch.rs:16-54` | ADAPT model + I/O | `Paint::SwatchRef` (planned `c/model.rs:122-134`) | L |
| Colour panel / picker | HAVE — picker v3 `a/ui/picker/` (wheel, sliders, harmony, mini, drawer); STATUS 2026-10-09 | Color panel | — | — | — | — |
| Colour Guide (variation grid) | PARTIAL — Harmony tab `a/ui/picker/harmony_rules.rs` (61), app-side | Color Guide + variations | `VC/color/src/harmony.rs` (507) | ADAPT (move to core) | — | S–M |
| Recolor Artwork | MISSING | colour wheel remap, reduce | `VC/color/src/recolor.rs` (586); `VCe/recolor.rs:29` | ADAPT math | swatches/groups | L |
| Eyedropper (I) attribute pickup | PARTIAL — copies fill, stroke, weight `c/editor.rs:5310-5333`; no opacity/options/Shift sampling; Mac screen eyedropper pending (STATUS) | options tree, Shift = sample colour | `VC/tools/src/xform/eyedropper.rs:32` (263) | IDEA | — | S |
| Opacity / blend / transparency panel | PARTIAL — object opacity `c/model.rs:206`, UI `a/ui/panels/properties.rs:108-119`; blend modes MISSING | Transparency panel | → APPEARANCE_MASKS_STUDY §2, slice S7 | — | study | — |
| Live Paint Bucket | MISSING | see B Live Paint | — | — | — | — |

## D. Layers & structure

| Capability | Varos today | Illustrator | Prior-art source | Lift | Depends on | Size |
|---|---|---|---|---|---|---|
| Rows: eye · lock · thumbnail · name; artboard sections | HAVE — `a/ui/panels/layers.rs:11-49,420-423`; design of record LAYERS_VISION | eye/lock/target/colour | — | — | — | — |
| Reorder / nest (3-zone drag), Alt-drag duplicate, move to board | HAVE — `c/editor.rs:5030,5142,5219` | drag in panel | — | — | — | — |
| New Layer / New Sublayer | PARTIAL — model nests Layer in Layer `c/model.rs:1887`; only auto "Layer 1" (`c/model.rs:1611-1614`); no command (LAYERS_VISION:176 says `add_layer` exists — 0 hits) | footer buttons | `VCe/layer.rs:25-34` | ADAPT | — | S |
| Thumbnails | HAVE — `a/ui/panels/layers.rs:213-222,339` | thumbnails | — | — | — | — |
| Search + kind filter | HAVE — `a/ui/panels/layers.rs:443-455,487` | (none in AI; Figma-like) | — | — | — | — |
| Lock / hide with cascade | HAVE — `c/editor.rs:5011-5018`; cascade `a/ui/panels/layers.rs:194-238` | per row | — | — | — | — |
| Target circle, selection square, layer colour | MISSING by decision (LAYERS_VISION:18-40, §7.5 :299); `Node.color` stored `c/model.rs:327` | target ring, colour bar | `VCe/layer.rs:154` (`layer.target`) | IDEA | Appearance (study) | M |
| Paste Remembers Layers | MISSING — paste lands on active layer `c/clipboard.rs:151-152` | panel option | `VCe/layer.rs:163` | ADAPT | — | S |
| Release to Layers (sequence/build), Collect in New Layer, Merge, Flatten, Locate Object, Hide/Lock Others, Template | MISSING | Layers menu | `VCe/layerpanel.rs:19-131`; `VCe/layer.rs:88` | ADAPT | New Layer | M |
| Symbols (define, instance, library, break link, edit) | MISSING | Symbols panel | `VC/doc/src/lib.rs:505`; `VCe/brushsym.rs:89-163` | IDEA | instance node + format bump (ADR) | XL |
| Symbolism tools (Sprayer …) | MISSING | 8 tools | `VC/tools/src/symbolism.rs` (323); `VCe/brushsym.rs:165-174` | IDEA | Symbols | L |
| Brushes: calligraphic, scatter, art, pattern, bristle; Paintbrush (B), Blob Brush (⇧B) | MISSING | Brushes panel | `VC/brush/src/lib.rs:47-53` (+ `calli.rs`, `warp.rs`, `track.rs`; 1937 total). Storage in `Document::unknown` (`lib.rs:4-6`) — do not copy | ADAPT geometry | stroke engine, width profiles, pressure | XL |
| Graphic Styles | MISSING | styles panel + libraries | `VCe/style.rs:29-120`; `stylelib.rs:42-60` | IDEA | Appearance stack (study S4) | L |
| Isolation mode | see A | — | — | — | — | — |
| Clipping mask Make/Release (⌘7), edit contents | PARTIAL — model/render/PDF exist (`GroupRole` `c/model.rs:295-300`; `clip_group`/`release_clip` `:1360,1375`, test-only callers); no UI/command | ⌘7 / ⌥⌘7 | `VCe/object.rs:111-138` | → study §2/§6, MASKS_PLAN | ADR-0013 | M |
| Opacity masks | MISSING — `MaskAlpha/MaskLuma` reserved `c/model.rs:290-300` | Transparency ▸ Make Mask | `VCe/maskedit.rs`, `VCe/opacitymask.rs:31-124` | → study | GPU layer stack (study) | L |

## E. Application

| Capability | Varos today | Illustrator | Prior-art source | Lift | Depends on | Size |
|---|---|---|---|---|---|---|
| Preferences (units, keyboard increment, GPU, UI, history depth) | MISSING — `settings.json` holds only `recovery_enabled` `a/storage/settings.rs:16-20`; units are per document `c/units.rs:21-28`, `c/editor.rs:2433`; nudge fixed 1/10 pt `a/main.rs:321,347-350` | Preferences dialog | `VCe/prefscmds.rs:117-335` spec table (`keyboardIncrement` :119, `gpuPreference` :274, `historyStates` :275) | IDEA (one spec table drives UI + validation) | settings store v2 | M |
| Keyboard shortcuts editor | MISSING — hard-coded `a/main.rs:270-351`, menu accels `a/chrome.rs:510-633` | Edit ▸ Keyboard Shortcuts | `VC/ui-egui/src/shortcut_editor.rs` (931, eframe) | IDEA | command registry | L |
| Illustrator shortcut parity table | PARTIAL — standing rule (owner memory); only test: every menu key has a native equivalent `a/mac_menu.rs:313` | — | `VC/tools/src/catalog.rs:20-140` (tool keys); `shortcut` field of each `cmd!` | COPY as data/checklist | — | S |
| Panels / workspaces (box system) | HAVE — `a/shell/boxtree.rs` (1477), layout memory `a/storage/layout.rs`, Reset layout `a/chrome.rs:532`; named workspaces MISSING; 4 real panels (`a/shell/registry.rs:25`) | workspaces | `VC/ui-egui/src/workspaces.rs` | IDEA | — | M |
| Menus vs Illustrator | PARTIAL — Varos/File/Edit/Object/View/Window, ~40 items `a/chrome.rs:536-631`. Missing menus: Type, Select, Effect, Help. Missing items: File Place/Document Setup/Print; Edit Preferences/Keyboard Shortcuts/Edit Colors/Paste in Front-Back; Object Transform/Path/Pathfinder/Clipping/Compound/Lock/Hide/Expand/Blend/Envelope/Artboards | 9 menus, hundreds of items | `VCe/*` `menu` paths (634 commands) | IDEA (menu map) | each feature row | M (map) |
| Undo / Redo | HAVE — whole-document clone stacks `c/editor.rs:456-457`, `:3435-3443`; Bridge batch = one step `:3316` | unlimited (pref) | structural sharing `VC/engine/src/lib.rs:61-94` | IDEA (measure first) | — | — |
| History panel | MISSING — `PanelId::History` dummy `a/shell/registry.rs:17,22-25`; `history_preview` exists `c/editor.rs:3396` | History (Photoshop-like) | `VC/ui-egui/src/panels/history.rs` (76) | IDEA | A3-next per-AI history | M |
| Actions / batch | PARTIAL — CLI `apply` batch file = one undo `cli/main.rs:222`; Bridge `edit` batch; no recorder | Actions panel, batch | `VC/ui-egui/src/panels/actions.rs:1-3`; journal `VC/engine/src/lib.rs:1018-1038` | IDEA | command registry / journal | M |
| Document tabs | HAVE — `a/workspace.rs` (1116); DFS S1 | tabs | — | — | — | — |
| Window management | PARTIAL — native Minimize/Zoom/Fullscreen `a/chrome.rs:513-514,628`; window size persistence pending (STATUS open items) | — | — | BUILD | — | S |
| Help / About | PARTIAL — native About `a/mac_menu.rs:240,262`; no Help menu | Help menu, search | — | BUILD | — | S |
| Updates (ADR-0007: visible, user-controlled) | MISSING — policy only | in-app update | — | BUILD | signing/distribution | M |
| Localisation, Arabic UI (ADR-0012) | MISSING — gate not passed `docs/specs/UI_SYSTEM.md:358-370`; ADR-0012 proposed; T1 `varos-text` incl. `labels.rs` merged | 30+ languages | `VC/ui-egui/src/i18n/` (TSV catalogs, no Arabic) | IDEA (string catalog) | ADR-0012 T2/T3 | XL |
| Tablet pressure / tilt | MISSING (0 "pressure" hits in `a/`) | Wacom pressure | `PC/tablet/src/macos.rs:25-34`, `appkit.rs` (248), `lib.rs:40-59` | COPY-WITH-ATTRIBUTION | a consumer (brush/width) | S |
| Accessibility (VoiceOver) | MISSING — 0 `accesskit` hits; K5 heading only `docs/specs/UI_SYSTEM.md:127` | macOS a11y | none | BUILD | hand-painted widgets need roles | L |
| Performance budgets | PARTIAL — frame pacing `a/pacing.rs:41-73`; K4 protocol `UI_SYSTEM.md:100-110`; F7/F8 not started (PLAN) | — | parity-test pattern `PC/gpu/tests/parity.rs` | IDEA | — | M |
| Crash safety / no-panic | PARTIAL — panic hook + crash-log dialog `a/main.rs:841-850`; `catch_unwind` in IO worker `a/storage/io_worker.rs:57` and thumbs `a/thumbs/mod.rs:81`; no clippy deny ratchet; no GPU device-lost watch (0 hits in `r/lib.rs`) | — | `VC/engine/src/guard.rs` (46); `PC/gpu/src/health.rs` (202); VC workspace lints | COPY guard + health; IDEA lints | — | S–M |
| Bridge / MCP | HAVE (API 1.1) — 9 MCP tools `b/mcp.rs:216-223`; 18 edit operations `b/dto.rs:161` (add shape/path, resize, rotate, rename, delete, align, distribute, group, ungroup, order, move, set paint, 7 artboard verbs) | (none: ExtendScript/UXP) | VC MCP 25 tools incl. `list_commands`/`run_command` over all commands `VC/mcp/src/tools.rs:77-93`; `enabled`/`disabled_reason` `VCe/mod.rs:109-118` | IDEA (disabled_reason) — keep typed verbs (ADR-0009) | each new capability | per verb S |
| Web / WASM mirror | MISSING — A6 deferred (PLAN) | Illustrator on web | `VC/apps/vectorcraft-web/src/main.rs:1-15` (eframe web runner, WebGPU/WebGL2) | IDEA only (eframe) | Bridge proven, ADR-0001 | XL |
| CLI headless render/export | HAVE — `cli/main.rs:171-279` describe/snapshot/export-pdf/save-as/apply/new/diff; CPU `varos-raster`; SVG writer `c/svg.rs` not wired to CLI/app | — | `VC/apps/vectorcraft-cli` | IDEA | — | S (svg/png verbs) |

## 1. Fifteen most valuable missing/partial rows (value ÷ size)

| # | Row(s) | Why | Size |
|---|---|---|---|
| 1 | Select ▸ Same / Inverse / Next / All on Artboard | daily Illustrator muscle memory; pure matching on our model | S |
| 2 | Lock/Hide selection ⌘2/⌘3 + Unlock/Show All | core setters exist (`c/editor.rs:4951-4961`); menu + keys only | S |
| 3 | Distribute 6 modes + Distribute Spacing + key object | `distribute_spacing` already in core, unwired | S |
| 4 | Join ⌘J, Average, Reverse, Add Anchor Points | core helpers partly exist; VC `edit.rs` is small | S |
| 5 | Compound path Make/Release ⌘8 | `holes` model + even-odd render already there | S |
| 6 | Star, Line, Polygon sides, Rounded Rect tool | shape math is copyable (`VC/geom/src/shapes.rs`) | S |
| 7 | Paste Remembers Layers + New Layer/Sublayer + Send to Current Layer | layer tree exists; only commands missing | S |
| 8 | Shortcut parity table (data) + Anchor/Convert tool exposed | protects the standing Illustrator-keys rule cheaply | S |
| 9 | Panic guard at command boundary + GPU device-lost watch | crash safety; both COPY-sized | S |
| 10 | Reflect tool, Shear (baked), Transform Each, transform dialogs | completes the transform family | S–M |
| 11 | Stroke caps/joins/miter (model + GPU tess + PDF) | foundation of the whole stroke chain | M |
| 12 | Isolation mode | essential for editing groups; LAYERS_VISION compatible | M |
| 13 | Offset Path + Outline Stroke | most-used Object ▸ Path items; after #11 | M |
| 14 | Dashes + arrowheads | high visible value; after #11 | M |
| 15 | Simplify + Pencil/Smooth (curve fitter) | one fitter unlocks four tools | M |

Runners-up: Pathfinder Divide/Trim/Merge/Crop/Minus Back (M–L), Lasso (S), Live Corners (M), Preferences store with keyboard increment (M), History panel (M), Swatches (L).

## 2. Dependency sketch

```
kurbo ADR note + Anchor<->PathData adapter (VC/geom/src/path.rs:33 ~ c/model.rs Anchor)
 ├─ curve fitter (VC fit.rs) ─ Simplify ─ Pencil/Smooth/Path Eraser ─ Blob Brush ─ (Image Trace after raster object B4)
 ├─ Curvature, Live Corners, Offset Path
 └─ StrokeStyle model (study S3, format v5) ─ GPU caps/joins + PDF caps/joins
       ├─ Dashes ─ Arrowheads
       ├─ Outline Stroke ─ Expand ─ Stroke align
       └─ Width profiles ─ Width tool ─ Brushes (art/pattern/calligraphic) ◄─ tablet pressure (PC/tablet)
i_overlay planar faces (port of VC planar.rs ideas)
 ├─ Divide/Trim/Merge/Crop/Outline ─ Divide Objects Below, Knife, Eraser
 └─ Shape Builder ─ Live Paint (+ live-object node)
Paint::Gradient (study S2) ─ WGSL LUT + PDF shading ─ Gradient tool ─ gradient on stroke ─ freeform ─ mesh
Paint::SwatchRef + document swatch table ─ Swatches panel ─ global/spot ─ Recolor Artwork, Colour Guide grid
Appearance stack (study S4) ─ Graphic Styles ─ target circle/layer colour ─ Select Same ▸ Appearance
Live-object node ADR ─ Blend, Repeat, Envelope (Free Transform distort), Symbols (instance node) ─ Symbolism tools
Xform → affine (format bump) or baked ─ live Shear
New Layer command ─ Release to Layers, Collect, Paste Remembers Layers
Command registry (one id per behaviour + enabled/disabled_reason) ─ shortcut editor, menu map, Actions, Bridge parity
Preferences store v2 ─ keyboard increment, GPU preference, history depth
ADR-0012 T2/T3 ─ Arabic UI ─ localisation catalog
Raster object (B4) ─ Image Trace, image opacity masks, Place
Every core capability ─ EditCommand ─ Bridge verb (API 1.2+)
```

## 3. Summary

1. Varos covers the Illustrator **core loop** well: Selection/Direct/Pen/Convert, 5 shapes, Rotate/Scale, Align, 4 Pathfinder modes, Layers with sections/thumbnails/search, artboards, picker v3, smart guides, tabs, undo, Bridge 1.1, CLI.
2. Of ~85 Illustrator tools Varos has 12 `ToolKind`s; the missing families are freehand (Pencil/Smooth), cutting (Scissors/Knife/Eraser), construction (Shape Builder/Live Paint), distortion (Width/Warp/Free Transform), and blend/mesh/perspective/symbols.
3. The **cheapest wins** are command wiring on things the core already half-has: Select Same, Lock/Hide keys, Distribute Spacing, Join/Average/Reverse, compound paths, star/line/polygon sides, layer commands — all S.
4. The **stroke engine** (caps/joins/miter → dashes/arrows → outline/offset → width) is the single most load-bearing missing piece; it gates Width tool, brushes and Expand.
5. **Paint** beyond solid (gradients, patterns, swatches, recolor) waits on `Paint` variants defined by the appearance study; nothing here should land before ADR-0013.
6. VectorCraft is a strong source for **geometry and data**: shape math, path edit ops, curve fitting, dash/arrow/offset, gradient model, swatch/palette I/O, select-same matchers, a faithful Illustrator tool/shortcut list. Its booleans sit on linesweeper, so planar features port as algorithms onto i_overlay.
7. Nothing from VC/PC UI, eframe web app or CPU renderer is portable; PhotoCraft gives COPY-sized wins only for tablet pressure and GPU health.
8. Application gaps: no Preferences beyond recovery, no shortcut editor, no History panel, no Help, no accessibility, no update flow, Arabic UI gate open, menus cover ~40 items vs Illustrator's hundreds.
9. Crash safety is half-built (panic hook, IO `catch_unwind`); command-boundary rollback and GPU device-lost handling are COPY-sized.
10. Every new capability must also become an `EditCommand` and a Bridge verb to keep Varos agent-native.
