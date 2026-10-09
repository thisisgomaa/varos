> **Status:** reference — read-only study of external prior art (2026-10-09). Not a decision; feeds a future Appearance ADR.

# Appearance study A — VectorCraft (code, not READMEs)

Source: `reference/artcraft/vectorcraft` (MIT OR Apache-2.0). `VC/` = `vectorcraft/crates/`. Claims cite `file:line`.
Varos today: no Appearance system (`LAYERS_VISION.md`), `Paint = None | Solid`, SaveLayer render groups, core has no kurbo.

## 1. The Appearance model (`VC/doc/src/appearance.rs`, `node.rs`)

Stack = ordered `Vec` of fills and strokes, **paint order bottom→top** (`items[0]` painted first). Each entry carries its own
paint, opacity, blend, visibility and **its own effect list**; the object also has an object-level effect list.

```rust
// appearance.rs:306  — one live effect: untyped params, interpreted by vectorcraft-effects
pub struct Effect { pub id: String, pub params: serde_json::Value, pub visible: bool }
// appearance.rs:323
pub struct FillLayer { pub paint: Paint, pub opacity: f32, pub blend: BlendMode, pub visible: bool,
                       pub effects: Vec<Effect>, pub overprint: bool }
// appearance.rs:534  (serde tag "kind": "fill" | "stroke")
pub enum AppearanceItem { Fill(FillLayer), Stroke(StrokeLayer) }
// appearance.rs:608
pub struct Appearance { pub items: Vec<AppearanceItem>, pub effects: Vec<Effect>,
                        pub contents_index: Option<usize> } // groups/type: where members paint among items
```

`StrokeLayer` (`appearance.rs:346-389`): paint, width, cap, join, miter_limit, align (Center/Inside/Outside), dash
(`pattern, offset, align_corners`), start/end arrow (40 kinds) + scale + align, width `profile`, `brush`, opacity, blend,
visible, effects, overprint, `gradient_mode: Within|Along|Across` (`:394`).

Object-level transparency lives on `Node`, not in `Appearance` (`node.rs:440-466`): `opacity`, `blend`, `isolate`,
`knockout` (tri-state), `knockout_shape`, `mask: Option<Box<OpacityMask>>` (`OpacityMask{art: Arc<Node>, clip, invert,
disabled, linked}`, `node.rs:502`). `graphic_style: Option<u32>` links to a `GraphicStyle{appearance, opacity, blend,
isolate, knockout}` (`doc/src/lib.rs:429`) — editing breaks the link.

| Question | Answer (cite) |
|---|---|
| Multiple fills/strokes? | Yes, any number, any interleaving (`appearance.rs:610`). `Appearance::basic` = 1 fill under 1 stroke (`:622`). |
| Per-entry opacity/blend/visible? | Yes on both kinds (`:323-337`, `:373-379`). |
| Where do effects attach? | **Both**: per entry (`FillLayer.effects`, `StrokeLayer.effects`) and per object (`Appearance.effects`). Per-entry effects see only that entry; object effects run first and feed every entry (`render/src/fx.rs:348-364`, `:628-655`). |
| Group appearance? | Yes. Groups/layers carry `items` + `effects`; `contents_index` puts the members between items (`appearance.rs:963-1010`, insert/move keep the slot). |
| How composed? | `effects::evaluate_container` (`VC/effects/src/group.rs:239-279`) rewrites the group into plain art: Pathfinder effects replace children, geometry effects reshape **every leaf with the whole group's bounds as reference box**, each own fill/stroke becomes one art node painting the union of member shapes, spliced below/above members per `contents_index`. Raster effects stay on the group → one combined shadow of the composite. |

## 2. Live effects (`VC/effects/src/lib.rs`, `engine/src/cmd/effectcmd.rs`)

Registry = `effect_catalog()` (`lib.rs:161`) → `EffectInfo{id, label, menu, params: doc string, defaults: json, raster,
lengths}` (`:70`), indexed once in a `OnceLock` (`:148`). **No parameter structs**: params are `serde_json::Value`
merged over dialog defaults (`merged_params`, `:388`) and read with `num(p,"key",default)`. Only raster effects get a typed
enum, `RasterFx` (`raster.rs:13-42`), produced per frame by `raster_effects()` (`raster.rs:84`).

| Id | Params (defaults) | Kind |
|---|---|---|
| `convertToShape.rectangle/roundedRectangle/ellipse` | relative, extraW/H 18, width/height, radius 9 | vector |
| `distort.freeDistort` | corners[4] unit box | vector |
| `distort.puckerBloat` | amount % | vector |
| `distort.roughen` | size, relative, detail/in, points smooth/corner, seed | vector (seeded hash noise) |
| `distort.transform` | scaleH/V, moveH/V, rotate, copies, reflectX/Y | vector |
| `distort.tweak` / `distort.twist` / `distort.zigZag` | h,v,anchors,in,out,seed / angle / size,ridges,points | vector |
| `path.offsetPath` | offset, joins, miterLimit → `pathops::offset_path` | vector |
| `path.outlineStroke` | width? (uses the stroke it sits on, `GeomContext`, `lib.rs:452`) | vector |
| `stylize.roundCorners` / `stylize.scribble` | radius / angle, overlap, strokeWidth, curviness, spacing, variation, seed (simplified) | vector |
| `warp.*` ×15 (arc…twist) | bend, horizontal, vertical, orientation | vector (`warp_point`, `doc/src/live.rs:322`) |
| `pathfinder.*` ×10 | none; groups/layers only (`group.rs:21`) | vector, container |
| `adjust.*` ×6 (brightnessContrast, curves, hueSaturation, levels, shiftToColor, temperatureTint) | see `lib.rs` doc strings | recolour (paints + images) |
| `stylize.dropShadow` / `outerGlow` / `innerGlow` | mode, opacity, x, y, blur, color / +source edge|center | **raster** |
| `stylize.feather` / `blur.gaussian` | radius | **raster** |
| `cropMarks` | style | adds art |
| `plugin.<id>` | manifest | vector, via **wasmi** WASM plug-ins (`lib.rs:511-514`) |

**Evaluation order.** Not a graph — a sequential stack, but split by class:
- Geometry: strict stack order, each effect's reference box = bounds of its *input* (first uses object bounds)
  (`apply_geometry_with`, `lib.rs:488-505`). Object effects → then each item's own effects on that result.
- Raster: **stack order is ignored**. The renderer always paints shadows/outer glows below, then the content with all
  Feather + Gaussian radii **summed linearly** into nested blur layers, then inner glows above (`render/src/fx.rs:484-582`;
  sum at `:508`). A shadow is the silhouette of the *un-blurred* content regardless of order. Simple, but not Illustrator.
- Colour adjustments recolour the art before drawing (`fx.rs:456`).

**Cache / invalidation.** No dirty flags: caches key on `Arc<Node>` pointer identity (structural sharing → an unchanged
node keeps its address) — geometry/bounds (`render/src/lib.rs:264-271`, `:638`), evaluated object-fx art `fx_arts`
(`fx.rs:409-448`, cleared wholesale at 1024), shadows/glows as pre-blurred tinted rasters keyed by (node ptr, slot, ink)
and checked by **deep `Node` equality + view linear part**, so pans reuse them (`fx.rs:190-345`). Gap: a plain path with
geometry effects re-runs `effected_path` every frame (`fx.rs:108`, `:349`) — not cached.

**Expand Appearance** (`effects/src/bake.rs:221-253`, `effectcmd.rs:437`): geometry effects applied; each fill → a path
copy with that fill; each stroke → outlined art; entry opacity/blend hoisted; group keeps id, transparency, mask, raster fx. Raster effects become images (`engine/src/cmd/rasterfx.rs:1-9`). Exporters call the same
machinery as `bake_document` (`bake.rs:330`) — renderer and exporters share one evaluation.

## 3. Rendering (`VC/render`, CPU `vello_cpu` 0.2)

Flow: Arc tree → `draw_arc` (cull, mask) → `draw_arc_body` (fast path/fx/text/live) → `draw_layer_node` (transparency
group) → `draw_content` → `draw_shape` → per-item paint → vello → pixmap. **No render-item list**; tree walked per frame.

- **Fast path** (`lib.rs:776-801`): plain path, no fx, ≤1 painted item → opacity folded into paint alpha; no layer.
- **Per item** (`lib.rs:1015-1062`): iterate `appearance.items` in order; an item with opacity<1 or non-Normal blend gets
  its own `push_layer(blend, opacity)`. Stroke: outline cached per (layer ptr, width, tol) (`:1204`); Inside = clip layer;
  **Outside = draw full stroke then DestOut the interior** inside a layer (`:1108-1115`).
- **Groups** (`render/src/group.rs:17-120`): `Composite{clip, blend, opacity, mask, isolated, blends, bounds}`. Isolated
  vello layer normally; a **non-isolated group whose children blend** is drawn offscreen over a copied backdrop and mixed
  back (DestOut + Plus) — real non-isolated blending on a CPU rasteriser. Knockout = per element DestOut then Plus
  against the group backdrop (`lib.rs:984-995`).
- **Opacity mask**: mask art rendered offscreen, luminance → `vello_cpu::Mask` (`lib.rs:751-770`, `mask_value :1516`).
- **Raster fx**: vello filter layers (`vello_common::filter_effects`), which only work single-threaded, so on the MT path
  each effect is rendered offscreen in a crop of its reach and composited back (`with_filters`, `fx.rs:587-624`).
  Shadows also have a pure-CPU 3×box-blur cache (`fx.rs:203-268`). σ = blur/2 everywhere (`raster.rs:10`).

## 4. Export (`VC/svg/src/export.rs`, `VC/pdf/src/export.rs`)

Both start from `bake_document` (`svg export.rs:26`, `pdf export.rs:43`): geometry effects, Pathfinder, container
appearance are already plain paths. Multi-fill/stroke → one element per item with `opacity` + `mix-blend-mode`
(`svg export.rs:950-1050`); groups `isolation:isolate` (`:1147`); masks → `<mask>`; knockout → nested masks (`:1161`).

| Feature | SVG | PDF |
|---|---|---|
| Shadows/glows/blur/feather | `<filter>` chains (`:1490-1550`), σ = blur/2 | engine pre-renders each at **document raster ppi, default 72** (`doc/src/lib.rs:690`) — under-vector image for shadows, replacement image for blur/feather/inner glow (`engine/cmd/rasterfx.rs:438-452`). Calling `vectorcraft_pdf` directly just warns and drops them (`pdf export.rs:704-708`) |
| Linear/radial + focal + aspect | `gradientTransform`, `fx/fy`, midpoints as extra stops tagged `data-vc-midpoint` (`:741-778`) | krilla shadings, two-point radial for focal (`:783`), midpoints as stops |
| Gradient along/across stroke | slices of linear gradients, warned (`:1059`) | same, warned (`:1226`) |
| Freeform gradient | image clipped to the shape, warned (`:1406`); **on type → a linear gradient** (`:746`) | image at raster ppi (`:1124-1140`); **in stroke slices → the plain average of the gradient's *stops*** (not of the freeform points), warned (`:801-812`) |
| Gradient mesh | expanded to many solid quads, **no warning** | same, **no warning** (`:999-1001`); no type 6/7 shading |
| Drop-shadow/glow **blend mode** | **silently dropped** — `RasterFx::DropShadow { .., }` ignores `mode` (`svg export.rs:1518`) | kept (image composited) |
| Mixed spot+process gradient | — | process colours, warned (`:684`) |

Also: screen interpolates CMYK stops in display RGB (`color/src/lib.rs:235` `lerp`) while PDF may keep CMYK.

## 5. Gradient (`VC/color/src/gradient.rs`, `freeform.rs`, tool `VC/tools/src/xform/gradient.rs`)

```rust
// gradient.rs:35            // gradient.rs:262                       // gradient.rs:445
pub struct GradientStop {    pub struct GradientGeom {                pub struct GradientPaint {
  offset: f32, color: Color,   start: Point, end: Point,                gradient: Gradient,      // {kind, stops}
  opacity: f32,                aspect: f64,          // radial h/w      geom: Option<GradientGeom>, // None = fit each render
  midpoint: f32, // .13–.87    focal: Option<Point>, // inside ellipse  angle: f64, swatch: Option<String>,
  swatch, tint }             }                                          freeform: Option<Freeform> }
```
`GradientKind = Linear | Radial | Freeform` (`:12`). Midpoint = piecewise-linear remap (`sample_with`, `:94-117`), exported
exactly as one extra stop (`expanded`, `:124`). Stop editing helpers are pure functions returning new stop vectors
(`insert/remove/move/duplicate/swap/set_midpoint`, `:185-258`). `GradientGeom::transform` (`:304-347`): linear keeps
isolines (normal-of-mapped-isolines trick), radial maps its ellipse via an eigen-decomposition → new end + aspect;
`rebase` (`:350`) for box moves; `param_at` solves the focal quadratic (`:360-381`); `set_focal` clamps to 0.99 of the
ellipse (`:407`). Unplaced (`geom: None`) gradients fit the bounds each render and get pinned on first transform
(`Appearance::pin_gradients`, `appearance.rs:858`).

Freeform (`freeform.rs:1-9`): colour points + Catmull-Rom lines, inverse-distance field with per-point spread discs;
rendered as a cached grid image sized from on-screen size (8–256 cells, power-of-two steps, `render/src/freeform.rs`).
Stroke: Within = page gradient through the stroke; Along/Across = rib polygons each with a short linear gradient, grown
1 px to hide seams (`effects/src/stroke/gradient.rs:274-299`). Render: peniko linear/radial/two-point radial (`paint.rs:19-44`).

Annotator (`tools/xform/gradient.rs:1-23`): bar start-ring → end-square, stop chips under the bar, midpoint diamonds,
radial dashed ellipse with aspect dot, focal dot inside the centre ring; drag past the end rotates; Shift constrains,
otherwise snaps. The tool is **UI-free**: it emits `Action::Begin/Preview/Commit` with `paint.setGradientGeom` /
stop commands (`tools/src/lib.rs:134-151`), so a whole drag = one undo step.

| Solid (worth adapting) | Weak |
|---|---|
| Stop model with per-stop opacity + midpoint + swatch link; exact midpoint export | **Pad only** — no reflect/repeat spread anywhere (no `Extend` in color/render) |
| `GradientGeom` transform maths (linear isolines, radial ellipse eigen, focal clamp) | Interpolation always display-RGB lerp; no colour-space/hue option; CMYK stops differ screen vs PDF |
| Fit-on-render → pin-on-transform lifecycle | Freeform on CPU grid image; PDF/SVG freeform = images; on type/slices degraded (§4) |
| Annotator gestures incl. aspect/focal, pure tool → command stream | Mesh = solid quads only (no smooth shading) |
| Stroke Along/Across slicing with seam control | No dithering (banding) on CPU path |

## 6. Undo / history (`VC/engine/src/lib.rs`)

Snapshot undo over a persistent tree: `HistoryEntry{label, doc: Arc<Document>, selection}` (`:61`); nodes hold
`Vec<Arc<Node>>` (`node.rs:438`). `Session::edit` (`:1099-1147`) clones the `Arc`, `Arc::make_mut` path-copies only the
edited spine (`Document::node_mut`, `doc/src/lib.rs:804-811`), pushes the *before* snapshot. No command inverses, no diffs;
a separate `journal` logs commands for recording/replay. Drags: `begin_interaction` snapshots, `preview(cmd)` re-runs the
command on the snapshot each move, `commit_interaction` keeps the last preview as **one** step (`:1176-1230`).
`begin_undo_group`/`end_undo_group` merge scrubbed-field edits (`:1258`, `push_undo :218`). The Effect dialog uses
exactly this: `begin_interaction` + `preview("effect.setParams", …)` on every slider change, OK = commit, Cancel =
restore (`ui-egui/src/dialogs/effect.rs:142-144`, `:338`).

"Effect edit = one undo step" for Varos: same shape — open a transaction on the effect dialog, re-apply
`set_effect_params(node, entry, index, params)` to the pre-dialog document on each change (pure, idempotent), commit once.
Arc-identity caches mean untouched siblings keep their cached shadows; only the edited spine misses.

## 7. Dependencies (licences per crates.io; moxcms/vello/kurbo/peniko/usvg verified in local registry)

| Area | Crates dragged in | Licence | egui/eframe? |
|---|---|---|---|
| Model (`doc`, `color`, `geom`) | kurbo 0.13, serde, serde_json, thiserror, **moxcms** (color CMS), image+tiff+zune-jpeg (doc) | MIT/Apache; moxcms BSD-3-Clause OR Apache-2.0 | no |
| Effects | + `vectorcraft-pathops` (**linesweeper**), `vectorcraft-text` (skrifa, harfrust, subsetter, unicode-*), `vectorcraft-plugins` (**wasmi**) | MIT/Apache | no |
| Render | vello_cpu 0.2, vello_common 0.2, image, flate2, jpeg-encoder, gif, weezl | MIT/Apache | no |
| SVG / PDF | usvg, svgtypes / krilla, hayro-*, aes/md-5/sha2 | MIT/Apache | no |
| Tools | geom/color/doc/pathops/text + serde_json | MIT/Apache | no |
| UI | `ui-egui`, apps (egui 0.36, eframe, egui_kittest) | MIT/Apache | **yes — unusable for us** |

Only `ui-egui` and `apps/*` touch egui. All else is pure data/geometry except `render` (CPU vello) and `pdf` (krilla).

## 8. Liftable vs avoid (licence rules: `EXTERNAL_PRIOR_ART.md` §License — NOTICE + licence copies on first borrow)

**Cleanly liftable (pure, kurbo/serde only)**
| What | Path (VC/) | Lines | Note |
|---|---|---|---|
| Appearance types (Effect, FillLayer, StrokeLayer, AppearanceItem, Appearance, contents slot) | `doc/src/appearance.rs:306-1010` | ~700 of 1,277 | ADAPT: keep our serde shape (ADR-0004); type the params |
| Gradient model + geom maths + stop helpers | `color/src/gradient.rs` | 794 (21 tests) | ADAPT; add spread + interpolation space |
| Freeform field | `color/src/freeform.rs` | 575 | later; render as a baked texture in WGSL |
| Raster effect descriptors | `effects/src/raster.rs` | 126 | ADAPT to typed params |
| Geometry effects: distort / warp / stylize / util | `effects/src/{distort,warp,stylize,util}.rs` + `doc/src/live.rs:278-400` | 232+34+161+103+~120 | ADAPT to our path type (theirs: anchor `p/in/out`, `geom/src/path.rs:33-356`) |
| Stroke gradient slicing | `effects/src/stroke/gradient.rs` | 432 | later, after stroke engine |
| Container evaluation (group fills/strokes/effects → art) | `effects/src/group.rs:142-279` | ~140 | IDEA / ADAPT |
| Expand Appearance / bake | `effects/src/bake.rs` | 397 | ADAPT: same evaluator for render, export, expand |
| Gradient annotator gesture logic | `tools/src/xform/gradient.rs` | 928 (≈585 code) | IDEA: hit-test + drag maths; our UI paints it |

**Avoid / do differently**
- `serde_json::Value` params — no schema, silent defaults on typos. Varos should use typed per-effect structs (ADR-0004).
- Raster stack-order collapse and linear blur summing (`fx.rs:484-582`) — evaluate raster effects in true stack order.
- Uncached effected geometry per frame (`fx.rs:108`); deep `Node ==` as shadow cache check (`fx.rs:294`).
- Effects crate pulling text + wasmi plug-ins into core; keep plug-ins out of `varos-core`. Skip vello/CPU-blur workarounds.
- Silent export losses (§4): every degradation must warn (our honesty law); no "average colour" fallbacks.
- 72-ppi default for rasterised effects in PDF — pick a print-sane default or ask.

## Summary (10 lines)
1. Appearance = ordered fills/strokes (paint order), each with opacity/blend/visible and its own effect list, plus object effects.
2. Groups carry their own fills/strokes/effects; `contents_index` places members among them; evaluated to plain art each frame.
3. Object opacity/blend/isolate/knockout/opacity-mask live on `Node`, not in the stack; graphic styles are linked Appearances.
4. ~50 effects; params are untyped JSON merged over defaults; geometry effects run in strict order, raster ones in a fixed order.
5. Render = tree walk on CPU vello with Arc-pointer caches; real non-isolated blending, knockout and luminance masks.
6. Exporters share the renderer's evaluation via `bake_document`; SVG uses filters, PDF rasterises effects at 72 ppi.
7. Quiet losses: SVG shadow blend mode, mesh → quads, freeform → images / stop-average, CMYK stops lerped in RGB.
8. Gradient model and transform maths are strong and liftable; missing spread modes, colour-space choice, smooth mesh.
9. Undo = Arc snapshots + interaction preview/commit; an effect dialog is already exactly one undo step.
10. No egui outside `ui-egui`/apps; lift model + geometry pieces (~3.5k lines), rebuild rendering in our WGSL.
