> **Status:** reference study (read-only inventory, 2026-10-09). Not a design, not a work order. Line numbers are `main` @ `3b83dbb`.
# Appearance study C — what in Varos an appearance stack, masks and gradients would touch

Owner rule: «نكمل بنضافة… مناخدش حاجة ممكن تبوّظ البرنامج». This is a **risk inventory**: every site that assumes
"one fill + one stroke + one opacity", every compatibility promise that must survive, and every renderer gap.
Paths are relative to `varos/crates/` unless they start with `docs/`.

## 0. Where the new data would sit (fit, not design)

- **Path** (`varos-core/src/model.rs:191-212`): `fill: Paint`, `stroke: Paint`, `stroke_width`, `opacity`, `holes`, `hidden`, `locked`, `name`,
  under `#[serde(deny_unknown_fields)]`. An `appearance: Vec<Entry>` would sit here, `#[serde(default, skip_serializing_if = "Vec::is_empty")]`,
  with today's four fields kept as the **base entry**. Then a file that uses no extra entries stays byte-identical, and Bridge 1.0 `fill`/`stroke` keep their meaning.
- **Group/Layer** (`Node`, `model.rs:305-354`): has **no opacity and no paint today**. "Group opacity" is really per-path (`Editor::set_opacity`
  writes each selected path, `editor.rs:4925-4937`). A group appearance is a **new concept**: a group-level isolated layer that does not exist in the scene or the GPU.
- **Gradient**: `Paint` (`model.rs:121-187`) was built to grow (`COLOR_SPEC.md:37,63-78`). Its serde is hand-written: `null` = None, `[r,g,b,a]` = Solid.
  A gradient becomes a tagged object (a new `visit_map` arm). Adding a `Vec` payload drops `Copy` (`model.rs:133-135`). The compiler then flags every implicit copy, which is safe but noisy.
- **Masks**: `GroupRole::{MaskAlpha, MaskLuma}` already exist (`model.rs:289-301`) but the validator **refuses** them (`format/validate.rs:69`).
  The locked law is one stored form: a clip is a property of a **group** with an authoritative `mask_child` (`docs/LAYERS_VISION.md:97-110,138-142`).
  So "a mask on one object from its row" must **desugar into a clip/mask group**. It must never become a `mask` field on `Path`.

## (a) One-fill-one-stroke assumption sites

Cost: S = local edit · M = several functions or a new UI state · L = new architecture or format/GPU work.
| file:line | What it assumes today | Cost |
|---|---|---|
| `varos-core/src/model.rs:198-206` | Exactly one `fill`, one `stroke`, one `stroke_width`, one `opacity` per path | M (additive field + base-entry rule) |
| `varos-core/src/model.rs:133-187` | `Paint` is `Copy`; serde knows only null/array | M (visit_map arm; Copy loss) |
| `varos-core/src/scene.rs:29-41` | `Prim::Fill/Stroke` carry one `color: Rgba`; no paint/shader payload | L (gradient prim or paint index) |
| `varos-core/src/scene.rs:423-520` | `fill_prims`/`stroke_prims` read `p.fill.solid()`/`p.stroke.solid()`; a non-solid paint is **silently not drawn** | M |
| `varos-core/src/scene.rs:555-617` | `emit_object`: treatment (Isolated / Knockout / folded / opaque) is decided from one fill + one stroke + `opacity` | L (N entries ⇒ an isolated unit per object; knockout rule per stroke) |
| `varos-core/src/scene.rs:54-58` | `Group::Isolated` exists per **object** only; no group-level isolated layer; `Clip` holds hard rings only | L |
| `varos-core/src/scene.rs:128-135` | `scene_signature` hashes only `.solid()` + width + opacity of live paths | S (a live gradient/entry edit would leave a **stale canvas**) |
| `varos-core/src/scene.rs:236-237,401` | View-cull pad = `stroke_width/2`. Wider extra strokes, shadow offsets and blur radius are not counted | M (art cut off at the screen edge while panning) |
| `varos-core/src/editor.rs:345-353` | `painted_half_width`: hit reach from the one stroke, only if solid | S |
| `varos-core/src/editor.rs:598,814` | Click/marquee "inside" only if `fill.solid().is_some()` (a gradient-filled shape can't be clicked inside) | S |
| `varos-core/src/editor.rs:1338,1366` | Pathfinder result takes the bottom path's **solid** fill/stroke via `Path::new`, which also resets opacity and name | S/M (gradients and appearance would be lost silently) |
| `varos-core/src/editor.rs:439-440,498-499,3349,4751,4884-4905` | Current paint = `cur_fill/cur_stroke: Option<Rgba>`; apply/swap/default work on two solids | M |
| `varos-core/src/editor.rs:4790-4803` | `paint_live(PaintTarget, Option<Rgba>)` writes one slot. `PaintTarget` = {Fill, Stroke} (`:31-36`) | M |
| `varos-core/src/editor.rs:4843-4850` | `document_colors` gathers solids only | S |
| `varos-core/src/editor.rs:5301-5319` | Eyedropper copies solid fill/stroke/width (drops gradients) | S |
| `varos-core/src/model.rs:1131-1138` | Duplicate keeps `fill/stroke` "EXACTLY (future gradients too)". The rest goes through `Path::new` | S (already safe for Paint; new fields need listing) |
| `varos-core/src/format/validate.rs:96-108` | Validates one width, one opacity, solid colours only | S (+ stop/entry checks) |
| `varos-core/src/format/limits.rs:11-47` | No limit for entries per path, gradient stops or effects (ADR-0008 rule 6 requires declared limits) | S |
| `varos-core/src/svg.rs:171-176,323-354` | SVG: one `<path fill stroke>` + `<g opacity>`; knockout via `<mask>`; solids only | M (`<linearGradient>`, several `<path>` per object) |
| `varos-pdf/src/write.rs:129-153` | `drawable`: `(fill, stroke) = solid()`; a non-solid paint means "not drawable", so the object **vanishes from the PDF** | M |
| `varos-pdf/src/write.rs:376-440` | `paint`: one B* with pooled `/ca /CA`, or one knockout XObject `/I /K` | L (shading, multi-op, SMask) |
| `varos-render-wgpu/src/lib.rs:16-25` | Vertex = `pos + color`; fragment returns vertex colour. No UVs, no paint buffer | L |
| `varos-render-wgpu/src/tess.rs:429-438,505-509` | `Draw::*` and `needs_knockout` assume ≤1 fill and one stroke colour per object | M/L |
| `varos-raster/src/lib.rs:166-190,246-263` | CPU snapshot paints solid `Prim` colours (tiny-skia shaders exist but are unused) | M |
| `varos-core/src/bridge.rs:458-461` | describe element: `"fill":p.fill`, `"stroke":{"paint","width"}`, detail `"geometry": json!(p)` (the whole Path) | M (see b) |
| `varos-core/src/bridge.rs:696-712` | `TargetEdit::Paint {fill, stroke, stroke_width, opacity}` | M (1.2 additive) |
| `varos-bridge/src/service.rs:1136-1146,1166-1168` | `color()`: any non-array paint becomes `null`, so a gradient reads to agents as **"no fill"** | S (but a contract decision) |
| `varos-bridge/src/mcp.rs:38,55,198`; `dto.rs:130-160`; `economy.rs:6` | Paint schema = `#RRGGBBAA` or null; `edit.defaults` keys fixed | M |
| `varos-app/src/ui/snap.rs:44-58,86,115-116` | UI selection snapshot: `fill/stroke: Option<Rgba>` + mixed flags | M |
| `varos-app/src/ui/panels/properties.rs:125-126` | Two `paint_row`s (Fill, Stroke) | M (the Section-home for appearance, ONE-HOME rule) |
| `varos-app/src/ui/control_bar.rs:116-117,161-162,201-231,282-348` | Fill/stroke chip pair; overlapping-squares hit test | S (mirror only) |
| `varos-app/src/ui/picker/mod.rs:16,380`; `picker/cluster.rs:104-255` | `MTarget::Paint(PaintTarget)`; Gradient tab disabled "coming with the gradient engine" | M |
| `varos-app/src/ui/panels/layers.rs:19-26,113-126,339-372,697-728` | Thumbnail `ThumbShape{fill,stroke: Option<Rgba>}`; `thumb_key` hashes solids only (stale thumbs); one 18×18 thumb per row | S/M |

## (b) Persistence and Bridge compatibility rules we must keep

**`.vrs` format (ADR-0004, ADR-0008, `docs/reference/VRS_FORMAT.md`)**
1. The model **is** the schema: Serde types in `varos-core` (ADR-0004 §Decision). Current writer = **format 4** (`format/mod.rs:33`), migrations v1→v4 (`format/migrate.rs:15`).
2. **Bump rule:** any key, variant, type or default the writer can emit raises the number (ADR-0008:34). This includes *emitting* the already-declared `MaskAlpha/MaskLuma`.
   Serde defaults let new builds read old files. They never excuse skipping a bump (ADR-0008:58).
3. Each bump ships before its writer: a named pure migration, frozen old/new fixtures, a rejection fixture, and VRS_FORMAT.md §5/§6c/§12/§13.
   Precedent: v4 (`fixtures/v4/`, `fixtures/refused/v5_future.vrs`, `future_v5_pdf.vrs`). The `v5_future` fixtures move to **v6** when v5 ships.
4. `deny_unknown_fields` on Path, Node, Document and the envelope (`model.rs:14,45,192,251,307,361,446,528,535`; `format/mod.rs:48`).
   An **older build refuses the whole newer file**, even if no object uses the new feature (ADR-0008 Consequences ¶1). No downgrade-save.
5. **v5 is already promised twice.** ADR-0010 text needs v5 (ADR-0008:131-133). The picker work order says gradients need "format v5" (`COLOR_PICKER_V3.md:34`).
   One number per writer change, so these must be **sequenced**, never both "v5". PLAN puts gradients **after text** (`docs/PLAN.md:50`).
6. Symmetric save: save runs the same structure/validate/size checks, so this build never writes what it would refuse (ADR-0008 rule 6, `format/mod.rs:9-11`).
   New limits (entries, stops) must be declared, and PDF object growth counts against `max_pdf_objects` 100 000 (`limits.rs:40`).
7. **Never persist caches.** Undo is whole-`Document` clones, 200 deep (`editor.rs:3418-3430`), and recovery snapshots serialize the model.
   Raster-effect results must live in the renderer, never in `Document`.
8. `content_eq` destructures `Document` exhaustively (`model.rs:668-690`). A new Document key is a compile error until classified; new Path keys are compared automatically.
   This is a good guard; keep it.

**Bridge (ADR-0009 :216-218, :425-432)**
1. API **1.0, omitted `api`, and 1.1 defaults stay byte-identical**; frozen request/result fixtures (`varos-bridge/tests/fixtures/*.json`, 9 files) are never edited.
   New fields/verbs = a new **minor** (1.2), opt-in via explicit `api:"1.2"`. Unknown request fields are rejected; clients tolerate additive response fields.
2. Byte traps found:
   - detail `geometry: json!(p)` (`bridge.rs:461`): any new Path key appears unless `skip_serializing_if` empty.
   - Group `geometry_digest` = digest of `json!(node)` (`service.rs:1199-1206`): a new Node key changes every group digest.
   - `settings_digest` = `json!(doc)` minus a fixed strip-list (`service.rs:1175-1190`): a new Document key (e.g. a gradient/swatch library) changes it on every board.
3. **Meaning trap:** 1.0 `fill`/`stroke` = hex or `null`, and null means "none".
   A gradient or a multi-entry object must not describe as `null` (`service.rs:1136`). The 1.0 answer needs a declared rule (e.g. base entry only, plus an additive `appearance`/`paint_kind` field).
   `set_paint` on a gradient object under 1.0 must keep a defined meaning: replace the base entry, never wipe hidden entries silently.
4. `snapshot` is the **CPU** renderer (`service.rs:49-55` → `varos-raster`), not the GPU canvas. Every new paint and effect needs CPU and GPU parity, or the agent sees a different picture than Ahmed.

## (c) Renderer capabilities (`varos-render-wgpu`)

| Capability | State | Evidence |
|---|---|---|
| Stencil-then-cover even-odd fills; bits 0x01 parity, 0x80 band, 0x02 clip | ✅ | `lib.rs:337-407` |
| MSAA 8× if the device allows, else 4×; non-sRGB surface; Mailbox | ✅ | `lib.rs:287-300` |
| Offscreen scene texture + blit; cached scene (signature hit = blit only) | ✅ | `lib.rs:54-56,1134-1137` |
| **One** reusable isolated layer (`layer_msaa`/`layer_view`) + premultiplied composite | ✅ single level | `lib.rs:58-63,505-521,870-917` |
| Hard clip mask (stencil 0x02), single level | ✅ render; gestures **not built** | `lib.rs:839-868`; `docs/MASKS_PLAN.md:6-15` |
| Nested layers / layer stack / texture pool | ❌ one layer texture, no nesting | `lib.rs:58-63`; `tess.rs:656-664` |
| Opacity of an Isolated object **inside a clip** | ❌ **dropped (renders opaque)**, known gap | `tess.rs:656-664`; `docs/foundation/P11_2_PERF.md:286-289` |
| Translucent strokes / knockout **inside a clip** | ❌ ignore the clip bit | `lib.rs:743-780`; `P11_2_PERF.md:290-292` |
| Second (mask) texture + two-texture composite (alpha/luma masks) | ❌ planned only | `docs/MASKS_PLAN.md:195-208` |
| Gradient paint (UV/paint buffer, shader, bind group) | ❌ shader is colour-only | `lib.rs:16-25` |
| Filters (blur/shadow): ping-pong targets, compute/fragment kernels | ❌ none | — |
| Texture budget accounting | ❌ none. Each full-frame MSAA target = W×H×4×samples bytes (≈95 MB at 4×, ≈190 MB at 8× on a 3024×1964 window); today ≈3 such + 2 resolves | `lib.rs:178-195,616-641` |

How a gradient plugs in: the cover step (`pipe_cover*`) would need a second pipeline family that samples a paint (stops in a uniform/storage buffer, or a 1-D ramp texture).
That means one per stencil state: cover, cover_clip, cover_knock, band ≈ 4–6 new pipelines, the same explosion MASKS_PLAN §3.1 warned about.
Strokes (`Draw::Fg`) need UVs or world-pos in the vertex, so `VATTRS` grows and every vertex buffer is touched.
The CPU raster (`varos-raster/src/lib.rs:166-190`) is *ahead*: it already nests Isolated/Clip recursively with tiny-skia `Mask`, so it would get gradients and alpha masks cheaply. Expect GPU and CPU to diverge first.

**Export (PDF/SVG).** The PDF has pooled ExtGState alpha, knockout Form XObjects (`/I /K`, `write.rs:294-331`) and hard clip runs (`q … W* n … Q`, `write.rs:229-262`).
It has **no** shading/pattern, **no** SMask, **no** image XObjects, and no nested transparency group for group opacity. Each is new writer code.
Raster effects (blur) cannot stay vector in the PDF. They need an image writer plus a "what prints" decision.
The PDF object budget (`write.rs:43-46` pooling, `limits.rs:40`) is a real ceiling for per-object shadings/SMasks.
SVG (`svg.rs:323-354`) would grow `<linearGradient>`/`<mask>`. Its group form already matches the canvas.

## (d) RISK TABLE

| # | Risk | What breaks | Guard |
|---|---|---|---|
| 1 | Writer emits a new key/variant without a bump | Older Varos opens a "damaged" file; ADR-0008 broken | Bump + migration + fixtures + refusal fixture *before* the writer; `format_v2.rs`/`old_reader_harness.rs` gates |
| 2 | v5 claimed by both text (ADR-0010) and gradients | Two incompatible "v5" files in the wild | One owner decision on order; rename `v5_future`→`v6_future` at the bump |
| 3 | Gradient read through `.solid()` = None at ~20 sites | Shape invisible on canvas, in PDF, SVG and thumbnail; unclickable; Bridge says "no fill" | Ship gradient paint end-to-end in one gated piece (model+scene+GPU+CPU+PDF+SVG+hit+bridge), or keep it unwritable (picker tab disabled) until then |
| 4 | Undo = 200 whole-doc clones | RAM growth if effect caches or rasters enter `Document` | Caches only in renderer; a test that `Document` holds no raster bytes |
| 5 | One offscreen layer, no nesting | Group appearance, effects inside clips, alpha masks inside groups render wrong (already: opacity lost inside clip) | Fix P11_2 limits 1–2 first; then a layer-stack/texture-pool with a depth cap; hand-test in the real window |
| 6 | Texture memory | Each nesting level ≈ +95–190 MB on Retina; OOM or swap on 8 GB Macs | Budget + depth cap + fallback (flatten deeper levels); measure, don't infer |
| 7 | Per-frame effect evaluation | Pan/zoom = signature miss every frame, so blur recomputes each frame; heat on an idle-sensitive Mac (`pacing.rs:1-12`) | Per-object effect cache keyed like `flatten.rs`; idle stays `ControlFlow::Wait`; effects never request frames |
| 8 | `scene_signature` / `thumb_key` hash solids only | Stale canvas or Layers thumbnail during live gradient/entry edits | Hash the full paint/appearance (`scene.rs:128`, `layers.rs:120`) |
| 9 | View-cull pad ignores effect extents | Shadows/blur/extra strokes chopped at the screen edge while panning | Cull pad = painted extent (largest stroke + effect radius) |
| 10 | Soft-mask role reached by code that checks only `== GroupRole::Clip` | Clipboard keeps a soft mask whose mask was not copied (`clipboard.rs:91`), so save is refused; `paint_role` paints the mask shape as art (`model.rs:783-816`) | Route every role check through one `is_mask_group()`; extend `validate.rs:69` only with the bump |
| 11 | Second mask stored form (e.g. `Path.mask`) | Breaks LAYERS_VISION §3.4 "one stored form forever"; two topologies | Row gesture desugars into a mask group with `mask_child` |
| 12 | Mask thumbnail beside the row thumbnail | Collides with the planned "drop onto THUMBNAIL = clip" target (`LAYERS_VISION.md:118-120`) and the 26 px row (`layers.rs:502`) | Owner mockup first; one drop target; fx badge right-aligned in the name column |
| 13 | Bridge describe/digest drift | Frozen 1.0/1.1 fixtures fail; agents' since-diffs flag every group | `skip_serializing_if` on every new key; 1.2 opt-in fields; parity tests in `contracts.rs` |
| 14 | CPU snapshot ≠ GPU canvas | Agent "sees" a different design than Ahmed | Shared scene `Group`/`Prim` contract; paired CPU/GPU fixture per new paint/effect |
| 15 | Pathfinder / eyedropper / apply-current drop non-solid paint | Silent paint loss on boolean ops | Preserve `Paint`/appearance by value (pattern at `model.rs:1132`) + tests |
| 16 | New UI (Appearance home, fx badge) | Ratchets fail: raw colours ≤31, FontId ≤16, `.size(` ≤18, CornerRadius ≤20, ui.rs ≤843 lines (`shell/ratchet_tests.rs:65-89`) | Tokens in `shell/tokens.rs` only; new panel module, not ui.rs |
| 17 | New panel id in the saved layout | Older build fails to parse the layout and falls back to the default (`shell/boxtree.rs:166-167`) | Acceptable, but say so; no data loss |
| 18 | PDF object/byte growth | Large docs hit `max_pdf_objects`; save refused | Pool shadings/SMasks per page like knockout ExtGStates; `save_budget.rs` |

## Tests that pin today's behaviour (counted `#[test]`, main @ 3b83dbb)

Workspace total ≈ 1 463 `#[test]` markers: core 416 · app 710 · bridge 120 · pdf 94 · render 41 · text 40 · cli 26 · raster 16.
**≈ 453 directly pin paint/opacity/clip/format/describe**:
- core 185: `tests/format_v2.rs` 28, `clipboard.rs` 32, `view_cull.rs` 16, `hit_thick_stroke.rs` 16, `svg_export.rs` 14, `format_validate.rs` 13, `edit_command.rs` 13, `boolean_corners.rs` 10, `masks.rs` 8, `occlusion.rs` 8, `opacity.rs` 6, `golden.rs` 5, `serde_roundtrip.rs` 5, `vrs.rs` 5, `colors.rs` 4, `history_lifecycle.rs` 2
- pdf 60: `export_pdf.rs` 31, `container.rs` 9, `frozen_v2.rs` 8, `old_reader_harness.rs` 7, `golden_pdf.rs` 2, `open_fill.rs` 2, `export_svg.rs` 1
- render 41: `tess.rs` 30, `tess_round3_tests.rs` 9, `lib.rs` 2
- raster 16
- bridge 100: `contracts.rs` 74, `connection.rs` 14, `artboards.rs` 12
- app picker 51 (`ui/picker/tests.rs`)

Frozen bytes: `varos-core/tests/fixtures/{v1..v4,refused,svg}` and `varos-bridge/tests/fixtures/*.json`.
**New tests go:** model/serde/migration → `varos-core/tests/format_v2.rs` + new `fixtures/v5|v6/` + `refused/`;
scene routing (entries → Groups) → a new `varos-core/tests/appearance.rs` beside `masks.rs`/`opacity.rs`;
GPU routing, CPU-only → `varos-render-wgpu/src/tess.rs` tests (no `Renderer`, no `EventLoop`, per CLAUDE.md);
CPU pixels → `varos-raster`; PDF → `varos-pdf/tests/export_pdf.rs` + `save_budget.rs`; Bridge 1.2 → `varos-bridge/tests/contracts.rs` (new fixtures only).
The real-window look stays Ahmed's hand test.

## (e) Summary

1. Path has exactly one fill, one stroke, one width and one opacity. ~30 sites across core, GPU, CPU, PDF, SVG, Bridge and UI read them through `.solid()`.
2. Today a non-solid paint is **silently dropped** at almost every site: invisible, unclickable, missing from the PDF, and "no fill" to agents.
3. `Paint` was designed for gradients (hand-written serde, enum). The cheapest safe step is model plus export, but it is only safe end-to-end, never half-wired.
4. Groups have no opacity or paint at all. Group appearance is new architecture, not a field.
5. The GPU has ONE offscreen layer and no nesting. Opacity inside a clip is already lost and translucent strokes ignore clips (P11.2 known limits). Fix these first.
6. Alpha/luma masks are declared (`GroupRole`), but the validator refuses them. Enabling them is a format bump plus a second-texture composite that does not exist yet.
7. A mask "on an object from its row" must desugar into the one stored form (a mask group with `mask_child`), never a per-path field.
8. Every writer-visible change = a format bump with migration and fixtures. v5 is already promised to text and to gradients, so the owner must sequence them.
9. Bridge 1.0/1.1 must stay byte-identical. Watch `json!(p)` in detail, group digests and `settings_digest`. New shapes ship as opt-in API 1.2.
10. Biggest silent dangers: per-frame effect cost (heat), texture memory per nesting level, stale signature/thumb hashes, and CPU-snapshot vs GPU drift.
