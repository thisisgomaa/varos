> Status: proposed scope only — 2026-10-06. No view rotation built in this piece.
# Rotatable view — implementation work order

Overall size: **L** (cross-cutting camera/input/overlay change, not one event handler).
Goal: rotate the camera without rotating document geometry or changing exported artwork.
Illustrator reference: Shift+H Rotate View; trackpad rotation pivots at the pointer;
Shift+Cmd+1 resets the view on macOS. Exact pan/zoom/reset interactions need hand checks.
Source: https://helpx.adobe.com/illustrator/using/rotate-view.html

## What changes
- **View / core geometry — M:** add angle (radians, default zero), forward/inverse
  transforms and pan-for-anchor with rotation. Audit all View literals and per-tab state.
  Decide whether angle is session-only; do not change the document schema by accident.
- **Rendering / varos-render-wgpu — L:** transform fills, strokes, artboards, masks and
  clipping consistently; audit screen-space primitives and scale-only assumptions.
  `build_content` and scene culling both depend on View, not just a shader uniform.
  Inverse-map all four viewport corners for conservative world-space culling bounds.
- **Hit-testing / cursor-world mapping — M:** use the inverse camera everywhere;
  keep pixel hit tolerances constant. Audit pen, drag, artboard tools and cursor angles.
- **Rulers / guides / grid — L:** define ruler semantics on a rotated canvas first;
  map guide creation/dragging correctly, rotate world grid and keep chrome horizontal.
- **Marquee / selection handles — L:** a screen rectangle becomes a world quadrilateral;
  choose containment rules, rotate selection geometry, preserve handle sizes and cursors.
- **Snapping — M:** keep document axes/grid in world space; measure proximity in screen
  pixels. Decide whether Shift constraints and smart guides follow world or screen axes.
- **Artboard chips / labels — M:** position from transformed bounds, keep text upright;
  audit click targets and overlays in `ui/canvas_overlay.rs` and core scene primitives.
- **Export — S audit:** PDF uses document geometry, so output must be unaffected;
  add a regression proving identical output before/after camera rotation.
- **Thumbnails — S audit:** keep document previews upright with their own zero-angle fit;
  review `thumbs/` and cache signatures so camera changes cannot alter preview content.
- **Tests — M:** round trips at 0/90/arbitrary angles; pointer-pivot invariance;
  culling, clipping, marquee, snapping, handles, reset and per-tab isolation. No GPU or
  EventLoop in unit tests; real-window screenshots are a separate acceptance gate.

## Recommended gated split
1. **M — camera foundation:** angle + transforms, zero-angle compatibility, session policy;
   enable no gesture yet. Gate pure math and Mac/Windows compatibility.
2. **L — rendering + picking together:** rotated content/culling and inverse input mapping;
   keep feature disabled until owner checks clipping and drawing in the real window.
3. **L — editing overlays:** marquee/handles/cursors, artboard chips, snapping and guides;
   decide ruler/grid semantics and gate editing accuracy with headless tests + screenshots.
4. **M — controls + release:** Rotate View tool, trackpad phase/pivot behavior, reset,
   pan/pinch interaction, tabs, export/thumbnail regressions; owner acceptance before enable.
Every piece: workspace tests, clippy, fmt, Windows target clippy; no merge claim without review.

## Risks / unknowns
- Axis-aligned shortcuts in scene/editor/overlay code may need more work than this estimate;
  a complete call-site inventory and GPU clipping review have not been done.
- Illustrator ruler orientation, marquee semantics, angle snapping and cancellation behavior
  were not verified hands-on; settle them before implementing the related pieces.
- Rotation mixed with pinch/momentum, Retina changes and very large coordinates needs
  macOS hardware testing. Estimated sizes are scope judgments, not measured delivery times.
