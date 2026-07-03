# Varos — Native GPU UI: Plan & Handoff (for the Production session)

> **⚠ UPDATE 2026-07-02 (Ahmed): FROSTED GLASS CANCELED — permanently.** Solid panels are the one final
> material; no settings toggle, one code path. Every frosted mention below is historical: Step 2's frosted
> half and gate 4's frosted-perf clause are VOID. The wgpu bump remains planned (foundation hygiene).

> **You (Production) implement this. Ahmed judges the result in the real window. The advisor wrote this
> plan.** It explains the GOAL, the TECHNOLOGY, the RESEARCH, the DIRECTION, your DECISION FREEDOM, and the
> TEST. Read it fully before writing code. Hard requirements are marked **MUST**; everything else is guidance
> you may improve on with justification.

---

## 1. The Goal (what we're building, and why)

Varos's UI must become **clean and professional — Figma/Affinity quality**. Concretely:

- The **board (canvas) takes 100% of the screen**, edge to edge, with its dot grid.
- Panels **float on top of the board** — **rounded corners**, light, tidy — with **ZERO black background**
  behind or around them. Around a rounded panel you see **the board**, never a black box.
- One **shared foundation** ("puzzle pieces"): every new panel is assembled from the same primitives.
- The **pen feel stays exactly as it is today** — this is non-negotiable.

**Why we're doing this (the root cause):** the current UI uses web panels (wry/WebView2) layered over the
wgpu canvas. On Windows, child WebView2 panels **cannot be transparent over a GPU surface** (they paint
black rectangles) and **cannot draw popups outside their own rectangle** (clipped menus). This is an
**architectural** limit, not a CSS/cosmetic one. No amount of styling fixes it. That is the "black behind
the panels" Ahmed keeps seeing, and the wasted space, and the messiness.

## 2. The Technology / Architecture

**We draw the UI ourselves on the GPU, in the same surface/frame as the wgpu canvas.** One coherent native
system — the model Blender and Zed use. This is the "swap the web panels for a native UI **behind the hard
seam**" that the project always planned (see the Constitution / memory).

- **`varos-core` (model / editor / tools) is UNTOUCHED.** This is a render/view-layer change only — the seam.
- **Keep the current web-panel build working and stable** in parallel. Do **not** delete it. We cut over only
  when the native UI reaches parity. The program must never be left broken.
- Because we own every pixel, transparency / rounded floating panels / no-black / blur / animation all become
  trivial and fully under our control.

## 3. The Research (what an evaluation already found — use it, don't redo it)

An 11-agent adversarial evaluation (that read this repo's actual `Cargo.toml`) compared the Rust GPU-UI
options against Varos's exact constraints. Findings, as **guidance**:

- **Recommended first choice: `egui` + `egui-wgpu` + `egui-winit`** via the **manual integration path**
  (egui owns nothing: keep our `winit` loop and our `wgpu` surface; draw the canvas pass first, then egui
  onto a render pass we own; share one `wgpu::Device`/`Queue`). **MUST NOT use `eframe`** — it seizes the
  event loop and the pen input.
- **Backup: `Iced`** (low-level `iced_wgpu`/`iced_winit`). **Last resort: Vello-renderer-only** (own the whole
  paint layer; weeks, only if the cheaper paths fail).
- **Disqualified** (they invert surface ownership / make our canvas a guest): Makepad, GPUI, Slint, Blitz/Dioxus.
- Our renderer **already owns the surface** and its `render(world, ui, view)` **already has a UI channel** — we
  are not starting from scratch.
- **wgpu version note:** the renderer is pinned to `wgpu 0.19`; current egui/Iced/Vello need `wgpu ~0.29`. For
  the **full** build the bump is needed; for an early visual proof an older compatible egui can answer the look
  question without it. **You decide the sequencing** (see 4).
- egui's default look is plain by design — that's fine: use it as the **skeleton** (layout / events / widgets)
  and **hand-paint the beautiful chrome** (rounded panels, soft shadow) in our own wgpu
  pass. That hand-painted layer is also what keeps the seam clean (egui can be swapped out later).

Full technical detail: **`D:/VAROS/docs/GPU_UI_SPIKE_BRIEF.md`**. Palette/look: **`D:/VAROS/docs/UI_FIGMA_SPEC.md`**.

## 4. Your Decision Freedom (read this carefully)

**The GOAL and the GATES (1, 7) are non-negotiable. The HOW is yours.** We trust your engineering judgment:

- egui is the *researched recommendation*, not a lock. If, with reasoning, you find a better fit for the goal,
  **you may choose it** — just state why.
- You choose the **sequencing** (e.g., prove the look first on a compatible version, or do the `wgpu` bump up
  front) — whichever de-risks fastest toward the goal.
- You own the implementation details, file layout, and the shape of the design-system primitives.

**The two firm rules in exchange for that freedom:**
1. **Do not cut corners on the goal.** "Zero black", "full-bleed board", "clean", and "pen feel unchanged" are
   not negotiable. A result that violates them is a fail, however quickly it was reached.
2. **Do not skip a required step silently.** If a step is hard (e.g., the `wgpu` bump), do it properly **or
   flag it and explain** — never quietly route around it and present a shortcut as done.

## 5. The Direction / Build Order (incremental — Ahmed verifies EACH step in the real window)

**Step 1 — the clean proof (DO THIS FIRST; it is the gate):**
**ONE** panel only — rounded corners (~12px), **solid** clean dark fill (**not** frosted yet), floating on a
board that **fills 100% of the screen**, with **ZERO black** anywhere around it (you see the board around the
rounded panel). Nothing else floating, no clutter. **Do not proceed until Ahmed approves this in the real window.**

**Step 2 — make it beautiful:** match the design exactly (colors, spacing, soft shadow from 6/UI_FIGMA_SPEC).
*(The frosted-glass half of this step was CANCELED 2026-07-02 — solid is the one final material, no toggle.)*

**Step 3 — the panel design-system ("puzzle pieces"):** build the reusable primitives once — the floating-panel
container, buttons, number fields (type + click-drag scrub + wheel), swatches, tabs, sections, tokens — so any
new panel = assembling primitives.

**Step 4 — build all panels on the foundation:** Properties (Transform/Appearance), Align, Pathfinder, Layers,
Color/Swatches, the tool rail with the Fill/Stroke swatch, the top bar, zoom. Port design + existing logic
(the `varos-core` calls already exist).

**Step 5 — finish:** full-bleed board, polish pass (Ahmed-led), then cut over from the web panels to native.
Retire the web panels only once parity is reached.

## 6. The Details (what Ahmed wants — the experience)

- **Board:** 100% of the screen, edge to edge, with the dot grid.
- **Panel:** rounded corners (~12px); clean **solid dark** fill (final — frosted canceled); floats on the board;
  **ZERO black** around it; draggable; can be shown/hidden.
- **Material:** **solid — FINAL** (frosted glass canceled 2026-07-02; no toggle).
- **Personality:** **balanced** — clean, but the important controls stay visible (not over-minimal, not cluttered).
- **Colors:** bg `#141313` · panel surface `#262627` · text `#e6e6e6` · muted `#8a8a8a` · accent `#0c8ce9`.
- **Fonts:** Inter for UI, a mono for numbers; ~13px base.
- **Pen feel:** identical to today — canvas pointer/pen input reaches `varos-core` directly; panels must never
  swallow canvas strokes.

## 7. The Test / Verification (the gates — Ahmed checks in the REAL window, never headless)

**Step 1 gate (now):** the board fills 100% of the screen; **one rounded panel floats on it with ZERO black**
around it; the pen feels unchanged. PASS → continue to Step 2. FAIL → fix before building anything on top.

**Foundation gates (when going to the full native build):**
1. **Beautiful / custom** — the panel matches the approved look via the hand-painted layer, and that effort
   feels like a **reusable primitive**, not a one-off hack. FAIL if it reads as flat default chrome you can't escape.
2. **Clean compositing, no seam** — one surface, one frame: canvas pass, then UI onto a pass we own, then
   present. Zero second window, zero OS transparency hack, no seam/z-fighting.
3. **Native pen feel untouched** — drawing/dragging on the full-bleed board has identical latency/feel to today.
4. **Tolerable dev loop + perf** — new panels are quick to author; idle CPU fine. *(The frosted-perf clause
   is VOID — frosted canceled 2026-07-02.)*

**Report PASS/FAIL honestly per gate, with Ahmed seeing it run.** Never claim "works" from an automated/headless pass.

## 8. Working Rules
- Start **clean and simple** — one panel, no clutter. **One thing at a time**, Ahmed verifies each.
- **No silent corner-cutting / no skipped steps** (4).
- **Never break the current app.**
- Talk to Ahmed in **simple, short Egyptian Arabic** (he's a designer, not a coder) — explain with design
  analogies, no engineering jargon.

## References
- `D:/VAROS/docs/GPU_UI_SPIKE_BRIEF.md` — technical detail (egui manual path, the gates, risks).
- `D:/VAROS/docs/UI_FIGMA_SPEC.md` — palette + look tokens.
- `D:/VAROS/docs/UI_MASTER_PLAN.md` — the same plan in Arabic (Ahmed's reference).
