> **Integration w2 (2026-10-09):** gradients are final format **7** (`GRADIENT_VERSION`; formerly the
> provisional `NEXT_GRADIENT_VERSION = 6`). The named migration is `migrate_v6_to_v7` (pure identity,
> no validation — Opus 10). `next_gradients` fixtures were restamped 6 → 7 (future 7 → 10); new refusals
> `refused/v6_gradient.vrs` / `v6_swatches.vrs` (format-6 files carrying format-7 paints). See
> VRS_FORMAT "Wave-2 formats 6–9".

# ADR-0008 amendment: next-format gradients (lane B)

Status: implemented on feat/w2-gradients; independent review and integration pending.
`NEXT_GRADIENT_VERSION` is provisional **6** against this worktree's format-5 base.
The moderator assigns the final number after the image writer, before the text writer.
No version reservation is made; renumber this constant, migration predecessor and new fixtures together.

New keys: optional `doc.swatches` (absent when empty); tagged path `fill` / `stroke` values.
None remains null; solid remains the original four-channel array. A gradient is
`{"type":"gradient","value":{"kind":"linear|radial","stops":[{"offset":0,"colour":[1,0,0,1],"opacity":1,"midpoint":0.5}],"spread":"pad|reflect|repeat","placement":[a,b,c,d,e,f],"focal":[x,y]}}`.
A reference is `{"type":"swatch_ref","value":{"id":1}}`. A swatch has `id,name,paint,global,group`;
`global` defaults false, `group` defaults empty and is omitted when empty. IDs are document-local.
The illustrative gradient above is incomplete: real paints require 2–256 sorted stops.
Placement is an invertible affine map from unit gradient coordinates into path coordinates.
Midpoints are 0.01–0.99; stops/channels/opacity are finite in 0–1; focal is strictly inside the unit circle.
Swatches are capped at 4096, names/groups at 256 bytes, unique nonzero IDs, and no reference chains.
Unknown nested fields, dangling references, singular placement and future versions are refused.

Named pure migration: `migrate_v5_to_next_gradients(Document, &Limits)` validates and returns
unchanged authored content. Missing table becomes empty during typed decode. Old paint encodings and
canonical document subtrees stay byte-identical. The JSON/PDF version stamps advance on Save.
Opening runs the existing migration notice and never writes the source file.
The next-keys precheck refuses even malformed tagged paints/table presence in earlier versions.
Existing v4/v5 JSON/PDF/SVG fixtures and hashes remain frozen; tests normalize only writer stamps.
New accepted/refusal fixtures: core `tests/fixtures/next_gradients`, SHA256SUMS;
PNG/SVG/PDF paint goldens: raster `tests/fixtures/gradients`, SHA256SUMS.

Bridge: `colour` under explicit API 1.2; registered through list_verbs/schema. Legacy tools/list
fixtures stay unchanged; legacy paint reads return `not-solid` for new variants. Table/palette reads
use describe fields `swatches,palette_gpl,palette_ase,palette_native` and retain reply budgets.
CLI: API 1.2 Colour commands in apply; palette-import / palette-export host the pure codecs.

PDF uses axial/radial sampled shadings and luminosity alpha masks, with an isolated knockout form;
4096 samples at 16 bits are disclosed as `gradient_sampled`. Generated stream budget is checked first.
Gradient strokes use filled coverage and are reported as `stroke_baked`; SVG keeps native gradients,
explicit midpoint samples, spread and transform. GPU uses a cached 1024-texel LUT and dither.
GPL/ASE cannot carry gradients/alpha; GPL cannot carry global linkage. Such export is refused,
with native JSON offered to preserve authored data. ASE supports RGB process/global blocks and groups;
spot/non-RGB and unknown blocks are refused. Geometry baking materializes gradient references by value;
solid global references stay linked. Copy/duplicate/eyedropper/Pathfinder own the copied paint.
