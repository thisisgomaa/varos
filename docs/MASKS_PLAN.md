> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Masks / Clipping — STAGED implementation plan

> **Feature:** clipping masks (Ahmed's word: **"Mask"**). The next major feature after the pain sweep + A7.
> **Design of record:** `LAYERS_VISION.md` §3 (the canonical model + the two gestures) and §5–§7 (the
> `paint_list` insurance). This doc turns that design into a code-grounded, stage-by-stage build order.
>
> **The thesis, unchanged from §3:** ONE stored clip form — a `GroupRole::Clip` node with an authoritative
> `mask_child: Option<u32>` — surfaced by TWO gestures (drag-onto-thumbnail + Ctrl+Alt+G), rendered through
> the `paint_list` / `PaintRole` seam that was **already built and wired** as anti-rewrite insurance.
>
> Written to be executed **stage by stage with a gate between each** — every stage compiles, keeps the whole
> suite green, and leaves the app fully usable. The **clip gesture does not exist until Stage 3**, so Stages
> 1–2 are plumbing with **zero** user-reachable behaviour change (nobody can make a clip yet), exactly the
> A7 "identity-until-the-flip" discipline.

---

## 0. Where we are today (verified in code)

The insurance seam is **already built and live** — this is the single biggest de-risker:

- `PaintRole { Normal, MaskSource, Hidden }` — `model.rs:221`. Runtime-only, never serialized.
- `Document::paint_list()` — `model.rs:554` — **already filters `MaskSource` out** of the content run
  (`filter(|(_, p)| self.paint_role(p.id) != PaintRole::MaskSource)`), and yields `(paths-index, &Path)`.
- `Document::paint_role(pid)` — `model.rs:559` — **always returns `Normal` today** (the one function masks flip on).
- **Every paint consumer already reads `paint_list()`, not `doc.paths`:** the scene content loop
  (`scene.rs:257`), PDF export (`varos-pdf/src/lib.rs:122`), and snap targets (`editor.rs:1714`). So making
  `paint_role` return `MaskSource` for a mask instantly and correctly excludes it from all three — no call-site sweep.
- The mask-era test is **already stubbed and waiting**: `tests/layers.rs:257`
  (`paint_list_is_exactly_doc_paths_until_masks_land`) is the golden the mask-exclusion test diffs against.

What does **not** exist yet (this plan adds it):
- `GroupRole` enum, `Node.role`, `Node.mask_child` — **absent** (grep confirms only the doc-comment mentions them).
- Any **shape-based** clip. The only clip in the engine is the **artboard RECT clip** (`scene.rs:143` `clip_map`
  / `clip_rects`, cutting art to a page rectangle via `clip_poly_rect`/`clip_polyline_rect` + a GPU scissor).
  A mask clips to an **arbitrary path silhouette**, not a rect.
- Any tree-walk in `build_scene` — it is still a **flat loop over `paint_list()`** (`scene.rs:257`).

The precedents masks copy exactly:
- **`Node.clip_exempt`** (`model.rs:258`, A30) and **`Node.xform`** (`model.rs:266`, A7) are the two existing
  `#[serde(default)]` **per-unit** fields keyed on `unit_of(pid)` (`model.rs:942`). `role`/`mask_child` are the third.
- **`Node.xform` / `unit_xform`** (A7) already funnel every WORLD-geometry read through one seam
  (`world_outline_px` `model.rs:736`) — masks read the **same world geometry**, so a rotated mask composes for free.

---

## 1. Model — the ONE canonical form (confirm §3.1, refined)

### 1.1 The fields (append to `Node`, `model.rs:231`; all serde-defaulted)
```rust
// ── clipping (append after xform, model.rs:266) ──
#[serde(default)] pub role: GroupRole,          // Normal unless a clip group
#[serde(default, skip_serializing_if = "Option::is_none")]
pub mask_child: Option<u32>,                    // AUTHORITATIVE mask node id; the OTHER children clip to it
```
```rust
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, Default)]
pub enum GroupRole {
    #[default] Normal,
    Clip,          // NOW: hard vector clip (silhouette)
    MaskAlpha,     // FUTURE §7.1 — parsed, treated as Normal until soft masks ship
    MaskLuma,      // FUTURE §7.1
}
```
- **On `Node`, at the Group, keyed by the unit** — mirrors `clip_exempt`/`xform`. A clip is a property of a
  `NodeKind::Group` node (`role = Clip`). `mask_child` is a **child node id** (a Path leaf now; a Group later).
- **`mask_child` is a STORED id, not "whatever is back-most"** (critique BLOCKER-5, §3.1). Creation places the
  mask as the back-most child (`children.last()` — children are FRONT-first), but from then on **the id is
  truth**: reordering the group's children does NOT reassign the mask.
- **What the clip group stores:** `{ role: Clip, mask_child: Some(m), children: [.. m ..] }` where `m`'s
  subtree = the mask shape, and every OTHER child's subtree = the clipped content. Byte-identical to the
  Illustrator `<Clip Group>` / the PDF-native form → round-trips into `.vrs` with zero translation.

### 1.2 Serde / back-compat
- `#[serde(default)]` ⇒ every pre-mask `.vrs` loads as `role: Normal, mask_child: None` — unchanged.
- `Node` derives `PartialEq` and is compared in `serde_roundtrip.rs`; a defaulted field is compatible (the same
  proof A7's `xform` and A30's `clip_exempt` already passed).
- **No `.vrs` format bump.** `GroupRole` name-keys; `mask_child` is `skip_serializing_if None`, so un-clipped
  new files stay byte-clean. Runtime `Group`/`GroupDraw` clip variants are never serialized.

### 1.3 New model ops (thin wrappers — the topology stays the existing `group()` tree)
```rust
// model.rs — the ONE stored form, two sugars desugar into it
pub fn clip_group(&mut self, pids: &[u32], mask_pid: u32) -> Option<u32>;  // group() + role=Clip + mask_child
pub fn release_clip(&mut self, clip_nid: u32);                            // role=Normal, mask_child=None
pub fn clip_group_of(&self, pid: u32) -> Option<u32>;                     // the nearest Clip ancestor (render/hit)
pub fn is_mask_source(&self, pid: u32) -> bool;                          // pid ∈ some clip's mask_child subtree
```
- `clip_group` reuses `group()` (`model.rs:1003`) verbatim, then sets `role`/`mask_child` on the returned node —
  **`group()` itself is untouched** (§3.4: "one grouping model, forever"; never reintroduce a `group_of` side-table).
- Illustrator rule (Ctrl+Alt+G): `mask_pid` = the **front-most (top) selected unit**; it becomes the mask of the
  ones below.

---

## 2. How `paint_list` / the scene surfaces it (§2, §5)

### 2.1 `paint_role` computes `MaskSource` (the flip — `model.rs:559`)
```rust
pub fn paint_role(&self, pid: u32) -> PaintRole {
    if self.is_mask_source(pid) { PaintRole::MaskSource } else { PaintRole::Normal }
}
```
Every path whose leaf is inside some clip group's `mask_child` subtree becomes `MaskSource`. Consequences,
**all automatic** because the three consumers already read `paint_list()`:
- **Excluded from the content/paint run** (`scene.rs:257`), from **export** (`pdf lib.rs:122`), from **snap
  targets** (`editor.rs:1714`).
- **Still present in `doc.paths`** for hit-test (`path_under` `editor.rs:476` iterates `doc.paths`), the Layers
  panel, rename, and the thumbnail — the mask stays a **first-class, clickable, renamable row** (§3.1).

`is_mask_source` is cheap (walk each clip node's `mask_child` subtree once, cache a `HashSet<u32>` per
`sync_tree`); recompute it where `sync_tree` recomputes the tree so callers never see a stale set.

### 2.2 What the scene must EMIT — the shape clip (the new work)
`paint_role` hides the mask, but it does **not** clip the content. The content run must additionally learn,
per member path, **which mask shape cuts it**. Add a per-path lookup parallel to the existing `clip_rects`
(artboard rect) and `unit_xform` (A7):

```rust
// scene.rs, alongside clip_map/clip_rects (scene.rs:143–183)
// mask_of(pid) -> the WORLD outline rings of the clip shape that clips pid, or None
```
built once per frame from `doc.clip_group_of(pid)` → the clip's `mask_child` → `world_outline_px` of its
path(s) (A7-composed, so a rotated clip's mask rotates for free).

### 2.3 The runtime scene seam (`scene.rs` `Group` enum, `:43`)
Add a clip variant so the renderer receives clip units as **self-contained, z-contiguous** blocks:
```rust
pub enum Group {
    Opaque(Vec<Prim>),
    Knockout(Vec<Prim>),
    Isolated { opacity: f32, prims: Vec<Prim> },
    Clip { mask_rings: Vec<Vec<Pt>>, members: Vec<Group> },  // members are Groups → nested clips recurse
}
```
- `mask_rings` = the mask silhouette in **world** space (outer + holes), even-odd.
- `members` are the already-built member Groups (Opaque/Isolated/Knockout), so a translucent or knocked-out
  object inside a clip keeps its exact existing treatment — the clip just wraps them.
- **The clip is renderer-agnostic** (the HARD SEAM promise): the core emits `Group::Clip`; whether the wgpu
  backend clips via stencil or offscreen-multiply (§3) is a `varos-render-wgpu` decision the core never sees.

### 2.4 Building `Group::Clip` — the flat loop, not a rewrite (recommended for NOW)
`build_scene` stays the **flat `paint_list()` loop** (`scene.rs:257`). Because a subtree is **contiguous** in
`doc.paths` after `flatten()` (pre-order DFS) and the mask is excluded, a clip group's non-mask members appear
as a **contiguous run** in `paint_list()`. Track "current clip = `clip_group_of(pid)`"; when it changes, flush
the accumulated member Groups into a `Group::Clip { mask_rings, members }`. This **reuses every existing
per-object branch** (opacity → `Isolated`, translucent stroke → `Knockout`) unchanged — they just accumulate
into the clip's `members` vec instead of the top-level `groups` vec.
- **NOW = single-level clips** (one `Clip` not inside another). This covers 100% of the two gestures.
- **Nested clips** (a `Clip` whose member is itself a `Clip`) need a small clip **stack** in the loop, or the
  `emit_node(nid) -> Vec<Group>` recursive tree-walk the vision sketches (§6.3). Deferred to **Stage 6** so the
  common case ships first. The `members: Vec<Group>` shape already admits the recursion with no seam change.

---

## 3. Render — the clipping mechanism (recommendation + hooks)

Two mechanisms fit; **both live behind the same `Group::Clip` core seam**, so the choice is purely a
`varos-render-wgpu` implementation detail.

### 3.1 Recommendation — **GPU stencil, a dedicated clip bit `0x02`** (matches §3.3, lightest for hard clips)
The stencil buffer already partitions its bits: **`0x01` = fill even-odd parity, `0x80` = knockout band**
(`lib.rs:280`). **`0x02` is free.** A hard vector clip is exactly a stencil test:

1. **Fan the mask** rings into bit `0x02` with even-odd (`Invert`), colour off → bit `0x02` = 1 inside the mask
   silhouette. New pipeline `pipe_mask_fan` = clone of `pipe_stencil` (`lib.rs:320`) with `read_mask/write_mask
   = 0x02`. (Unlike the fill cover, we do **not** clear it — it must persist while members draw.)
2. **Draw the members** with their existing draw steps, but each member pipeline gains a **clip-test variant**
   that adds `compare` requiring `(stencil & 0x02) == 0x02`. Members outside the mask fail the test → not drawn.
3. **Clear** bit `0x02` (a cover quad over the mask bbox writing `0x02→0`) so the next group starts clean.

**Cost, stated honestly:** members reuse `pipe_stencil/pipe_cover/pipe_main` (+ the 3 knockout pipes); each
needs a `_clip` variant whose stencil state also tests `0x02`. That is ~4–6 extra `make_pipe` calls at init
(`lib.rs:318–324`). Mechanical, and the stencil-state combos are the risk (see §7). Hooks:
- Stencil states: `lib.rs:283–317`; pipeline construction: `lib.rs:318–324`; pipeline fields: `lib.rs:33–37`.
- Draw dispatch: `Renderer::draw_steps` (`lib.rs:571`) gains a `Draw::MaskFan`/`Draw::MaskClear` and clip-test
  branches; `record_scene` (`lib.rs:673`) handles a new `GroupDraw::Clip`.
- Tessellation: `tess.rs` `Group`/`GroupDraw` (`:278`) + `build_content` (`:333`) emit the mask fan (reuse
  `build_fills` `:167`) + the members + the clear quad.

### 3.2 Alternative — **offscreen layer × mask coverage** (reserve for FUTURE soft masks §7.1)
Render the clip's members into the existing isolated-layer offscreen (`layer_msaa`/`layer_view`, `lib.rs:53`),
render the mask into a **second** coverage texture, then composite `member × mask.alpha` onto the scene — a
clone of the proven `GroupDraw::Layer` path (`lib.rs:692`) + `pipe_composite` (`lib.rs:421`) with a
two-texture shader. **Pros:** no per-member pipeline explosion; **is exactly the machinery alpha/luma soft
masks need** (a soft mask multiplies by luminance/alpha instead of a hard 0/1 coverage). **Cons:** an extra
offscreen target + resolve per clip group; heavier than a stencil test for a pure hard edge.

**Decision:** ship **stencil (`0x02`)** for the NOW hard vector clip (lightest, no new textures, matches the
design of record). When soft masks (§7.1) land, add the offscreen-multiply path; the two coexist behind
`Group::Clip` (hard) vs a future `Group::SoftMask` (soft). If stencil bit-interplay proves too fragile in
Stage 2 (§7), fall back to offscreen-multiply — same core seam, no re-plumb.

---

## 4. The two gestures (UI) — §3.2

Both produce the **byte-identical tree** (`clip_group`), never two topologies (§3.4).

### 4.1 Primary — Ctrl+Alt+G / right-click "Make Clipping Mask"
- **Shortcut:** `main.rs:172` — the `"KeyG"` arm already branches `shift → ungroup`, else `group`. Add the
  `alt` axis (already a param of `apply_key`, `main.rs:149`):
  ```
  "KeyG" => if ctrl && alt { ed.make_clip_selection() }   // Ctrl+Alt+G
            else if shift  { ed.ungroup_selection() }
            else           { ed.group_selection() }
  ```
- **Editor op** (new, mirrors `group_selection` `editor.rs:1202`): require `objsel.len() ≥ 2`; `begin()`;
  `mask = front-most selected unit` (top of z); `doc.clip_group(&pids, mask)`; `dirty = true`; `commit()`.
  A **second** Ctrl+Alt+G when the selection is already a clip → `release_clip` (toggle, §3.2).
- **Right-click menu:** add *"Make Clipping Mask"* / *"Release Clipping Mask"* to the canvas + panel context
  menus (same op). Mirror the existing enable/disable gating pattern of `sel_clip_exempt` (`editor.rs:3648`).

### 4.2 Secondary — drag a row onto another row's THUMBNAIL
- **Where:** the panel drop-zone logic (`ui.rs:3930–3954`) currently classifies the whole row into zones
  `0 Before / 1 Into / 2 After / 3 cross-board`. The thumbnail sub-rect is computed at `ui.rs:4051–4056`
  (`row.thumb`). Add a **distinct target**: when the drop pointer is over the target row's **thumbnail rect**
  specifically (a small, deliberate zone — *not* the middle "Into" sliver), classify as a **new mask zone**.
- **Rule (§3, safety):** thumbnail-drop **onto a leaf/shape** → make a clip (dragged art clipped inside that
  shape). The row's **middle "Into" zone stays a plain group, always** (§3.2 rejected auto-clip). Show a
  **distinct mask indicator** (e.g. a filled accent thumbnail ring, not the plain nest box at `ui.rs:4192`) so
  the user always sees mask-vs-nest before releasing.
- **Op:** new `Op::LayerClip(payload, target_row)` (alongside `LayerMove` `ui.rs:112`); dispatch (near
  `ui.rs:5316`) → `ed.clip_rows(payload, target)` → `doc.clip_group`. Release by dragging the clipped child
  back out to a Before/After zone (already a `LayerMove` — the model's `move_node_to` pulling the last non-mask
  child out empties/dissolves the clip via `sync_tree`).

### 4.3 Release
Ctrl+Alt+G again · right-click *"Release Clipping Mask"* · drag the clipped child out (§4.2). **Ungroup
releases the clip** (§8.3, confirmed): `ungroup` (`model.rs:1067`) must clear `role`/`mask_child` on the
dissolved node so nothing dangles (§6 audit).

---

## 5. Every subsystem masks touch

| Subsystem | Site | What masks require |
|---|---|---|
| **Render** | `scene.rs:184/216/257`, `tess.rs`, `lib.rs` | §2–§3. Content run emits `Group::Clip`; wgpu clips to the mask silhouette (stencil `0x02`). |
| **Hit-test** | `path_under` `editor.rs:476` | A click on **clipped-away** content must **MISS**. Today it tests each path's own geometry. Add: if `pid` is a clip member, additionally require the point (in the clip unit's local frame) to lie **inside the mask region** (`point_in_path` against the mask). Outside the mask ⇒ the member is not touched ⇒ z-walk continues to what's below (exactly the "unfilled cover is click-through" rule, `occlusion.rs:90`). The **mask's own row** is excluded from paint but still in `doc.paths`: a click inside the clip result selects the clip **group** (Illustrator), not the invisible mask. |
| **bbox** | `outline_bbox` `model.rs:757`, `obj_bbox` `editor.rs:551` | The clip **unit's** visible extent can't exceed the mask. Define the clip group's effective bbox = **the mask path's bbox** (Illustrator: the clip path defines the group bounds). Members keep their raw geometry bbox individually; the **unit-level** bbox used by align/snap/board-membership uses the mask. One change in the unit-bbox helper, read by many. |
| **Board membership** | `node_boards` `model.rs:580` | Follows `outline_bbox`. For a clip unit, membership = the **mask's** boards (the visible clipped art), so a member overhanging the mask doesn't drag the unit onto an extra page. Free once the unit bbox uses the mask (row above). |
| **Export (PDF)** | `pdf lib.rs:122` (loop), `emit_rings` `:282` | The mask is already excluded (paint_list). For each clipped member, wrap its paint in `q … Q` and set the **PDF clip path** = the mask outline via `emit_ring(mask); W* n;` (even-odd) **before** the member's fill/stroke. Native, exact, no rasterization. One clip group → one `q/W*/…/Q` block around its members. |
| **Artboard clip (A2/A30)** | `clip_map`/`clip_rects` `scene.rs:143`, `clip_exempt` `editor.rs:3634` | The mask AND the page rect **both** cut → content clipped to **mask ∩ artboard-rect**. Stencil: the member's draw already respects the artboard scissor (`Draw::Fg{scissor}` `tess.rs:248`) **and** now the `0x02` mask test — they AND naturally. `clip_exempt` (A30 bleed) releases **only the artboard rect**, never the mask (a mask is intentional, a page bleed is a per-object exception). |
| **A7 live-transform** | `unit_xform` `model.rs:949`, `world_outline_px` `model.rs:736` | **A rotated clip group:** the clip Group node IS the `unit_of` its members AND its mask (`top_group_of_path`), so both resolve the **same** `xform` → mask + content rotate as one rigid unit. Build `mask_rings` from `world_outline_px` (A7-composed) and it's automatic. **A mask on a rotated object / a rotated mask:** same seam — the mask geometry is always read in world. The only rule: **never build `mask_rings` from the local outline** (that is the A7 split-brain class, §7). Members hold identity `xform` (A7 writes only the unit), so no double-transform. |

---

## 6. STAGED order (each stage compiles, gates, ships)

**Safety invariant:** the **clip-creating gesture does not exist until Stage 3.** Stages 1–2 add the model,
the `paint_role` flip, and the render, but no user can construct a clip → **zero user-reachable change**. A
test can construct one directly to gate Stages 1–2. By Stage 3 both model and render are proven, so lighting
the gesture is safe. (Mirrors A7's "identity until Stage 4.")

### Stage 1 — model + `paint_role` flip + ops (no render)
- **Change:** add `GroupRole`, `Node.role`, `Node.mask_child` (§1.1); `clip_group`/`release_clip`/
  `clip_group_of`/`is_mask_source` (§1.3); make `paint_role` return `MaskSource` (§2.1); `sync_tree`
  (`model.rs:1358`) **validates `mask_child` is still a DIRECT child** of its clip group — demote to
  `role=Normal, mask_child=None` if the id vanished **or was re-parented away** (critique BLOCKER-5).
- **Could break:** `serde_roundtrip` (new field), the paint_list golden (`layers.rs:257`).
- **Test proves it:** update `layers.rs:257` per its own note — construct a clip group, assert the mask path is
  **excluded from `paint_list`** but **present in `doc.paths`** and hit-testable; assert a non-clip doc is still
  byte-for-byte `paint_list == doc.paths`; `serde_roundtrip` green (old files load `Normal`).
- **App state:** usable; making a clip (test-only) hides the mask, content paints **unclipped** — not
  half-broken because no gesture reaches it.

### Stage 2 — render clips to the mask
- **Change:** `Group::Clip` variant (`scene.rs:43`); the flat-loop clip accumulation (§2.4) building
  `mask_rings` from `world_outline_px`; `tess.rs` `GroupDraw::Clip` + mask fan/clear (§3.1); wgpu `0x02`
  pipelines + `draw_steps`/`record_scene` dispatch (`lib.rs:571/673`).
- **Could break:** the stencil-bit interplay (`0x02` vs `0x01`/`0x80`) — the top risk (§7); a missed member
  pipeline draws unclipped.
- **Test proves it:** a `tess.rs` CPU test (like `knockout_object_emits_band_fan_and_two_covers` `tess.rs:588`)
  asserting a clip group emits a mask fan + clipped member draws + a clear. `golden.rs`/scene tests unchanged
  for **non-clip** docs (a doc with no clip emits **no** `Group::Clip` → byte-identical prims). Visual: a rect
  clipped by a circle shows only the disc-shaped part.

### Stage 3 — the two gestures
- **Change:** Ctrl+Alt+G (`main.rs:172`) → `make_clip_selection`/toggle-release (§4.1); thumbnail-drop zone +
  `Op::LayerClip` (`ui.rs:3930/5316`) with the distinct mask indicator (§4.2); right-click menu items.
- **Could break:** gesture ambiguity (thumbnail zone vs Into sliver); the front-most-unit mask pick.
- **Test proves it:** editor test — select 2 rows, `make_clip_selection` → a `role=Clip` group whose
  `mask_child` = the front unit; the mask is `MaskSource`; the other paints clipped. Second Ctrl+Alt+G →
  `role=Normal`, both paint. Panel test — a thumbnail-drop builds the same tree as Ctrl+Alt+G (one topology).
- **App state:** masks are now a real, reachable feature — and render (Stage 2) already clips.

### Stage 4 — hit-test + bbox respect the mask
- **Change:** `path_under` mask-inside test (§5 hit-test); the clip-unit bbox = mask bbox (`outline_bbox`/
  `obj_bbox`/`node_boards`, §5).
- **Could break:** the draw-vs-click split-brain (click where clipped-away art *looks absent* but is still
  hit) — the exact bug this stage kills.
- **Test proves it:** `occlusion.rs`-style — click **inside** the mask over a clipped member → selects the clip
  group; click **outside** the mask but over the member's raw geometry → the member is **not** hit (falls
  through to what's below). Board-membership test — a member overhanging the mask does not add a page.

### Stage 5 — PDF export clip
- **Change:** wrap clipped members in `q … W* n … Q` with the mask outline (§5 export, `pdf lib.rs:122`).
- **Could break:** the cull box (`bbox_hits` `lib.rs:310`) must use the **mask** extent so a fully-clipped-out
  member isn't emitted; nested `q/Q` balance around knockout XObjects (`lib.rs:147`).
- **Test proves it:** extend `container.rs` — save a circle-clipped rect, reopen → `role=Clip`/`mask_child`
  preserved in the model; the page stream contains a `W* n` clip around the member; the member is not culled.
  Opens in Chrome/Acrobat showing the clipped shape.

### Stage 6 — interactions (nested clips, mask + artboard-clip, mask + rotation)
- **Change:** nested-clip support — a clip **stack** in the flat loop, or the `emit_node` recursive walk
  (§2.4); confirm mask ∩ artboard-rect compose (stencil `0x02` ∧ scissor, §5); confirm A7 rotation composes
  (mask built from world outline, §5).
- **Could break:** two stencil clip regions at once (nested) exceed one bit → needs a 2nd clip bit or the
  offscreen-recursion fallback; `clip_exempt` accidentally releasing the mask.
- **Test proves it:** nested clip (A clips B clips C) renders C cut by both; a rotated clip group — `path_under`
  at the drawn (rotated) location hits, and the PDF shows rotated clipped coords; a clip on a clipped page
  shows mask ∩ page.

---

## 7. Biggest risks & fallback

### Biggest risk — **stencil-bit interplay** (`0x02` clip × `0x01` fill × `0x80` knockout)
The knockout path already juggles two bits at once (`lib.rs:306–317`: fill parity `0x01` AND band `0x80` in one
object). Adding a **persistent** clip bit `0x02` that must survive across every member draw — while members
themselves fan `0x01` and knockouts mark `0x80` — is the subtle failure: a `read_mask`/`write_mask` that
accidentally touches `0x02` corrupts the clip mid-group, or a member pipeline that forgets the `0x02` test
draws outside the mask. **Mitigations:** (1) keep `0x02` strictly read-only for members (write only in mask-fan
and the final clear); (2) every member clip-test variant is a mechanical clone with `write_mask` **excluding**
`0x02`; (3) the Stage-2 `tess.rs` CPU test locks the emitted step sequence; (4) a rotated-fixture debug
assertion `path_under(center_of_clipped_region)` hits (the A7 tripwire pattern).
**Fallback:** the **offscreen layer × mask-coverage** path (§3.2) — the proven `GroupDraw::Layer` seam, no
bit-sharing at all — behind the same `Group::Clip` core. Ultimate fallback: **geometric intersection** (Clipper2
of member outline ∩ mask outline, exact, GPU-free) reusing the `clip_poly_rect` shape of the artboard clip.

### Other risks
- **Mask + A7 rotation split-brain.** If any site builds `mask_rings` from the local outline while content
  reads world, the clip cuts in the wrong place. **Structural fix:** mask geometry **only** ever via
  `world_outline_px`/`world_ring_px` — grep the mask builder for `outline`/`ring` and confirm each is a `world_`
  variant (same checklist A7 §6 uses).
- **Nested clips vs one stencil bit.** NOW supports one clip level (both gestures produce that). Nesting needs a
  2nd clip bit or offscreen recursion — fenced to Stage 6; `members: Vec<Group>` already admits it.
- **`sync_tree` mask-child desync.** Reorder/ungroup/dup must not dangle `mask_child`: `sync_tree` re-validates
  (Stage 1); `ungroup` clears the clip (§4.3); `dup_paths` (`model.rs:1093`) **remaps `mask_child` through its id
  map** (audit item, like it already remaps subtree ids). Covered by a groups-test.
- **Hit-testing the invisible mask row.** The mask is in `doc.paths` (for the panel) but must not be the click
  result on canvas — `path_under` skips a path that is a `MaskSource` for canvas picking, selecting the clip
  group instead. Explicit in Stage 4.

### Reconciliation rule (write it on the wall — §3.4)
**ONE stored form (parent-child clip group), two sugars (Ctrl+Alt+G, drag-onto-thumbnail).** Any future
Photoshop "clip to sibling-below" gesture **desugars** into this same tree — never a second grouping topology.
One grouping model, one ordering source (`children` position), one paint indirection (`paint_list`).

---

## Appendix — key file:line index
- **Insurance seam (built):** `PaintRole` `model.rs:221`; `paint_list` `model.rs:554`; `paint_role`
  `model.rs:559`; consumers `scene.rs:257`, `varos-pdf/src/lib.rs:122`, `editor.rs:1714`; golden test
  `tests/layers.rs:257`.
- **Model:** `Node` `model.rs:231` (+`clip_exempt` `:258`, `xform` `:266`); `NodeKind` `:202`; `unit_of` `:942`;
  `unit_xform` `:949`; `world_outline_px` `:736`; `outline_bbox` `:757`; `point_in_path` `:777`; `node_boards`
  `:580`; `group` `:1003`; `ungroup` `:1067`; `dup_paths` `:1093`; `sync_tree` `:1358`; `move_node_to` `:1487`.
- **Scene:** `Group` enum `scene.rs:43`; artboard `clip_map`/`clip_rects` `:143/178`; `fill_prims` `:184`;
  `stroke_prims` `:216`; content loop `:257`; `clip_poly_rect` `:548`.
- **wgpu:** pipe fields `lib.rs:33–37`; stencil states `:283–317`; pipe construction `:318–324`; `draw_steps`
  `:571`; `record_scene` `GroupDraw` handling `:673`; isolated-layer seam `layer_msaa`/`layer_view` `:53`,
  `pipe_composite` `:421`, `GroupDraw::Layer` `:692`. Tess: `Draw` `tess.rs:242`; `GroupDraw` `:278`;
  `build_content` `:333`; `build_fills` `:167`; `push_group` `:444`.
- **Editor / UI:** `path_under` `editor.rs:476`; `group_selection` `:1202`; `ungroup_selection` `:1214`;
  `set_clip_exempt` `:3634`; `begin`/`commit` `:2437/2441`. Op enum `ui.rs:104+`; drop zones `:3930`; thumbnail
  rect `:4051`; drop release `:4212`; Op dispatch `LayerGroup` `:5314`, `LayerMove` `:5316`. Shortcut
  `apply_key` `main.rs:149`, `KeyG` `:172`.
- **Export:** loop `varos-pdf/src/lib.rs:122`; `emit_rings`/`emit_ring` `:282/288`; knockout XObject `:147`;
  cull `bbox_hits`/`world_bbox`/`page_bbox` `:310–332`.
