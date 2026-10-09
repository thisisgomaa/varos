# Phase 11 live nodes — Lane E

Owner authorization: provisional UI from existing kit, final owner design review pending.
This implementation adds no UI dependency to core and adds no runtime crates or token definitions (tests reuse the existing lopdf crate).

The v13 JSON enum is `NodeKind::Live` with internally tagged `effect`:
- `blend`: `spine` (optional path ID, an owned child), `steps` (intermediate copies), `orientation` (`page`/`path`). Two source paths; matching open/closed state and subpath count. Unequal anchor counts subdivide cubic segments without changing source curves.
- `repeat`: radial count/radius; grid rows/cols with two-axis gap; mirror horizontal/vertical axis at the content's bottom/right edge.
- `envelope`: warp Arc/Flag/Bulge, bend -1…1; or four mesh points ordered top-left, top-right, bottom-left, bottom-right. Geometry samples each source cubic at 64 intervals before applying the point map, consistently in canvas and exports. Nonlinear gradient envelopes are refused explicitly.

Sources remain path children with stable IDs. Make does not flatten the authored sources.
Double-click enters existing isolation; Direct edits the source/spine there. Object ▸ Blend ▸ Replace Spine consumes one selected independent open path alongside a selected blend; the old spine is released. Blend/Repeat/Envelope Options change one undoable command; Expand replaces the container by derived editable paths; Release returns a group containing the source paths (including any spine).

A per-node cache compares exact resolved source paths, options and spine, so live gestures, swatch changes and undo invalidate it without relying on revision alone. The canvas's existing scene-signature cache keeps idle frames unevaluated. Derived output is bounded to 4,096 paths and 200,000 anchors; expansion checks ID space. Sources are bounded to finite coordinates within ±1,000,000 pt, spines to 64 anchors. No GPU or event loop in tests.

API 1.2: `live_make`, `live_options`, `live_release`, `live_expand`, `live_isolate`, `live_spine`; `schema` and `list_verbs` expose full parameters. New verbs do not appear in the inline `tools/list` summary. CLI: existing `apply INPUT.vrs --batch COMMANDS.json --out OUTPUT.vrs`, envelope `api: "1.2"`, commands `{"Live": {"action": …}}`.

Version 13 is pre-assigned. `migrate_v12_to_v13` is pure identity: absent live nodes stay absent. Lane-local v9→v10→v11→v12 placeholders are identity; integrator must replace them with the other wave-3 lane migrations. Older-era stamps carrying `Live` are refused before typed decoding. Original fixtures remain frozen; new v13 fixtures exercise live JSON, native PDF container and refusal cases.

Initial scope refuses grouped/image/text sources and mask-owned live containers; subpath count changes are not synthesized. Final native visual/interaction acceptance, independent integration review and mixed-lane semantics remain pending.
