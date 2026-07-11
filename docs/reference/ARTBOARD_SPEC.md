> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Artboard System Spec (for review with Ahmed)

> Synthesised from a 4-lens study (Illustrator standard · Figma/Affinity feel · Varos code · interaction)
> plus the live code. **Base standard = Illustrator; borrowed feel = Figma/Affinity.** v1 = Illustrator-class.
> We review this line-by-line, settle the open decisions, THEN build — one Stage-1 piece at a time.

## ✅ DECISIONS LOCKED — 2026-06-30 (Ahmed)
1. **Clip:** a per-artboard **Clip toggle** (button) in the Properties panel — clip the overflow on canvas or
   not. **Default OFF** (Illustrator). It's a control, not a fixed rule.
2. **Not a z-object** — confirmed (own tool, own list; never in the path order / Layers).
3. **Move artwork with artboard** — a toggle, **default ON**.
4. **Multiple artboards = NOW** (not deferred). Since no real `.varos` files exist yet, change the model to
   `Vec<Artboard>` + active index immediately (no migration).
5. **Default size = SQUARE** (1080×1080). A categorised new-file size **modal** (logo / poster / web / print)
   is FUTURE; for now a standard-size preset dropdown is enough.
6. **Page colour** = white default, **changeable** per artboard.
7. **Units** = px. **Tool shortcut** = **Shift+O**.
8. **Editing is gated** (deliberate limitation): the artboard is editable ONLY when the **Artboard tool
   (Shift+O)** is active, OR via the **⋮ menu** on its on-canvas label. Name label sits **top-left**; a **⋮**
   button on the **right** of the label opens edit. Editing must be **easy**.
9. **Property panel (now):** standard-size preset · W/H box · portrait/landscape icons · background colour ·
   **artboard count** · Clip toggle · "Move artwork with artboard" toggle.

**Post-review refinements (planning team — accepted 2026-06-30):**
10. **`bleed` in the model NOW** (data only, default 0pt): the artboard *is* a PDF page; print needs
    bleed / TrimBox. Add the field today, with the `Vec` change, so multi + bleed never force a second format
    migration. Its UI is Stage 3.
11. **Page colour = `Option<Rgba>`**: `None` = **transparent** (for PNG export with no background). Default
    `Some(white)`. On-canvas a transparent page shows just its edge (a faint checkerboard can come later) so it
    stays locatable on the dark board.
12. **Default size fixed to SQUARE 1080×1080** everywhere — kills the stale 1920×1080 wording further down.
13. **"Move artwork with artboard" default ON matches BOTH** Illustrator (its control-bar toggle ships ON) and
    Figma, so decision #3 is on-model. The spec must say exactly *which* art moves: artwork whose **bounds
    intersect the artboard at grab time**.
14. Piece-C panel widgets (number box · swatch · toggle) are **built here and extracted** into the design
    system afterwards (the deliberate first use).

---

## 0. The core model — the invariant to LOCK first

- **Board** = the infinite dark dotted canvas (already built). **Artboard** = a *defined page* (rectangle)
  sitting on it, in world **points** (1pt = 1/72in), top-left origin, +y down (the `units.rs` contract).
- The artboard is **page furniture, NOT a selectable object**: never in the path z-order or a Layers row;
  objects are **never *contained*** by it (the Illustrator model). An object's artboard is decided only at
  **export** time by bounds overlap. *(Recommended — confirm in §2.)*
- The artboard is **inert to the Object (V) / Direct (A) / Pen / shape tools** — clicks over the page edge
  pass through to artwork / marquee, exactly like Illustrator. It has its **own tool** (Artboard, **Shift+O**)
  that is the ONLY way to create / select / move / resize / rename / recolour it.
- **The seam (from the code):** add a `ToolKind::Artboard` variant and an `ab_sel` + `ab_drag` editor state
  **parallel** to the object selection (`objsel` / `selected`), scoped to the artboard rect — so the two
  *never cross-grab*. The Artboard tool **mirrors the existing** frame + 8-handle + Scale machinery
  (`editor.rs` `Drag::Scale`, `frame_corners` / `frame_handles`) but applies it to the artboard rect, never
  to artwork. Today the page is already drawn (white Fill in `content` + a screen-constant edge hairline in
  `overlay`, `scene.rs`).

## 1. Where the code stands today (grounding)
- ✅ `Artboard { x, y, w, h, name }` in pt (`model.rs`); `DocUnits { ppi, display }` (`units.rs`);
  page rendered (white Fill + 1px edge); fit math (`View::fit`).
- ➕ Piece A adds to the model: `artboards: Vec<Artboard>` + `active: usize`, and per-artboard
  `bleed: f32` (pt, default 0) + `page_color: Option<Rgba>` (`None` = transparent). All `#[serde(default)]`
  so the `.varos` format stays stable.
- 🔴 No `ToolKind::Artboard`, no artboard selection/drag, no property panel, no presets, no multi-artboard.

## 2. Three places Illustrator and Figma disagree — pick one (recommendations marked)
1. **Clip content that overflows the page?** Illustrator **NO** · Figma yes → **recommend NO** (clip is an
   export-time option only; on-canvas the art bleeds past the edge freely).
2. **Is the artboard a selectable object in the z-stack / Layers?** Illustrator **NO** · Figma yes →
   **recommend NO** (its own tool, its own list; not mixed into the path order).
3. **Does moving the artboard move the artwork on it?** Illustrator = a **"Move artwork with artboard"
   toggle** · Figma = always → **recommend a toggle, default ON** (Figma feel) but switchable.

---

## 3. STAGE 1 — one real, excellent single artboard
*(stays inside today's single-`Artboard` model — no multi yet)*

**Tool & selection**
- `Artboard` tool, **Shift+O**. Entering it: hide object selection; show the artboard with a highlighted
  **bounds + 8 resize handles** + a **name label** at the top-left corner.
- **Select:** click the artboard (only in this tool) → it becomes active; its X/Y/W/H fill the property panel.
- Leaving the tool (V/A/…): handles/label disappear; the page is inert again.

**Edit**
- **Resize:** drag the 8 handles (mirror `Drag::Scale`); **Shift** keeps the ratio; a live **dimensions HUD**
  by the cursor shows W×H while dragging *(Figma borrow)*.
- **Move:** drag inside the artboard (this tool only) → moves the page; the §2.3 toggle decides if art follows.
- **Numeric X/Y/W/H** in the panel with a **9-point reference** (reuse the Transform refpoint), in the
  artboard's local origin; values shown in `DocUnits` (px default), typed units accepted (`10mm`, `2in`).
- **Rename:** inline field in the panel + **double-click the on-canvas label**.
- **Page colour:** the page fill — default **white**, changeable via a swatch in the panel.

**Presets & navigation**
- **Preset list** in the panel: a starter set — *1080×1080 (square, **default**), 1920×1080 (screen), A4,
  Letter, 1080×1920 (story)* + **"fit to artwork bounds"**.
- **Fit Artboard in Window** — a button in the zoom control (since `Ctrl+0` is awkward on a 60% keyboard)
  + the shortcut where it works.

**Property panel (Stage-1 contents, Illustrator layout in Varos dark skin)**
- Header: "Artboard" + name field · Presets dropdown · W / H (+ link) · X / Y (9-pt ref) · Orientation
  (portrait/landscape swap) · Page colour swatch · "Move artwork with artboard" toggle.

## 4. STAGE 2 — multiple artboards
- **Model change:** `artboard: Artboard` → `artboards: Vec<Artboard>` + `active: usize`
  (serde migration: keep reading the old single field via `#[serde(default)]`, write the new shape).
- **Create:** drag with the Artboard tool to draw a new one (preset snapping while dragging).
- **Artboards panel:** list (name + thumbnail), **add / delete / duplicate / reorder**, **Rearrange All**
  (lay them in a grid), double-click → rename / fit to that artboard.
- **On canvas:** every artboard labelled; the **active** one highlighted; sensible default gaps.
- Next/prev artboard navigation.

## 5. STAGE 3 — advanced / print
- Bleed + print/crop/registration marks · per-artboard **export** targeting · video-safe area / center mark /
  crosshairs · per-artboard ruler **origin** (0,0 at the artboard's top-left) · a fuller presets library ·
  full Artboard Options dialog.

## 6. Rendering & engine notes (grounded in the Prim/Op system)
- Page **fill** → `content` (scales with zoom). Page **edge + handles + name label + active highlight** →
  `overlay` (constant screen size). Reuse the existing `Prim` kinds; add a text label primitive only when the
  text engine lands, else a simple boxed label for now.
- All artboard edits flow through the existing **deferred-Op** bus, bracketed by `begin()`/`commit()` so each
  is one undo step.
- Keep `ab_sel` / `ab_drag` strictly separate from `objsel` / `selected` (the no-cross-grab guarantee).

---

## 7. OPEN DECISIONS — ✅ all settled
Nothing left open (see the LOCKED block + planning-team refinements at the top). Defaults: **square 1080×1080** ·
page colour `Option<Rgba>` (white default, `None` = transparent) · **multi NOW** (`Vec<Artboard>` + `active`) ·
Shift+O · px · top-left name label · "move artwork with artboard" default **ON** · `bleed` field from day one.
Build order: **Piece A** (model + multi render) → **Piece B** (Artboard tool) → **Piece C** (panel + ⋮ menu).

## 8. Risks / notes
- The biggest care point: the **no-cross-grab** seam (artboard selection must never leak into object
  selection or vice-versa) — design `ab_sel`/`ab_drag` cleanly up front.
- The single→multi model change (Stage 2) is a serde migration — plan the format so it's painless.
- A real on-canvas **name label** wants text rendering; until the text engine exists, use a small boxed
  label drawn from rectangles, or an egui-drawn label pinned to the artboard's screen position.
