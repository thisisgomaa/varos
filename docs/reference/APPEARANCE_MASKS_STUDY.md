> **Status:** reference — synthesis study, 2026-10-09. Not a decision and not a work order: it feeds a future **ADR-0013 "Appearance & masks"**. Nothing below lands before that ADR is accepted (owner: «نكمل بنضافة… مناخدش حاجة ممكن تبوّظ البرنامج»). Varos line numbers are `main` @ `bd71d60`.

# Appearance, effects and masks — synthesis of studies A, B, C

Inputs: [study C — Varos fit/risks](study/APPEARANCE_STUDY_C_varos_fit.md) · [study A — VectorCraft](study/APPEARANCE_STUDY_A_vectorcraft.md) · [study B — PhotoCraft](study/APPEARANCE_STUDY_B_photocraft.md) · [EXTERNAL_PRIOR_ART](EXTERNAL_PRIOR_ART.md) · [LAYERS_VISION §3](../LAYERS_VISION.md) · ADR-0004 · ADR-0008 · ADR-0009 · ADR-0010 · [COLOR_PICKER_V3 §3](../foundation/work_orders/COLOR_PICKER_V3.md).
Paths: `varos-*/…` = `varos/crates/…`; `VC/` = VectorCraft `crates/` @ `a469568`; `PC/` = PhotoCraft `crates/` @ `4cb7cf3`.

## 1. Verdict

**The owner's idea is sound, and it is the right call for a vector editor.** Photoshop users already think "every Layers row can carry a mask and effects, whether it is one thing or a folder". PhotoCraft proves the model in code: one row struct for leaf and group, with mask, clip, effects, blend and opacity as row fields (`PC/doc/src/lib.rs:431-472`). Illustrator spreads the same powers across three places (Appearance panel, Transparency panel, clip groups). Varos can offer them on one row, on vector, keeping the vector editability that Photoshop loses. The cost is real but bounded. Today a path has exactly one fill, one stroke and one opacity, read through `.solid()` at ~30 sites (study C §a). Groups have no paint or opacity at all (`model.rs:305-354`). The GPU has **one** offscreen layer and no nesting (`varos-render-wgpu/src/lib.rs:58-63`). Two clip bugs are already live (P11_2_PERF.md:286-292). The format sits at v4, and "v5" is already promised twice. So the price is roughly: **a format bump per writer change, a GPU layer stack, new PDF writer code (shadings, soft masks, images), Bridge API 1.2, and one new UI home**. That means eight gated slices (§6). No slice may break a v4 file, and a plain object must stay byte-for-byte what it is today.

## 2. Proposed model (for the ADR — not final)

**Idea in one line:** every Layers row shows one **Appearance**, a name taken from study B. It has the row's ordered paint entries, the row's own effects, its opacity/isolate (blend later), and its mask. Storage reuses what exists. Nothing is stored twice.

```rust
// varos-core (pure). Names are proposals. Every new key: #[serde(default, skip_serializing_if = "<is default>")].
pub enum Paint { None, Solid(Rgba), Gradient(Box<Gradient>) }      // S2. Box keeps Paint small; drops `Copy`
pub struct Gradient { kind: GradKind /*Linear|Radial*/, stops: Vec<Stop>, geom: Option<GradGeom>, spread: Spread }
pub struct Stop { offset: f32, color: Rgba, midpoint: f32 }        // per-stop alpha lives in `color`
pub struct EntryOpts { opacity: f32, visible: bool, effects: Vec<Effect> }       // + blend from S7 only
pub enum StackItem {                                               // paint order: [0] is painted first
    Base(BaseSlot /*Fill|Stroke*/, EntryOpts),                     // marker: paint stays in Path.fill/.stroke
    Fill { paint: Paint, opts: EntryOpts },                        // extra fills (S4)
    Stroke { paint: Paint, style: StrokeStyle, opts: EntryOpts },  // extra strokes (S4)
}
pub struct StrokeStyle { width: f32, cap: Cap, join: Join, miter: f32, dash: Vec<f32>, align: Align } // S3
pub enum Effect {                                                  // TYPED params, units = points (never JSON blobs)
    Offset { d: f32, join: Join }, ZigZag { size: f32, ridges: u16, smooth: bool },
    Transform { sx: f32, sy: f32, dx: f32, dy: f32, rot: f32, copies: u16 },
    Warp { style: WarpStyle, bend: f32, h: f32, v: f32 },          // vector effects: S5
    Shadow { dx: f32, dy: f32, blur: f32, color: Rgba }, Blur { radius: f32 },   // raster effects: S7
}
pub struct Look { opacity: f32, isolate: bool, effects: Vec<Effect> }            // + blend from S7 only
// Path (leaf row):   fill, stroke, stroke_width, opacity            ← UNCHANGED, the v4 bytes
//                    stack: Vec<StackItem>    (empty ⇒ implicit [Base(Fill), Base(Stroke)] = today)
//                    effects: Vec<Effect>     (object effects; leaf opacity stays in Path.opacity)
// Node (group/layer): look: Option<Box<Look>> (None ⇒ pass-through = today; refused on a Path leaf node)
// Mask: NO new field. Group role ∈ {Clip, MaskAlpha, MaskLuma} + authoritative mask_child (LAYERS_VISION §3.4)
```

**Why this storage shape.**
- **One source of truth.** Today's four Path fields stay the *base* entries. `StackItem::Base` only places them in the stack and carries their options.
  So Bridge 1.0's `fill`/`stroke` keep meaning "the base fill/stroke" (study C §b-Bridge-3). A non-empty `stack` must contain exactly one `Base(Fill)` and one `Base(Stroke)` (validator rule).
- **Leaf vs group.** Leaf opacity already lives on `Path.opacity` (`model.rs:203-204`), so a leaf row's look is read from Path. A group row's look lives on `Node.look`.
  The UI shows one Appearance for both. The validator refuses `look` on a `NodeKind::Path` leaf, so no value has two homes.
- **Masks.** "Add mask" on a row (leaf or group) **desugars** into a mask group: `role = MaskAlpha` (or `Clip`), `mask_child` = the mask shape, and the row's content becomes the other child.
  That is the one stored form (LAYERS_VISION §3.1/§3.4). There is never a `mask` field on `Path`. This is the opposite of VectorCraft's `Node.mask` (`VC/doc/src/node.rs:440-502`) and PhotoCraft's sibling `clipped: bool` (study B "Avoid" 2).

**Composition order — a single object** (adapts VectorCraft's per-item loop `VC/render/src/lib.rs:1015-1062`; true stack order, unlike its raster shortcut):
1. Object vector effects run in stack order and reshape the geometry. Each effect's reference box is the bounds of its input.
2. Stack items are painted bottom→top. For each one, its own vector effects run first. Then it paints its paint (solid/gradient) at the entry opacity: even-odd fill, or a stroke with `StrokeStyle`.
3. **Isolation**, only when needed. Today's `emit_object` rule (`scene.rs:555-617`) is extended: more than one visible entry with opacity < 1, or any raster effect, or object opacity < 1 over overlapping entries. In those cases the entries render into an isolated buffer. Otherwise the folded-alpha fast path runs, as today.
4. Raster effects (S7) run on that buffer in stack order. A shadow composites *below* its input.
5. The result composites onto the backdrop at `Path.opacity` (blend from S7).

**Composition order — a group** (adapts PhotoCraft `PC/compose/src/lib.rs:680-705,1049-1186`):
1. With `look = None` the group is a **pass-through**: exactly today, no buffer, zero cost.
2. Otherwise its children are composed (each by its own order) into an **isolated buffer**, one layer-stack level.
3. If the group is a mask group, `mask_child`'s silhouette (Clip) or alpha (MaskAlpha) multiplies the buffer.
4. The group's effects run on the masked buffer. Masks therefore shape effects, as in PhotoCraft. A child's own shadow runs at step 2 and is cut by the mask.
5. The group's opacity (and blend later) composites the buffer onto the backdrop.

So "masked thing + shadow" has two legal answers: put the shadow on the mask-group row (it follows the mask) or on the child (the mask cuts it). The Layers row must make clear which row the user is editing (decision D7).

**What stays exactly as today.** A plain object has an empty `stack`, empty `effects`, and no `look` on any node. It behaves as Appearance = [Base(Fill), Base(Stroke)] with default options.
- It emits the same `Prim`s, takes the same `emit_object` treatment and writes the same JSON. Every new key is `skip_serializing_if` default.
- **Provable:** S1 is in-memory only and adds no writer key, so there is no bump. Every file in `varos-core/tests/fixtures/v4/` must load → save → produce **identical bytes**, and the scene `Prim` list for each golden must be equal before and after.
- After a later bump, a v4 file migrates through an **identity step on `doc`**. The proof is a test that the `doc` subtree bytes are equal and only the `varos` stamp moved.
- The Bridge 1.0/1.1 frozen fixtures (`varos-bridge/tests/fixtures/*.json`) stay untouched throughout.

## 3. Lift vs adapt vs build

Licence: VectorCraft and PhotoCraft are both **MIT OR Apache-2.0** (study B corrects the earlier "Apache-only"). Both are usable in GPL-3.0 Varos under EXTERNAL_PRIOR_ART §License:
- the first borrow creates `varos/NOTICE` (it does not exist yet) plus their `LICENSE-MIT`/`LICENSE-APACHE` copies;
- every borrowed block carries `// Adapted from <repo> <path>@<commit> (MIT OR Apache-2.0)`;
- everything borrowed gets our headless tests before it lands.

| What | Source (path : lines) | Mode | Lands in |
|---|---|---|---|
| Appearance stack shape (items, per-entry opacity/visible/effects, object effects) | `VC/doc/src/appearance.rs:306-1010` (of 1 277) | ADAPT — our serde shape (ADR-0004), `Base` markers, typed effects | `varos-core` model |
| Gradient model, stop helpers, midpoint remap, geom transform maths (linear isolines, radial ellipse, focal clamp), fit→pin lifecycle | `VC/color/src/gradient.rs` (794, 21 tests) | ADAPT — add `spread` (they have Pad only) and an interpolation-space field | `varos-core` paint |
| Vector effects: distort / warp / stylize / util + `warp_point` | `VC/effects/src/{distort,warp,stylize,util}.rs`, `VC/doc/src/live.rs:278-400` | ADAPT to our `Anchor` type | `varos-core` effects (pure) |
| One evaluator shared by render, export and Expand Appearance | `VC/effects/src/bake.rs` (397) | ADAPT (idea + structure) | `varos-core` |
| Group container evaluation / contents slot | `VC/effects/src/group.rs:142-279` | IDEA only (see D5) | — |
| Gradient annotator gestures (bar, stop chips, midpoint, aspect/focal) | `VC/tools/src/xform/gradient.rs` (928) | IDEA — hit/drag maths; our painting, our tokens | `varos-app` |
| Stroke gradient along/across slicing | `VC/effects/src/stroke/gradient.rs` (432) | later, after S3 | `varos-core` |
| Raster effect data (Shadow/Glow, FxCommon) | `PC/doc/src/effects.rs` (341) | ADAPT — **points, not pixels** | `varos-core` effects |
| Effect order + fill-vs-layer opacity rule (as a written spec) | `PC/compose/src/effects.rs:1-60,1189-1466` | IDEA/spec | ADR text |
| EDT/chamfer distance, tent blur, contour LUTs | `PC/compose/src/effects.rs:60-440,711-870` | ADAPT as the CPU reference | `varos-raster` |
| Mask feather (3-box Gaussian) + density formula | `PC/compose/src/masks.rs` (204) | ADAPT | `varos-raster`; WGSL rebuilt |
| Blend formulas (W3C + PS variants) | `PC/compose/src/psblend.rs`, `PC/color/src/blend.rs` (285+520) | ADAPT as pure functions (no thread-locals) | `varos-core`; WGSL rebuilt |
| GPU device-health watch + `Unsupported` → readable fallback contract | `PC/gpu/src/health.rs` (202) | ADAPT near-verbatim (wgpu **29** here vs 30 there) | `varos-render-wgpu` |
| Separable blur / dilate / glow / finish kernels | `PC/gpu/src/compose.wgsl:1029-1178` (~150) | REBUILD in our WGSL from the maths | `varos-render-wgpu` |
| Layer stack + texture pool with depth cap and budget; gradient cover pipelines; two-texture mask composite | — (MASKS_PLAN §3.1, study C §c) | BUILD | `varos-render-wgpu` |
| PDF shading (axial/radial), SMask groups, nested transparency groups, image XObjects for raster effects | — | BUILD our way. **No silent fallback:** every unsupported case is listed in an export report and shown to the user. Never "averaged", never dropped quietly (VectorCraft averages gradient stops, `VC/pdf/src/export.rs:801-812`; it also drops mesh without a warning, `:999-1001`) | `varos-pdf` |
| SVG `<linearGradient>/<radialGradient>`, `<mask>`, `<filter>`, one element per entry | — (VC `svg/src/export.rs` as reading only) | BUILD, same report rule (VC silently drops the shadow blend mode, `:1518`) | `varos-core/src/svg.rs` |
| Bridge 1.2 (`appearance`, `paint_kind`, gradient paints, effects, mask verbs) | — | BUILD, opt-in `api:"1.2"` | `varos-core/src/bridge.rs`, `varos-bridge` |

Never lifted: anything that touches egui 0.36/eframe (`VC/ui-egui`, `PC/ui-egui`), vello/CPU-renderer code, wasmi plug-ins, `serde_json::Value` effect params, raw-PSD side channels.

## 4. Risks and guards

Rows 1–18 come from study C §d; rows A1–A6 and B1–B5 are the "avoid" items of studies A and B.

| # | Risk | What breaks | Guard |
|---|---|---|---|
| 1 | Writer emits a new key without a bump | Older Varos mis-opens; ADR-0008 broken | Bump + migration + fixtures + refusal fixture *before* the writer; old-reader harness |
| 2 | "v5" claimed by text (ADR-0010) and gradients (COLOR_PICKER_V3:34) | Two incompatible v5 files | Decision D1; `v5_future` fixtures move up at each bump |
| 3 | Gradient read through `.solid()` = None at ~20 sites | Invisible, unclickable, missing from PDF/SVG/thumbs, "no fill" to agents | Gradient ships end-to-end in ONE slice (S2); picker tab stays disabled until then |
| 4 | Undo = 200 whole-doc clones (`editor.rs:3418-3430`) | RAM grows if rasters/caches enter `Document` | Caches live only in the renderer; a test that `Document` holds no raster bytes |
| 5 | One offscreen layer, no nesting | Group looks, effects in clips and masks in groups render wrong | Fix the clip bugs first (S0); layer stack with a depth cap (S4) |
| 6 | Texture memory, ≈95–190 MB per full-frame MSAA level on Retina | OOM or swap on 8 GB Macs | Budget + depth cap + measured fallback (flatten deeper levels, reported) |
| 7 | Per-frame effect evaluation | Heat; blur recomputed while panning | Per-object effect cache keyed like `flatten.rs`; idle stays `Wait`; effects never request frames |
| 8 | `scene_signature`/`thumb_key` hash solids only (`scene.rs:128-135`, `layers.rs:117-122`) | Stale canvas/thumbnail on live edits | Hash the full `Paint` + stack + look (S0) |
| 9 | Cull pad = `stroke_width/2` (`scene.rs:236-237,401`) | Shadows/extra strokes cut at screen edge | One `painted_extent()` used by cull + hit (S0, returns today's value) |
| 10 | Role checks written as `== GroupRole::Clip` (`clipboard.rs:91`, `model.rs:783-816`) | A soft-mask copy loses its mask; mask shape painted as art | One `is_mask_group()` helper before any soft mask (S0) |
| 11 | A second mask stored form (`Path.mask`) | Breaks "one stored form forever" | Row gesture desugars to a mask group (§2) |
| 12 | Mask thumbnail beside the row thumbnail | Collides with drop-onto-thumbnail = clip and the 26 px row | Owner mockup first (D2/D7); one drop target |
| 13 | Bridge describe/digest drift (`json!(p)`, `json!(node)`, `settings_digest`) | Frozen 1.0/1.1 fixtures fail; every group looks "changed" | `skip_serializing_if` on every new key; 1.2 opt-in; parity tests in `contracts.rs` |
| 14 | CPU snapshot ≠ GPU canvas | The agent sees a different design than Ahmed | Shared `Group`/`Prim` contract; a paired CPU/GPU fixture per paint/effect |
| 15 | Pathfinder/eyedropper/apply-current drop non-solid paint | Silent paint loss | Carry `Paint`/stack by value (pattern `model.rs:1131-1138`) + tests |
| 16 | New UI breaks ratchets (`shell/ratchet_tests.rs:65-89`) | Gate failures or token drift | Tokens only in `shell/tokens.rs`; new panel module |
| 17 | New panel id in saved layout | Older build resets to the default layout | Acceptable, stated in the hand-off; no data loss |
| 18 | PDF object/byte growth (`limits.rs:40`) | Big docs hit `max_pdf_objects`; save refused | Pool shadings/SMasks per page; `save_budget.rs` |
| A1 | Untyped `serde_json::Value` effect params (VC) | Typos default silently; no schema | Typed per-effect enum (§2); validator bounds every number |
| A2 | Raster order collapsed, blur radii summed (VC `fx.rs:484-582`) | Results differ from the stack the user sees | True stack order; a test per order case |
| A3 | Effected geometry recomputed every frame (VC `fx.rs:108`) | Heat | Cache (row 7) |
| A4 | Silent export losses (averaged stops, dropped blend, mesh → quads) | Print ≠ screen with no warning | Export report lists every degradation; tests assert the report |
| A5 | Raster effects in PDF at 72 ppi | Blurry print | Decision D6: default 300 ppi, shown in the report |
| A6 | Plug-ins/text engine pulled into core (wasmi) | `varos-core` loses purity | No plug-ins in core; dependency-direction gate |
| B1 | Process-wide colour/blend state (PC globals, thread-locals) | Racy tests, wrong colours across threads | Explicit render context passed through |
| B2 | Sibling-run `clipped: bool` (PC) | A second grouping topology | Map the gesture onto our Clip group |
| B3 | Effect sizes in pixels (PC `MAX_REACH = 512` px) | Effects change with zoom/DPI | Points in the model; rasterise at device or export DPI (D6) |
| B4 | Groups recomposited on CPU per tile (PC) | Slow, heat | Group renders once into a GPU layer; effects read that |
| B5 | egui 0.36/eframe UI, wgpu 30 | Breaks our stack laws | Never lifted; ideas only |

**Fix FIRST — slice S0, before any appearance code:**
1. **Opacity lost inside a clip.** `tess.rs:650-666`: `group_draws` never applies an Isolated member's opacity. Needs a masked offscreen pass and an owner hand test.
2. **`Draw::StrokeCov` and `Draw::Knockout` ignore the clip bit.** These are in `lib.rs` `draw_steps` (~`:743-780`); translucent strokes are not cut by masks.
3. **Stale hashes and culling.** Hash the full paint in `scene_signature` and `thumb_key`, and add one `painted_extent()` for cull and hit. Behaviour stays the same today, but the stale-canvas trap is closed.
4. **One shared mask-role check.** `is_mask_group()` replaces every `== GroupRole::Clip` that means "this group masks". `validate.rs:69` keeps refusing soft masks until their bump.
5. **v5 ordering written down (D1).** Then ADR-0010, COLOR_PICKER_V3 §3 and PLAN stop saying "v5" for different things.
6. A guard test that `Document` holds no raster bytes, plus the export-report type (empty today). Later slices then have a place to report losses.

## 5. Decisions the owner must make

| # | Question | Recommendation |
|---|---|---|
| D1 | **v5 ordering:** text vs gradients vs appearance — one bump reserving all three, or a sequence? | **Sequence; nobody reserves.** A number goes to whichever writer *merges* first (ADR-0008 rule 3). "Reserving" would mean persisting fields with no behaviour, which ADR-0010:103 forbids. Text is first in PLAN (B1 before B2), so text most likely takes v5, gradients the next number, appearance/masks after. Docs say "next format bump", never a number, until merge. |
| D2 | **Appearance UI home** (ONE-HOME rule) | **An "Appearance" section in Properties is the one home.** It replaces the two paint rows at `properties.rs:125-126` and lists the stack, effects, opacity and mask for the selected row (leaf or group). The Layers row gets a small right-aligned **fx badge** as a mirror: it shows that something is there, and a click focuses the home. The control bar stays a mirror. Not a popover off the row: that would be a second home. Mockup in Figma first. |
| D3 | **Mask kinds in v1** | **Clip (exists) + alpha.** Luma later: same texture path, one more shader switch, but it needs its own explanation in the UI. Raster painted masks stay FUTURE (LAYERS_VISION §3, no brush engine). |
| D4 | **Pixel (raster) effects in v1** | **None in the first appearance release.** Shadow + blur only, in S7, after the effect cache and layer budget exist and are measured on a real Mac. Heat is a product law here. |
| D5 | **Group "contents slot"** (group fills/strokes painted around its members, VC `contents_index`) | **No for v1.** A group look = opacity + effects + mask. Group fills/strokes are niche Illustrator power with a high evaluation cost. Revisit after S7. |
| D6 | **Effect units at zoom / DPI** | **Points (document units) everywhere.** Effects scale with zoom like the art and rasterise at device pixels on screen. PDF export rasterises at **300 ppi** by default (not VC's 72), listed in the export report. |
| D7 | **How a masked row looks** (desugared mask group + the row the user edits) | Owner mockup before code. Recommendation: the mask group shows as **one row** with the art thumbnail and a small mask chip, and it can be expanded to reach the mask shape and the art. Keep the single drop target of LAYERS_VISION §3.2. |

## 6. Slice order

Rules for every slice: gated, shippable alone, owner hand test in the real window, none breaks a v4 file, and the plain-object invariant (§2) is re-proved every slice. Effort is a rough count of focused worker sessions, honest ±50%.

| Slice | What ships | Format / Bridge | Prior art used | Effort |
|---|---|---|---|---|
| **S0 Fixes first** | §4 list 1–6: clip opacity, clip bit for StrokeCov/Knockout, full-paint hashes, `painted_extent`, `is_mask_group`, v5 note, no-raster guard, empty export report | none | PC `masks.rs` maths only if needed | 3–5 (GPU passes need hand tests) |
| **S1 Appearance type, one-entry invariant** | `Appearance` read-view + `StackItem`/`EntryOpts`/`Look` types in memory. Every reader (scene, hit, PDF, SVG, Bridge) goes through the view. **No behaviour change** | **no bump**: v4 fixtures load→save byte-identical; scene `Prim` equality on goldens | VC `appearance.rs` shape | 2–3 |
| **S2 Gradient paint end-to-end** | `Paint::Gradient` (linear + radial, stops, midpoint, pad). Built on: GPU cover pipelines with a paint buffer; CPU tiny-skia shaders; PDF axial/radial shading; SVG gradients; hit-inside for any painted fill; eyedropper/Pathfinder/duplicate keep it; picker **Gradient tab enabled**; on-canvas annotator | bump; Bridge **1.2** opt-in gradient paint; 1.0/1.1 never say `null` for a gradient (exact token in the ADR-0009 amendment) | VC `gradient.rs`, annotator idea | 8–12 |
| **S3 Stroke engine** | caps, joins, miter, dashes, align (inside/outside), arrowheads later, all on the base stroke via `StrokeStyle` (owner piece B2) | bump; 1.2 fields | VC stroke engine (`geom`, `pathops`) per EXTERNAL_PRIOR_ART | 5–8 |
| **S4 Multiple fills/strokes + group look** | extra `StackItem`s with per-entry opacity/visible; `Node.look` (group opacity, isolate); **GPU layer stack + texture pool + depth cap + budget**; Appearance section (D2) + Layers fx badge; Expand Appearance | bump; 1.2 `appearance` read/write | VC `appearance.rs`, `bake.rs` | 6–9 |
| **S5 Vector effects** | Offset, Zig-Zag, Transform (copies), Warp, typed and cached; one evaluator for canvas, CPU snapshot, PDF, SVG, Expand | bump; 1.2 effect verbs | VC `distort/warp/stylize/util`, `live.rs` | 4–6 |
| **S6 Masks from the row** | "Add mask" on any row → desugared mask group; Clip + **MaskAlpha** (validator allows it with the bump); second mask texture + two-texture composite; PDF SMask, SVG `<mask>`; clipboard and paint-role via `is_mask_group` | bump (emitting `MaskAlpha` is a writer change); 1.2 mask verbs | PC mask shapes/feather maths; LAYERS_VISION §3 | 5–8 |
| **S7 Raster effects on GPU** | Shadow + Blur (then glow) in points; blend modes join here (backdrop-copy pass); effect cache in the renderer only; PDF image XObjects at 300 ppi, reported; device-health watch | bump; 1.2 | PC `effects.rs` data, EDT/blur maths, WGSL kernels, `health.rs` | 8–12 |

Order notes:
- S2 and S3 are independent, but both are owner piece B2, which comes after text (PLAN:50). They can run in either order.
- S4 needs S1. S6 needs S0 (clip fixes) and the S4 layer stack. S7 needs S4.
- Each bump ships its named migration, frozen fixtures, refusal fixtures and VRS_FORMAT.md update *before* its writer (ADR-0008 rule 3).

## 7. Summary (10 lines)

1. The owner's "effects and masks on an element or a whole group, from the same row" is sound; PhotoCraft (and Photoshop) prove the row model.
2. Every row gets one **Appearance**: ordered fill/stroke entries (solid|gradient, opacity, visible, effects), row effects, opacity/isolate, mask.
3. Storage reuses today's fields as the *base* entries; extras, effects and group looks are new keys that are absent unless used.
4. A plain object stays byte-identical (S1 has no bump; the v4 fixtures must round-trip exactly; Bridge 1.0/1.1 fixtures never change).
5. Masks keep ONE stored form: a row's "Add mask" desugars into the existing mask group with `mask_child`, never a `Path.mask`.
6. Group order: children → isolated buffer → mask → effects → opacity (blend later); a group with no look stays a free pass-through.
7. Lift VectorCraft's stack and gradient model plus PhotoCraft's effect data, maths and health watch; rebuild all GPU/WGSL, PDF and SVG our way.
8. Exports never fall back silently: every unsupported case is reported, nothing is averaged or dropped quietly.
9. Fix first (S0): the two clip bugs, stale hashes, cull extent, one mask-role check, and the v5 ordering.
10. Then S1 type → S2 gradients → S3 stroke → S4 multi-entry + group look → S5 vector fx → S6 masks → S7 raster fx; ADR-0013 before any code.
