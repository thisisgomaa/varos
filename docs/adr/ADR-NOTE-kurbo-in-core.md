> **Status:** proposed dependency/adoption note — 2026-10-09; Phase 2.0, pending independent review.

# kurbo in varos-core

The approved [PLAN](../PLAN.md) needs curve stroking now and offset/fitting/simplification later.
Adopt `kurbo` as a pure geometry dependency behind `varos-core/src/geom/kurbo.rs`.
This note authorizes the dependency boundary; it neither adds the dependency nor changes persisted data.
The [Stroke engine contract](../foundation/work_orders/STROKE_ENGINE.md) owns user-visible semantics.

## Dependency review

| Item | Reviewed choice / evidence |
|---|---|
| Version | `kurbo = { version = "=0.13.1", default-features = false, features = ["std"] }` for the initial adoption. Already present transitively in `varos/Cargo.lock`; 0.11.3 is also locked. Do not upgrade unrelated users. |
| License | Package declares `Apache-2.0 OR MIT`; retain both upstream license texts and attribution under the repo's borrowing rules. |
| Toolchain | Published 0.13.1 manifest declares Rust 1.85 and edition 2024; verify workspace toolchain in the implementation gate. |
| Features | Explicit `std`; no requested `serde`, `schemars`, `mint`, `euclid` or `libm`. At least `std` or `libm` is required. |
| Runtime dependencies | `arrayvec`, `smallvec`, `polycool`; optional integrations may be unified by other workspace consumers. Inspect resolved feature graph rather than promising features remain globally disabled. |
| Native footprint | No UI/window/render backend, IO service or build script in the reviewed package; geometry uses allocation and floating-point math. |
| wasm32 | Architecturally suitable for `wasm32-unknown-unknown` with `std`/allocator; no native UI dependency. This is source review, not a completed target build. |
| Windows | Core remains portable; require the repo's Windows compile gate. This does not imply native Bridge attachment/UI support. |
| Upgrades | Review changelog, numerical output and golden changes explicitly; no silent patch upgrade of the initial exact pin. |

Verified against the cached 0.13.1 `Cargo.toml`, `LICENSE-MIT`, lockfile and upstream
[API documentation](https://docs.rs/kurbo/0.13.1/kurbo/) and [feature manifest](https://docs.rs/crate/kurbo/0.13.1/features).
The upstream docs expose stroke expansion, cubic offsets, fitting and path simplification; they do not define Varos's
dash fitting, arrow placement, error budgets, persistence or boolean semantics. Those remain our reviewed contracts.
No benchmark, numerical-equivalence result, security audit or cross-target pass is claimed here.

## Adapter contract

| Direction / concern | Requirement |
|---|---|
| `Anchor` → `BezPath` | Promote f32 positions/absolute handles to f64; MoveTo first anchor, LineTo with no active handles, otherwise CubicTo using absent handle = endpoint. Emit the final closing segment and ClosePath when closed. |
| Compound paths | Main contour plus separate hole subpaths; preserve contour order and closure. Do not silently connect contours or change even-odd fill meaning. |
| `BezPath` → anchors | Lines/cubics map to Varos anchors; convert quadratics to exact equivalent cubics. Reject nonfinite/overflow values before checked f32 conversion. Preserve subpaths and closure. |
| Winding | kurbo stroke output is nonzero-filled geometry. Convert its region to Varos's even-odd compound paths through existing topology helpers before committing Outline/Expand; do not simply relabel the fill rule. |
| Identity | Geometry-only evaluation allocates no document IDs. New destructive results get fresh IDs through the checked allocator in a transaction. No IDs inferred from coordinates. |
| Metadata | Anchor IDs, smooth flags and absent-vs-degenerate handles need a side mapping when preserving an existing path; `BezPath` alone cannot round-trip them. Never rewrite an untouched Path through a lossy geometry round-trip. |
| Tolerances | Explicit document-space tolerance/work budget supplied by caller; f64 internals, checked f32 result. Screen tolerance derives from device scale. |
| Errors | Typed unsupported/numeric/budget errors; no panic, silent truncation or empty-path substitution on error. Cancellation at bounded generation checkpoints. |
| Ownership | Temporary geometry and evaluator caches stay outside serialized `Document`. Public model/Bridge DTOs expose Varos types, never kurbo serde. |

The adapter may expose kurbo types within core's geometry implementation; renderer/exporter entry points consume a
Varos-owned stroke plan/outline interface. Consumers must not independently implement cap, dash, trim or alignment rules.
Do not treat a self-intersecting outline as a single outer ring with every later contour a hole without region classification.
Keep existing f32 authored anchors as the schema; this is not a document-wide conversion to f64.

## Allowed and excluded use

| Use | Boundary |
|---|---|
| Stroke outlines | `kurbo::stroke` expands shared evaluated runs; feed fill tessellation/coverage, not a new CPU canvas architecture. |
| Offset | Curve offset/expansion primitives may support the destructive Offset Path command; topology cleanup stays in the existing geometry stack. |
| Fitting | Pure curve fitting for later Pencil/Smooth work; separate behaviour tests and phase approval still apply. |
| Simplify | Pure path simplification with explicit error tolerance; never automatic load/save normalization. |
| Metrics | Arc length, tangents and bounds behind adapter helpers when required by the above operations. |
| Booleans | **Do not replace `flo_curves` or `i_overlay` booleans without a separate ADR**, topology oracles and compatibility evidence. No implicit adoption of linesweeper. |
| Out of scope | Replacing Anchor/Paint/Document, using kurbo serialization on disk, UI geometry/layout types, egui/eframe, vello/CPU-renderer imports or wholesale VectorCraft crates. |

Inside/Outside clipping and self-intersection cleanup reuse reviewed existing boolean operations. A difficulty there
does not authorize substituting a new boolean engine under this dependency note.

## Prior art and attribution

Follow [EXTERNAL_PRIOR_ART](../reference/EXTERNAL_PRIOR_ART.md). VectorCraft is pinned at `a469568`, PhotoCraft at `4cb7cf3`.
Read/adapt `VC/crates/geom/src/path.rs`, `effects/src/stroke.rs`, `stroke/dash.rs`, `stroke/arrow.rs` and
`doc/src/appearance.rs` for adapter/field/algorithm ideas. PhotoCraft `crates/vector/src/stroke.rs:101` demonstrates
GPU-friendly positive-oriented outline pieces: use the idea, not its renderer or its area-adjusted arc approximation.
Build original arrow geometry; adapt placement/trim logic with attribution. No brand assets or Illustrator artwork.
Do not copy VC dash fallback-to-solid on budget failure; the Varos evaluator returns a typed failure and export report.

When the first dependency/borrow lands, add this line to `varos/NOTICE` (not in this docs-only task):

```text
kurbo 0.13.1 — Copyright (c) 2018 Raph Levien; Apache-2.0 OR MIT; https://github.com/linebender/kurbo — used for pure curve geometry in varos-core.
```

Retain kurbo's `LICENSE-MIT` and `LICENSE-APACHE` under distinct third-party paths alongside NOTICE; do not overwrite
the application's GPL license. For copied/adapted blocks, also retain the ArtCraft license texts/copyright and list the
actual source paths and pins in NOTICE. Source headers use the repository's exact attribution pattern, for example:

```text
// Adapted from VectorCraft crates/effects/src/stroke/dash.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
```

A dependency attribution does not cover copied ArtCraft code; enumerate only what the implementation actually borrows.

## Implementation gate

- Inspect `cargo tree -p varos-core -e features` and the target-specific tree; retain a pure dependency graph.
- Adapter tests: empty/open/closed paths, closing cubic handles, holes, degenerate segments, quadratics, overflow and metadata preservation.
- Geometry tests: original default appearance, caps/joins/dashes, offset/simplify error bounds, cancellation and work caps.
- Run existing core/format/boolean tests; preserve frozen v4 serialized bytes and Bridge 1.0/1.1 fixtures.
- `cargo check -p varos-core --target wasm32-unknown-unknown` and the repo's Windows target gate; record unavailable targets as blocked.
- Review license copies/NOTICE/source headers and numerical goldens independently before the implementation merge.

No dependency files, license files, NOTICE, adapter code or tests are created by this note.
