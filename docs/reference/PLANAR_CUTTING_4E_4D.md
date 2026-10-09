# Slice 4E / 4D — provisional implementation, 2026-10-09

Owner decision lifts the Figma gate for this lane. Existing kit controls, palette, box system and
shortcut patterns remain the visual authority; owner design review and native hand test are pending.

The arrangement uses the already-installed i_overlay 7, with even-odd compound fill membership.
Each face carries ascending source owners and signed input winding. Curves are sampled at 32 points
per cubic for construction operations; the existing four curve-preserving booleans remain unchanged.
Open selected paths slice Shape Builder's filled faces without adding coverage. No linesweeper.
Divide keeps individual connected faces; Trim keeps visible source fragments; Merge joins visible
fragments with identical fill and opacity; Crop consumes the front cutter and keeps lower visible
fragments inside it; Outline emits split, open input-boundary edges; Minus Back retains the top-only
area. Trim/Merge/Crop remove strokes. Source paint, opacity, name, layer/group and sibling position
are retained by replacement fragments, with live transforms baked inside the single undo transaction.

Shape Builder (Shift+M) records a pointer walk: traversed faces union, Alt-drag deletes. Narrow faces
crossed between pointer events count. Highlights use the existing scene overlay. Scissors (C) splits
an outer cubic exactly at its parameter with de Casteljau; the interactive hit parameter is sampled then locally refined.
Knife slices selected closed fills by a freehand polyline. Eraser (Shift+E) subtracts a round capsule
sweep (provisional fixed 8 screen-pixel radius; headless radius is in document units). Divide Objects
Below consumes the selected front-most cutter and partitions editable closed objects below it.

Bridge `edit` API 1.2 adds `pathfinder` (all ten modes), `shape_builder`, `scissors`, `knife`, `eraser`,
and `divide_objects_below`. Each requires explicit `ids`; gestures take `points`, Scissors one target
plus `segment`/`t`, Eraser `radius`. APIs 1.0/1.1 reject these verbs, including nested repeat. Existing
fixtures and the default MCP tools/list schema stay unchanged. The expanded MCP schema is
explicitly requested by tools/list params `{ "api": "1.2" }`. Headless CLI: `apply --batch` with internal `EditCommand` JSON; attached
CLI: `bridge edit` with `api: "1.2"`. No file-format bump, new dependency, GPU test or install.

Fix round (2026-10-09): replacements touching any clipping-mask source are refused atomically;
Scissors refuses compound contours (checked callers receive an error), retaining hole coverage.
Shape Builder retains disjoint selected source components verbatim and skips unchanged merges.
Arrangement caching is transient, keyed by document revision and selected IDs, invalidated by
commits, undo/redo and document replacement; drag highlighting checks only new walk segments.
Merge contract: construction command variants/Bridge verbs and the `Construction` drag are additive;
C / Shift+E / Shift+M dispatch is a self-contained guarded block, with Ctrl+C retained. No new settings.
Sibling shortcut lanes must add Shift+C and plain E without removing those guards. API 1.2 capability
and schema additions must be unioned, keeping the existing size limit and frozen default schema.
Crash-safety integration must wrap `Editor::execute` and checked creation/batches centrally; this
checkout has no sibling panic guard to test. Keep `source.clone()` when integrating stroke fields,
and reconcile the documented 32-step flattening policy with the geometry lane. No storage edits.
