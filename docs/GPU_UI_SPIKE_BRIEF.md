# Varos — Native GPU UI: Phase 0 Spike Brief (for Production)

> **⚠ UPDATE 2026-07-02 (Ahmed): FROSTED GLASS IS CANCELED — permanently.** SOLID panels are the one final
> material (what's shipped today). No frosted material, no settings toggle, one code path. Strike gate #4's
> frosted-perf requirement and every frosted mention below. Reason: priorities = clean, world-class,
> build-on-able, no rework. The wgpu bump stays planned — as foundation hygiene, not for frosted.

**Goal of the spike (the GATE):** prove we can draw the Varos UI **ourselves on the GPU**, composited with
the existing wgpu canvas in ONE surface/frame — replacing the web (wry/WebView2) panels — **without** losing
the native pen feel and **with** the look Ahmed approved (solid floating panels, zero black — frosted canceled 07-02).
This is the "swap the panels to native, behind the seam" that the Constitution/PANELS_PRO_SPEC always planned.

> **This is a SPIKE, not the product.** Build the minimum to answer the 4 gate questions (below). If it
> PASSES, we commit and build the panel design-system on it. If it FAILS, we learned cheap. Ahmed judges
> the result in the REAL window — never claim "works" from a headless pass.

## Hard rules
- **`varos-core` (model/editor/tools) is UNTOUCHED.** This is a render/view-layer change only (the seam).
- **Keep the current web-panel build working & stable** in parallel. Do NOT delete it. We cut over only when the native UI reaches parity.
- **Protect the pen feel.** Canvas pointer/pen input must reach `varos-core` directly — NO extra input layer over the canvas.

---

## Decision (from a researched, adversarial evaluation — see notes at bottom)
- **Spike FIRST: `egui` + `egui-wgpu` + `egui-winit` (manual integration path).**
- **Backup: `Iced`** (`iced_wgpu`/`iced_winit` low-level integration) if egui's hand-painted look is too painful.
- **Last-resort fallback: Vello-renderer-only** (own the entire paint layer) — weeks not days, so only if BOTH egui and Iced fight us on the beauty. Prove the cheap path fails first.
- **Disqualified for our "host owns the surface" constraint:** Makepad (own GPU, not wgpu), GPUI / Slint / Blitz(Dioxus) (they invert ownership and make our canvas a guest).

## TASK 0 (do this FIRST — it gates everything, and it's needed for ANY modern toolkit)
**Bump `varos-render-wgpu` off `wgpu 0.19` → `wgpu ~0.29`** (egui/Iced/Vello are all on ~29; 0.19 runs none of them).
- Our renderer already **owns the surface** (`get_current_texture` / `frame.present`) and already does
  **stencil-then-cover** with `Depth24PlusStencil8` + MSAA + Mailbox. Its `render(world: &Scene, ui: &[Prim], view: View)`
  signature **already has a UI channel** — we are not starting from scratch.
- Expect API churn across the bump: `DepthStencilState`, `SurfaceConfiguration`, the present/encoder APIs, and
  the stencil-then-cover pipeline all changed across those majors. Treat this as in-scope spike work.
- `winit` stays `0.29` (matches egui-winit of that era) unless the wgpu bump forces a winit bump too — keep them paired.

## Integration model (the ONLY correct path)
- **Keep OUR `winit` loop and OUR `wgpu` surface.** egui owns NOTHING.
- Per frame: draw the `varos-core` Scene first (our existing stencil-then-cover pass into the surface view),
  **then** `egui_wgpu::Renderer::update_buffers(...)` + `Renderer::render(...)` onto a render pass **we** own,
  then `frame.present()`. (egui-wgpu wants `RenderPass<'static>` — use `forget_lifetime()`.)
- **egui shares our `wgpu::Device`/`Queue`** (same instance — mandatory).
- **🚫 Do NOT use `eframe`.** eframe creates the winit loop and seizes the event loop + input. If you reach for eframe, you've already failed gate #2.
- **Input routing:** feed events to `egui-winit` only for panel regions; canvas pointer/pen events go straight to `varos-core`. Panel hit-testing must NOT swallow canvas strokes.

## What to BUILD in the spike (minimum)
1. **One floating inspector card** = the mini "Properties" we mocked: object header (`Rectangle`), a `TRANSFORM`
   section (X/Y/W/H mono fields), a `FILL` row (swatch + `#0C8CE9` + opacity). Palette from `UI_FIGMA_SPEC.md`:
   bg `#141313`, surface `#262627`, text `#e6e6e6`, muted `#8a8a8a`, accent `#0c8ce9`. Radius 12px, soft shadow.
2. ~~Frosted glass material (DEFAULT)~~ **CANCELED 2026-07-02** — solid panels are the one final material.
3. ~~A `Frosted ↔ Solid` toggle~~ **CANCELED 2026-07-02** — no toggle; one material, one code path.
4. **A SECOND overlapping panel** to test z-order: clicking a panel must bring it to front. egui::Window's native
   z-ordering is basic — use the `egui_tool_windows` crate for click-to-front. Verify two overlapping panels behave.
5. Personality = **balanced** (clean, but the key controls visible) — don't over-minimize, don't clutter.

## The GATE — 4 pass/fail checks (Ahmed verifies in the real window)
1. **BEAUTIFUL / CUSTOM (the gate):** the solid + soft-shadow card matches the approved look, achieved via
   the hand-painted pass, AND the effort feels like a **reusable primitive** (not a one-off hack). FAIL if reaching
   the look means fighting egui's global Style, or it reads as flat dev-tool chrome you can't escape.
2. **CLEAN COMPOSITING, NO SEAM:** one surface, one swapchain — canvas pass, then egui onto a pass we own, then present.
   Zero second window, zero OS transparency hack, no seam/z-fighting, egui shares our Device/Queue.
3. **NATIVE PEN FEEL UNTOUCHED:** dragging/drawing on the full-bleed canvas has **identical** latency/feel to today;
   panel hit-testing doesn't swallow canvas strokes. FAIL if input must route through egui first or feel regresses.
4. **TOLERABLE DEV LOOP + PERF:** accept no Rust hot-reload; a new panel = a plain Rust fn + normal cargo rebuild,
   fast enough to iterate happily; idle CPU fine in reactive (no-repaint-when-idle) mode. *(The frosted-perf
   clause is VOID — frosted canceled 2026-07-02.)*

## Scope guards (don't waste the spike)
- **Text test = number-field entry + IME-caret-position only.** egui has no Arabic shaping and a weak text editor —
  that's FINE: the future Type tool lives on our own HarfBuzz canvas, bypassing egui. Do NOT test egui for the Type tool.
- **Don't try to make egui's DEFAULT look good** — it won't. The whole point is hand-painting the chrome.

## Risks to watch (from the evaluation)
1. The wgpu 0.19→29 bump (Task 0) — gates everything; shared by all candidates.
2. Aesthetic = hand-painted (egui is skeleton only; the beauty is our wgpu shaders). This also keeps the seam clean (egui can be ripped out later, the paint layer intact).
3. The eframe control-inversion trap — never adopt eframe.
4. egui pre-1.0 churn (egui/egui-wgpu/egui-winit/wgpu/winit move in lockstep) — a bounded standing cost.
5. egui::Window z-order needs `egui_tool_windows` for bring-to-front.

## If egui fails a gate
Fall to **Iced** and run the SAME 4-point test (its custom-shader widget is a slightly richer canvas for the
hand-painted beauty, at the cost of its Elm/TEA architecture which fights a stateful direct-manipulation editor).
If BOTH fight us on the beauty → **Vello-renderer-only** (own the whole paint layer; weeks).

---
*Report PASS/FAIL per gate honestly, with Ahmed seeing it run. The wgpu bump is real work — that's expected and unavoidable.*
