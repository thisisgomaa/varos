> **Status:** historical — Preserved project history; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — "Professionalize the Panels" stage (from Ahmed's Illustrator references)

> **Goal: NO new tools.** Finish + professionalize the panels that already exist, to Illustrator's
> standard, in **Varos's dark skin** (colors/radii/fonts from `UI_FIGMA_SPEC.md`) — NOT Illustrator's grey.
> Source = 6 real Illustrator screenshots Ahmed gave (described inline below). **Most of this already
> exists in `varos-core/src/editor.rs`** — this stage is mostly UI structure + the color system + pro number inputs.

> ⚠️ **Reconciliation:** This **supersedes the right-dock "Design | Layers" tabs** in `UI_FIGMA_SPEC.md 2`.
> The right dock becomes Illustrator's two-group structure (below). Everything else in UI_FIGMA_SPEC
> (palette, 12px radius, floating shadow, Inter/JetBrains Mono, the cursor set) **still applies.**

> **Tag legend:** `EXISTS` = engine fn already in editor.rs, just wire UI · `NEW-UI` = webview only ·
> `NEW-SMALL` = small model/engine add · `DEFER` = not this stage.

> 🏃 **Working style (Ahmed's call, 2026-06-26):** this stage is **cosmetic/visual**, so **build the WHOLE
> stage autonomously — do NOT stop to show Ahmed after each control or each step.** Match the design that
> was made for him (his Figma `97:273` + the dark skin in `UI_FIGMA_SPEC.md`). Show him the **finished
> result** (or at most a couple of natural milestones). Per-step verification is only for risky vector
> *interaction* — not for panel styling.

## 0.1 ARCHITECTURE DECISION (2026-06-26) — SOLID DOCKED, not floating-transparent
The "multiple floating web windows over the GPU canvas" approach is **abandoned.** It hits two hard
Windows limits with child WebView2 windows over a GPU surface: (a) **transparency doesn't composite**
(opaque black rectangles instead of the canvas showing through), and (b) **a window can't paint outside
its rectangle** (dropdowns/menus get clipped). These are architectural, **not CSS-fixable.**

**Decision (Ahmed approved) — go SOLID DOCKED:**
- Panels are **solid (opaque)** and **docked to the window edges** (top bar · left tool rail · right dock). No transparency over the canvas.
- The **wgpu canvas stays a NATIVE surface in the center**; pointer input goes **straight to native**. This **protects the pen feel** (our #1 validated asset). **Do NOT route canvas input through the web layer.**
- Any popup that must overlay the canvas (Color Picker, etc.) = its **own borderless window** (the Color Picker already works this way). Keep panel dropdowns inside their docked panel bounds.
- Everything else in this spec + `UI_FIGMA_SPEC.md` still holds: the Illustrator-style organization, the dark Varos palette/fonts, and the cheap show/hide flexibility (0.5).

**Dropped for now (revisit post-v1 ONLY via a pen-feel spike):** the floating-glass / canvas-shows-through-panels
look (= one fullscreen webview + IPC input forwarding). Not now — the pen-feel risk is unverified, and we
don't bet the crown jewel. *(Ahmed's standing rule: "if floating isn't working out, cancel it" — it isn't, so it's cancelled.)*

## 0.2 SPACE EFFICIENCY — kill the wasted black (Ahmed flagged 2026-06-26)
The chrome eats too much canvas. Two causes: (1) the top bar is **two stacked rows** (`TOP_H=88`: a 36px
tab row + a 48px floating-control-pill row); (2) the right panels render as **floating cards inside a
300px dock**, so there's black margin AROUND the cards AND empty black BELOW them — double waste. Fixes:
- **Top bar → ONE slim row.** `TOP_H: 88 → 44`. Document tab(s) + a compact contextual control
  (Align / X / Y / Rotate — only when something is selected) + status dot + Share, all in a single row.
  **Drop the separate floating control-pill row.** (X/Y/Rotate also live in Properties, so nothing is lost.)
- **Right dock → narrower + FLUSH, not floating cards.** `DOCK_W: 300 → ~248`. Panel content fills the
  dock **edge-to-edge** (like Figma / Illustrator real docks) — **no inner card margins, no rounded
  floating cards inside a black dock.** Removes the black around the cards.
- **Consolidate to ONE right panel** (Ahmed's call — **supersedes the two-group A/B split in 1**):
  `Align` + `Pathfinder` become **collapsible sections inside the Properties panel** (or a compact tab
  strip at the panel top) — one continuous flush panel, not two stacked cards with a gap.
- Keep the **`Panels` toggle** (already present) to hide the whole right dock → full-bleed canvas on demand.

Net: ~44px more canvas height + ~52px more width, and the layout reads as intentional, not wasteful.

## 0.5 Panel flexibility — CHEAP TIER ONLY (approved 2026-06-26; full docking DEFERRED)
Build every panel (Transform, Align, Pathfinder, Properties, Layers, Color, Swatches) as an
**independent, self-contained module**. Then, this stage only:
- **Show / hide each panel individually** — a **Window menu** + a close `✕` on each panel. (`NEW-UI`, cheap.)
- Ship the **good default layout** in 1 (the Illustrator-style arrangement from Ahmed's screenshots).
- **Seam rule:** give each panel a standard interface so a real **docking system can be added LATER
  without rewriting the panels.**
- 🚫 **DO NOT build now (post-v1):** the full drag-to-dock / tear-off / tab-stacking / resizable-splitter
  / save-workspace system. That is a separate large project (weeks + a panel re-architecture). Out of scope.

Net this stage: clean modular panels + per-panel show/hide + a solid default layout. Free rearranging comes later.

---

## 1. RIGHT DOCK — two stacked floating panel groups (Varos dark skin)

### GROUP A (top): tabs `Transform · Align · Pathfinder`  (＋ panel menu ≡)
**Transform tab** — 9-point reference selector · `X` `Y` `W` `H` · rotation `∠` · flip-H / flip-V.
- `EXISTS`: `obj_bbox`, `start_transform`, bbox handles. **The numeric fields must become editable** (see 4). Flip = `NEW-SMALL` (reflect selection H/V).

**Align tab** (exact layout from screenshots 1 & 3):
- **Align Objects:** 6 buttons — horiz: left / center / right · vert: top / middle / bottom. → `EXISTS` `align(mode)`.
- **Distribute Objects:** 6 buttons — vertical distribute ×3 · horizontal distribute ×3. → `EXISTS` `distribute(axis)`.
- **Distribute Spacing:** 2 buttons (vert / horiz) + a `px` value field. → `NEW-SMALL` (distribute by an exact gap).
- **Align To:** 3 buttons — Selection / Artboard / Key Object. → Selection = `EXISTS`. Artboard = `DEFER` (no artboard yet). Key Object = `DEFER`.

**Pathfinder tab** (IL has two rows):
- **Shape Modes (build now):** Unite · Minus Front · Intersect · Exclude. → `EXISTS` `pathfinder(op)` + `boolean.rs`.
- **Pathfinders:** Divide · Trim · Merge · Crop · Outline · Minus Back. → Divide first (likely `EXISTS`), rest `DEFER`.

### GROUP B (bottom): tabs `Properties · Layers · Libraries`  (＋ menu ≡)

**Properties tab** — contextual (screenshot 5). It is a **reflow of existing controls**, no new engine:
- Header: selected object type (e.g. "Rectangle").
- **Transform** section (same controls as Group A Transform).
- **Appearance** section: Fill swatch · Stroke swatch + weight (stepper) · Opacity %. → `EXISTS` paint + `NEW-SMALL` opacity field if missing.
- **Align** condensed row (convenience mirror of the Align panel).
- **Pathfinder** row — appears only when 2+ shapes are selected (convenience mirror). → `EXISTS`.
- **Quick Actions** (Offset Path / Expand / Arrange / Align to Pixel Grid / Recolor / Global Edit / Generative) → **`DEFER` ALL** (new features; Ahmed: no new tools). Only keep **Arrange** (z-order) since `arrange()` `EXISTS`.

**Layers tab** (screenshot 1):
- Top: **Search** field + filter (funnel) icon.
- Rows: `eye` (visibility) · expand `>` (nesting) · thumbnail · name (double-click = rename) · target circle · selection-color chip.
- Footer: new layer · new sublayer · make/release clip · locate · delete.
- → `EXISTS`: object/group tree + two-way selection sync. `NEW-SMALL`: per-object **visibility** + **lock** flags, inline **rename**. `NEW-UI`: search/filter, thumbnails, footer.

**Libraries tab** → `DEFER` (cloud assets, irrelevant offline). Show the tab as a stub or hide it.

---

## 2. LEFT TOOLBAR — floating vertical column (screenshot 2)
- Detach the tool rail from the edge → **floating rounded column** on the left (Varos floating style, UI_FIGMA_SPEC 3).
- Tools stay as-is (no new tools). 
- **Bottom cluster = the Fill / Stroke control** (the thing Ahmed pointed at):
  - **Fill square** overlapping **Stroke square**; **Swap** arrow (↩, `Shift+X`) top-right; **Default** (small black/white, `D`) bottom-left.
  - Row under it: **[Color]** · **[Gradient]** · **[None ⃠]** — active paint type. Color + None = `EXISTS` (`apply_paint(None)`); Gradient = `DEFER` (show disabled).
  - (IL's three draw-mode buttons below = `SKIP`.)
- Clicking the Fill or Stroke square → opens the **Color Picker** (3).
- → `EXISTS`: `PaintTarget` (Fill/Stroke), `swap_paint`, `apply_paint`, `swap_colors`. Wire keys: `X` focus · `Shift+X` swap · `D` default · `/` none.

---

## 3. COLOR SYSTEM (screenshot 4) — the pro picker
**Color Picker dialog:**
- Big **Saturation/Value square** + vertical **Hue bar**.
- Live, mutually-linked fields: **HSB** (H° / S% / B%) · **RGB** (0–255) · **Hex** (#) · **CMYK** (C/M/Y/K %).
- new-vs-current preview swatch · **Color Swatches** button (→ Swatches) · OK / Cancel.
- Color math: HSV↔RGB↔Hex exact. RGB↔CMYK **approximate** (no ICC profile — for parity, display only).
**Docked Color panel** (compact): hue bar + hex + active mode, for quick edits without the dialog.
**Swatches:** a saved-colors grid — add current, click-to-apply, remove. Store in the document (or app prefs).
- All number fields here use the 4 pro-input behavior.

---

## 4. PRO NUMBER INPUTS — apply to EVERY numeric field
(Transform X/Y/W/H, rotation, stroke weight, opacity, spacing, all color fields.) One reusable webview component:
- **Type** to set (commit on Enter / blur).
- **Scrub:** click-drag horizontally on the field (or its label) → changes the value (Figma/After-Effects style). `Shift` = ×10 step, `Alt` = fine (÷10).
- **Mouse wheel** over the field → ±1 (`Shift` = ±10).
- **Arrow ↑/↓** → ±1 (`Shift` = ±10).
- Respect min/max + unit suffix (px / ° / %). Pure JS in the webview — build once, reuse everywhere.

---

## 5. SKIN (do not skip)
Use `UI_FIGMA_SPEC.md`: palette `#141313 / #262627 / #2c2c2c / #e6e6e6 / #8a8a8a / #0c8ce9`, panels = floating
rounded **12px** cards with the floating shadow, **Inter** UI + **JetBrains Mono** numbers, base 13px.
**Match Illustrator's STRUCTURE, Varos's COLORS.** Never the Illustrator grey.

---

## Build order (no new tools — finish what exists; build the whole stage, then show Ahmed the result)
1. **Pro number inputs** (4) — reusable; unlocks every field at once.
2. **Color system** (3) — Color Picker (HSB/RGB/Hex/CMYK) + docked Color panel + Swatches + the toolbar Fill/Stroke swatch (2 bottom).
3. **Right-dock restructure** (1) — Group A `Transform·Align·Pathfinder` + Group B `Properties·Layers·Libraries(stub)`; wire the existing Align/Distribute/Pathfinder into the new tabs. Build each panel as an independent module + a **Window menu** to show/hide each (cheap-tier flexibility, 0.5).
4. **Properties** contextual panel (1 Group B) — reflow existing controls; defer Quick Actions (keep Arrange).
5. **Layers** upgrade (1) — visibility, lock, inline rename, search/filter, thumbnails, footer.
6. **Floating left toolbar** polish (2) + Fill/Stroke swatch wired to the picker.

## Deferred (NOT this stage)
Gradient · Libraries · Artboard (blocks Align-to-Artboard) · Quick Actions (Offset/Expand/Recolor/Global Edit/Generative) · Key-Object align · Export.
