# Gradient fix-round integration notes

This worktree is based on committed `ece20e5`. No main merge or git write was performed.
The image-object lane and main's `stroke/canvas.rs` are absent here. These are explicit
integration requirements, not completed fixes or measured runtime claims.

## Canvas stroke seam (Opus 9)

`varos-core/src/scene.rs` calls `gradient_canvas::coverage` for gradient strokes.
`varos-core/src/gradient_canvas.rs` is the single marked substitution point. Its local
adapter clamps tolerance to 0.01–0.1 and passes the headless gradient-stroke test. It
still uses this base's evaluator; it does not claim the hotfix's 60k cap/cache/back-off.

After integrating `9f14e1e`, bind this seam to `ed.canvas_strokes.lookup(path, xf, ppu)`:

- Cache rings are already in world coordinates: remove the subsequent ring transform
  in the gradient branch. Continue transforming the gradient placement exactly once.
- Keep main's bounded retries, cache retention, native fallback and report handling.
- Preserve all coverage notes in the gradient branch, including `stroke_simplified`.
- The cache's painted predicate must accept resolved gradients as well as solids.
- Add combined tests for rotated dashed gradient strokes, repeated cache hits, caps,
  fallback and clipping. Do not claim runtime GPU heat from the headless seam test.

## Image findings and semantic clashes

Opus image findings 1–4 cannot be applied to this tree: there is no `pdf/images.rs`,
image model/asset table or `Prim::Image`. Finding 5's weakened image migration/refusal
tests are absent as well. This lane retains v5 JSON/PDF/SVG byte goldens and checksums;
its gradient refusal fixtures now assert typed errors and parser details.

The image integrator must put `Prim::Image` into the main PDF writer, retain isolation
and knockout, compress RGB/SMask streams and preserve JPEG DCT originals, avoid full
save-time image decoding, degrade open-time budget failures to proxies, hash live
image transforms/opacity, and restore image byte/error fixtures. A synthetic future
asset marker in the frozen reader harness verifies refusal ordering only, not image IO.

Audit the merged SVG image companion and PDF writer for resolved gradient dispatch;
never send gradients through the solid placeholder branch or swallow `GradientFill`.
Audit every production `Prim` wildcard in scene, raster and tessellation after adding
images. Freeze a mixed image/gradient document and test PDF/SVG/CPU parity.

## Format sequence

Recommended phase order: images → Live Corners → gradients → text. Corners belongs
to the intervening tools phase, per PLAN; the integrator owns final numeric stamps.
Do not combine independently authored format-6 writers without named migration steps.
Renumber gradient accepted/refused/future fixtures and pin the final literal version;
compose older-version key refusals and update frozen v5/v6 reader gates for the actual
merged writers. Existing image fixtures must stay byte-frozen except documented version
stamp changes. The standalone gradient writer remains provisionally version 6.
