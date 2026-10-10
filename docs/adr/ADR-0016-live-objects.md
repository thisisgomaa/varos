# ADR-0016 — Live objects

Status: proposed. Date: 2026-10-09. Lane B, Phase 10.

Authored Path.effects is an ordered typed list of Offset {delta,join,miter},
ZigZag {size,ridges,smooth}, Transform {copies,move,scale,rotate,reflect}, and
Warp {style,bend,h,v}. StrokeStyle.width_profile stores seven presets or custom
length-fraction/left/right points. Both keys default away on plain artwork.
The lane writer is v11. migrate_v10_to_v11 is pure identity on decoded legacy
models; absent keys have empty/uniform defaults. v9→v10 is a temporary identity
reservation for the integrator to replace with the appearance migration.

One bounded pure evaluator resolves corners then effects. Canvas flattening,
stroke coverage, CPU raster, PDF, SVG and Expand consume that same result.
Transform copies resolve to separate painted leaves (including open paths);
the editable source stays one Path until Expand. Mask-source copies refuse
until the integrator defines their clipping ownership.
A bounded per-object cache compares authored geometry and effect parameters;
idle has no evaluator calls or frame scheduling. Edits invalidate by comparison.
Nonlinear maps subdivide curves before mapping. Transform copies retain the
original. Refuse invalid/nonfinite parameters and over-budget geometry before
publication. Width outlines use length fractions in the shared stroke engine.

Effects currently live on Path. Appearance-stack integration belongs to the
integrator. Provisional sheets use the existing kit and tokens. Preview uses a
transient draft; Cancel discards it and OK publishes one EditCommand undo step.
Width tool uses Illustrator Shift-W; Apply Last Effect uses Shift-Command-E,
Last Effect uses Option-Shift-Command-E. No GUI acceptance is claimed in this lane.

Every headless edit is exposed through API 1.2 progressive-disclosure verbs and
CLI. API 1.0/1.1 and ratchet fixtures remain frozen. Prior art is adapted from
VectorCraft effects/{distort,warp,stylize,util}.rs, effects/stroke/width.rs and
doc/src/live.rs with attribution in source and NOTICE.
