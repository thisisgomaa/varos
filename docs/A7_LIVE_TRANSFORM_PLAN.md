# A7 — Illustrator-style LIVE per-object transform (STAGED implementation plan)

> **Pain A7** (PAINS_LOG): *"Transform/rotation values reset the moment I leave the object — they should stay remembered until merge / Expand / Apply."*
> **Ahmed's decision:** match Illustrator. An object keeps a **LIVE rotation** that persists through save/reload and reselect; the panel shows the **TRUE rotated W/H** (the object's own un-rotated dimensions, not the axis-aligned box of the rotated points); geometry is baked into coordinates **only** on Expand / merge (boolean) / Apply.
>
> This is a foundation change, not a bug fix. It is written to be executed **stage by stage with a gate between each** — every stage compiles, passes the existing test suite, and leaves the app fully usable. The transform stays **identity everywhere** until Stage 4, so Stages 0–3 are provably behaviour-neutral.

---

## 0. Where we are today (verified in code)

Rotation is **baked** into anchor coordinates. There is no stored transform:

- `Anchor { id, p, hin, hout, smooth }` — `model.rs:13`. `p/hin/hout` are **world** points.
- `Path { id, anchors, closed, fill, stroke, stroke_width, holes, opacity, hidden, locked, name }` — `model.rs:91`. **No transform.**
- `Node { id, kind, name, parent, children, hidden, locked, color, clip_exempt }` — `model.rs:186`. **No transform.** `clip_exempt` (`model.rs:212`) is the precedent: a `#[serde(default)]` per-top-level-unit flag keyed on `unit_of`.
- `Editor::obj_angle` (`editor.rs:350`) is a **transient**, never-serialized scalar — the orientation of the selection frame *during* a gesture. It is reset to `0.0` in ~16 places on any selection change (`editor.rs:820, 973, 1018, 1063, 1102, 1123, 1164, 1203, 1215, 1254, 2507, 3297, 3304, 3339, 3718, 3735, 3777, 3911` + the tools `object.rs:15/34/40/60`).
- A rotate **drag** (`Drag::Rotate`, `editor.rs:3191`) and `set_obj_rotation` (`editor.rs:1169`) **overwrite `anchor.p`** via `rotate_about` and then set `obj_angle = 0.0`. That write is literally the bake.
- The panel reads W/H from `obj_bbox()` (`editor.rs:548`, the **axis-aligned world** box of the already-rotated outline) and angle from `obj_angle` (`ui.rs:533, 566`). So after deselect→reselect, `obj_angle == 0`, and W/H = the AABB of the rotated shape → **wrong**, exactly A7.

**Consequence for the plan:** the entire selection-frame math already works in an *angle + local-bbox* model — `obj_local_bbox` (`editor.rs:570`) un-rotates the outline by `-obj_angle`; `frame_handles`/`frame_corners` (`editor.rs:600/610`) re-rotate by `+obj_angle`. We are **persisting what is already transient**, and moving the bake from "every rotate" to "only Expand/merge/Apply."

---

## 1. Representation

### Recommendation: **angle + pivot, stored per top-level unit, behind a tiny `Xform` type + one lookup seam.**

Store, per selection unit, a rigid transform expressed as **rotation angle `θ` about a pivot point `piv`** (3 floats). Anchors become the object's **local (un-rotated)** geometry; the world position of any point is

```
world(p) = rotate_about(p, piv, θ)          // geom.rs:73 already implements this
```

Identity = `θ == 0` (pivot irrelevant when θ is 0). At `θ == 0`, `world == local`, so an un-rotated object is byte-for-byte what it is today.

#### Why angle+pivot over a 2×3 affine matrix
| | angle + pivot (3 f32) | 2×3 affine (6 f32) |
|---|---|---|
| Matches Illustrator's **bounding-box-rotation** model | **Yes** — an oriented bbox + un-rotated W/H is exactly this | Yes, but needs decomposition |
| Panel **true W/H** | **Trivial** — W/H = bbox of *local* anchors, no decomposition, no ambiguity | Needs polar/QR decompose; shear ambiguity if scale ever added |
| Reuses existing frame math (`obj_angle`, `obj_local_bbox`) | **Directly** | Rewrites it |
| Composition of rotations (rotate again about a different pivot) | Closed: two rotations compose to **one** rotation `θ₁+θ₂` about a derived center (pure-translation only in the measure-zero `θ₁+θ₂≡0` case, which bakes into anchors) | Matrix multiply (also clean) |
| Stable under direct-select local edits | **Yes** — `piv,θ` are frozen; editing one local anchor moves only that anchor's world image | Yes (M fixed) |
| Live **non-uniform scale / shear** later | Not representable — but **not needed** (scale/flip bake into local geometry, same as today, and that keeps W/H exact) | Free |

Scale, flip, and numeric W/H all **bake into local geometry** in both Illustrator and today's code, and baking scale into the *local* frame keeps W/H exact. So the only transform that must go live is **rotation**, and angle+pivot is the smallest faithful model.

**Escape hatch (do this from day one):** never let call sites touch `θ`/`piv` directly. Route everything through a small value type and one document helper, so the internal representation can be swapped to a 2×3 matrix later with a **one-file** change (the same HARD-SEAM discipline `paint_list`/`unit_of` already use):

```rust
// model.rs — new
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xform { pub rot: f32, pub piv: Pt }   // rot in radians; identity == rot 0
impl Default for Xform { fn default() -> Self { Xform { rot: 0.0, piv: [0.0,0.0] } } }
impl Xform {
    pub fn is_identity(&self) -> bool { self.rot.abs() < 1e-7 }
    pub fn apply(&self, p: Pt) -> Pt { if self.is_identity() { p } else { rotate_about(p, self.piv, self.rot) } }
    pub fn inverse_apply(&self, p: Pt) -> Pt { if self.is_identity() { p } else { rotate_about(p, self.piv, -self.rot) } }
    pub fn then_rotate(&self, dtheta: f32, about: Pt) -> Xform { /* compose two rotations → one (rot+dθ, derived center) */ }
    pub fn translated(&self, d: Pt) -> Xform { Xform { rot: self.rot, piv: add(self.piv, d) } } // move carries the pivot
}
```

Hand-write `Serialize`/`Deserialize` (or derive) — either is fine; `Xform` is plain floats.

#### Where it lives — on `Node`, at the **unit** level (mirrors `clip_exempt`)
```rust
// model.rs, in Node:
#[serde(default, skip_serializing_if = "Xform::is_identity")]
pub xform: Xform,
```
- **On `Node`, not `Path`, not `Anchor`.** A path inside a group must rotate *with the group as one rigid unit*; putting the transform on `Path` would let members rotate independently and break group rigidity. The unit that owns the transform is exactly `unit_of(pid)` (`model.rs:865`) = the top-level Group node, else the path's own leaf node — the **same key the scene clip map already uses** (`scene.rs:148`). An ungrouped path's unit is its leaf node, so that case is covered by the same field.
- **One transform per path, looked up once.** Add the seam:
  ```rust
  // model.rs
  pub fn unit_xform(&self, pid: u32) -> Xform {
      self.unit_of(pid).and_then(|n| self.node(n)).map(|n| n.xform).unwrap_or_default()
  }
  ```
  Every consumer reads `doc.unit_xform(pid).apply(local_p)`. This parallels `paint_list()`/`unit_of` exactly: **one indirection point** the whole codebase already funnels through.
- **Serde-default = old files load unchanged.** Field absent ⇒ `Xform::default()` (identity) ⇒ the anchors, which are already the baked world geometry, render/hit/export exactly as before. `skip_serializing_if` keeps un-rotated new files byte-clean. `Node` derives `PartialEq` and is compared in `serde_roundtrip.rs`; adding a defaulted field is compatible.
- **`obj_angle` becomes a cache, not the source of truth.** Its value is derived from the selected unit's stored `xform.rot` on selection change (see Stage 5). Keep the field to minimise churn; stop treating it as "the rotation."

#### Nested groups
Transforms are only ever **written** to the top-level unit (Stage 4 writes `unit_of(sel)`), and members hold identity. So the read is a single lookup (`unit_xform(pid)`), not a chain walk — sufficient and correct while transforms live only on unit nodes. If inner-group rotation (isolation-mode) is ever added, upgrade `unit_xform` to compose the ancestor chain; no call site changes.

---

## 2. Every subsystem that must compose the transform

Rule for the whole table: **reads `anchor.p` / `outline*` / `ring*` directly today → must read `unit_xform(pid).apply(...)` instead.** For point/edge *hit-tests* it is cheaper and lower-risk to map the **cursor into local space once** (`unit_xform(pid).inverse_apply(cursor)`) and keep the existing local-space test untouched. For **bboxes** and **rendering** we transform the outline points to world.

### 2a. Render — `scene.rs`
| Site | Reads today | Must read |
|---|---|---|
| `fill_prims` `scene.rs:185` | `outline_px(pi)` + `ring_px(hole)` (`model.rs:677/647`) | world outline = `xform.apply` over each flattened point |
| `stroke_prims` `scene.rs:216` | same | same |
| overlay skeleton `scene.rs:312` | `outline_px` | world outline |
| grabbed-segment hi `scene.rs:329` | `a.p/hout/hin` of the segment | world points |
| transform frame `scene.rs:342` | `frame_corners()/frame_handles()` | (fixed once those compose, §2d) |
| handle lines/discs `scene.rs:436/445` | `a.p, a.hin, a.hout` | world points (draw at rotated positions) |
| anchor markers `scene.rs:455` | `a.p` (+ smooth/sel) | world points |
| snap `PathHi` `scene.rs:504` | `outline_px` | world outline |

**Cleanest choke point:** add `Document::world_outline_px(pi, ppu)` and `world_ring_px(hole, pi, ppu)` that call the existing `outline_px`/`ring_px` then map through `unit_xform(paths[pi].id)`. Convert the ~8 sites above to the world variants. The anchor/handle overlay markers (`scene.rs:436–475`) read raw `a.p/hin/hout` for **the current path** — wrap each in `xf.apply(...)` where `xf = doc.unit_xform(p.id)`.

### 2b. Hit-test — `editor.rs` + `model.rs`
| Site | Note |
|---|---|
| `path_under` `editor.rs:476` | pre-map cursor: `let lp = doc.unit_xform(id).inverse_apply(pos);` then call the existing `edge_dist`/`point_in_path` with `lp`. `edge_r` tolerance is rotation-invariant. |
| `edge_dist` `model.rs:604`, `nearest_seg` `model.rs:578`, `point_in_path` `model.rs:700` | **leave operating in whatever frame they are handed** — only their callers pre-map the cursor. Minimises blast radius. |
| `nearest_anchor` `editor.rs:453` | compare in world: test `dist(pos, xf.apply(a.p))`, or map cursor per path and test local. |
| `handle_hit` `editor.rs:491`, `which_handle` `editor.rs:515` | same — handles are drawn at world positions, so hit them in world. |
| `path_in_rect` (marquee) `editor.rs:649` | transform outline to world before the rect test (`point_in_path` centre test also needs a local-mapped centre). |

### 2c. Bounding boxes
| Site | Change |
|---|---|
| `outline_bbox` `model.rs:682` | world bbox = min/max over `xform.apply(outline point)`. This is the **align/distribute/snap/board-membership** bbox, so fixing it here fixes many consumers at once. |
| `bbox` (anchor+handle box) `model.rs:717` | world bbox over transformed anchors/handles. Used by shape-cleanup on `pointer_up` (`editor.rs:2806`) and PDF cull. |
| `obj_bbox` `editor.rs:548` | world AABB of the selection (used by panel X/Y and `set_obj_bbox`, `flip`, `pivot_point`). Transform outline to world. |
| `path_boards` `model.rs:512`, `node_boards` `model.rs:526` | already via `outline_bbox` → fixed for free once that composes. |

### 2d. Transform frame / handles — `editor.rs`
`obj_local_bbox` (`editor.rs:570`) currently un-rotates by `-obj_angle`. In the new model, **anchors are already local**, so for a single-unit selection `obj_local_bbox` = the plain bbox of local anchors (no un-rotation), and `frame_handles`/`frame_corners` rotate local→world by the **unit's stored `θ`** (read from `xform`, not the transient field). `start_transform` (`editor.rs:696`), `transform_hit` (`editor.rs:661`), `pivot_point` (`editor.rs:693`) follow. For a **multi-unit** selection there is no common local frame → frame is axis-aligned (`θ = 0`), exactly today's "selecting multiple resets `obj_angle`."

### 2e. Snapping — `editor.rs`
| Site | Change |
|---|---|
| `snap_lines_ex` `editor.rs:1700` (object_bounds/bbox_mids candidates) | uses `outline_bbox` → fixed once §2c composes. |
| key-point / geometry / segment-mid candidates `editor.rs:2228, 2270, 2327` | candidate anchor points must be **world** (`xform.apply`) so you snap to where the rotated anchors actually are. |
| `snap_xy`/`snap_anchor` `editor.rs:1970/2310` | operate on the world base points already produced by the drag; no change if the base is world (Stage 4 keeps the drag base in world for move). |

### 2f. Export — `varos-pdf/src/lib.rs`
| Site | Change |
|---|---|
| `emit_ring` `lib.rs:288` | reads `a.p / a.hout / a.hin` directly → apply `unit_xform` before the world→page map `t`. Cleanest: have the caller pass world anchors, or thread a `&Xform` and pre-map. Cubics are affine-invariant, so mapping control points by the same rotation is exact. |
| `emit_rings` `lib.rs:282` | pass the unit xform down. |
| `world_bbox`/`bbox_hits`/`page_bbox` `lib.rs:310–332` | cull box must be the **world** (rotated) bbox → apply xform to anchors/handles first. |
| main artwork loop `lib.rs:122` | it already iterates `doc.paint_list()`; add `let xf = doc.unit_xform(p.id);` and thread it into `emit_rings` + the bbox helpers. |

**No SVG exporter exists** (save is PDF-native only, `SAVE_EXPORT_PLAN §1`). If/when SVG lands it composes the same way (emit a `transform="rotate(...)"` or bake — bake is simplest and matches the PDF path).

### 2g. Groups
A group's transform is the **unit node's `xform`**, composed over its children automatically because every child path resolves its transform through `unit_xform(pid)` → the same top-level Group node. Leaf nodes inside the group stay identity. `clip_map` already keys on this unit (`scene.rs:144–174`), so clipping and transform share the same unit boundary.

---

## 3. Every mutating op — local geometry vs. the transform vs. bake

| Op | Site | Rule |
|---|---|---|
| **Move** (Object drag) | `editor.rs:3131`, `translate_path` `editor.rs:722` | Edits **local** anchors (translate) **and** carries the pivot: `xform.translated(d)` (`xform.piv += d`). Translation commutes with rotation, so translating local anchors + pivot leaves `θ` untouched and the world result correct. |
| **Rotate** (drag / `set_obj_rotation`) | `editor.rs:3191 / 1169` | **Edits the transform**, not geometry: `unit.xform = unit.xform.then_rotate(dθ, pivot)`. Anchors are untouched. This is the crux of A7. |
| **Scale handle / Scale tool** | `editor.rs:3141 / 3236` | **Bakes into local** geometry: map cursor→local (`inverse_apply`), scale local anchors on the frame's local axes (the code already un-rotates by `angle` — now `angle` = the unit's `θ`), keep `θ`. W/H stays exact because scale rewrites the local bbox. |
| **Numeric W/H / X/Y** | `set_obj_bbox` `editor.rs:1130` | Bakes into local (scale local anchors); operates on the **local** bbox now, not the world AABB. Keeps `θ`. |
| **Align / distribute** | `editor.rs:957/980`, `align_anchors:845` | Translation only → same as Move: shift local anchors + pivot, keep `θ`. Uses `outline_bbox` (now world) so alignment is by the true rotated extent (Illustrator). |
| **Flip** | `editor.rs:1107` | Bakes into local (reflect local anchors about local centre); negate `θ` so the visual flip is correct with the live rotation. |
| **Boolean (Pathfinder)** | `editor.rs:769`, `path_to_segs:738` | **BAKES** — see §4. Participants are flattened to world segs (apply `unit_xform`) before the boolean engine; the result is fresh axis-aligned geometry (`θ = 0`). |
| **Direct-select** (anchor / handle / segment) | `direct.rs`, `Drag::Anchors/Handle/Segment`, `begin_anchor_drag:2638`, `ConvPull` | Edits **local** anchors. Because the frame is fixed (`θ, piv` frozen), moving one local anchor moves only that anchor's world image — no swim. The white arrow grabs anchors at their **world** positions, so the down-hit must map the cursor to local (§2b) and the drag delta must be rotated into local (`inverse_apply` on the delta direction) so dragging feels 1:1 on screen. |
| **Duplicate** (`dup_paths` / `clone_path`) | `model.rs:1008 / 770` | Copies must inherit the unit's `xform`. `dup_paths` mirrors the node subtree — set the mirrored unit node's `xform` from the source unit. `clone_path` (flat copy → new leaf) must copy the source unit's `xform` onto the new leaf so an Alt-drag copy keeps its rotation. |
| **Transform Again** (Ctrl+D) | `editor.rs:1223` | For a remembered **rotate**, apply `then_rotate` to the (new) unit's transform instead of baking; move/scale unchanged. |

---

## 4. The bake path (already exists)

Baking = compose the stored transform into the coordinates, then reset it to identity. **This is literally today's rotate code**, just triggered later:

```rust
// pseudo — Editor::bake_unit(unit_node)
let xf = node.xform;
if xf.is_identity() { return; }
for a in anchors_of(unit) {           // outer + holes of every path in the unit's subtree
    a.p    = xf.apply(a.p);
    a.hin  = a.hin.map(|h| xf.apply(h));
    a.hout = a.hout.map(|h| xf.apply(h));
}
node.xform = Xform::default();        // reset to identity
```

The body is exactly `Drag::Rotate` (`editor.rs:3198–3204`) / `set_obj_rotation` (`editor.rs:1183–1189`). Triggers:
- **Expand** / **Apply** (an explicit menu action; add if not present) → `bake_unit`.
- **Boolean** (`pathfinder`, `editor.rs:769`) → participants are baked *implicitly* by emitting world segs in `path_to_segs` (`editor.rs:738`); results are new identity-`θ` paths.
- **merge/flatten** and any op that needs raw world geometry can call `bake_unit` first.

Because baking reuses the proven rotate math, Stage 7 is low-risk.

---

## 5. SAFE STAGING ORDER (the important part)

**Invariant that makes this safe:** the stored transform is **identity for every unit until Stage 4.** Stages 0–3 add the field, the `Xform` type, the `unit_xform` seam, and route every reader through it — but since `apply(identity) == p`, behaviour is **byte-identical**. The **entire existing test suite** (`transform.rs`, `align.rs`, `snap.rs`, `boards.rs`, `boolean_corners.rs`, `serde_roundtrip.rs`, `golden.rs`, `occlusion.rs`, `layers.rs`, …) is the gate for each of Stages 1–3: it must stay 100% green with zero edits. Only Stage 4 flips a behaviour, and by then every reader already composes, so the whole app moves together.

### Stage 0 (optional interim, ship-first candidate) — **persist + show the angle; geometry still baked**
- **Change:** add `Node.xform` (serde-default) and, on a rotate, ALSO record `θ` into the unit's `xform.rot` **without** removing the bake. Panel reads angle from the stored `xform` of the selected unit instead of the transient `obj_angle`. W/H **stays wrong** (still the AABB of baked geometry).
- **Breaks:** almost nothing — geometry path is unchanged; only a new serialized field + a panel read.
- **Proof:** rotate a rect 30°, deselect, reselect → the **angle field still reads 30°** (today it reads 0). Save/reload → still 30°. `serde_roundtrip` green.
- **Value:** delivers the *"values don't reset"* half of A7 immediately and de-risks the serde/format change in isolation. **Recommended as the first shippable slice** (see §6).

### Stage 1 — field + type + seam, identity everywhere
- **Change:** land `Xform`, `Node.xform` (`#[serde(default, skip_serializing_if)]`), `Document::unit_xform`, `world_outline_px`/`world_ring_px` helpers. **No consumer uses them yet.** No behaviour change.
- **Breaks:** only compile risk (PartialEq/serde on `Node`).
- **Proof:** whole suite green; `serde_roundtrip` proves old files load and un-rotated new files serialize without the field.

### Stage 2 — render composes it
- **Change:** point `scene.rs` fills/strokes/overlay/skeleton/anchor-markers and PDF `emit_rings` at the world variants (§2a, §2f).
- **Breaks:** if a render site is missed it draws un-rotated — but with identity transform there is literally nothing to miss visually yet; the risk is *latent* until Stage 4.
- **Proof:** suite green; `golden.rs` / visual scene tests unchanged (identity ⇒ identical prims). PDF `container.rs`/`open_fill.rs` green.

### Stage 3 — hit-test + bbox compose it
- **Change:** `path_under`, `nearest_anchor`, `handle_hit`, `path_in_rect` pre-map the cursor; `outline_bbox`, `bbox`, `obj_bbox`, and the frame math (§2b–2d) transform to world. Snap candidates become world (§2e).
- **Breaks:** the split-brain class (draw vs. click mismatch) — but still masked by identity.
- **Proof:** `snap.rs`, `align.rs`, `boards.rs`, `occlusion.rs`, `input_feel.rs`, `transform.rs` all green (identity ⇒ same numbers).

### Stage 4 — rotate WRITES the transform instead of baking (**the flip**)
- **Change:** `Drag::Rotate` (`editor.rs:3191`) and `set_obj_rotation` (`editor.rs:1169`) set `unit.xform = xform.then_rotate(dθ, pivot)` and **stop** rewriting anchors. Move carries the pivot (`translated`). Frame angle now derives from the unit's stored `xform` on selection change (replace the ~16 `obj_angle = 0.0` resets with `obj_angle = self.sel_stored_angle()`); keep `obj_angle` only as a per-frame cache.
- **Breaks:** this is where real behaviour changes. Highest-risk stage. Direct-select drag deltas must be rotated into local (§3). Multi-select must fall back to axis-aligned.
- **Proof:** a **new** test `live_rotation_persists`: rotate a rect 30°, `commit`, clear+rebuild selection → `sel_stored_angle()≈30°` AND every anchor's *stored* `p` is unchanged (proof geometry was **not** baked) while `unit_xform.apply(p)` matches the old baked coordinates. Re-run `transform.rs` (adapt the asserts that read baked `anchor.p` to read `unit_xform.apply(anchor.p)`). Rotate → click another object → click back → angle still 30°.

### Stage 5 — panel shows TRUE W/H (+ angle)
- **Change:** `Snap::read` (`ui.rs:533`) reads W/H from the selected unit's **local** bbox (un-rotated) and angle from its `xform.rot` (`ui.rs:566`). `set_obj_bbox`/`set_obj_rotation` already updated in Stages 3–4.
- **Breaks:** panel-only; X/Y semantics for a rotated object (Illustrator shows the local bbox's transformed reference point — pick and document one: world AABB top-left is simplest and matches `obj_bbox`).
- **Proof:** rotate a 100×40 rect 90° → panel reads **W 100 / H 40** (today: W 40 / H 100), angle 90°. Deselect/reselect/reload → unchanged.

### Stage 6 — export parity
- **Change:** confirm `varos-pdf` (Stage 2 already threaded `unit_xform` into `emit_rings` + cull) round-trips a rotated object: the embedded model keeps live `θ`; the rendered PDF page shows the rotation.
- **Breaks:** cull box if the world bbox wasn't rotated (clipped-away artwork). Knockout/opacity XObject bbox (`lib.rs:327`) must use the rotated world bbox.
- **Proof:** extend `container.rs` — save a rotated rect, reload → `θ` preserved in the model; rasterise/inspect the page stream shows rotated coordinates; the object is not culled.

### Stage 7 — Expand / merge bakes
- **Change:** add `bake_unit` (§4); wire Expand/Apply to it; make `pathfinder`/`path_to_segs` emit world segs so booleans bake participants; result paths are identity-`θ`.
- **Breaks:** boolean on rotated inputs if `path_to_segs` forgot to apply the transform (wrong geometry fed to `flo_curves`).
- **Proof:** new test — rotate two overlapping rects, `Unite` → result matches the union of the **rotated** shapes and the result unit's `xform` is identity; `boolean_corners.rs` still green. Expand a rotated object → `θ` becomes 0 and stored anchors now equal the old baked coordinates.

---

## 6. Risks & fallback

### Biggest risk — **split-brain between drawn and interactive geometry**
Many sites read `anchor.p`/`outline*` directly across `model.rs`, `editor.rs`, `scene.rs`, `varos-pdf`. If Stage 2/3 misses even one, that subsystem sees un-rotated geometry while others see world — you click where the object *looks* but nothing is hit (or the marquee/snap/align/export disagree with the canvas). This is the single most likely failure and it is **invisible until Stage 4** because everything is identity before then.

**Mitigations (all structural, not vigilance):**
1. **One seam.** Every read goes through `Document::unit_xform` + the `world_outline_px`/`world_ring_px` helpers. Grep the tree for `\.p\b`, `\.hin`, `\.hout`, `outline`, `outline_px`, `outline_bbox`, `ring`, `ring_px`, `bbox(` and confirm each is either (a) inside the seam, (b) a bake site, or (c) genuinely local. Keep that checklist in the PR.
2. **Identity-until-Stage-4** so Stages 1–3 are provably neutral — the existing suite is a hard gate with **zero** test edits allowed.
3. A **debug assertion** (test-only) that renders a rotated fixture and cross-checks `path_under(center_of_drawn_bbox)` returns that path — a direct split-brain tripwire, added in Stage 4.

### Other risks
- **`obj_angle` reset sites (~16).** Mechanical but wide. Replace `= 0.0` with `= self.sel_stored_angle()`. Risk: miss one and the frame shows the wrong angle after that op. Contained to Stage 4; covered by the reselect test.
- **Direct-select feel.** Drag deltas must be rotated into local or the point runs off-axis under the cursor. Covered by an `input_feel`-style test in Stage 4.
- **Pivot composition.** `then_rotate` must handle the `θ₁+θ₂ ≡ 0` degenerate (pure translation) by baking the residual translation into anchors. Unit-test `then_rotate` directly in Stage 1 (pure math, no UI — allowed by the math-test rule).
- **Multi-select of differently-rotated units.** Define now: axis-aligned frame, `θ = 0` shown, rotating the multi-selection writes each unit's own transform about the shared world pivot. Simplest correct behaviour; matches Illustrator.

### Fallback / interim
**Yes — ship Stage 0 first.** It delivers the *"transform values don't reset"* half of A7 (angle persists through reselect + save/reload) with almost no blast radius, and it de-risks the serde/format addition in isolation. W/H stays wrong until Stage 5, which is acceptable as an interim and is exactly Ahmed's option (b) — but because Stage 0 lays the **same** `Node.xform` field the full plan uses, it is *not* throwaway: Stages 1–7 build straight on top of it. If the full live-rotation work must pause, Stage 0 is a coherent stopping point that already improves the pain.

---

## Appendix — key file:line index
- Model: `Anchor` `model.rs:13`; `Path` `model.rs:91`; `Node`(+`clip_exempt`) `model.rs:186/212`; `unit_of` `model.rs:865`; `outline`/`outline_px`/`ring`/`ring_px` `model.rs:673/677/628/647`; `outline_bbox` `model.rs:682`; `bbox` `model.rs:717`; `point_in_path` `model.rs:700`; `nearest_seg` `model.rs:578`; `edge_dist` `model.rs:604`; `path_boards`/`node_boards` `model.rs:512/526`; `clone_path` `model.rs:770`; `dup_paths` `model.rs:1008`.
- Editor: `obj_angle` field `editor.rs:350`; `Drag::Rotate/Scale/ScaleLive/TfPending` `editor.rs:87–90`; `path_under` `:476`; `nearest_anchor` `:453`; `handle_hit` `:491`; `obj_bbox` `:548`; `obj_local_bbox` `:570`; `frame_handles/corners` `:600/610`; `transform_hit` `:661`; `start_transform` `:696`; `translate_path` `:722`; `pathfinder` `:769`; `path_to_segs` `:738`; `flip` `:1107`; `set_obj_bbox` `:1130`; `set_obj_rotation` `:1169`; `transform_again` `:1223`; align `:941/957/980/845`; `snap_lines_ex` `:1700`; snap candidates `:2228/2270/2327`; `begin_anchor_drag` `:2638`; `pointer_up` `:2796`; `Drag::Rotate` apply `:3191`; `Drag::Scale` apply `:3141`; `begin/commit` `:2431/2435`.
- Scene: fills/strokes `scene.rs:185/216`; overlay skeleton `:312`; frame `:342`; handle/anchor markers `:436/455`.
- Export: `emit_ring(s)` `varos-pdf/src/lib.rs:282/288`; cull `:310–332`; artwork loop `:122`.
- Panel: `Snap::read` W/H+rot `ui.rs:533/566`; `Op::SetBBox/SetRot` dispatch `ui.rs:5290/5291`; Transform block `ui.rs:4302`.
- Serde/format: `file.rs` (`doc_to_blob`/`doc_from_blob`, `#[serde(default)]` hygiene); `Paint` hand-serde `model.rs:59`.
