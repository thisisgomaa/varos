# Lane B — Phase 5 gradients

Baseline: committed `ece20e5`; this fix round is uncommitted; no git writes, merge, push, GUI launch or install.
Scope: 5.0–5.4 plus within-stroke gradients implemented; optional 5.5 along/across/freeform deferred; UI provisional.
Format: provisional NEXT_GRADIENT_VERSION=6; named pure migrate_v5_to_next_gradients; integrator assigns final number.
Keys: doc.swatches {id,name,paint,global,group}; tagged gradient {kind,stops(offset,colour,opacity,midpoint),spread,placement,focal}; swatch_ref {id}.
Frozen v4/v5, gradient/raster goldens and legacy Bridge fixture trees unchanged; no dependencies or ratchets raised.

## Fix round

- Astra 1: picker opens/frames/selection changes are read-only; explicit edits create gradients; post-Undo frame regression passes.
- Astra 2–4: resolved None exports no artwork; Pathfinder bakes inherited placement through unit_xform; Wand compares resolved variants.
- Astra 6 / Opus 10: owned current paints survive eyedropper→shape/pen; owned gradients fit recipients; global references stay linked; deletion materializes defaults.
- Astra 5 / Opus 7: thumbnails sample resolved gradient paints and hash live swatch contents/transforms; control-bar chips and colour lists use representative colours.
- Astra 7 / Opus 8: LUTs keyed only by stops/interpolation; shared sampler; pan/zoom writes existing uniforms, preserving textures/bind groups.
- Opus 10: migration no longer validates; literal provisional version pinned; identity test proves no validation/repair.
- Refusal fixtures assert typed errors/details; frozen v5/v6 gates test raw JSON/PDF refusal before malformed models/assets; v5 JSON/PDF/SVG byte goldens pass.
- Opus 9: marked gradient_canvas::coverage seam + headless test; integrator must bind stroke/canvas.rs hotfix cache/world rings/fallback/report.
- Opus images 1–5: image model, Prim::Image and pdf/images.rs are absent here; image writer/compression/budget/signature/tests remain external requirements.
- Disagreement with applicability of Opus 5 here: this lane already retains v5 byte goldens; weakened image-lane tests are not present.
- Integration checklist: [W2_GRADIENT_FIX_INTEGRATION.md](docs/reference/W2_GRADIENT_FIX_INTEGRATION.md); proposed images→Corners→gradients→text; mixed-export/Prim audit pending.
- Gates: fmt, dependency directions, diff check PASS; offline workspace -j 3: 1931 primary + 3 subprocess passes / 0 failed / 15 ignored.
- Native + x86_64-pc-windows-msvc workspace/all-targets clippy --offline -j 3 -D warnings: PASS, zero warnings.
- Shell ratchets 3/3, Bridge ratchets 6/6, Bridge fixture contracts 5/5 PASS; legacy 1.0/1.1 bytes frozen; API 1.2 22705 B; ui.rs 816/843.
- Evidence: varos/target/lane-b-fix-{fmt,deps,workspace-final,clippy-native,clippy-windows,shell-ratchets,bridge-ratchets,bridge-fixtures}.log; re-review/native acceptance/GPU heat unverified.
