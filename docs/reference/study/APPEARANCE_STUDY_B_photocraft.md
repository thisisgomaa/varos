> **Status:** reference study (read-only), 2026-10-09. Source: PhotoCraft `4cb7cf3` at `~/Documents/AI workspace/reference/artcraft/photocraft`; paths are relative to its `crates/`. Question: owner idea "effects and masks attach to the element or to a group, from the same Layers row".
> **Licence correction:** `license = "MIT OR Apache-2.0"` (`../Cargo.toml:9`, `../NOTICE:3`), not Apache-only as `docs/reference/EXTERNAL_PRIOR_ART.md:11` says. Both are GPL-3.0-usable; that doc's borrowing rules apply.

# Appearance study B — PhotoCraft masks, layer effects, compositing, GPU

## 0. The answer in one table: one row model for leaf and group
PhotoCraft has **one `Layer` struct** for every row, and a group is just `content: LayerContent::Group(..)`
(`doc/src/lib.rs:431-439`). So **mask, vector mask, clip flag, effects, blend, opacity and fill opacity are row
fields that work the same on a leaf and on a group** (`doc/src/lib.rs:456-472`). The engine commands only check
`has_layer`, so they accept a group the same way they accept a leaf (`engine/src/commands.rs:589-626`,
`engine/src/layer_style.rs:597-614`).

| Attachment (row field) | Leaf | Group | Notes |
|---|---|---|---|
| `mask: Option<LayerMask>` (pixel) | yes | yes | Applied to the group's isolated composite (`compose/src/lib.rs:678-705`) |
| `vector_mask: Option<VectorMask>` | yes | yes | Multiplied with the pixel mask (`compose/src/lib.rs:650-669`) |
| `clipped: bool` (clip to the base below) | yes | yes (base or member) | Sibling-run topology, collected while compositing (`compose/src/lib.rs:507-526`) |
| `effects: Effects` (layer style) | yes | yes | A pass-through group that has effects is forced to isolate (`compose/src/lib.rs:1058-1062`) |
| `opacity` / `fill_opacity` / `blend` | yes | yes | Fill opacity scales the content but not the effects (`doc/src/lib.rs:462-465`) |

## 1. Masks
| Kind | Type | Where it lives | How it is applied |
|---|---|---|---|
| Layer (pixel) mask | `LayerMask` | `Layer.mask` | Value is `1 − density·(1 − v)`; disabled = 1 (`doc/src/lib.rs:133-161`) |
| Vector mask | `VectorMask { path, enabled, linked, density, feather }` | `Layer.vector_mask` | Rasterised by `photocraft_vector::CompiledVectorMask` (`vector/src/lib.rs:85-115`) |
| Clipping mask | `Layer.clipped = true` | flag on the upper sibling | Members composite *atop* the base's isolated content (`compose/src/lib.rs:1411-1486`) |
| Group mask | (no separate type) | the same `mask` / `vector_mask` on a group row | Applied after the children are composited (`compose/src/lib.rs:680-704`) |
| Smart-filter mask | `SmartObject.filter_mask: Option<LayerMask>` | `doc/src/lib.rs:370` | Masks the smart filters only |
| Artboard clip | `Group.artboard: Option<Artboard>` | `doc/src/lib.rs:187-192` | Children and their effects are clipped to the board (`compose/src/lib.rs:1025-1047`) |
| Opacity mask (Illustrator-style luminosity) | **none** | — | Not modelled. Knockout shallow/deep is not modelled either; only the drop-shadow `knocks_out` flag exists |

```rust
// doc/src/lib.rs:115-123                      // doc/src/vector.rs:187-197
pub struct LayerMask {                         pub struct VectorMask {
    pub surface: Surface, // GRAY, default px      pub path: Path,       // + fill_rule, inverted
    pub enabled: bool,                             pub enabled: bool,
    pub linked: bool,                              pub linked: bool,     // moves with the layer
    pub density: f32,                              pub density: f32,
    pub feather: f32,                              pub feather: f32,
}                                              }
```

- **Link / unlink.** A move translates the mask only when `linked` is set (`engine/src/commands.rs:1152-1162`,
  vector masks at `engine/src/vector_cmds.rs:436-441`). The toggles are `layer.layerMask.linked` and
  `layer.vectorMask.linked`. In the row, a chain icon sits before each mask thumbnail
  (`ui-egui/src/mask_thumbs_ui.rs:1-10`).
- **Enable / disable.** `layer.layerMask.enabled` and `layer.vectorMask.enabled`, or ⇧-click the thumbnail, which
  then shows a red X (`mask_thumbs_ui.rs:3-6`).
- **Density / feather.** Feather is a Gaussian with sigma equal to the feather radius, capped at 1000 px. Because
  the blur crosses tiles, the feathered mask is cached as one canvas-wide combined surface, keeping 64 LRU
  entries (`compose/src/masks.rs:1-90`). The vector-mask feather *is* rendered (`masks.rs:has_feather`), so the
  doc comment that says "rendered unfeathered for now" is out of date (`doc/src/vector.rs:195`). A vector mask's
  density and feather can be edited through `layer.vectorMask.edit` (`engine/src/vector_cmds.rs:1119-1126`). I
  found **no engine command** that edits a pixel mask's density or feather; those values only come in from PSD.
- **Editing.** A pixel mask is an ordinary gray `Surface`. Paint, brush and filter commands take
  `"target":"mask"` and route through `channel_cmds::target_surface` (`engine/src/commands.rs:87-89`,
  `brush_cmds.rs:225-248`). Create, apply and delete use `revealAll`, `hideAll`, `revealSelection`,
  `hideSelection`, `fromTransparency`, `apply` and `delete` (`commands.rs:605-626`, `layer_menu_cmds.rs:725-728`).
  A vector mask is edited as a path (`layer.vectorMask.add/fromPath/edit/currentPath/delete`). ⌥-click shows the
  mask itself; ⌘-click loads it as a selection.
- **`.pcraft` storage.** `MaskM { surface: SurfaceM, enabled, linked, density, feather }`
  (`format/src/manifest.rs:174-180`). Tiles are content-addressed, 256², zstd-compressed (`format/src/lib.rs:5-11`).
  The vector mask is stored as serde of the `VectorMask` itself (`manifest.rs:209`).
- **PSD export.** The pixel mask goes into the layer record's mask data plus a mask channel. Density and feather
  go into the mask parameters, and the vector mask's density and feather share that same block
  (`io/src/psd_export.rs:287-345`). The vector path is written as `vmsk`/`vsms` (`psd_export.rs:515-524`). The
  clip flag is the record's `clipping` byte (`psd_export.rs:391`).

## 2. Layer effects (styles)
```rust
// doc/src/lib.rs:165-176 (container)          // doc/src/effects.rs:79-84, 267-289 (items)
pub struct Effects {                            pub struct FxCommon { enabled, blend: BlendMode, opacity }
    pub enabled: bool,          // master switch     pub enum Effect { DropShadow(Shadow), InnerShadow(Shadow),
    pub items: Vec<Effect>,     // multi-instance      OuterGlow(Glow), InnerGlow(Glow), Stroke(StrokeFx),
    pub psd_raw: Option<Arc<Vec<u8>>>, // lfx2/lfxs     ColorOverlay{common,color}, GradientOverlay{..},
    pub reference: Option<(f64, f64)>, // pattern anchor PatternOverlay{..}, Satin(Satin), BevelEmboss(Bevel) }
}
```

- **Per-effect controls.** Every effect carries `FxCommon` (enabled, blend mode, opacity). Bevel has two of them,
  one for the highlight and one for the shadow. You can add several of the same kind (`effects.rs:263-266`).
  Global light lives on `Document.global_light`, and an effect follows it when `use_global_light` is set.
- **Render order.** This is fixed by kind, not by list order (`compose/src/effects.rs:1-20`, body at
  `1194-1466`). Within one kind, the first listed instance is drawn on top, so the list is painted in reverse.
  From the bottom up:
  1. drop shadows (knocked out only where the fill is see-through)
  2. outer glows
  3. outside parts of strokes, then the outer bevel. All of these paint straight onto a **copy of the backdrop**
     (`work`), each with its own blend mode.
  4. the layer at **fill opacity**, then the interior effects in this order: pattern, gradient and colour
     overlays, satin, inner glow, inner shadow, the vector stroke, the inside parts of strokes, the inner bevel.
     Each is painted relative to the shape and then multiplied by the shape's alpha.
  5. that interior result merged onto `work` with the **layer's blend mode**
  6. emboss and pillow emboss shade the merged result
  7. **layer opacity** mixes `work` against the original backdrop, premultiplied (`effects.rs:1450-1466`)
- **Effect shape.** This is the content's alpha *after* its masks (`compose/src/lib.rs:604-635`), joined with the
  vector outline for filled shapes. So masks shape the effects; there is no "Layer Mask Hides Effects" option.
- **Effects on a group.** The group **rasterises first, then the effects run.** `render_content` composites the
  children into a transparent isolated buffer and applies the group's mask (`compose/src/lib.rs:680-704`). Clipped
  layers then go atop, and `composite_with_effects_prepared` treats that buffer exactly like a leaf's pixels
  (`lib.rs:1141-1162`). A pass-through group with effects behaves like Normal (`lib.rs:1058-1062`,
  `effects.rs:1432`). On the GPU the group's effect shape is still a **CPU composite of the children**
  (`gpu/src/fx.rs:503-508` → `compose::layer_shape`), keyed by a deep `group_key` over every descendant
  (`fx.rs:587-620`).
- **Pixel-space caveat for Varos.** Sizes, distances and `MAX_REACH = 512` are in document pixels
  (`effects.rs:36-60`). A vector editor has to choose a resolution: canvas zoom or export DPI.

## 3. Compositing (`compose`, CPU reference, 7.9 kLoC)
- **Buffers.** Straight-alpha `[f32; 4]` (`compose/src/lib.rs:37-42`). Mixing is premultiplied
  (`lib.rs:1072-1083`, `mix_premul`). Blending happens in display RGB, not linear (`lib.rs:12-14`). Blend modes
  are the W3C formula plus Photoshop variants (`psblend.rs:81-125`).
- **Passes.** `composite_stack` walks siblings bottom to top and gathers each clip run (`lib.rs:507-526`). Then
  `composite_layer` → `composite_layer_any` (artboard) → `composite_layer_plain` (`lib.rs:1049-1186`):
  - pass-through group: draws straight into the backdrop, then mixes with mask and opacity
  - adjustment: transforms the backdrop
  - effects path
  - stroked-shape split
  - plain `render_content` + clip members atop + `blend_into`
  - after that, channel restrictions and Blend If (`lib.rs:997-1010`)
- **Mask order.** A mask scales the content's alpha *before* clip members and effects (`lib.rs:700-704`). For a
  pass-through group, the mask scales the *mix* between backdrop-before and backdrop-after (`lib.rs:1064-1083`).
- **Parallelism and caching.**
  - Rendering uses 256² tiles across rayon bands (`lib.rs:80-187`).
  - Effect maps are built once per layer state *before* the tiles start (`prepare_effects`, `lib.rs:467-505`), in a
    global LRU with a byte budget of 768 MB by default (`lib.rs:1277-1400`). The key is a hash of the layer's
    identity plus its region.
  - There is no cache of a group's composite. Groups are recomposited every tile.
  - Damage-limited redraws: `render_reduced_damage` (`lib.rs:314`). Undo damage: `layer_multi_cmds::step_damage`.

## 4. GPU (`gpu`, 6.4 kLoC, `wgpu = "30"`, features std+wgsl)
- **Shape.** A planner turns the tree into linear passes over 1024² `Rgba32Float` chunk slots, falling back to
  `Rgba16Float` (`gpu/src/lib.rs:1-31, 55-79`; `plan.rs:1-13`). Execution is one ~1.2 kLoC `compose.wgsl` with
  fullscreen fragment entry points. Layer pixels live in 2048² pages, uploaded per copy-on-write tile and evicted
  LRU (4 GB budget).
- **Effect kernels.** `MShift`, `MDilate`, `MBlur`, `MGlow`, `MFinish`, `MBevelH`, `MBevelShade`, `MBevelTex`,
  `MStroke` (`plan.rs:55-64`; WGSL `compose.wgsl:1029-1178`). `fs_mblur` is a separable 1-D convolution; its
  weights come from a LUT texture.
  - **Important:** distance fields (EDT and chamfer) and the effect shape are computed **on the CPU** and
    uploaded (`fx.rs:1-20, 492-508`). The GPU only chains map programs (`fx.rs:273-450`).
  - The GPU also covers colour adjustments (`fs_adjust`, `compose.wgsl:511-630`) and every blend mode.
- **CPU fallback.** `plan()` / `supports()` return `Unsupported(String)` for: Blend If, Multichannel, effect noise,
  blurs wider than 4096 taps, oversized patterns, and devices without float targets (`lib.rs:698-760`,
  `plan.rs:312-490`). The caller then renders with `photocraft-compose`. A parity test suite holds both paths
  within about 1/255 (`gpu/tests/parity.rs`, 1.5 kLoC).
- **Device health.** `DeviceHealth::watch` replaces wgpu's uncaptured-error handler and lost callback with
  atomics (`health.rs:1-130`). Every entry point checks `fault()` and stops issuing work, which makes the app fall
  back to the CPU. Device-loss test: `gpu/tests/device_loss.rs`.
- **Reuse for a wgpu vector renderer.** The *ideas* carry over, but the code does not lift directly:
  - Varos pins wgpu **29** (`varos/crates/varos-render-wgpu/Cargo.toml:10`); PhotoCraft uses wgpu **30**.
  - The planner is tied to `photocraft_doc::Layer`, and the effect inputs come from CPU rasters.
  - **Liftable:** the separable-blur and dilate/glow/finish kernel *math* (about 70 lines of WGSL), the
    map-program chaining idea, `health.rs` almost verbatim, and the `Unsupported` → CPU fallback contract.
    That contract fits ADR-0001's "GPU failure must stay readable".

## 5. Undo for mask and effect edits
Every command runs inside `Session::edit(label, |doc, active| …)` (`engine/src/lib.rs:447-475`), which clones the
`Arc<Document>`, mutates the clone and records the *whole previous document* (`ops/src/lib.rs:1-10, 70`). This is
cheap because tiles are `Arc` copy-on-write: a mask stroke only re-stores the tiles it touched. A slider drag
merges into one step through `coalesce_request` (`lib.rs:459-466`). There are no per-field inverse commands.
`History` has a state cap (50) and a byte budget for tiles that only history holds.

## 6. Dependencies and UI binding
| Crate | Deps | egui/eframe? |
|---|---|---|
| `doc` | geom, color, raster, serde, serde_json | no |
| `compose` | doc, vector, raster, color, geom; rayon (native) | no |
| `gpu` | wgpu 30, half 2, log, compose, doc; rayon | no (the version is only *matched* to eframe's wgpu) |
| `vector` | geom, color, raster, doc; rayon | no |
| `ops` (History) | doc | no |
| `ui-egui` | eframe 0.36 (wgpu), egui 0.36, egui_extras | **yes**. Varos runs egui 0.35 with no eframe, so nothing here is usable |

All external crates are MIT/Apache-class (wgpu, half, rayon, serde). `doc` depends on `raster::Surface`, so the
mask types pull in the tiled-surface crate (`raster`, 1.0 kLoC).

## 7. What lifts cleanly vs what to avoid
| Liftable (adapt; NOTICE + header per EXTERNAL_PRIOR_ART rules) | Path | Lines |
|---|---|---|
| Effect data model (Shadow/Glow/Stroke/Satin/Bevel/overlays, FxCommon, Contour, GlobalLight) | `doc/src/effects.rs` | 341 |
| `Effects` container + `LayerMask` / `VectorMask` field shapes (enabled/linked/density/feather) | `doc/src/lib.rs:115-176`, `doc/src/vector.rs:187-203` | ~80 |
| Effect stack order + fill-vs-layer opacity rule + knockout (as a spec or port) | `compose/src/effects.rs:1-60, 1189-1466` | ~330 |
| Distance transforms (EDT, chamfer), tent blur, contour LUTs | `compose/src/effects.rs:60-440, 711-870` | ~550 |
| Mask feather Gaussian (3-box) + density formula | `compose/src/masks.rs` | 204 |
| Photoshop blend formulas (straight alpha, W3C + PS variants) | `compose/src/psblend.rs`, `color/src/blend.rs` | 285 + 520 |
| GPU device-health watch | `gpu/src/health.rs` | 202 |
| WGSL map kernels (shift/dilate/blur/glow/finish/stroke) | `gpu/src/compose.wgsl:1029-1178` | ~150 |
| PSD `lfx2` effect writer (only if PSD export is ever wanted) | `io/src/effects_map.rs` | 841 |

**Avoid:**
1. **Process-wide colour and blend state.**
   - `TEXT_GAMMA_SETTING` global atomic (`compose/src/psblend.rs:140-156`); its own comment warns that tests race
     on it.
   - Thread-local `LAB_MIX` (`psblend.rs:74-78`) and thread-local `ACTIVE_CMYK` (`color/src/convert.rs:62-90`),
     which every rayon worker must re-enter.
   - Global caches keyed by pointer addresses (`compose/src/lib.rs:406-423`, `masks.rs:20-45`, `fx_cache`).
   Varos should pass all of this through an explicit render context.
2. **The sibling-run `clipped: bool` topology.** It is exactly the "flagged sibling-run" form that
   LAYERS_VISION §3.4 rules out. Map PhotoCraft's clip *gesture* onto our single parent-child Clip group.
3. **Whole-document snapshot undo.** It only works because every pixel lives in an `Arc` tile. Varos's
   history/agent model needs reviewable per-AI steps (VISION_AI_NATIVE).
4. **Raw-PSD side channels** (`psd_raw`, `psd_blocks`) on the core model. They add bulk to `Layer` and are not
   relevant until there is a PSD importer.
5. **Recomposing groups on the CPU every tile for effect shapes.** For a vector group, render the group once into
   an offscreen target on the GPU (our existing SaveLayer path) and build its effects from that.

## Summary (10 lines)
1. PhotoCraft attaches mask, vector mask, clip, effects, opacity, fill opacity and blend to **every row**; a group is just a content variant.
2. So the owner's "effects/masks on a single layer *or* a whole group from the same row" is exactly PhotoCraft's (and Photoshop's) model.
3. Group rule: children composite into an isolated buffer → group mask → clip members → effects built from that alpha → layer blend + opacity.
4. A pass-through group that has effects is forced to isolate. Masks shape effects; there is no "mask hides effects" option and no knockout groups.
5. Mask kinds: pixel `LayerMask` and `VectorMask` (both enabled/linked/density/feather), the clip flag, the smart-filter mask, the artboard clip. No Illustrator opacity mask.
6. Effects are a `Vec<Effect>` (ten kinds, multiple instances), each with blend, opacity and enabled. The stack order is fixed by kind, and fill opacity is separate from layer opacity.
7. GPU: wgpu 30 fragment-pass planner with a CPU fallback through `Unsupported` and a solid `DeviceHealth` watch. Distance fields and group shapes are still computed on the CPU.
8. Undo is a whole-document `Arc` snapshot per command, with slider coalescing; there are no per-mask or per-effect inverse ops.
9. Liftable: `doc/effects.rs` (341), the effect-order spec, EDT/blur math, mask feather, `health.rs` (202), and about 150 lines of WGSL map kernels. Licence is MIT OR Apache-2.0, not Apache-only.
10. Avoid: global/thread-local colour state, the sibling `clipped` flag (it breaks our one-clip-form law), egui 0.36/eframe UI code, and pixel-unit effect sizes taken without a zoom/DPI decision.
