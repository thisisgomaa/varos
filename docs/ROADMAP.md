> **Status:** historical — Preserved project history; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Roadmap & Backlog (⚠ SUPERSEDED — historical)

> **⚠ SUPERSEDED (2026-06-29):** priority + live status moved to **`docs/plan.html`** + **`docs/DETAILED_ROADMAP.md`**.
> Do not work from this file — kept for history only. (2026-07-02: frosted glass CANCELED — solid is the one
> final material; ignore any frosted mention below.)

> Everything Varos needs, by priority, in build order. **✅ = done.** Work top-down, one phase at a time.
> Ahmed sets/adjusts priority; the advisor maintains this file; Production executes the current phase.
> Tool-level detail: `ILLUSTRATOR_TOOLS_CATALOG.md`. UI detail: `UI_PLAN_EN.md` / `GPU_UI_SPIKE_BRIEF.md`.

## ✅ DONE — the engine, basic editing, and the native-UI foundation
- Pen + bezier; anchor trio (add / delete / convert)
- Direct Selection (white arrow); Selection (black arrow) + bounding-box move / scale / rotate
- Shapes: rectangle, ellipse, triangle, polygon
- Boolean engine + Pathfinder ops; Groups / Ungroup (nested)
- Layers panel (basic, two-way selection); Align & Distribute; z-order (front/back)
- Fill / Stroke (basic apply) + swap / default; Eyedropper; Undo / Redo
- **Native GPU UI foundation (Step 1):** full-bleed board, floating rounded panels (zero black), tool rail,
  inspector, top bar, splash; cursors; web→native migration + repo cleanup

## ▶ PHASE 1 — Finish the UI (NOW) — *the multiplier: makes every later panel cheap*
1. **Engine update** (wgpu bump) — enables modern egui *(frosted canceled 2026-07-02)*
2. **Design-system "puzzle pieces":** panel container · sections · tabs · buttons · **number fields
   (type + click-drag scrub + wheel)** · swatches · color/spacing/type tokens · icons
3. ~~Frosted-glass material (default) + a Solid toggle~~ **CANCELED 2026-07-02 — solid is the one final material**
4. **Show / hide panels** (Window menu)
5. **Assemble the panels** on the system:
   - **Properties** (contextual: Transform 9-point · Appearance · opacity)
   - **Color:** picker (HSB / RGB / CMYK / Hex) + Color panel + **Swatches**
   - **Stroke** panel (weight / caps / joins / dashes / align)
   - **Align / Distribute** panel · **Pathfinder** panel
   - **Layers** (full: visibility · lock · rename · search · thumbnails · nesting)
   - **Tool rail + Fill/Stroke swatch** cluster · **Top bar** · **Zoom**

## PHASE 2 — Make it fully usable (paint · arrange · navigate)
- **Exact numeric Transform** (type / scrub X / Y / W / H / rotate)
- **Opacity + blend modes**
- **Stroke** full options wired
- **Navigation:** zoom (wheel · fit `Ctrl+0` · 100% `Ctrl+1`) + **Hand / pan** (space)
- **Rulers / guides / grid / snapping**
- **Clipping mask**; group & z-order shortcuts

## PHASE 3 — Complete the vector toolset (the differentiator)
- **Shape Builder** (engine exists) — flagship
- **Cut tools:** scissors / knife / eraser
- **Line tool**
- **Artboard** (pages / frames)
- **Gradients** (linear / radial)  *(can slip to post-v1 if we want v1 leaner)*

## PHASE 4 — Text · Export · Save → **ship v1** (the gateway)
- **Smart Type tool** (point / area / path) + a simple text engine (Latin first)
- **Character / Paragraph** panels
- **Export** (SVG / PNG)
- **Save / Load** (`.varos` files)
- → **SHIP v1**

## DEFERRED — post-v1
- **Arabic + kashida + RTL** (the eventual moat-deepener)
- Gradient mesh · brushes / blob brush · live paint · recolor artwork
- Advanced transforms (free transform · warps · puppet · width tool)
- Appearance panel (multi fill/stroke) · global edit · image trace / raster
- **Plugin SDK · single-schema formalization · AI-native automation**
- Mac / Linux · collaboration / cloud sync · symbols / components

---
*Phases are dependency-ordered: Phase 1's design-system turns "many panels" into a quick assembly job;
most of Phase 2 is then just wiring those panels to the engine (which already exists).*
