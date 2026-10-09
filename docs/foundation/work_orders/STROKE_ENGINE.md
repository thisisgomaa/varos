> **Status:** proposed implementation contract — 2026-10-09; docs only, pending independent review.

# Stroke engine: Phase 2.0–2.3

Authority: [PLAN](../../PLAN.md), Phase 2 and format schedule; [ADR-0004](../../adr/ADR-0004-v1-schema-policy.md),
[v5 amendment](../../adr/ADR-0008-amendment-v5-stroke-style.md), and [kurbo note](../../adr/ADR-NOTE-kurbo-in-core.md).
This refines study S3 in [APPEARANCE_MASKS_STUDY](../../reference/APPEARANCE_MASKS_STUDY.md).
The later approved PLAN brings arrows into 2.1; no dormant arrow fields may ship ahead of their behaviour.
No gradients, width profiles, brushes, appearance stacks, effects or UI geometry belong in this change.

| Slice | Delivery and exit |
|---|---|
| 2.0 | Pure kurbo adapter, dependency review and attribution; no writer change. |
| 2.1 | Entire style below, validation, all consumers, Bridge 1.2, migration/refusal fixtures before writer. Depends on 0.1. |
| 2.2 | Properties controls and control-bar mirror after owner-selected Figma design; no bump. |
| 2.3 | Destructive Outline Stroke, Offset Path, Expand and scale-strokes option; no new stored keys. |

Baseline evidence: `varos/crates/varos-core/src/model.rs` has `Path.stroke`, `stroke_width` and `Paint::None|Solid`.
The gap study §C cites round-only `varos-render-wgpu/src/tess.rs:131–149` and PDF round defaults at `write.rs:211,395`.
In this checkout the second PDF default is at line 422. Replace both page and isolated-object defaults.
Source roots are under `varos/crates/`; source line numbers are evidence, not patch locations.

## Stored data and validation

`Path.stroke_style: StrokeStyle` is the only added Path key. Width and paint keep their current homes and encoding.
All keys below are exact, case-sensitive JSON keys; enum strings use the exact PascalCase spellings shown.
Use `#[serde(default, skip_serializing_if = "StrokeStyle::is_default")]` on `Path.stroke_style`.
Each nested field also defaults and skips its default (`default = "..."` for numeric 10/1 helpers).
All three structs deny unknown fields. Missing style or `{}` means the same default; a null style is invalid.

| Key inside `stroke_style` | Rust type / wire values | Default | Validation |
|---|---|---|---|
| `cap` | `StrokeCap`: `Butt`, `Round`, `Square` | `Round` | Known string only |
| `join` | `StrokeJoin`: `Miter`, `Round`, `Bevel` | `Round` | Known string only |
| `miter_limit` | `f32` | `10.0` | Finite, 1–1000 inclusive; dimensionless |
| `dash` | `Vec<f32>` | `[]` | 0, 2, 4 or 6 entries: up to three dash/gap pairs |
| `dash_phase` | `f32` | `0.0` | Finite, absolute value ≤ 1,000,000 pt |
| `align_dashes_to_corners` | `bool` | `false` | No coercion |
| `align` | `StrokeAlign`: `Center`, `Inside`, `Outside` | `Center` | Known string only |
| `arrows` | `StrokeArrows` object | all-default object | Omitted when all fields default |

| Key inside `arrows` | Type / wire values | Default | Validation |
|---|---|---|---|
| `start`, `end` | `Option<ArrowHead>`; string below or null | `None` | Omit `None` on save; reject unknown string |
| `scale_start`, `scale_end` | `f32` multipliers of stroke width | `1.0` | Finite, 0.01–100 inclusive; UI shows 1 as 100% |
| `align` | `ArrowAlign`: `Tip`, `Extend` | `Tip` | Applies to both ends |

Every dash entry is finite, nonnegative and ≤ 1,000,000 pt; every positive entry is ≥ 0.0001 pt.
Every pair must have a positive sum. Zero on-length supports dots; zero gap joins adjacent painted intervals.
Odd arrays, all-zero pairs, NaN/infinity and over-limit input are refused, never duplicated, clamped or made solid.
Empty dash ignores phase/corner alignment at evaluation time but preserves their authored values.
Existing width validation stays finite and nonnegative; do not retroactively impose a new width cap on v4 files.
Width zero or `Paint::None` paints nothing, including arrows (never a PDF hairline). Style remains editable.
Apply the same style validation on edit, load and save, before normalization; errors name path ID and field.
Check derived f64 geometry and f32 conversion for overflow; fail visibly instead of dropping pieces.

Canonical minimal example: `"stroke_style":{"cap":"Butt","dash":[6.0,3.0],"arrows":{"end":"Triangle"}}`.
Round cap/join, no dash, Center and no arrows reproduce today's appearance; an entirely default style emits no key.
Plain-object `doc` JSON stays byte-identical. A v5 save changes wrapper/catalog stamps and PDF container offsets;
whole-file byte identity across a format bump is not promised. No defaulting loophole waives ADR-0008's bump rule.

## Geometry contract

All distances are document points (1/72 inch), independent of zoom and ppi. Work in f64 behind the adapter.
Core produces one shared plan: centreline, dash runs/dots, arrow filled outlines, effective alignment and notes.
Outline generation uses 0.01 pt tolerance for export/Expand; screen tessellation error ≤ 0.25 device pixel.
Dash/arrow arc-length solving uses 0.000001 pt tolerance. Cache keys include all style fields, geometry and tolerance.
No evaluation or GPU uploads on idle frames; no geometry or raster cache is persisted in `Document`.

Butt ends at the endpoint; Round extends by half-width; Square extends by half-width along the tangent.
Miter uses the ratio of outer-tip distance from the centreline corner to half-width, equivalently 1/sin(half-angle).
Above `miter_limit` use a bevel, not a clipped miter. Round and Bevel ignore the limit without erasing it.
Skip coincident segments when finding tangents. A zero-length open contour paints a disk for Round, an axis-aligned
square for Square, nothing for Butt. It has no arrows and reports `stroke_arrow_no_tangent` if arrows were requested.

Dashes measure arc length, restart per contour, and use `dash_phase.rem_euclid(period)` as distance into the pattern.
A positive phase advances into the pattern, matching PDF `d`; SVG emits the equivalent positive dash offset.
Merge an on-run across a closed seam so it receives a join, not two caps. Cap each exposed dash end.
Zero on-length yields a disk (Round), tangent-oriented square (Square), or no mark (Butt).
When corner alignment is true, ignore but retain phase. For each run between tangent discontinuities/path ends,
choose `n=max(1,floor(length/period+0.5))`, scale every pair by `length/(n*period)`, and centre the first dash at both ends.
A corner is a nonzero incoming/outgoing tangent direction difference > 0.000001 radians; zero segments are skipped.
For a smooth closed contour fit the whole loop, starting at its first anchor; deduplicate the seam dot.
Zero-length runs contribute no intervals. Very short positive runs still fit or fail the numeric/work budget.

Inside/Outside apply only to closed contours. Define inside by the compound path's even-odd region, including holes,
independent of winding and whether fill paint exists. Intersect a centred stroke of width `2*w` with that region
for Inside; subtract the region for Outside. Apply the same rule to dashed coverage; reuse existing boolean machinery.
An open path evaluates as Center, retains its requested alignment, and emits `stroke_align_open_center` in reports.
Arrows apply only to the open main contour's endpoints, never holes or dash endpoints. Closed paths retain settings
but emit no heads and report `stroke_arrows_closed_ignored`. Reverse Path exchanges start/end roles with the endpoints.

## Arrowhead library

The request names 29 kinds; pinned VC `a469568` actually declares 40 in `doc/src/appearance.rs:58–105`.
Proposed Phase 2 subset: its first 29 declared kinds below. Moderator must confirm this subset before wire freeze.
Adapt placement/trim algorithms from `effects/src/stroke/arrow.rs`; author original geometric outlines, not copied artwork.
Names are stable wire identifiers, not Illustrator numbering. Adding any other kind later requires another writer bump.

| Exact strings | Geometry family |
|---|---|
| `Triangle`, `TriangleOpen`, `Circle`, `CircleOpen`, `Square`, `SquareOpen` | Filled or hollow primitives |
| `Bar`, `Diamond`, `Arrow`, `ArrowOpen`, `Barbed` | Stop bar, diamond, notched/open/barbed arrows |
| `HalfArrowLeft`, `HalfArrowRight`, `Concave`, `DoubleBar`, `Feather` | Asymmetric arrows, inward notch, paired bars, feather |
| `DotOnBar`, `Chevron`, `DoubleArrow`, `Target`, `Star` | Combined marks, chevron, paired arrows, ring/dot, star |
| `Cross`, `Plus`, `Hexagon`, `HexagonOpen`, `Tag`, `TagOpen`, `HalfCircle`, `Drop` | Geometric marks and hollow variants |

Each library entry supplies a closed filled outline, tip and shaft-attachment inset in units of `w*scale`.
Freeze original geometry and inset fixtures together before 2.1 ships; hollow entries contain actual holes.
Start points outward opposite the first nonzero tangent; end follows the last nonzero tangent (world y-down).
Left/right mean relative to that outward direction. `Tip` places the tip at the endpoint and trims shaft arc length
by its inset; `Extend` leaves the shaft intact and places the tip that inset beyond the endpoint.
Insets account for cap projection so shafts never cross a hollow head's opening or extend past its tip.
If trims overlap, omit the shaft and keep both heads; overlapping heads paint once as one stroke coverage region.
Compute dashes on the untrimmed path, then clip to shaft intervals; Tip must not reset dash phase.
Heads use the stroke paint and object opacity. Union overlapping shaft/head coverage before alpha compositing.

## Consumers, bounds and failure reporting

| Consumer | Required implementation |
|---|---|
| GPU | Core `kurbo::stroke` outline → nonzero fill tessellation; do not extend the round-only triangle stroker. Preserve stroke coverage, knockout, clip bits and object-opacity isolation. |
| CPU raster | tiny-skia native stroke for ordinary centred runs, mapping cap/join/miter/dash explicitly. Shared pre-split runs/dots for corner fitting; fill shared outlines for Inside/Outside, degenerate cases and arrows. Composite stroke coverage once. |
| PDF | Native `w`, `J` (Butt=0, Round=1, Square=2), `j` (Miter=0, Round=1, Bevel=2), `M`, `d` for representable centred strokes. Reset state per stroke with q/Q, including inside isolated forms. Arrows are filled paths. |
| PDF exceptions | Corner-fit/trimmed patterns use shared explicit runs with solid `d`; dots and aligned regions use filled outlines. Union overlapping arrow/shaft coverage as fills when needed for alpha parity; never rasterize. |
| SVG | Native `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit`, `stroke-dasharray`, `stroke-dashoffset` for ordinary centred strokes. Arrows are baked filled paths, **no markers**. Complex aligned/fitted/trimmed cases bake the shared coverage. |
| Native save | Same PDF appearance rules plus editable Path/style in embedded JSON; baked exports never replace the saved editable centreline. |

Vector baking is recorded as `stroke_baked` with object ID, target and reason (editable stroke representation changed).
Every fallback/ignored setting appears in the existing export report, UI and Bridge/CLI result; no silent solid-line
substitution, dropped head or raster fallback. Unsupported/numerically invalid generation aborts export before file replacement.
Budget: ≤100,000 generated dash runs/dots per path; ≤1,000,000 generated path elements per operation/export job.
Count before allocation where possible, check during generation and support cancellation. Budget overflow is a typed
`limit_exceeded`, not a fidelity fallback. Canvas shows an explicit failure; rejected edits leave live state untouched.

`painted_extent` includes fill plus every painted shaft, cap, join, dash dot and arrow outline, transformed into world space.
For conservative pre-culling, expand centreline bounds by `r*max(sqrt(2),miter_limit)` for Miter, otherwise `r*sqrt(2)`;
`r=w/2` for Center, `r=w` for Inside/Outside. Union exact transformed arrow bounds, then tolerance/AA padding.
Use shared coverage bounds for final culling/export selection; never cap a miter allowance at half-width.
Hit-test actual evaluated coverage (even-odd fill, nonzero stroke union), including arrows and excluding dash gaps/hollows.
Use existing screen-space pick slop around coverage, not the entire undashed centreline; clip masks still constrain hits.
Hidden/locked rules stay intact. Direct anchor editing still uses the authored centreline. Legacy Bridge `bounds` stays geometric.

## Bridge 1.2 and CLI

These are proposed DTO extensions to ADR-0009, not serialization of Rust `EditCommand`. Require explicit `api:"1.2"`;
advertise capabilities and complete schemas only after all consumers work. Connection protocol remains ADR-0011's own version.

| Surface | Exact addition |
|---|---|
| `describe`, `fields:["paint"]` | Each path detail gains sibling `stroke_style` with all defaulted fields expanded; existing flat `fill`, `stroke` (colour string/null), `stroke_width`, `opacity` shapes stay. Arrow options read as null when absent. |
| `describe`, `fields:["stroke_style"]` | Return ID and full style only; composes with other field selections, existing revision/pagination rules. |
| `edit`, `op:"set_paint"` | Optional `stroke_style` object replaces the complete style; omitted preserves it. Existing colour/width fields unchanged. |
| `edit`, `op:"set_stroke_style"` | Required `ids` and `stroke_style`; replace full style on every explicit editable path. `{}` resets defaults; null invalid. |
| 1.2 receipts/diffs | Full style on changed path details; IDs-only receipts unchanged. Changes participate in revision, retry hash and journal. |

No new top-level `paint` tool: `paint` is the describe field group and `set_paint` the existing edit verb.
Example op: `{"op":"set_stroke_style","ids":["path:12"],"stroke_style":{"dash":[6,3],"cap":"Butt"}}`.
Use existing `edit` envelope (`api`, `request_id`, `board`, `expected_rev`, `ops`). No selection side effects or partial edits.
Unknown keys, mixed unsupported targets, locked paths and unavailable 1.2 features fail before commit; one batch = one undo step.
Omitted API and explicit 1.0/1.1 retain byte-frozen fixtures and projections; they expose base paint only, never new style keys.
Legacy paint edits preserve unseen style. New style fields/verbs under old APIs fail; no silent version upgrade or style reset.
CLI uses the same service: `varos-cli bridge edit --attach auto --request-file stroke.json --json`; describe likewise uses versioned JSON.
Expose 2.3 as 1.2 `outline_stroke {ids}`, `offset_path {ids,distance,join,miter_limit}`, `expand {ids,stroke:true}`;
all have checked core commands and the same CLI JSON route, typed receipts/errors, cancellation and undo semantics.

## Properties and destructive commands

Properties ▸ Stroke controls only: weight/unit; cap; join; miter limit (enabled for Miter); Center/Inside/Outside;
dash toggle and three dash/gap pairs; phase; preserve lengths/align to corners; start/end head selectors;
independent scale percentages; Tip/Extend; swap heads; reset style. Mixed selection shows mixed values.
Show open-path alignment/closed-path arrow notes; disable phase while fitting without clearing it. One field gesture = one undo.
Control-bar mirror uses the same commands. Geometry, spacing, icons and Offset sheet await the owner Figma round.

Outline Stroke replaces painted stroke coverage (including dashes/arrows/alignment) with editable filled compound paths,
painted with the former stroke paint and no stroke. Keep the original fill below it when present; discard invisible stroke.
Preserve parent, stacking, masks and object opacity; use a pass-through group if two objects are needed. Since group opacity is
not yet stored, partition overlapping fill/stroke regions to reproduce existing opacity/knockout semantics, or refuse atomically.
Allocate fresh IDs for generated geometry, return replacements in receipts, preserve unaffected IDs; one undo restores the source.
Offset Path creates a filled closed offset copy above each eligible closed path; keeps source; positive distance grows the even-odd
region, negative shrinks it (holes respond oppositely); zero is no-op. Use requested join/miter and source fill; no copied stroke.
Reject open or unfilled targets explicitly; erased regions yield no copy plus an empty-result note. No live Offset effect is stored.
Expand bakes supported transforms and checked stroke coverage; an already filled path is a no-op. Reject unsupported future live
features; do not imply Expand Appearance exists. All three commands share evaluated geometry with export and bound generated IDs.
Transform “scale strokes” is a transient command option: off preserves width/dash/phase; on scales them by abs(uniform scale).
Head multipliers/miter limit stay fixed. Nonuniform scale with it on first outlines the stroke then transforms its filled geometry;
use the same atomic replacement/opacity rules. No new transform preference is persisted by v5.

## Acceptance gates

- Golden PDF/SVG plus CPU/GPU comparison for each cap and join, miter threshold, 1/2/3 dash pairs, phase signs, dots and corners.
- Every one of the 29 heads at both ends, both placements and independent scales; hollow, short, reversed and degenerate paths.
- Closed seams, winding reversal, holes/self-intersections, Inside/Outside, open fallback, zero width, opacity overlap and clips.
- Style serde/default/unknown-field boundaries; v5 round-trip; pure v4→v5 migration; unchanged `doc` bytes for every frozen v4 fixture.
- Frozen 1.0/1.1 Bridge fixtures; 1.2 schemas, rollback, retry, pagination, read/write parity, CLI and undo/redo after 2.3 commands.
- Refusal fixtures, v4 old-reader harness, no-raster guard, report assertions, bounds/hit coverage, work budgets and cancellation.
- Adapter/core/format tests on macOS; core check for `wasm32-unknown-unknown`; Windows CI `cargo clippy --locked --workspace --all-targets -j 4 -- -D warnings` (compile, not native UI).
- Independent review, gates and installed owner hand tests per PLAN: dashed arrow, exported PDF/SVG, reopen, Outline Stroke, undo.
