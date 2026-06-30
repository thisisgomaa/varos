# Varos — Snap + Transform System Spec (for review with Ahmed)

> Synthesised from a 4-lens study (Illustrator standard · Figma/Affinity feel · Varos code · interaction feel)
> plus the live code. **Base standard = Illustrator; borrowed feel = Figma/Affinity; depth borrowed = Affinity.**
> v1 = Illustrator-class. We review this line-by-line, settle the open decisions, THEN build — one Stage-1
> piece at a time.
>
> **Headline:** *Transform is largely already built.* The oriented frame, 8 handles, scale/rotate, the numeric
> X/Y/W/H/angle setters, align/distribute/flip, group-aware move, nudge, Alt-drag duplicate — all live in
> `editor.rs`. **The genuinely new work is the Snap substrate** — a pure `snap()` choke-point feeding off a
> richer, Affinity-grade `SnapConfig` — plus a thin Transform completeness pass (live measurement HUD, center
> point, live numeric rotation, power-duplicate / transform-again). Read §0 then §1.

---

## ✅ DECISIONS LOCKED — 2026-06-30 (Ahmed, via Illustrator references)
1. **Snapping menu = Illustrator's "Snapping Quick Access".** A **magnet icon** in the top bar opens a
   dropdown: *Snap to Grid · Snap to Pixel · Snap to Point · **Smart Guides*** (default ON) with sub-prefs
   *Alignment Guides* + *Geometric Guides* (Snap to Glyph comes with text). Build the menu faithfully now;
   the Grid/Pixel toggles are present but only act once the grid model lands (Stage 2). Defaults match
   Illustrator's panel: Smart Guides ON · Alignment + Geometric ON · Grid/Pixel/Point OFF.
2. **Toggle shortcut = Ctrl+U** — exactly Illustrator's Smart Guides toggle — plus the **magnet icon** state
   in the bar. (Settles open-decision §8 for this binding.)
3. **Center point** — show the selected object's CENTER as a small mark on canvas (Illustrator "Show
   Center"), tied to the Transform 9-point reference; it is also a snap target.

**Post-research refinements (planning team — accepted 2026-06-30):**
4. **The magnet quick-menu is the *daily* surface; a fuller "Snapping Preferences" panel (Affinity layout)
   is the *advanced* surface.** Illustrator-feel defaults under the magnet; Affinity-depth toggles under an
   "advanced" disclosure / a Preferences pane (Stage 2). This matches the standing rule (feel = Illustrator)
   while harvesting Affinity's richer model. (§2.)
5. **One unified `snap()` choke-point fed by one `SnapConfig`.** Every clever behaviour — bbox alignment,
   equal gaps, equal sizes, key-point snap, geometry snap, grid, guides — is *"just another candidate
   generator"* feeding *one* collect → cap-at-N → score → snap-strongest pipeline (the Affinity architecture).
   No naive nearest-thing snapping. (§0, §2.)
6. **Tolerance is screen-px via `ppu`.** A constant ~8 px on screen at every zoom, exactly like every existing
   grab radius in the code (`CONST / self.ppu`). `radius_px` lives in `SnapConfig`. (§0.)
7. **Center-point / Show-Center is its own toggle**, defaulting ON for the selected object, mirroring
   Illustrator's per-object "Show Center" attribute but applied to the live selection for v1. (§3.)

> 📎 **See `SNAP_TRANSFORM_FEATURES.md` for the sourced feature matrix this spec draws on** (web-grounded,
> verified against Adobe/Serif/Figma official docs + a Sketch/CorelDRAW/Inkscape/Penpot survey).

---

## 0. The core model — the invariant to LOCK first

**The SnapEngine seam.** Today every interactive drag in `editor.rs` ends in the same shape: take a raw world
`pos` (and a base set of anchors), optionally `snap45(d)` it on Shift, write the result. Snapping is *one new
pure function in that exact spot* and nowhere else.

```rust
// pure: takes doc + live ppu + the moving id-set + cfg; returns a corrected point and the guides that fired.
fn snap(&self, raw: SnapInput, cfg: &SnapConfig) -> SnapResult
//   SnapInput  = { kind: Move | Point | Handle, moving_bbox, key_pts: &[Pt], exclude: &[usize] }
//   SnapResult = { delta: Pt,            // the correction to apply to the raw delta/point (Δx, Δy)
//                  guides: Vec<SnapGuide>,// magenta lines / pips / glyphs to draw this frame
//                  hud:    Option<Hud> }  // the live measurement value, if any
```

**0.1 One choke point, never scattered.** `pointer_move` already routes through a single `match self.drag`
(and `ab_move` through `match self.ab_drag`). The SnapEngine is consulted **once per drag arm**, right where
`let mut d = sub(pos, down); if shift { d = snap45(d); }` lives today — concretely at:

| Arm | File:line | Constrained quantity | What `snap()` adjusts |
|---|---|---|---|
| `Drag::Object` | `editor.rs:953` | `d` (after `snap45`) | snap the moved bbox edges/centers/mids → adjusted `d` |
| `Drag::Scale` | `editor.rs:962` | the live handle point `lp` | snap the dragged handle (world), back-solve `sx,sy` |
| `Drag::Rotate` | `editor.rs:984` | `d` (after 45° snap) | optional angle-snap to cardinal/guide angles |
| `Drag::Anchors` | `editor.rs:875` | `d` | snap the moved anchor / its key point (point snap) |
| `Drag::Handle` | `editor.rs:886` | `q` | snap the bezier handle endpoint (point / angle) |
| `AbDrag::Move` | `editor.rs:586` | `d` | snap the page rect edges/center |
| `AbDrag::Resize` | `editor.rs:595` | the moved corner `pos` | snap the dragged page edge |

The cleanest insertion is two sibling methods — `snap_translation(moving_bbox, key_pts, d) -> (Pt, guides)`
called right after the `snap45` line in `Drag::Object` / `Drag::Anchors` / `AbDrag::Move`, and
`snap_point(world_pt) -> (Pt, guides)` for `Drag::Scale` / `Drag::Handle` / `AbDrag::Resize` where you snap
the live point and back-solve the factor (the inverse of the `(lp[0]-pivot[0])/dx0` math at `editor.rs:968`).
Tools never reach into snap internals.

**0.2 Pure & headless.** `snap()` takes the document, the live `ppu` (so tolerance is constant *screen* px),
the moving id-set (excluded from being its own target), and `&SnapConfig`. It returns geometry only — no
rendering, no `View`. This mirrors the `units.rs` discipline ("pure math, no View/Editor coupling") and makes
it unit-testable like `tests/math.rs`.

**0.3 The seam invariant — *snapping changes the delta, not the gesture*.** A drag with snapping off must be
byte-identical to today's drag. Snap is a **post-filter on the raw point**, applied *after* Shift's
`snap45`/aspect constraints, *before* the base→anchor write. This guarantees we can ship it dark (config flag
off) and turn it on without touching any tool.

**0.35 Independent snap-SOURCE vs snap-TARGET point sets (the biggest gap the big three miss — model both
from day one).** Inkscape/Corel let you pick *which point of the moving object leads* (a specific anchor, a
bbox edge-mid, the center, the rotation pivot) **independently** of *which targets it can land on* (other
objects' geometry, guides, grid, …). This source/target split is *the* thing that makes a vector tool feel
"precise" rather than "approximate". **`snap()` is built around it even though v1's UI exposes only a subset:**
`SnapInput.key_pts` + the moving bbox are the **source** set; the enabled `SnapConfig` generators are the
**target** set. v1 routes sources by tool (V drags lead with bbox edges/mids/center; A/Pen lead with the moving
anchor — §8.4) and the full per-point source picker is the Affinity-depth "advanced" surface (Stage 2). *(v1
architecture; UI subset.)*

**0.4 The candidate model (the Affinity architecture — adopt this, not naive nearest-thing).** Every frame,
`snap()` runs two phases:

1. **Collect** — from every *enabled* source (page edges, guides, grid, each visible/unlocked object's
   bbox/mids/centers/key-points/geometry, plus gap/size *relationships*), gather every candidate whose
   influence reaches within **`radius_px / ppu`** of the moving selection. Sources are independent generators;
   adding "equal gaps" or "match width" is just adding a generator.
2. **Cap → score → snap strongest.** Keep the **N nearest** candidates (`candidate_max`, default 8) to bound
   cost and noise. Score each: `score = priority_weight − dist_px/tol_px + type_bonus`. **Priority is the
   dominant term** (a center-to-center beats a merely-closer edge); within a tier the closest wins; ties break
   on stable id (no flicker). **Snap X and Y independently** — you can be edge-snapped horizontally while free
   vertically. The strongest per axis pulls the point; the rest are drawn faintly if *show candidates* is on.

   Priority order (highest wins a contested overlap): **key points** (anchors / handle ends / path
   intersections) → **object geometry** (point on a path segment) → **bbox features** (corners / mids /
   center) → **bbox edges** (alignment lines) → **guides** → **grid** → **artboard edges** → **pixel grid**
   (lowest — it's everywhere, so it must never mask real geometry). *Specific beats general; intentional beats
   automatic; geometry beats scaffolding.*

**0.5 Tolerance & hysteresis.** Every grab radius in this codebase is `CONST / self.ppu` (e.g.
`editor.rs:198, 286, 294, 532`); the snap threshold uses the same idiom — `cfg.radius_px / self.ppu` — so it's
a constant ~8 px on screen at any zoom (`ppu = view.zoom`, set before every `pointer_move`, `main.rs:295…`).
To kill jitter at the boundary, **acquire** at `radius_px` (~8) but **hold until** `~1.6× radius_px` (~11) — a
dead-band so micro-moves can't strobe the snap state. Once a candidate wins it gets a small score bias next
frame so the winner doesn't flicker between two near-equal targets.

**0.6 Guides are output, not state.** The guides a snap produced this frame live in transient editor state
(`self.snap_guides: Vec<SnapGuide>`), rebuilt every move, drawn as constant-screen `overlay` Prims, cleared on
`pointer_up`. They are **never serialized**. (Contrast: *ruler guides* — the draggable blue lines — ARE
persistent document furniture; see §6.)

**0.7 Two snap families, one engine.**
1. **Geometry snap** — the moving point/bbox grabs onto fixed targets (other objects' key-points / edges /
   centers / geometry, artboard edges/center/mids, guides, grid, pixel). Output: a corrected point.
2. **Smart alignment + spacing/size** (the Figma/Affinity borrow) — when the moving bbox's edge/center lines
   up with another, draw the magenta extension line; when gaps to neighbours match, draw equal-spacing pips
   with `=` ticks; when a resized dimension matches a sibling's, flash a matched-size bar. Output: the same
   corrected point + the guides.

   Both are the same `snap()` call; family 2 just also emits `SnapGuide`s and can snap the bbox (not only the
   cursor point).

---

## 1. Where the code stands today (grounding, file refs)

**✅ Transform — already built in `varos-core/src/editor.rs` + `model.rs`:**
- **Oriented transform frame:** `obj_angle` (`editor.rs:143`), `obj_local_bbox` (`:238`), `frame_corners`
  (`:260`), `frame_handles` (`:253`), `bbox_handles` (`:248`) — an 8-handle box that *rotates with the
  selection*. `objsel_base()` (`:267`) snapshots every anchor each drag.
- **Scale:** `transform_hit` (`:282`) → `TfHit::Scale`, `start_transform` (`:299`), `Drag::Scale` (`:962`) —
  works in the frame's LOCAL un-rotated space, Shift = equal/aspect, Alt = scale-from-center.
- **Rotate:** `TfHit::Rotate` (corner ring, radius `22.0/ppu`, only over empty space so nearby objects stay
  clickable), `Drag::Rotate` (`:984`), Shift = 45° steps.
- **Numeric transform setters (panel-ready):** `set_obj_bbox(x,y,w,h, ax,ay)` (`:466`) with a **9-point
  reference** already implemented; `set_obj_rotation(deg)` (`:487`); `flip(horizontal)` (`:451`).
- **Move:** `Drag::Object` (`:952`, group-aware), `Drag::Anchors` (`:875`); **Alt-drag duplicate**
  (`Drag::DupPending` → `dup_paths`); **nudge** (`:1036`) arrows, Shift = ×10.
- **Align / distribute / distribute-spacing:** `align` (`:388`), `distribute` (`:408`),
  `distribute_spacing(axis, gap)` (`:430`).
- **Artboard transform (parallel engine):** `AbDrag` (`:65`), `ab_move` (`:584`), `ab_resized` (`:107`),
  `rect_from_corners` (`:124`) — already does `if shift { d = snap45(d) }`. **This is the second seam** snap
  must hook (artboard move/resize should snap to guides/grid/objects too).
- **All of it is bracketed by `begin()`/`commit()`** = one undo step each; the deferred-Op bus + 200-deep
  history already exists.
- **The only "snap" today is `geom::snap45`** (45° angle constraint on Shift) — that's *constraint*, not
  *object snapping*.

**🟡 The current minimal `SnapConfig` (`model.rs:90-101`), and how it expands:**
```rust
pub struct SnapConfig {        // stored at Document.snap (model.rs:127, #[serde(default)])
    pub smart: bool,      // Smart Guides master (Ctrl+U)       default true
    pub alignment: bool,  // alignment guides + equal-spacing    default true
    pub geometric: bool,  // snap to object geometry             default true
    pub point: bool,      // Snap to Point                       default false
    pub grid: bool,       // Snap to Grid (Stage 2)              default false
    pub pixel: bool,      // Snap to Pixel (Stage 2)             default false
}
```
The struct is `Serialize/Deserialize` and `Document.snap` is `#[serde(default)]`, but the **fields are not
individually `#[serde(default)]`** — the one thing to fix so adding fields never breaks an old `.varos` file.
§2 below is the full expansion: same three Affinity concerns (behaviour / page / object), every field
serde-defaulted, with a clean rename map (`geometric→object_geometry`, `alignment→alignment_guides`,
`point→key_points`).

**🔴 Snap — entirely missing:**
- No `snap()` choke-point call, no candidate pipeline, no per-target snapping (key-point / edge / center /
  geometry / artboard / grid / guide / pixel), no scoring/hysteresis.
- No **smart alignment guides** (the magenta align/measure overlay), no **equal-spacing pips**, no
  **match-size** feedback.
- No **live measurement HUD** (W×H / dx,dy / angle by the cursor) — values exist mid-drag, nothing draws them.
- No **ruler / guide / grid model** in the serde document. (`DocUnits{ppi, display}` exists in `units.rs`; the
  infinite dot grid is a render-time visual only — no snappable grid-spacing field; `model.rs:88` flags
  grid/pixel as Stage 2.)
- No **transient snap-guide state** on `Editor` — to be added (see §6).
- No **magnet menu** in the topbar, no Snapping Preferences panel.

**🟡 Transform gaps (small, real — see §3):** no live drag HUD; no center-point mark; rotate has no numeric
live readout mid-drag (only the after-the-fact `set_obj_rotation`); no power-duplicate / transform-again
(`Ctrl+D`); no draggable transform origin; no align-to-key-object.

---

## 2. The SNAPPING MODEL — a unified, Affinity-grade SnapConfig

One config, three groups (Affinity's organisation), every field serde-defaulted so adding fields never breaks
old files. The rule that makes it safe: **struct-level `#[serde(default)]` *plus* per-field
`#[serde(default = "…")]`** — both "old file has no `snap` key" and "old file's `snap` predates field X"
round-trip cleanly. (Proven template: the `DocUnits` serde round-trip test, `units.rs:156`.)

```rust
fn t() -> bool { true }
fn snap_radius() -> f32 { 8.0 }     // screen px (÷ ppu at use)
fn cand_max()  -> usize { 8 }
fn grid_spacing() -> f32 { 72.0 }   // world pt

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapConfig {
    // ── BEHAVIOUR (global) ──
    #[serde(default = "t")]            pub enabled: bool,          // master kill switch (Affinity "Enable snapping")
    #[serde(default = "t")]            pub smart: bool,            // Smart Guides master (Ctrl+U)
    #[serde(default = "snap_radius")]  pub radius_px: f32,         // screen-px capture tolerance
    #[serde(default = "cand_max")]     pub candidate_max: usize,   // N strongest kept per frame
    #[serde(default = "t")]            pub show_candidates: bool,  // draw the alignment lines / pips live
    #[serde(default)]                  pub force_pixel_align: bool,// land snapped result on whole device px
    #[serde(default)]                  pub move_whole_px: bool,    // quantise the whole motion to integer px

    // ── PAGE / ARTBOARD snapping (the document scaffold) ──
    #[serde(default)]                  pub grid: bool,             // construction grid (Stage 2)
    #[serde(default = "t")]            pub grid_lines: bool,       // snap to a single grid LINE (one axis), not only intersections
    #[serde(default)]                  pub baseline_grid: bool,    // typographic baseline grid (Stage 2/3)
    #[serde(default = "t")]            pub guides: bool,           // user ruler guides
    #[serde(default = "t")]            pub artboard: bool,         // page/"spread" edges
    #[serde(default = "t")]            pub artboard_mids: bool,    // spread centre lines + centre point
    #[serde(default)]                  pub margins: bool,          // page margin box (Stage 2/3)
    #[serde(default)]                  pub margin_mids: bool,      // margin-box centre lines (Stage 2/3)

    // ── OBJECT snapping (other artwork) ──
    #[serde(default = "t")]            pub visible_only: bool,     // candidates from on-screen objects only
    #[serde(default = "t")]            pub object_bounds: bool,    // bbox edges + corners
    #[serde(default = "t")]            pub bbox_mids: bool,        // bbox edge-mids + centre
    #[serde(default = "t")]            pub gaps_and_sizes: bool,   // equal-spacing + equal-dimension snap
    #[serde(default = "t")]            pub key_points: bool,       // anchors / handle ends (was `point`)
    #[serde(default = "t")]            pub object_geometry: bool,  // snap anywhere on the path edge (was `geometric`)
    #[serde(default = "t")]            pub segment_mids: bool,     // midpoint between two adjacent nodes (Corel/Inkscape)
    #[serde(default = "t")]            pub path_intersections: bool,// where two path edges cross (Inkscape/Corel diamond)
    #[serde(default)]                  pub pixel_bounds: bool,     // raster-selection bounds (Stage 3)

    // ── alignment-guide feedback (Affinity "candidate" guides) ──
    #[serde(default = "t")]            pub alignment_guides: bool, // the magenta align lines (was `alignment`)
    #[serde(default = "t")]            pub equal_spacing: bool,    // equal-gap pips + `=` ticks
    #[serde(default = "t")]            pub equal_size: bool,       // match-width/height hints

    // ── grid spacing lives here so the engine has no magic constants ──
    #[serde(default = "grid_spacing")] pub grid_spacing: f32,      // world pt
}
```

### 2.1 BEHAVIOUR group — what each does, + Stage

| Field | What it does | Stage |
|---|---|---|
| `enabled` | The master kill switch for the entire system. Off = raw pixel freedom, no candidates collected, no guides drawn. | **1** |
| `smart` | Smart Guides master (the magenta alignment + measurement layer). `Ctrl+U`. | **1** |
| `radius_px` | Capture radius in **screen px** (÷ `ppu` per frame). The single most important feel knob — too high = sticky/magnetic, too low = dead. Default **8**; expose 4–12. Points get a ~1.3× bonus (≈ 9 px effective) so discrete targets feel "grabby". | **1** |
| `candidate_max` | Caps how many candidates are tracked per frame. Higher = more relationships, more noise/CPU. Internal constant **8** at first; expose only if power users ask. | **1** (internal) |
| `show_candidates` | Renders the live candidate relationships (alignment lines, gap pips, matched-size bars). Pure feedback. Non-negotiable for trust — Illustrator's Smart Guides set the bar. | **1** |
| `force_pixel_align` | Constrains the *snapped result* to whole device-pixel boundaries — crisp edges, no half-pixel blur. Gated behind a "pixel/UI mode". | **2** |
| `move_whole_px` | Quantises the *whole motion* (drag + nudge) to integer device px while active. Companion to `force_pixel_align` (that fixes the landing; this quantises the path). | **2** |

### 2.2 PAGE / ARTBOARD group — what each does, + Stage

| Field | What it does | Stage |
|---|---|---|
| `grid` | Snap to the document grid lattice (intersections). Needs the grid-spacing model (the current dot grid is decorative). | **2** |
| `grid_lines` | Snap to a **single grid line on one axis** (not only full intersections) — Inkscape/Corel line-snap. Small refinement, high value: lets you be grid-locked horizontally while free vertically. | **1** (with grid math) |
| `baseline_grid` | Snap object tops / text baselines to the typographic baseline rhythm. Publisher-class; defer until text. | **3** |
| `guides` | Snap to user-dragged ruler guides (and their intersections). | **1** (snap) / guide UI **2** |
| `artboard` | Snap to the page/"spread" rectangle — the 4 page edges. (Affinity calls the page a "spread".) | **1** |
| `artboard_mids` | Adds the page's H/V centre lines + centre point — instant centre-on-page. **Dependent child** of `artboard` ("Include spread mid points"). | **1** |
| `margins` | Snap to the page margin rectangle (inset from the edge). Needs a margins model. | **2/3** |
| `margin_mids` | Adds the margin box's centre lines. | **2/3** |

### 2.3 OBJECT group — what each does, + Stage

| Field | What it does | Stage |
|---|---|---|
| `visible_only` | Restrict candidates to objects in the viewport (perf + relevance — don't snap to something 4000 px off-screen). Implement as an internal cull regardless; expose the toggle later. | **1** (internal cull) |
| `object_bounds` | Snap to other objects' axis-aligned bbox — 4 edges + 4 corners. The daily-driver alignment. Source: `outline_bbox` (`model.rs:238`). | **1** |
| `bbox_mids` | Adds each bbox's edge-midpoints + centre (centre-to-centre, edge-centre alignment). **Dependent child** of `object_bounds` (Affinity "Include bounding box mid points" pattern — greyed out unless bounds are on). | **1** |
| `gaps_and_sizes` | The marquee Affinity feature: while dragging C between A and B, snap so the C↔neighbour gap **equals** an existing gap (distribution-on-the-fly); while **resizing**, snap W/H to **match** another object's dimension. Both are just two more candidate generators feeding the pipeline. | **1** |
| `key_points` | Snap to a path's anchor points and handle ends — the real node geometry, not the bbox. Vital for a pen-tool editor. (was `point`.) | **1** |
| `object_geometry` | Snap anywhere along a path edge/curve (nearest-point-on-outline), not only nodes/bbox. Heavier (per-frame nearest-point); source: `nearest_seg` (`model.rs:182`). (was `geometric`.) | **1** (bbox-level) → richer **3** |
| `segment_mids` | Snap to the **midpoint between two adjacent nodes** (Corel "Midpoint" triangle / Inkscape segment-midpoints). Cheap to compute, used constantly, missing from the big three. | **1** |
| `path_intersections` | Snap to a **path intersection** — where two path edges cross (Inkscape/Corel "Intersection" diamond marker), including the crossing of two guide projections. A pro must-have conspicuously absent in the big three. | **1** |
| `pixel_bounds` | Snap to an active raster-selection's bounds. Only meaningful once pixel layers exist. | **3** |

### 2.4 Alignment-guide feedback group

| Field | What it does | Stage |
|---|---|---|
| `alignment_guides` | The magenta H/V extension lines drawn from the moving feature to the feature it matched, with end-tick "T" marks showing *which* objects aligned. (was `alignment`.) | **1** |
| `equal_spacing` | Equal-gap detection: when gaps become equal, draw matching gap pips with paired `=` ticks (read "these spaces are the same" at a glance) + the numeric distance on at least one gap. | **1** |
| `equal_size` | Match-width/height: when a resized dimension equals a sibling's within tolerance, snap and flash a matched-size bar. | **1** |

**Feedback feel to lock now (web-surfaced — §2.4a):**
- **Two-colour language (lock the system now).** One colour = **alignment / measurement** (the magenta snap lines, distance readouts, equal-spacing pips); a *second, distinct* colour = **arrange / smart-selection** (the drag-handle / reflow affordances, Stage 2). Figma uses red vs pink for exactly this split; keep two reserved hues so the canvas never overloads one colour. (Our snap-line hue stays `SNAP_MAGENTA` per the locked decision; the *second* arrange hue is reserved now, used in Stage 2.) **v1 (reserve both now).**
- **Readable rounded PILL for on-canvas numbers (Sketch).** Every on-canvas number (dx/dy, W×H, angle, gap value) renders inside a small rounded pill with a solid backing, not bare text — legibility over the artwork is the most-felt detail. Apply to the `build_snap_hud` label (§6.2). **v1.**
- **Continuous ΔX/ΔY + position readout *during* the whole drag (not only on the snap).** Show the live offset and current position the entire time the pointer moves — Varos beats Figma (which reveals the gap only on the snap) by keeping it continuous. Feeds the same HUD. **v1.**
- **Quick Measure via `Alt`-hover (Figma).** With a selection, hold **`Alt`** and hover another object to read the **dual-axis distance** between them (drawn in the measure/align colour, in a pill); nested children = **`Ctrl+Alt`**-hover. Additive to Illustrator (no IL conflict). **v1** — see the additive-bindings table in Shortcuts §5.

### 2.5 The magnet quick-menu (Illustrator layout — the daily surface)

Home: the **topbar**, beside Search/Layout/Panels (`ui.rs:755-768`). Add a `magnet` `TopIcons` entry (Lucide
magnet via `load_icon`, `ui.rs:197`) + a `topbtn` cell opening a `popup_below_widget` of `check_row`s
(`ui.rs:717`). Faithful to Illustrator's "Snapping" popover:

```
🧲 Snapping
 ─────────────
 ☐ Snap to Grid     Shift+Ctrl+'   (Stage 2 — present, inert until grid lands)
 ☐ Snap to Pixel    Alt+Ctrl+Y     (Stage 2 — gated to px documents)
 ☐ Snap to Point    Alt+Ctrl+'     (key points)
 ☑ Smart Guides     Ctrl+U
     ☑ Alignment Guides
     ☑ Geometric Guides
 ─────────────
 ⚙ Snapping Preferences…           (opens the fuller panel — §2.6)
```
Each toggle pushes `Op::SnapCfg(SnapConfig)` (extend the `Op` enum `ui.rs:75`, `apply_ops` `ui.rs:1140`),
calling `ed.set_snap(cfg)` which writes `ed.doc.snap` — a **mode flag, not undoable** (mirror
`ab_set_move_art`, `editor.rs:654`). `Ctrl+U` toggles `snap.smart` via `main.rs`'s keyboard path. The
`check_row` + popup mechanics are already proven by the Panels checklist (`ui.rs:808-812`).

### 2.6 The fuller "Snapping Preferences" panel (Affinity layout — the advanced surface, Stage 2)

A disclosure / Preferences pane that exposes the whole `SnapConfig` in Affinity's three blocks, so power
users get the depth without cluttering the daily magnet menu:

```
SNAPPING                                    [ Preset: UI design ▾ ] [Manage…]
─ BEHAVIOUR ────────────────────────────────────────────────
  ☑ Enable snapping              Tolerance [ 8 ] px
  Candidates [ 8 ]               ☑ Show snapping candidates
  ☐ Force pixel alignment        ☐ Move by whole pixels
─ PAGE ─────────────────────────────────────────────────────
  ☐ Snap to grid                 ☐ Snap to baseline grid
  ☑ Snap to guides
  ☑ Snap to spread (page edges)  ☑ …include spread mid points
  ☐ Snap to margins              ☐ …include margin mid points
─ OBJECTS ──────────────────────────────────────────────────
  ☑ Only snap to visible objects
  ☑ Snap to bounding boxes       ☑ …include bbox mid points
  ☑ Snap to gaps and sizes
  ☑ Snap to shape key points     ☑ Snap to object geometry
  ☐ Snap to pixel selection bounds
```
**Presets** (Stage 2): named bundles of the whole panel state — ship 3–4 curated built-ins
(*Illustration · UI/pixel · Layout*) before any save/rename/delete CRUD. Most users pick an intent, never
touch individual toggles — this is the UX trick that makes a deep panel usable.

---

## 3. The TRANSFORM completeness pass — built + the gaps to add

**✅ Built (see §1):** oriented 8-handle frame; scale (Shift = aspect, Alt = from-center); rotate (Shift =
45°); numeric `set_obj_bbox` with 9-point reference; `set_obj_rotation`; `flip`; group-aware move; Alt-drag
duplicate; nudge / Shift-nudge ×10; align / distribute / distribute-spacing; all one-undo-step via
`begin()`/`commit()`.

**🟡 Gaps to add (each marked with its Stage):**

| Gap | What it is | Where it plugs in | Stage |
|---|---|---|---|
| **Live measurement HUD** | A small readout near the cursor, updated every frame: **move →** `dx, dy`; **scale →** `W × H` (+ %); **rotate →** `angle°` (+ Δ from start); **artboard create/resize →** `W × H`. Values already exist mid-drag — nothing draws them. | egui on-canvas `Area` pinned via `view.w2s` — the `build_ab_chrome` trick (`ui.rs:1089`); format via `units::format_pt` (`units.rs:101`). | **1** |
| **Center point / Show-Center** | A small crosshair at the selection's centre (Illustrator "Show Center"), tied to the Transform 9-point reference; also a snap target. Default ON. | overlay `Disc`/`Square` Prim at `obj_bbox` centre, in the transform-frame block (`scene.rs:117`). | **1** |
| **Numeric rotation live** | The rotation field shows the live angle *during* `Drag::Rotate` (readout), not only after the fact; the panel field drives `set_obj_rotation`. | read `obj_angle + d` mid-drag into the HUD + the panel `Snap::read` (`ui.rs:108`). | **1** |
| **Power-duplicate / Transform-again** | `Ctrl+D` repeats the **last transform delta** (move / rotate / scale / + Copy). If the last action was a *Copy* variant it steps-and-duplicates (build a row/array); if plain, it re-applies. Affinity fuses this as "Power Duplicate" (`Ctrl+J`); we bind Illustrator's `Ctrl+D`. | store `last_transform: Option<TfDelta>`; a new `transform_again()` op bracketed by `begin()/commit()`. | **2** |
| **Transform origin (movable + snappable pivot)** | A draggable rotate/scale pivot (default = bbox center): **drag to move it** (even outside the object), **double-click to reset to center**, and it **obeys snapping** (snaps to key points / centers / geometry like anything else — this is what makes hand-built radial arrays work). Persists between drags. Illustrator: Alt-click with R/S; Affinity: free draggable "Show Rotation Center"; Inkscape: persists per object, doubles as flip axis. | extend `start_transform` to seed pivot from a clicked/dragged (snapped) point instead of always `center`. | **2** |
| **Align to key-object** | Click an already-selected object once more (Selection tool, 2+ selected) to make it the **key** (thick highlight); others align to it, the key doesn't move. Illustrator's gesture — no menu. | extend `align`/`distribute` to take an optional `key: usize`; the extra-click gesture sets it. | **2** |
| **Transform-each** | Transform each selected object around **its own** reference point independently (vs one shared origin). Distinctive Illustrator behaviour; `Alt+Shift+Ctrl+D`. | a per-object loop over `align`-style ops. | **3** |
| **Inline math + mixed-unit expressions in *every* numeric field** | Every field accepts arithmetic + unit expressions: `100mm/2`, `+10`, `100+20`, `4*3mm`, `50%`; `Tab`/`Shift+Tab`/`Enter` commit semantics. Muscle memory for every Illustrator user; cheap, huge payoff — promoted to v1 by the web research (universal across IL/Affinity/Inkscape). | extend `num_field` (`ui.rs:425`) + `units::parse_to_pt`. | **1** |
| **Reference-point locator governs TYPED transforms only** | The 9-point reference (already built in `set_obj_bbox`) anchors **typed/panel** transforms (e.g. type a new W and the chosen handle stays put). It does **not** govern interactive drags — those use the live grab / pivot. Lock this rule explicitly so the two paths never fight. | the existing 9-pt `ax,ay` in `set_obj_bbox` (`editor.rs:466`); drags ignore it. | **1** |
| **Resize modifiers: Shift / Alt / Shift+Alt** | `Shift` = constrain proportions · `Alt` = from-center · **`Shift+Alt` = both**. Identical across IL/Figma/Affinity — bind verbatim. (`Shift` + `Alt` singly already built; ensure the *combined* path is covered.) | `Drag::Scale` modifier branch (`editor.rs:962`). | **1** |
| **Rotation snap to 15° on `Shift`** | Hold `Shift` while rotating to constrain to fixed increments. Figma/Affinity use **15°**; Illustrator uses its **Constrain Angle** increment (factory default; verify before binding per the standing rule). Spec's current built behaviour is 45° — reconcile to the verified IL increment. | `Drag::Rotate` snap step (`editor.rs:984`), today `snap45`. | **1** (verify increment) |
| **Rotate / Scale / Reflect tools (Shear = Stage 2)** | Dedicated single-key tools `R` / `S` / `O` (Reflect). **Shear** deferred to Stage 2 (no IL factory key). | tool routing + the existing transform setters. | **1** (R/S/O) · **2** (Shear) |
| **Move… dialog (numeric move)** | A numeric move dialog (`Shift+Ctrl+M`) — type exact dx/dy (or distance + angle), with **Copy** + **Preview**. Promoted to v1 (the simplest, most-used transform dialog). The fuller Rotate/Scale/Reflect/Shear dialogs stay Stage 3. | small modal driven by the move op. | **1** (Move) · **3** (others) |
| **Configurable nudge + big-nudge (two fields)** | Two independently configurable values (Figma "small / big nudge"); default to **Illustrator's Keyboard Increment + `Shift`=10×**. Arrows / `Shift`+Arrows already bound — this adds the two settings. | a `nudge_small`/`nudge_big` pref read by `nudge` (`editor.rs:1036`). | **1** (defaults) · settings UI **2** |
| **Scale Strokes & Effects toggle** | When scaling, optionally scale stroke widths + effects with the geometry — **off by default in Illustrator (classic gotcha)**. Ship an explicit toggle the moment strokes scale, so a 2× box doesn't silently keep a 1 px stroke (or silently double it). | a `scale_strokes: bool` pref consulted in `Drag::Scale` / `set_obj_bbox`. | **1** |
| **Pixel snapping — document-aware (only when doc units = px)** | The snap-to-pixel toggle (magnet menu) only engages when the document's units are **px**; in pt/mm/in documents it is inert (and ideally greyed). Prevents nonsensical pixel-quantising of print artwork. | gate `force_pixel_align` on `DocUnits.display == Px` (`units.rs`). | **1** (toggle, gated) |
| **Transform-each** | Transform each selected object around **its own** reference point independently (vs one shared origin). Distinctive Illustrator behaviour; `Alt+Shift+Ctrl+D`. | a per-object loop over `align`-style ops. | **3** |
| **Move / Rotate / Scale / Reflect / Shear dialogs** | The *full* numeric dialogs with **Copy + Preview** (beyond the v1 Move dialog above); `Alt`-click a transform tool opens it at the clicked origin; double-click the tool opens it at center. | new modal driven by the existing setters. | **3** |
| **Offset Path** | Offset (±), Joins (miter/round/bevel), miter limit — we already have the boolean/path engine, this is a small addition. | new path op. | **3** |

---

## 4. STAGES 1 / 2 / 3

### STAGE 1 — the excellent professional minimum

The set that makes Varos feel like an Illustrator-class tool the first time you drag something:
- **The SnapEngine seam + candidate pipeline** — the pure `snap()` choke-point wired into the 7 drag arms
  (§0.1); collect → cap-at-8 → score → snap-strongest; independent X/Y; hysteresis (acquire 8 / hold 11);
  snap-OFF == byte-identical to today.
- **Smart alignment guides** — magenta H/V lines (edges + centers) with end-tick marks; the reserved snap
  colour, 1 px device-snapped.
- **Equal gaps & sizes** — equal-spacing detection with `=` ticks; match-width/height while resizing.
- **Live measurements** — the HUD (dx,dy / W×H+% / angle).
- **Snap to** — object bbox edges/mids/centers · object **key-points** · object **geometry** (bbox-level) ·
  **segment midpoints** · **path intersections** · **artboard** edges/center/mids · "only snap to **visible**
  objects".
- **The magnet quick-menu** (§2.5) + **Ctrl+U** (Smart Guides) + the magnet-icon bar state.
- **Center point** (Show-Center) on the selection.
- **Transform completeness now-bits** — inline math + mixed-unit expressions in every field; resize
  `Shift`/`Alt`/`Shift+Alt`; `Shift`-rotate increment; R/S/O tools; the **Move… dialog** (`Shift+Ctrl+M`);
  two-field nudge config (IL defaults); **Scale Strokes & Effects** toggle; reference-point governs typed-only.
- **Feedback feel** — readable rounded pill behind on-canvas numbers; continuous ΔX/ΔY + position during the
  whole drag; **quick measure** (`Alt`-hover); the two-colour language reserved.
- **Snap-to-pixel toggle** present in the menu, **document-aware** (engages only when doc units = px; acts once
  the grid/pixel math lands — the `units.rs` round-trip is trivial so a basic pixel snap can ship in Stage 1 if
  Ahmed wants it on the daily surface).
- **Suspend-snap modifier** (momentary — hold **`Ctrl`** to disable mid-drag; resolve the `eff_tool` conflict
  per §8.1).

> Stage 1 explicitly **excludes**: the construction grid, baseline grid, persistent ruler UI, the
> guides-drag-out model, the full Snapping Preferences panel + presets, margins, pixel-selection bounds,
> transform-again / movable pivot / dialogs.

### STAGE 2 — grid · baseline grid · rulers · guides model & drag-out · full prefs panel · presets
- **Grid model + snap:** `grid: GridSpec { spacing_pt, subdivisions, visible }` in the document (serde);
  snap to grid intersections; the decorative dot grid becomes the *visual* of this same spec.
- **Baseline grid** (data + snap) for typographic rhythm.
- **Rulers (top/left):** screen-constant bars showing `DocUnits`; **live cursor position** indicator;
  right-click to change unit.
- **Guides model + drag-out:** `guides: Vec<Guide{ axis, pos_pt, locked }>` (serde); drag off rulers (Alt
  swaps orientation, Shift snaps to ticks); move / delete / lock / show-hide; numeric placement; make-guides
  from selection.
- **Full Snapping Preferences panel** (§2.6) + **presets** (curated built-ins).
- **Margins** model + snap; **transform-again** (`Ctrl+D`) + **movable pivot** + **align-to-key-object**;
  **inline math / mixed units** in number fields.
- **Transform panel proper:** X/Y/W/H/angle + 9-pt ref + link-ratio, extracted into the design system like
  the artboard panel widgets.

### STAGE 3 — advanced / print
- **Pixel:** Pixel Preview, per-object Align-to-Pixel-Grid, pixel-selection-bounds snap, stroke-parity
  half-pixel crispness.
- **Print:** snap to artboard **bleed / margins / safe-area**; per-artboard ruler **origin** (0,0 at page
  top-left); construction/angle guides (snap to a typed angle); polar/iso grids.
- **Transform:** **Transform-each** (per-object), the Move/Rotate/Scale/Reflect/Shear **dialogs** (Copy +
  Preview), **Offset Path**, **Free Transform** core (the `Ctrl`-after-grab shear/distort grammar),
  **Measure tool**.

---

## 5. Shortcuts — EXACT Illustrator (Windows) bindings
> STANDING RULE: every Varos shortcut must equal Illustrator's exactly. W3C `code` strings match `apply_key`
> in `varos-app/src/main.rs`. **Bold = not yet bound; bind to match Illustrator.**

> **Verified** against Adobe's official *Default keyboard shortcuts for Illustrator* (Windows) — bind verbatim.

| Action | Illustrator (Win) | W3C code | Status |
|---|---|---|---|
| Smart Guides on/off | **Ctrl+U** | `KeyU` + ctrl | **new (Stage 1)** |
| Snap to Point on/off | **Alt+Ctrl+'** | `Quote` + ctrl+alt | **new (Stage 1)** |
| Snap to Grid on/off | **Shift+Ctrl+'** | `Quote` + ctrl+shift | **new (Stage 2)** |
| Show/Hide Grid | **Ctrl+'** | `Quote` + ctrl | **new (Stage 2)** |
| Pixel Preview | **Alt+Ctrl+Y** | `KeyY` + ctrl+alt | **new (Stage 3)** |
| Show/Hide Rulers | **Ctrl+R** | `KeyR` + ctrl | **new (Stage 2)** |
| Show/Hide Guides | **Ctrl+;** | `Semicolon` + ctrl | **new (Stage 2)** |
| Lock/Unlock Guides | **Alt+Ctrl+;** | `Semicolon` + ctrl+alt | **new (Stage 2)** |
| Make Guides (from selection) | **Ctrl+5** | `Digit5` + ctrl | **new (Stage 2)** |
| Release Guides | **Alt+Ctrl+5** | `Digit5` + ctrl+alt | **new (Stage 2)** |
| Transform Again | **Ctrl+D** | `KeyD` + ctrl | **new (Stage 2)** |
| Transform Each | **Alt+Shift+Ctrl+D** | `KeyD` + ctrl+alt+shift | **new (Stage 3)** |
| Move dialog | **Shift+Ctrl+M** | `KeyM` + ctrl+shift | **new (Stage 1 — Move)** |
| Free Transform | **E** | `KeyE` | **new (Stage 2)** |
| Rotate · Scale · Reflect tools | **R · S · O** | `KeyR`/`KeyS`/`KeyO` | **new (Stage 1)** |
| Align panel | **Shift+F7** | `F7` + shift | **new (Stage 2)** |
| Group / Ungroup | **Ctrl+G / Shift+Ctrl+G** | `KeyG` (+shift) | ✅ |
| Constrain (45°/aspect) while drag | **Shift** (held) | `shift` mod | ✅ `snap45` / aspect |
| Transform a Copy while drag | **Alt** (held) | `alt` mod | ✅ `Drag::DupPending` |
| Scale/Rotate from center | **Alt** (held) | `alt` mod | ✅ `Drag::Scale` Alt |
| Nudge / Nudge ×10 | **Arrows / Shift+Arrows** | `Arrow*` (+shift) | ✅ `nudge` |
| Select (V) · Direct (A) · Artboard (Shift+O) | V · A · Shift+O | `KeyV`/`KeyA`/`KeyO`+shift | ✅ |

**Additive bindings — no Illustrator conflict (safe to introduce):**

| Action | Binding | Notes | Status |
|---|---|---|---|
| Suspend snapping during a drag | **hold `Ctrl`** | Figma convention; momentary — release re-enables. ⚠️ collides with `eff_tool` Ctrl-morph; resolve per §8.1. | **new (Stage 1)** |
| Quick measure | **`Alt`-hover** a 2nd object | dual-axis distance in a pill; nested children = **`Ctrl+Alt`**-hover. (Figma.) | **new (Stage 1)** |

> 📏 **STANDING RULE — match Illustrator exactly.** Every binding above mirrors Illustrator's factory Windows
> keys verbatim; the only non-IL bindings are the two *additive* rows, chosen because Illustrator leaves them
> free. Verify any future key against Illustrator before binding. (Earlier drafts misremembered Show/Hide Grid
> as `Ctrl+"` and claimed Snap to Grid/Point had no shortcut — both **corrected above** against Adobe's
> official list: Snap to Point = `Alt+Ctrl+'`, Snap to Grid = `Shift+Ctrl+'`, Show/Hide Grid = `Ctrl+'`.)

---

## 6. Rendering & Op notes (grounded in the Prim/Op + serde systems)

**6.1 Guides / center / HUD as overlay Prims (constant screen size).** `scene.rs::build_scene(ed, ppu)`
(`scene.rs:108-192`) is the single place chrome is emitted into `Scene.overlay`; `content` scales with zoom,
`overlay` stays constant px. Add a snap block right after the transform-frame block (`scene.rs:117-126`),
using the existing `Prim` kinds (`Stroke`, `Dashed`, `Disc`, `Square`, `scene.rs:22`) and a new `SNAP_MAGENTA`
const next to `ACCENT` (`scene.rs:12`):
```rust
for g in &ed.snap_guides {
    match g {
        SnapGuide::VLine{x,y0,y1} => s.overlay.push(Prim::Stroke{ pts: vec![[*x,*y0],[*x,*y1]], width:1.0, color:SNAP_MAGENTA }),
        SnapGuide::HLine{y,x0,x1} => s.overlay.push(Prim::Stroke{ pts: vec![[*x0,*y],[*x1,*y]], width:1.0, color:SNAP_MAGENTA }),
        SnapGuide::Spacing{..}    => /* gap pips + `=` ticks */,
        SnapGuide::Point{p}       => s.overlay.push(Prim::Square{ c:*p, half:4.0, color:SNAP_MAGENTA }),
    }
}
// center point — Show-Center
if let Some(c) = ed.obj_center() { s.overlay.push(Prim::Disc{ c, r:2.5, color:ACCENT }); }
```
Overlay positions are **world** coords; the renderer applies `view.w2s` and keeps overlay widths constant
(the transform handles at `scene.rs:121-124` are the template — the same accent-square idiom Affinity uses for
candidates). 1 px lines must be device-pixel-snapped (offset 0.5 in the overlay transform) or they blur.

**6.2 The measurement number → egui on-canvas label (the "trick").** Overlay `Prim`s can't draw text (there
is no text primitive yet). The HUD label is painted exactly like `build_ab_chrome` (`ui.rs:1089`): an
`egui::Area` pinned to a world point via `view.w2s`:
```rust
let tl  = view.w2s([wx, wy]);                 // world → physical px
let pos = egui::pos2(tl[0]/ppp, tl[1]/ppp);   // px → egui points
egui::Area::new(id).fixed_pos(pos).order(Order::Middle).show(ctx, |ui| {
    ui.painter().text(.., format!("{w:.0} × {h:.0}"), ..);   // units::format_pt for the unit
});
```
`run()` already receives `view: View` + `ppp` (`ui.rs:286`) and already calls
`build_ab_chrome(ctx, view, ppp, …)` (`ui.rs:330`) — add a sibling `build_snap_hud(ctx, view, ppp, snap)`
there, fed a tiny snapshot (the drag's measured dx/dy/w/h/angle or the spacing value) read off the editor the
way `Snap::read` (`ui.rs:108`) reads the bbox. Labels clamp to the viewport edge. No new infrastructure — this
is the exact mechanism the artboard name labels already ship with.

**6.3 `snap()` hooks per Drag arm.** Each relevant `Drag`/`AbDrag` arm calls `snap()` *after* the Shift
constraint and *before* writing the base→anchor delta (the table in §0.1), then stashes `self.snap_guides`
and the HUD value for the renderer. No tool code changes; the seam is the same place `snap45` sits.

**6.4 Transient snap-guide state.** There is no place yet — add it, mirroring `drag`/`ab_drag`/`hover_path`
(plain `Editor` fields cleared in `clear_transient`/`pointer_up`):
```rust
// editor.rs, on Editor (near hover_path:144) — never serialized
pub snap_guides: Vec<SnapGuide>,   // this-frame snap feedback (overlay)
pub snap_hud:    Option<Hud>,      // this-frame live measurement
```
`SnapGuide` lives in `editor.rs` or a new `snap.rs`, **not** in `model.rs` (transient UI, not document data —
same reasoning as `Drag`). **Populate** at the end of each snapping drag arm; **clear** in `pointer_up`
(`editor.rs:838`, beside `self.drag = Drag::None`), in `ab_up` (`:620`), and in `clear_transient` (`:710`)
so undo/redo and tool switches wipe them.

**6.5 Deferred-Op discipline.** Snapping only changes *where* a drag lands — it does **not** add commits. The
existing `begin()`/`commit()` bracket (set in `pointer_down`/`pointer_up`) already makes each gesture one undo
step. **Toggling a snap mode is a non-undoable mode flag** (mirror `ab_set_move_art`, `editor.rs:654`).
**Creating/moving/deleting a ruler guide IS undoable** (its own `begin()`/`commit()`), like `ab_set_rect`.

**6.6 Where every SnapConfig / guide / grid field lives in the serde model** (`model.rs Document`, all
`#[serde(default)]` for format stability):
- `snap: SnapConfig { … }` — the full §2 struct, already at `Document.snap` (`:127`); the only fix is
  per-field `#[serde(default)]` so additions never break old files. *Add tests*
  `snap_config_round_trips_via_serde` + `partial_json_fills_defaults` to the headless suite (allowed — pure
  serde/math, the `units.rs:156` template).
- `grid: GridSpec { spacing_pt: f32, subdivisions: u32, visible: bool }` — **Stage 2**. The rectangular grid
  draws as **dots** (Varos is already on a dot grid) with configurable **spacing + subdivisions**; the same
  spec drives both the visual and `grid` / `grid_lines` snapping.
- `guides: Vec<Guide { axis: Axis, pos_pt: f32, locked: bool }>` — **Stage 2**. Guides are **dragged off the
  rulers**, shown/hidden (`Ctrl+;`), and locked (`Alt+Ctrl+;`); see Shortcuts §5.
- Geometry stays in **pt**; tolerances are **screen px via `ppu`**; all HUD/ruler text formats through
  `units::format_pt` + `parse_to_pt` (the single conversion seam).

**6.6a Coordinate / origin convention — LOCK ONE, never deviate.** Rulers (`Ctrl+R`) and the grid/guide model
all assume a **single origin convention: top-left origin, Y-down**. Ruler origin is set by **dragging from the
top-left corner** and **reset by double-clicking** that corner (the clean discoverable interaction Illustrator/
Inkscape ship). Pick this once and never deviate — mixing origin conventions is the classic source of "why is
everything 0.5 px off" bugs. (Per-artboard vs global origin is a Stage 3 refinement; the *convention* is locked
now.)

**6.7 One correctness note for the Scale snap.** The live handle in `Drag::Scale` is computed in **local**
space (`lp = rotate_about(pos, [0,0], -angle)`, `editor.rs:966`), but snap targets are **world**-space. Snap
the handle in world and rotate the snapped result back to local (or rotate the candidate targets into local
first) — don't mix the two spaces, or rotated-selection snapping will drift.

---

## 7. PROFESSIONAL COMPLETENESS CHECKLIST
> `v1` = Stage 1 (ship-or-it-feels-unprofessional) · `S2` = Stage 2 · `S3` = Stage 3.

### A. Snap engine
- [ ] `v1` Independent snap-SOURCE vs snap-TARGET point sets modelled (UI exposes a subset — §0.35)
- [ ] `v1` Screen-constant tolerance (`radius_px/ppu`, default 8), zoom-invariant
- [ ] `v1` Candidate collect → cap-at-N (default 8) → score → snap-strongest (priority-dominant)
- [ ] `v1` Independent X/Y snapping
- [ ] `v1` Hysteresis (acquire 8 / hold ~11) — no jitter, winner-latch bias
- [ ] `v1` Suspend-snap momentary modifier (hold to disable, mid-drag)
- [ ] `v1` Master on/off (`enabled`) + Smart Guides master (`smart`, Ctrl+U)
- [ ] `v1` Per-source toggles in the magnet menu
- [ ] `v1` `visible_only` viewport cull (internal)
- [ ] `S2` Spatial index when object count demands it (O(log n))
- [ ] `S2` Candidate-max + tolerance exposed in the prefs panel
- [ ] `S3` Intersection candidates (guide×guide, edge×grid) with bonus

### B. Snap sources
- [ ] `v1` Object bbox edges + corners
- [ ] `v1` Object bbox mids + center
- [ ] `v1` Object key-points (anchors / handle ends)
- [ ] `v1` Object geometry — snap anywhere on a path edge (nearest-point-on-path, bbox-level)
- [ ] `v1` Object **segment midpoints** (between two adjacent nodes)
- [ ] `v1` **Path intersections** (where two edges cross; + guide×guide projections)
- [ ] `v1` Artboard/spread edges + center + mids (mids = dependent child of edges)
- [ ] `v1` Smart/alignment guides to other objects (edge + center)
- [ ] `v1` Equal-spacing (gaps) detection · Equal-size (match W/H) detection
- [ ] `v1` "Only snap to visible objects" (`visible_only`)
- [ ] `v1` User guides (snap — guide UI is S2)
- [ ] `v1` Single grid **line** (one axis), with the grid math
- [ ] `S2` Grid intersections
- [ ] `S2` Margins + margin mids
- [ ] `S3` Baseline grid
- [ ] `S3` Curve extrema · richer geometry (tangent/perpendicular/quadrant)
- [ ] `S3` Pixel-selection bounds

### C. Visual feedback
- [ ] `v1` Magenta alignment lines (H/V) with end-ticks
- [ ] `v1` Equal-spacing pips + `=` ticks
- [ ] `v1` Matched-size bar while resizing
- [ ] `v1` Continuous ΔX/ΔY + position readout the *whole* drag (not only on the snap)
- [ ] `v1` Live W×H (+%) while scaling
- [ ] `v1` Live angle while rotating
- [ ] `v1` Readable rounded **pill** behind every on-canvas number (Sketch)
- [ ] `v1` Two-colour language reserved (align/measure vs arrange/smart-select)
- [ ] `v1` Quick measure — `Alt`-hover dual-axis distance (nested = `Ctrl+Alt`)
- [ ] `v1` Center-point mark (Show-Center)
- [ ] `v1` All feedback screen-constant; vanishes when snap suspended
- [ ] `v1` `show_candidates` toggle (candidate halo)
- [ ] `S2` Snap-type glyphs (◇ anchor · ⊙ center · ⊥ on-path · # grid · ∥ guide)
- [ ] `S2` Gap distance numbers on every alignment line (needs text Prim)

### D. Transform & move
- [ ] `v1` Shift = constrain move 45° / aspect · Alt = from-center · **Shift+Alt = both** on resize (built/extend)
- [ ] `v1` Rotation snap on `Shift` (verified IL Constrain-Angle increment; Figma/Affinity = 15°)
- [ ] `v1` Alt = duplicate-drag · scale/rotate from center (built)
- [ ] `v1` Numeric X/Y/W/H + 9-point reference (built) · numeric rotation
- [ ] `v1` Reference-point locator governs **typed** transforms only (not interactive drags)
- [ ] `v1` Live rotation readout mid-drag
- [ ] `v1` Nudge (arrows) · big-nudge (Shift+arrows ×10) (built) · two configurable fields (IL defaults)
- [ ] `v1` Align / distribute / distribute-spacing (built)
- [ ] `v1` Rotate / Scale / Reflect tools (`R`/`S`/`O`) — Shear is S2
- [ ] `v1` Move… dialog (`Shift+Ctrl+M`, Copy + Preview)
- [ ] `v1` Inline math + mixed-unit expressions in every field (`100mm/2`, `+10`, `50%`)
- [ ] `v1` Scale Strokes & Effects toggle (off by default — IL gotcha)
- [ ] `v1` Pixel snapping toggle, document-aware (engages only when doc units = px)
- [ ] `S2` Transform-again (`Ctrl+D`) / power-duplicate
- [ ] `S2` Movable + snappable transform origin (drag pivot, double-click reset, obeys snapping)
- [ ] `S2` Align-to-key-object (extra-click gesture, key doesn't move)
- [ ] `S3` Transform-each (per-object reference point, `Alt+Shift+Ctrl+D`)
- [ ] `S3` Full Rotate/Scale/Reflect/Shear dialogs (Copy + Preview)
- [ ] `S3` Free Transform (`E`, Ctrl-after-grab shear/distort) · Offset Path

### E. Guides
- [ ] `S2` Drag guides off rulers (H+V, Alt swaps, Shift snaps to ticks)
- [ ] `S2` Show/hide guides (`Ctrl+;`) · lock guides (`Alt+Ctrl+;`) · move / delete · snap to guides (snap is v1)
- [ ] `S2` Numeric guide placement · make/release guides (`Ctrl+5` / `Alt+Ctrl+5`)
- [ ] `S3` Per-artboard guides · guide colour/style pref

### F. Grid
- [ ] `S2` Rectangular **dot** grid (configurable spacing + subdivisions) · show/hide (`Ctrl+'`) · snap
- [ ] `S2` Single grid **line** snap (one axis) · grid in document units, origin-aware
- [ ] `S3` Grid behind/front · independent X/Y spacing · iso/polar

### G. Rulers & units
- [ ] `v1` Document units (px/pt/mm/in) selectable — `DocUnits` (built)
- [ ] `v1` ONE origin convention locked: top-left, Y-down — never deviate (§6.6a)
- [ ] `S2` Rulers (top+left) show/hide (`Ctrl+R`) · live cursor position indicator
- [ ] `S2` Right-click ruler to change unit
- [ ] `S2` Ruler origin = drag-from-corner to set, double-click to reset
- [ ] `S3` Per-artboard ruler origin

### H. Pixel / export precision
- [ ] `v1` Snap-to-pixel toggle present in the magnet menu
- [ ] `S2` Force pixel alignment · move-by-whole-pixels
- [ ] `S3` Pixel Preview · per-object Align-to-Pixel-Grid · stroke-parity crispness

### I. Settings / preferences
- [ ] `v1` Magnet quick-menu (per-source toggles, Ctrl+U)
- [ ] `S2` Full Snapping Preferences panel (Affinity layout) + tolerance slider
- [ ] `S2` Curated presets (Illustration / UI-pixel / Layout)
- [ ] `S3` Smart-guide colour, angle-snap tolerance, sensitivity knob, preset CRUD

---

## 8. OPEN DECISIONS — settle before we build (recommendation given for each)

1. **Suspend-snap key — and the `eff_tool` Ctrl conflict.** The chosen binding (matching Figma, additive to
   Illustrator) is **hold `Ctrl` to suspend snapping during a drag**. But Varos already morphs the active tool
   on Ctrl via `eff_tool` in `editor.rs` (Ctrl temporarily enters Direct-select). A naive "Ctrl = suspend snap"
   would *also* swap the tool mid-drag — wrong. → **Recommend resolving so that, *once a drag is already
   active*, a held `Ctrl` suspends snap **without** re-entering Direct-select** (i.e. `eff_tool`'s Ctrl-morph is
   gated to the *idle / press-time* state, not consulted while `self.drag != Drag::None`). This keeps both
   behaviours — Ctrl morphs the tool when you press *to start*, and suspends snap when held *during* the drag —
   with no new key to learn. Confirm the gating point with Ahmed. The persistent `enabled` toggle stays in the
   magnet menu as the non-momentary off-switch.
2. **Tolerance default — 6, 7, or 8 px?** → **Recommend 8 px** (matches the existing grab radii; points get a
   ~1.3× bonus). Hand-feel call at multiple zooms; single global vs per-source — recommend **single global**
   for v1 (Affinity uses one), per-source later.
3. **Hysteresis ratio.** → **Recommend release = 1.6× acquire** (8 → ~11). Hand-tune; >2× feels like glue.
4. **Snap targets by tool (routing).** The `Drag` arm already tells us the intent: **V (Object) drags snap
   bbox edges/mids/centers** (Figma feel); **A/Pen drags snap the moving anchor to key-points / geometry /
   grid** (Illustrator feel). → **Recommend this routing** — it's free from the existing arms. Any case where
   V should also snap anchors?
5. **Equal-spacing scope.** Neighbours-only (cheap) vs full all-pairs (Figma-grade). → **Recommend
   neighbours-only for v1**, all-pairs later.
6. **Match-width/height default.** Valuable but can surprise ("why did it jump to 240?"). → **Recommend ON**
   with the matched-size bar making it legible; behind a toggle.
7. **Gentle rotate auto-snap without Shift.** A soft ~3° snap onto 0/45/90 always-on, or Shift-only? →
   **Recommend Shift-only for v1** (keep free rotation truly free); add the gentle cardinal snap later.
8. **What snaps to the artboard.** Edges + center + mids (v1), or also bleed/margins now? (`bleed` already
   exists in `Artboard`.) → **Recommend edges + center + mids only** for v1; bleed/margins Stage 2/3.
9. **Does snapping respect rotation?** When `obj_angle ≠ 0`, snap the oriented frame's edges (correct, but
   mind §6.7's space mix) or fall back to the axis-aligned `obj_bbox` (simple)? → **Recommend axis-aligned
   bbox snap in Stage 1**; oriented-edge snap deferred.
10. **Locked / hidden objects as snap targets.** Illustrator: guides yes, objects configurable. → **Recommend
    skip hidden + locked objects as targets in v1** (match `paths_on_ab`, `editor.rs:546`); revisit for
    guides-from-locked later.
11. **Multi-object snap source.** Snap from the group bbox (simple) or every member's features (powerful,
    noisier)? → **Recommend group bbox in v1**, all-features later.
12. **Snap colour.** Magenta is the Adobe/Figma convention. → **Recommend magenta `SNAP_MAGENTA`** distinct
    from `ACCENT`; confirm the exact hue reads on the dark board and offer a colour-blind alternate later.
13. **How much of the snap-SOURCE/TARGET model to expose in the v1 UI.** The engine models both sets fully
    (§0.35), but Inkscape/Corel's full per-point source picker is a *lot* of UI. → **Recommend: build the full
    engine, but in v1 expose only the Illustrator-feel subset in the magnet menu** (per-source *target*
    toggles, with source-set chosen automatically by tool — §8.4); surface the **Affinity-depth controls
    (independent source picker, candidate-max, per-source tolerance) under an "advanced" panel in Stage 2**
    (the §2.6 Snapping Preferences pane). Daily users never see the depth; power users find it when they look.
14. **Default screen tolerance.** (See also §8.2.) → **Recommend 8 px** as the shipped default for `radius_px`
    — one global value, screen-px via `ppu`, exposed 4–12 in the Stage-2 prefs panel.

---

## 9. Build order (once decisions are locked)
**Piece A** — `snap()` core + the §2 `SnapConfig` expansion (serde-defaulted, with the rename map), wired
into the 7 drag arms with snap-OFF == today; covered by `tests/snap.rs` (like `tests/math.rs`) +
`snap_config_round_trips_via_serde`. **Piece B** — object/artboard/key-point/geometry candidate generators +
the magenta smart-alignment guides, equal-spacing pips, matched-size bars in `overlay`; the magnet menu
(`Op::SnapCfg`) + Ctrl+U + center point. **Piece C** — the live measurement HUD (egui `build_snap_hud`) +
live numeric rotation. **Stage 2 onward** — grid + rulers + guides model & drag-out + full Snapping
Preferences panel + presets; transform-again / movable pivot / align-to-key-object / inline-math fields.

## 10. Risks / notes
- **The seam is the whole game.** If `snap()` is consulted in *one* place per drag arm and is pure, this stays
  clean; if snapping logic leaks into individual tools it rots — design the choke point first (§0).
- **Tolerance must be screen-px via `ppu`** or snapping feels wrong at different zooms — the code already has
  this discipline for every grab radius; reuse it.
- **Don't naive-snap.** Ship the collect → cap → score → strongest pipeline from day one; bolting scoring on
  later is a rewrite. Every feature is "another candidate generator".
- **Serde forward-compat:** the *only* load-bearing thing for old files is `Document.snap`'s `#[serde(default)]`
  (already there) plus per-field defaults — do the rename (`geometric→object_geometry`, etc.) in the same
  commit as the expansion; it's a one-symbol change in not-yet-built UI.
- **Text rendering is the one true dependency** for snap-type glyphs and ruler ticks; the HUD sidesteps it via
  the egui `Area` label (§6.2), so Stage 1 isn't blocked.
- **Don't double-undo:** snapping must not add commits; only guide create/move/delete are their own undo
  steps. Keep mode toggles non-undoable (mirror `ab_set_move_art`).
