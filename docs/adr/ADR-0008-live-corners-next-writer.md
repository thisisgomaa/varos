> **Integration w2 (2026-10-09):** Live Corners are final format **9** (`CORNERS_VERSION`), together
> with Lane F's container-only Quick Look preview. The named migration is `migrate_v8_to_v9` (identity;
> formerly `migrate_v5_to_live_corners`). Fixtures restamped 6 → 9; new refusals
> `refused_corners_on_v8.json` and `refused_future.json` (10). See VRS_FORMAT "Wave-2 formats 6–9".

# ADR-0008 amendment — live corners, next writer (Lane C)

Status: implemented in feat/w2-export-paths; provisional UI and owner design review pending.
The integrator assigns the next version in merge order; local CORNERS_VERSION is 6.

The additive key is `doc.paths[].corners`, omitted when empty. Each outer-anchor-indexed
entry contains finite `radius` (0–1000000 points) and `kind` (round/inverted/chamfer).
Missing entries mean zero radius; unknown fields/kinds, null, negative/nonfinite radius,
more entries than anchors and nonzero radius on curved/ineligible vertices are refused.
Render, hit/flatten, SVG/PDF/raster and outline/offset consume evaluated geometry; authored
anchors stay unchanged. Expand and anchor edits bake corners with fresh stable anchor IDs.

The named pure migration `migrate_v5_to_live_corners` is identity: omitted arrays already
decode empty; no IO, normalization or allocations. Earlier formats reject presence of
`corners` before typed decoding, including an empty or null value. No downgrade is offered.

Frozen fixtures: core tests/fixtures/lane_c/{next_corners,refused_corners_on_v5,
refused_negative_radius,next_live_round}.json. Existing v1–v5 and Bridge 1.0/1.1 fixture bytes stay untouched;
v5 migration tests compare the authored JSON bytes and PDF page content, and current-reader
future-version refusal uses a synthetic undecodable next-version sentinel. The historical
v4 reader remains frozen; an isolated frozen v5 header replay also refuses the next writer before decoding.

The model/container use one version constant. Moderator must renumber the constant,
migration table and lane fixtures together when sibling writer slices merge.
