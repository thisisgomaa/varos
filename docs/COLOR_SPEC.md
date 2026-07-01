# Varos — Colour System SPEC

Render-agnostic core (no wgpu in the model), hand-painted UI, Illustrator-exact behaviour & shortcuts.
Replaces the two `color_edit_button_srgba` call-sites (ui.rs:577, ui.rs:1144) with a custom picker, then
layers Swatches and Gradients on a serde-compatible model evolution.

**Current baseline (verified in tree):**
- `geom.rs:5` `pub type Rgba = [f32;4]` — single opaque colour per target (4th channel is alpha).
- `model.rs:17` `Path { fill: Option<Rgba>, stroke: Option<Rgba>, stroke_width, opacity, holes, hidden, locked, name }`.
- `editor.rs:185` `cur_fill / cur_stroke: Option<Rgba>`, `cur_sw`, `paint: PaintTarget` — active-target + defaults already exist.
- `apply_paint` copies fill/stroke between paths; `scene.rs` multiplies colour alpha by `p.opacity`.

---

## 1. Decisions to lock (Ahmed confirms)

Each is a recommendation + one-line rationale.

1. **Alpha lives in the colour, keep object-opacity too.** `Rgba`'s 4th channel is already alpha and the
   renderer multiplies it by `p.opacity` — expose per-colour alpha in the picker now; keep `opacity` as the
   separate object-level multiplier (like Illustrator's Transparency panel). *No model change; both already
   coexist in the render path.*

2. **Store canonical RGBA; keep HSV only as picker UI state — do NOT persist HSB.** The model stays `Rgba`
   float. The picker holds a live `PickerColor { h,s,v,a }` as its single source of truth while open,
   converting to `Rgba` on apply. *Avoids a serde migration + a second source of truth; solves the "hue
   resets at S=0" bug locally in the widget.*

3. **Fields = RGB 0-255 + Hex + HSB, all three visible (Illustrator/PS), not a Figma dropdown.** *Standing
   rule = match Illustrator; it shows all rows. CMYK / Web-Safe / gamut warnings = skip (print heritage).*

4. **Defer global/spot swatches to Later; Stage 2 ships plain saved colours first** — but design the paint
   enum now so the jump is additive. *Global-swatch linking (object holds a ref, renderer resolves) is the
   biggest model change — do it once, correctly, not rushed into v1.*

5. **Gradients: store as a `Paint` enum on the path (linear/radial stop ramp + midpoints), freeform later.**
   Replace `fill: Option<Rgba>` with `fill: Paint` where `Paint::Solid(Rgba)` is today's behaviour.
   *`Option<Rgba>` → `Paint` is the one structural change that unlocks both swatch-refs and gradients; land
   it at the Stage 2/3 boundary with a serde alias so old files load.*

6. **Picker is a non-modal popover anchored to the swatch, live-apply while dragging, one coalesced undo.**
   *Matches Illustrator's inline Color panel (F6) + our docked swatches; the modal Color Picker dialog is the
   dated path we skip.*

7. **Shortcuts (Illustrator-exact): `X` toggles fill/stroke focus · `Shift+X` swaps them · `D` default
   (fill white / stroke black — the bare key `D`, NOT Ctrl+D which is Transform Again) · `/` sets None.**

---

## 2. Model deltas (serde-compatible, per stage)

**Stage 1 — no model change.** Add only editor-side ephemeral state:
```rust
// editor.rs — not serialized
pub recent_colors: Vec<Rgba>,   // MRU, deduped, cap ~12
pub last_solid: [Rgba; 2],      // [fill, stroke] last non-None, so None can round-trip back
// document colours are DERIVED: scan doc.paths on demand, not stored.
```

**Stage 2 — introduce the paint enum + swatch store (the load-bearing migration):**
```rust
pub enum Paint { None, Solid(Rgba), SwatchRef { id: SwatchId, tint: f32 }, Gradient(Gradient) /*Stage 3*/ }
// Path.fill / .stroke migrate Option<Rgba> → Paint via a serde adapter (None→Paint::None, Some→Paint::Solid)
pub struct Swatch { id: SwatchId, name: String, color: Rgba, ctype: SwatchColorType, builtin: bool }
pub struct ColorGroup { id: u32, name: String, members: Vec<SwatchId> }
// Document += swatches: Vec<Swatch>, groups: Vec<ColorGroup>  (both #[serde(default)]); seed [None]+white+black.
```
Invariant: apply of a *local* swatch → `Paint::Solid` (snapshot, unlinked); *global/spot* → `Paint::SwatchRef`.
Renderer resolves `swatch.color * tint` at draw — never writes colour back into paths.

**Stage 3 — gradient payload (SVG-shaped, freeform deferred):**
```rust
pub enum GradKind { Linear, Radial }
pub struct GradStop { location: f32, color: Rgba, opacity: f32 }   // >=2, sorted by location
pub struct Gradient { kind, stops: Vec<GradStop>, midpoints: Vec<f32>, /* + linear axis / radial geometry */ spread }
```
`scene.rs` gains a gradient tessellation branch beside `Prim::Fill/Stroke`; wgpu stays out of the model.

---

## 3. Staged build plan (one excellent piece at a time)

### Stage 1 — Hand-painted picker (v1) — replaces the egui default
- Custom popover: **SV square** (S→x, V→y) + **vertical hue rail** + **alpha rail on a checkerboard**.
  Hollow-ring SV thumb; capsule rail thumbs; white-with-dark-edge for contrast.
- **Hex** (3/6/8-digit incl. `RRGGBBAA`, commit on Enter/blur) + **RGB 0-255** + **HSB** fields, two-way
  synced; **HSV is the single source of truth** (preserve H when S→0, H&S when V→0).
- Field touches: arrow ±1, **Shift+arrow ±10**, Tab/Shift+Tab, select-all-on-focus, click-to-jump +
  pointer-capture clamping.
- Wired to `cur_fill/cur_stroke` + selection via `apply_paint`; **recent-colours** MRU strip +
  **document-colours** strip (derived scan) + **None** slash swatch.
- **Shortcuts:** `X` / `Shift+X` / `D` / `/` exactly as Illustrator.
- Live-apply while dragging, coalesced into one undo (begin on down, finalize on up).

**Acceptance feel:** click swatch → popover under it; drag the SV square → selection recolours live with a
*single* undo; hue never snaps back to red at low saturation; typing hex updates thumb + RGB/HSB instantly;
`X/Shift+X/D//` behave exactly like Illustrator. No egui default widget visible anywhere.

### Stage 2 — Swatches panel
Hand-painted panel (thumbnail + list views), New/Edit/Duplicate/Delete/apply, drag-reorder,
`[None]`+white+black builtins (immutable), colour groups (folders), "Add to Swatches" from the picker.
The `Paint` enum migration lands here; global/spot flag present but inert (all local for now).

### Stage 3 — Gradients (panel + on-canvas tool)
Gradient panel (Linear|Radial, fill/stroke toggle, ramp with house-shaped stops + midpoint diamonds,
per-stop colour/opacity/location, angle, radial aspect/focal, Reverse). Gradient Tool (`G`) with on-canvas
annotator: drag = direction+length, Shift = 45° snap, Alt-drag stop = duplicate, drag-out = delete.

### Later
Global/spot colours (live `SwatchRef`, edit-propagates, tints) · Recolor Artwork / harmony · eyedropper
loupe · freeform gradients · `.ase` / Pantone libraries. **Skip:** CMYK / Web-Safe modes + gamut warnings.

---

## 4. Feature matrix

| Feature | Tag |
|---|---|
| SV square + vertical hue rail + alpha rail (hand-painted) | v1 |
| Hex (3/6/8-digit) + RGB-255 + HSB fields, HSV source of truth | v1 |
| Arrow ±1 / Shift+arrow ±10, click-to-jump, pointer-capture clamp | v1 |
| `X` focus / `Shift+X` swap / `D` default / `/` None | v1 |
| Recent-colours MRU strip · Document-colours strip · None swatch | v1 |
| Live-apply while dragging, single coalesced undo · non-modal popover | v1 |
| `Paint` enum migration (Option<Rgba> → Paint, serde-compat) | Stage 2 |
| Swatches panel: new/edit/dup/delete/apply/reorder + builtins + groups | Stage 2 |
| Linear + Radial gradients (model + render) | Stage 3 |
| Gradient panel (ramp/stops/midpoints/opacity/reverse) + Gradient tool (`G`) | Stage 3 |
| Global/spot swatches + live SwatchRef + tints · Recolor · eyedropper loupe · freeform gradients | Later |
| CMYK / Web-Safe modes · out-of-gamut warnings | skip |

---

*Key files:* the picker replaces `ui.rs:577` and `:1144`; helpers `rgba_to_c32`/`c32_to_rgba`/`hex_of` fold
into the new widget's HSV↔RGB↔hex conversions; editor state extends `editor.rs`; the Stage-2 `Paint` enum
lands in `model.rs` and threads through `scene.rs`.
