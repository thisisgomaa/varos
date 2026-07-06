# Varos Layers — Vision & Spec

> The design of record for the Layers system after the **2026-07-03 pivot**. Supersedes the Illustrator-
> panel direction in `LAYERS_SPEC.md` (that doc stays for its pixel/interaction research; this doc owns
> the model + the pivot). Produced by a 5-agent design pass (3 research → synthesis → adversarial critique).
> Ahmed: *"Layers are 100% the program — if we don't get this right, nothing else will be right."*
> ✅ locked · 🟡 near-term stage · 🔒 FUTURE (spec-only, must NOT force a rewrite).

---

## 0. The Pivot — why simple

The Illustrator-style panel (target-ring, coloured select-square, per-layer identity bar, +layer/+sublayer
buttons, N-counter) was **too complex** and half of it (target ring, select square) only exists to feed an
**Appearance/effects system Varos does not have**. So it read as decoration that got in the way, not tools.
Photoshop / Affinity / Figma dropped all of it years ago: **the row IS the object; clicking it selects it.**

Varos goes the same way. Crucially, **this is a VIEW change, not a model change** — the real scene-graph
(`Document.nodes` arena, `roots`, FRONT-first children, z = pre-order) already built in Stage A stays exactly
as-is. We are simplifying the surface and adding two model capabilities (clipping, structural bands) on the
existing tree. Nothing already built is thrown away.

---

## 1. The Simple Panel (NOW ✅)

### 1.1 Row anatomy
`eye · lock · thumbnail · name`. Top of the list = top of the z-order. That is the entire row.

### 1.2 What we DELETE from today's panel (UI only — the model fields stay defined)
- the target ring ○/◎ (Appearance handle — no Appearance system yet)
- the coloured **select-square** ▢ (Ahmed: *"I honestly don't understand it"* — correct; it's an Appearance-
  era artifact)
- the per-row identity **colour bar**
- the **+layer / +sublayer** footer buttons and the **N Layers** counter
- `Node.color` stays a field, just not drawn. Nothing is removed from the model.

### 1.3 Footer (NOW)
Two buttons only: **Group** (⌘G-equivalent) and **Delete** (trash). Search stays at the top.

### 1.4 The model underneath is unchanged
Simple is a rendering of the same arena. The `≥1 Layer` invariant in `sync_tree` stays; when bands ship it
tightens to *the three band layers always exist*.

---

## 2. Interaction table (NOW ✅)

| Input | Action |
|---|---|
| Click a row | Select that object (replace selection) |
| **Ctrl**+click | Add / remove that object from the selection |
| **Shift**+click | Range-select from the last click to here |
| Double-click name | Inline rename |
| Drag a row (Before/After zone) | **Reorder** in z |
| Drag a row (Into/middle zone) | **Nest** into a group — **plain group, NEVER auto-clip** (see §3.4) |
| **Alt**+drag a row | **Duplicate** the object(s) |
| Ctrl+G / Ctrl+Shift+G | Group / Ungroup |
| Make/Release clip | **Explicit only** — see §3.2 |

Every one of these already has a model op (`move_node_to`, `move_paths_to`, `group`, `ungroup`, `dup_paths`)
from Stages A/B. The panel is a thinner skin over them.

> **Bug note — ROOT-CAUSED 07-03.** Ahmed: "Shift-select and Alt-drag don't work in the panel."
> - **Canvas Alt-drag-copy: WORKS.** Proven by a headless test (`tests/layers.rs::alt_drag_on_canvas_leaves_a_moved_copy`).
> - **Panel Shift/Ctrl-select: BROKEN by design.** `ui.rs:1983` — the **row-body click emits `Op::LayerFocus`
>   which ignores all modifiers**. Shift was only read on the tiny **select-square** (`ui.rs:1982`), and Ctrl
>   nowhere. So Shift-clicking a *row* (the natural target) just re-selects. **Fixed by the simple panel:**
>   click/Ctrl/Shift/Alt act on the ROW (Ctrl=toggle, Shift=range, Alt+drag=duplicate). The confusing
>   select-square is deleted.

---

## 3. Masks / Clipping (NOW ✅) — Ahmed's decisions 07-03

Answers Ahmed's Q1, **updated with his 07-03 ruling**. Called **"Mask"** in the UI (he prefers the word).
Both gestures he wanted are live, folded onto **one stored form**:
- **Drag a row INTO the shape below it → it's masked/clipped inside that shape** (Affinity nest = mask).
- **Ctrl+Alt+G** (select ≥2 rows) → mask, exactly like Photoshop.
- Both produce the byte-identical tree; never two grouping topologies.

**Two mask modes Ahmed wants (his words):**
1. **Put an element inside** → the container/shape masks what you drop in. This is the NOW build (vector clip).
2. **Add a mask on top and paint/erase on it** (Photoshop raster layer-mask) → a bottom **"Add Mask"** button.
   The *button + a vector mask* ships with the clip work; **painting into a raster mask is FUTURE** (needs a
   brush/raster engine we don't have — §7).

**Safety kept (so a mask is never a silent accident):** dragging a row into a **Group** = plain nest (no
mask); dragging onto a **shape** (a leaf) = mask (wrap into a mask group). The drop shows a **distinct mask
indicator** (not the plain nest box) so you always see when it will mask vs. nest.

### 3.1 The ONE canonical form (locked)
A clip is a property of a **Group** node:
```
role: GroupRole::Clip
mask_child: Some(child_id)   // AUTHORITATIVE STORED ID — the shape whose silhouette clips the others
```
- The **mask** is a real, selectable, renamable child row — but it is **not painted as itself**; its fill
  rings become the clip region for every *other* child. (Photoshop's "clip to the base below"; Illustrator's
  `<Clip Group>` with a designated clip path; the PDF-native form — round-trips into `.vrs` with zero
  translation.)
- **`mask_child` is a stored id, not "whatever is back-most."** Creation places the mask as the back-most
  child (children are FRONT-first, so `children.last()` at creation), but from then on the **id is truth** —
  reordering the group's children does NOT silently reassign the mask. *(Critique BLOCKER-5: positional mask
  identity desyncs the first time `arrange`/reorder touches child order.)*

### 3.2 Gestures — clipping is EXPLICIT (critique MAJOR-4)
Clipping must **not** share a gesture with duplicate or reorder-nest, or clips happen when the user didn't
mean them (destructive: it hides a child). So:
- **Primary — Make Clipping Mask:** select ≥2 rows → **Ctrl+Alt+G** or right-click → *"Make Clipping Mask"*.
  The **top** selected shape becomes the mask of the ones below (Photoshop's shortcut + Illustrator's mental
  model, unified). Internally: `group()` the selection, `role=Clip`, `mask_child = the top shape`.
- **Secondary — drag onto the THUMBNAIL:** drag a row and drop it **onto another row's thumbnail** (a
  distinct, small target — *not* the row's middle "Into" zone) → the dragged art is clipped inside that
  shape. Nesting-into-a-group (the middle zone) stays a **plain group**.
- **Release:** Ctrl+Alt+G again / right-click *"Release Clipping Mask"* / drag the clipped child back out to a
  Before/After zone.

> **Rejected: Alt-click the hairline between rows (Photoshop) as a NOW gesture.** It collides with Alt-drag-
> duplicate (Alt + press on a 2px line is indistinguishable from Alt + press-move). Deferred to FUTURE §6, and
> only if it earns a razor-thin (≤3px) zero-motion hit-zone with its own cursor.

> **Rejected: plain "Into" nest auto-promotes to Clip when the target is non-empty.** The synthesis proposed
> it; the critique killed it. The Into zone is a thin middle sliver — aiming to reorder frequently lands
> there, and auto-clip would silently mask artwork. **Into = plain group, always.**

### 3.3 Render seam
Reuses the **isolated-layer SaveLayer path** already built for group opacity/knockout. A new runtime
`Group::Clip { mask_rings, members }` variant: fan the mask rings into a stencil bit (`0x02`), draw members
with a stencil-test pipeline, clear the bit. `pipe_smask` / `pipe_cover_clip` are clones of the existing
stencil pipelines. No new architecture — the same seam knockout already uses.

### 3.4 The reconciliation rule (write it on the wall)
**ONE stored form (parent-child clip group), two sugars (Make-Clip command, drag-onto-thumbnail).** The
Photoshop "clip to sibling-below" gesture, if ever added, **desugars** into this same tree — we never store a
second "flagged sibling-run" grouping topology. One grouping model, forever. *(This is the trap that the old
`group_of` side-table was; we just finished migrating off it — do not reintroduce it.)*

---

## 4. Structural bands — Top / Middle / Background (⏸ DEFERRED — Ahmed 07-03: *"drop them entirely for now"*)

**Not built now.** Ahmed pulled bands out of the current scope. The design below is kept intact so it can be
switched on later with **zero rework** — the model is band-neutral until then (every layer is `Free`), and the
enforcement is one clamp in `move_node_to` that simply isn't wired yet. Revisit after the simple panel + masks
land and he actually wants persistent Top/Middle/Background strata.

> Original intent (kept for later): a **Background** (backdrops that get covered), a **Middle** (working
> content), a **Top** (grids/overlays that must stay on top, toggled, worked-on, duplicated).

### 4.1 The model — three REAL layers, position = truth (critique BLOCKER-2)
The bands are **three ordinary root Layers**, tagged:
```
band: Band   // Top | Middle | Background | Free(default)   — meaningful on root Layers only
```
- Their **position in `roots` IS the z-order** — exactly like every other layer. **There is NO sort.** One
  source of truth (`roots` order = z), on disk and in memory. The synthesis proposed a stable-sort in
  `sync_tree`; the critique proved that fights the user's drag (snaps a dragged layer back → the "fake
  movement" feeling that already bit the lock bug) and desyncs export z. **Killed.**
- **"Top stays on top" is enforced by a clamp, not a sort:** in `move_node_to`, a root-level drop **clamps its
  insertion index to within its band's contiguous run**. You physically cannot drag a Top layer below the
  Middle band. The drag **stops at the boundary** (honest) instead of snapping back (fake).

### 4.2 Defaults
- **Today (bands UI not shipped):** `Document::default()` unchanged — one `Free` "Layer 1". Zero behaviour
  change; legacy files load as a single `Free` layer with exact z preserved.
- **When bands ship:** a fresh doc has Background/Middle/Top; **`active_layer` = Middle**, so new art lands in
  the Middle band automatically. Background stays a backdrop; Top stays overlays.

### 4.3 Add more / cross-band drag
More strata = more layers (`add_layer` exists; a new layer is `Free` and lives in the Middle band). Dragging a
layer across a band divider **re-tags** its `band` to the destination band (one field write in
`move_node_to`), and the clamp keeps everything ordered. The three bands are a rhythm, not a cage.

### 4.4 Clips are BAND-LOCAL (critique MAJOR-3)
A clip may only form **within one band**. Because clipping = reparenting, an unrestricted "drag the Top grid
onto a Background shape to clip it" would drag the grid *into the Background band* — the thing that must stay
on top silently goes to the bottom. So: a clip whose members would straddle bands is **refused** (return
`false`, matching the existing cycle / Layer-into-Group guards in `move_node_to`). Written into the gesture
rules, not discovered at runtime.

### 4.5 Save / PDF
The bands are ordinary layers → `.vrs` (embedded model) and PDF export (flatten `roots` order) already carry
them. **No format bump.**

---

## 5. The one indirection that prevents the 2-month rewrite (NOW ✅ — land FIRST)

*(Critique's single biggest-risk prevention.)* Today every consumer — hit-test (`path_under`), snapping,
per-layer thumbnails, `outline_bbox`/align, the overlay/anchor loops, **and** `build_scene`'s content loop —
reads `doc.paths` as *the* flat, 1:1, z-ordered list of paintable objects. Clipping introduces the **first
object that is in the tree but must not paint as itself** (the mask). Future masks add more; future pages add
objects filtered by active page. The day two of those are true at once, "re-flatten to a flat vector"
collapses and every consumer must be rewritten to walk the tree. **That is the rewrite Ahmed fears.**

**Prevention — one derived indirection, landed before any new field:**
```
// computed by sync_tree, sits between doc.paths and every consumer
PaintRole { Normal, MaskSource, Hidden }        // per path id
fn paint_list(&self) -> impl Iterator<Item=&Path>   // today: exactly doc.paths in order
```
- **Today it returns exactly `doc.paths`** — zero behaviour change, the `tests/layers.rs` z-order golden stays
  green.
- **Clipping** makes `paint_list()` exclude `MaskSource` paths from the *content* run (but they stay in
  `doc.paths` for hit-test/rename/thumbnail — the mask is still a first-class row).
- **Pages** (future) makes it filter by active page.
- Every consumer reads through `paint_list()` (or checks `PaintRole`), so each future filter is **one edit,
  not thirty**.

> **Test the blocker directly:** the golden asserting `paths == [10,11]` does **not** catch a mask that
> double-paints. Add a test: a clip mask is **excluded from the content/paint run** but **present for
> hit-test**. *(Critique BLOCKER-1.)*

---

## 6. Model reference — the concrete delta

### 6.1 `Node` — appended fields, all serde-defaulted (old `.vrs` loads unchanged)
```rust
// ── clipping ──
#[serde(default)] pub role: GroupRole,          // Normal unless a clip (or, future, a mask) group
#[serde(default)] pub mask_child: Option<u32>,  // AUTHORITATIVE mask id; others clip to it

// ── structural bands (root Layers only) ──
#[serde(default)] pub band: Band,               // Free = ordinary layer (today's behaviour)

// ── FUTURE, reserved & inert (see §7) ──
#[serde(default)] pub dimmed: bool,             // template layer: draw faded
#[serde(default)] pub non_printing: bool,       // template layer: export skips
```
```rust
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum GroupRole { #[default] Normal, Clip, MaskAlpha, MaskLuma } // Mask* parsed but treated as Normal until masks ship

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Band { Top, Middle, Background, #[default] Free }
```
**Dropped from the synthesis's field list (critique MINOR-6):**
- `page: Option<usize>` — **contradicts** the documented spatial page model (`model.rs:82-83`: page membership
  is decided by *bounds overlap at export*, not containment). Re-introducing containment pages is a second,
  conflicting semantics — a future rewrite disguised as a serde default. Pages stay §7, resolved when built.
- `Node.opacity` — **not** a free field flip. The Isolated seam today is driven by **`Path.opacity` per
  object** (`scene.rs:148`), not by group nodes. Real group opacity is a new cascade + composite. Deferred as
  real work, not marketed as NOW-cheap.

### 6.2 Runtime (not serialized) — `scene.rs`
```rust
// add to the Group enum:
Clip { mask_rings: Vec<Vec<Pt>>, members: Vec<Group> }   // members are Groups so nested clips recurse
```

### 6.3 Functions to touch
| Function | Change |
|---|---|
| `Node` struct | append the fields in §6.1 |
| **`paint_list()` / `PaintRole`** (new) | §5 — land FIRST; returns `doc.paths` today |
| `sync_tree` | (a) compute `PaintRole` / mask-source set; (b) **validate `mask_child` is still a DIRECT child** of its clip group — demote `role=Normal, mask_child=None` if missing *or* re-parented away (not just "if the id vanished" — critique BLOCKER-5) |
| `move_node_to` | (bands) **clamp** a root drop to its band's contiguous run + re-tag `band` on cross-band drop; (clip) forbid cross-band clip formation (return false) |
| new `clip_group(pids, mask)` | thin wrapper: `group()` + set `role=Clip, mask_child`; `group()` itself untouched |
| `group` / `ungroup` / `arrange_units` / `dup_paths` / `move_paths_to` | **audit for `role`/`mask_child`**: `dup_paths` remaps `mask_child` through the id map; `ungroup` releases the clip (decided: yes) and leaves no dangling `mask_child`; `arrange`/reorder must not silently reassign the mask (id is authoritative); `group` must not bury the mask a level down |
| `Document::default` | (bands stage only) 3 tagged root Layers, `active_layer` = Middle. No change until then. |
| `build_scene` → recursive `emit_node` walk | replace the flat content loop with a tree walk; `Normal` emits byte-identical Opaque/Isolated/Knockout; `role==Clip` emits `Clip`. Land as a pure refactor, prove identical against the z-order golden **plus the new mask-exclusion test**, before wiring clip. |
| `tess.rs` / `lib.rs` | `Group::Clip` → `GroupDraw::Clip`; stencil pipelines `pipe_smask`(0x02) / `pipe_cover_clip`; reuse `layer_msaa`/`layer_view` offscreen seam |
| `build_layers` (ui.rs) | delete target-ring/select-square/colour-bar/+buttons/counter; wire Ctrl+Alt+G + drop-onto-thumbnail → `clip_group`; (bands stage) band-divider hairlines with pre-filled names |

**No `.vrs` format break.** Old files load as all-`Normal`, all-`Free`, reserved-`false`; serde name-keys every
field; `Group`/`GroupDraw` are runtime-only.

### 6.4 Recommended sequencing
1. **`paint_list()` indirection** — silent, golden stays green. *(insurance first)*
2. Simple-panel UI (delete the Illustrator chrome; rewire Shift/Ctrl/Alt onto rows). *(the pivot Ahmed feels)*
3. Additive `Node` fields + `sync_tree` guards — silent.
4. `build_scene` → `emit_node` tree walk — pure refactor, prove identical (golden + mask-exclusion test).
5. `Clip` variant + stencil pipelines + explicit clip gestures.
6. Structural bands — clamp in `move_node_to` + tagged default doc + dividers.
7. Masks / pages / template layers stay inert reserved fields (§7).

---

## 7. 🔒 FUTURE — spec-only, fenced, NOT built now

Every item here is a `#[serde(default)]` field or a pure-additive enum arm — **no `.vrs` break, no topology
change**. This is the promise that lets us build simple now without repainting ourselves into a corner.

- **7.1 Alpha & luminance masks.** `GroupRole::MaskAlpha / MaskLuma` reuse the **same `mask_child`**; render via
  the SaveLayer seam (soft mask instead of hard stencil clip). The clip work already lays the rail.
- **7.2 Template / dimmed / non-printing layers** (Illustrator template layers). `dimmed` + `non_printing`
  bools (reserved in §6.1); renderer draws faded, export skips. Orthogonal to `band`.
- **7.3 Pages / artboards-as-pages.** ✅ **DECIDED & BUILT 2026-07-06 (Ahmed) — see §9.** The reconciliation
  this item demanded an owner for is resolved: **spatial stays king; the panel is a DERIVED view.** No
  containment flip, no `page:` field — membership is computed from geometry per frame.
- **7.4 More structural bands / per-page masters** (InDesign = band axis × page axis).
- **7.5 Re-expose per-object/-layer colour + a real target** once an Appearance/effects system exists to
  receive a "target" (the reason the ring/square existed).
- **7.6 Photoshop Alt-click-between-rows clip** as a razor-thin (≤3px), zero-motion, distinct-cursor gesture —
  only if it can be made unambiguous against Alt-drag-duplicate.

**The invariant that protects all of it:** clipping/masks/pages each become **one filter in `paint_list()`**,
and each new capability is a serde-defaulted field or an additive enum arm. One grouping topology
(parent-child), one ordering source (`roots`/`children` position), one paint indirection. That is the whole
insurance policy.

---

## 8. Decisions — RESOLVED (Ahmed 07-03)
1. **Mask gestures:** ✅ **BOTH** — drag-a-row-into-the-shape-below AND Ctrl+Alt+G (Photoshop). Named "Mask"
   in the UI. Two modes: put-element-inside (NOW, vector) + Add-Mask-button/paint (button+vector now, raster
   paint FUTURE). (§3)
2. **Bands:** ✅ **DROPPED for now** (§4) — spec kept for later, zero rework to switch on.
3. **Ungroup releases the mask/clip** — ✅ confirmed default. *(Plain-language: if you group-break a masked
   set, the mask is lifted and everything paints normally again — the shapes are NOT deleted, they just stop
   being cut to the mask. Nothing is lost; you can re-mask anytime.)*

**Build order (agreed):** simple panel first (it fixes the Shift/Alt bugs by making click/Ctrl/Shift/Alt act
on the ROW — see §2 bug note), then masks, then (much later, optional) bands.

---

## 9. 🔀 2026-07-06 — SECTIONS BY ARTBOARD [D11] (Ahmed) — decided & built

Ahmed: *"ينفع يكون التقسيم على حسب الأرت بورد… زي فيجما والحاجات المودرن"* — the panel splits by PAGE.
**The §7.3 reconciliation, resolved: spatial stays king; the panel is a DERIVED view.** Artboards still
never own artwork; export is untouched; there is no `page:` field. One membership source of truth in the
model — `path_boards(pi)` / `node_boards(nid)` = the boards a bbox **visibly overlaps** — read by the
panel, the render clip, and (next) board-level eye/lock.

Resolved with Ahmed (his answers, 07-06):
1. **Mirror rule** (his idea, adopted over my centre-rule): a straddler lists under **every** page it
   stands on — same object, same state on both rows; the render cuts **one copy per member page** (its
   part shows on each page, "زي ميرور"). With clip ON everywhere and no overlap → exactly one home.
2. **Floaters:** art on **no** page sits **loose at the bottom, under no header** — visibly outside every
   page and outside export ("بدون لير عايمين… مش هيطلعوا في الإكسبورت").
3. **One board still shows its header** ("واحدة زي ٢٠ عادي") — unlike the dead "Layer 1" wrapper, a page
   is a real named thing on canvas.
4. **Ordering: this before masks.** **MASKS DEFERRED indefinitely** ("ممكن الماسكات تتعمل في المستقبل
   أصلاً") — §3's spec and the ✅ `paint_list()` insurance stay ready for whenever they return.
5. **Page clip DEFAULT ON** for new boards (modern cut, Figma-feel); the per-board toggle re-enables
   Illustrator bleed, and a clip-off member page ⇒ the object draws **uncut** (bleed wins). Old files
   keep their stored value.

Built 07-06 (🟡 pending Ahmed's hand-verify): header rows (click = active board · dbl-click = rename ·
collapsible · accent edge on the active board), mirror rows, floater strip after a hairline, clip-per-
member-page render (`tests/boards.rs` locks membership + mirror + floater/bleed), drag-drop constrained
to the source row's own section(s). **Next piece:** drag a row onto ANOTHER board's section/header =
spatial move of the art onto that page (translate, Figma-style) + header eye/lock (board-level hide =
the first real `paint_list()` filter). Mirror-row costs accepted for v1: rename opens on the first
instance only; both instances dim while dragged (same object — honest).
