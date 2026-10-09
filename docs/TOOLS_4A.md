# Slice 4A select and transform tools

Owner decision 2026-10-09 lifts the Figma gate for this slice. The UI is provisional and uses
existing kit fields/actions, box frames, and append-only tokens. Owner design review and native
hand testing remain pending; this lane must not launch or install the GUI.

Core implementation lives in `varos-core/src/select_transform.rs` and `tools/select_transform.rs`.
Reflect (O), Shear, Free Transform (E), and Magic Wand (Y) share the existing pointer engine.
Command-drag on a Free Transform side handle shears; distort and perspective remain deferred.
Option-click Rotate/Scale/Reflect/Shear opens its sheet at the clicked reference point.
Object > Transform opens the same sheets; Transform Each keeps whole groups as units.
All angles follow the existing core screen-coordinate rotation convention; scale ratios are
shown as percentages. Shear is baked into anchors and Bézier handles, including holes.

Preview commands are `TransformBegin`, `TransformLive`, `TransformCommit`, `TransformCancel`.
Each live frame starts from the captured document; Copy preserves source units and creates
fresh stable IDs. Apply/Copy publishes one undo step; Cancel publishes none. Lifecycle actions
settle a valid preview before save or tab switch. The sheet blocks canvas input while open.

The provisional tool-options home offers tools, sheets, wand tolerances, eyedropper pick toggles,
and Layers operations. Native Object > Transform and Window > Layers mirror its command paths.
Shift-click Eyedropper samples colour into the active paint target only. Double-click a group
isolates; the breadcrumb, Escape and double-click-empty exit. Isolation is ephemeral, confines
selection, and dims other objects through scene opacity without changing document paint.

Bridge API 1.2 opt-in edit verbs: `transform` (ids + spec), `magic_wand` (ids + options + mode),
`eyedropper` (ids + source + options + colour_only), `isolation` (ids + exit), `layers`
(ids + action), and `tool_options` (wand and/or eyedropper). Request-local targets work.
`spec` includes scale, movement, angle, reflect axis, shear/shear_axis, origin, each, random,
seed and copy. Random is deterministic and uses one factor per object/group. Bridge applies
transforms atomically; preview steps are available through the core command/CLI batch path.

Headless CLI uses `varos-cli apply input.vrs --batch edits.json --out output.vrs`, with a
`{"api":"1.2","commands":[{"SelectPaths":[12]},{"Transform":{"reflect":90,"copy":true}}]}`
envelope. API 0.x batches refuse new commands; attached Bridge 1.0/1.1 refuse new verbs.
Existing Bridge fixtures and format version remain unchanged. Prior-art provenance is in NOTICE.

Fix round (2026-10-09): Release Build refuses requests whose peak paths, anchors (including
holes), tree nodes or stable-ID allocations exceed the existing budgets, before copying art.
Targets must be unique and must not overlap ancestors; refusal leaves document/history intact.
Flatten and Merge carry removed layer visibility/lock flags onto their surviving children.
Identical sampling and repeated Hide/Lock Others preserve revision, undo and redo. Transform
Each reflection toggles refresh the preview before Apply; unchanged options dispatch no command.
