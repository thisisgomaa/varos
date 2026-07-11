> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# UI Mockups — Editor Shell Variants

3 visual references for what the Pivot editor proper could look like — _shells only, not functional_. Each is standalone HTML, open directly in browser.

> All 3 variants render the same sample content on the canvas: a centered Arabic heading + subheading + a selected rectangle. The differences are entirely in the UI chrome around it.

---

## Variant 1 — `01_figma_like.html`

**Layout:** Top bar (file name + avatars + share) · narrow vertical tool strip on the start side (right in RTL) · large center canvas · single 280px inspector on the end side (left in RTL) with tabbed switcher between properties/layers/library · slim status bar at the bottom.

**Feel:** Spacious, modern, Figma-DNA. Tools always visible but minimal. Inspector is one focused column, not stacked panels.

**Strengths:**
- Light cognitive load — only one panel column at a time
- Easy to add collaboration affordances later (avatars/share already there)
- Tools panel reads cleanly in RTL (start-side = where eye lands)

**Trade-offs:**
- Inspector tabs hide layers/library — extra click to switch
- Less screen real estate for inspector content vs. Illustrator-style stacks

**Look at:**
- The artboard label `Artboard 1 — 1080 × 720` (top-right of artboard in RTL)
- The hover tooltips on each tool (icon hover)
- The selected `مستطيل` row in the layers list (gold tint)

---

## Variant 2 — `02_illustrator_like.html`

**Layout:** Menubar (ملف، تحرير، عنصر، …) · context-sensitive control bar that changes per-tool · dense 2-column tool grid on start side (right in RTL) showing _all_ tools at once · stacked accordion panels on end side (left in RTL) for properties, colors, paths, layers, alignment, appearance · status bar with workspace switcher and color profile.

**Feel:** Pro tool, Adobe-DNA, every affordance visible. High information density.

**Strengths:**
- Power user can reach any tool/setting without nav
- Control bar at top makes per-tool params obvious
- Stacked panels = more inspector info at once
- Workspace switcher hints at future layout presets

**Trade-offs:**
- High chrome ratio — canvas is smaller relative to viewport
- Can feel cluttered for newcomers
- Two RTL panels (tools right, panels left) splits attention

**Look at:**
- The 26 tools in the right-side panel (single column dense grid)
- The control bar specific to the Rectangle tool — fill/stroke/style toggle
- The accordion panel stack on the left — collapsed/expanded headers
- The rulers along the top and end edges of the canvas

---

## Variant 3 — `03_minimal_hybrid.html`

**Layout:** No frame chrome by default — canvas fills 100% of viewport. Floating glass-panel toolbar in bottom-start corner (left in RTL) · slide-in inspector that appears on selection (top-start area) · "Layers" pull tab in top-end corner (right in RTL) — collapsed by default · floating zoom/coordinates pill bottom-center · keyboard hints bottom-end.

**Feel:** Linear/Notion-DNA, focus-mode, gesture-led. The chrome respects the canvas.

**Strengths:**
- Maximum canvas real estate — feels like the work is the star
- Floating glass panels look distinctively modern in dark theme
- Keyboard-first ergonomics surfaced via visible keys (V, P, R, …)
- Inspector only appears when you've selected something — no empty state

**Trade-offs:**
- New users may not discover layers/panels without guidance
- Panels overlap canvas — could occlude work
- Less obvious "where everything lives" for designers used to docked UIs

**Look at:**
- The floating tool palette in the bottom corner (glass blur effect)
- The keyboard hints visible in tooltips (V, P, R, O, T, H, Z)
- The "Layers (4) [L]" pill — collapsed by default, click to expand
- The inspector card showing only what's relevant to the current selection
- How the artboard breathes when there's no chrome around it

---

## Questions for Ahmed (to pick a direction)

1. **Always-visible vs progressive disclosure** — should every tool be one click away (Illustrator-like), or should the workspace be cleaner with discovery via shortcuts (Minimal-Hybrid)? Which matches how _you_ work?

2. **Docked vs floating panels** — do you want panels glued to edges (Figma/Illustrator) or floating glass cards that can be dragged (Minimal-Hybrid)? Floating is sexier but can occlude the canvas.

3. **Information density** — do you want a "Pro" tool feel (Illustrator-like, lots of visible options) or a "Calm" tool feel (Figma-like or Minimal-Hybrid, fewer options visible at once)?

4. **Onboarding signal** — Pivot's pitch is Arabic-first + perfect kashida. Should the chrome _show_ that (e.g., big visible Arabic-only nav, kashida-justified labels) or stay neutral and let the canvas do the talking? Variant 3 is most neutral.

5. **Hybrid?** — none of these is a final design. They're poles. You might want, e.g., Figma's spaciousness with Illustrator's control bar — or Minimal-Hybrid's canvas dominance with Figma's docked inspector. After looking, tell me which 1-2 things from each you'd keep, and I'll spin a 4th hybrid mockup.

---

## RTL observations

A few things became visible only while building these:

- **Tools panel placement:** In LTR designs, tools usually live on the left (Figma/Illustrator). In RTL, the eye lands on the right first, so tools belong on the right side. All 3 variants do this — but it _looks wrong_ if you've spent a decade in LTR design tools. Worth testing whether Arabic designers prefer the RTL-native placement or the mirrored-from-Latin placement.
- **Menubar order:** Standard Adobe order in RTL is ملف → تحرير → عنصر → نص → تأثير → عرض → نافذة → مساعدة (start to end). Implemented this way in variant 2.
- **Numeric inputs (X, Y, W, H):** Numbers themselves are LTR but the field _labels_ are RTL. JetBrains Mono handles both well.
- **Tooltip direction:** RTL tooltips appear to the right of the tool icon (start-side), not left. Implemented in all 3.

---

_Next: pick a direction (or ask for a hybrid), and we'll wire one of these into the actual editor in Phase 1._
