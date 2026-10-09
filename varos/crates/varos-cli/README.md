# varos-cli — provisional headless Bridge groundwork

Added 2026-10-06 under the owner's explicit headless work order, without waiting for the Bridge ADR. This is host 2 of `docs/VISION_AI_NATIVE.md`: the existing Editor and document format, without a window. These contracts are **provisional**; wire spellings are explicitly pinned for Bridge API 0.x, with future changes governed by versioning and ADR-0009. No MCP server, desktop attachment, chat UI or provider integration is included.

Run from `varos/`: `cargo run -p varos-cli -- <subcommand>`. The binary is `varos-cli`; the desktop binary remains `varos`. The CLI has exactly three direct dependencies: `varos-core`, `varos-pdf`, `varos-raster`. The raster crate shares the existing CPU thumbnail implementation with the app; no GPU/window/UI dependency reaches the CLI or core.

## Common JSON envelope and errors

Exactly one compact JSON object plus newline is written to stdout:

```json
{"ok":true,"result":{}}
```

Errors return exit status 1 and a reason (including argument, load, validation, write errors and caught panics):

```json
{"ok":false,"error":{"index":1,"reason":"unknown path id 999999"}}
```

`index` is the zero-based failing batch entry, or `null` for errors outside a batch. Invalid top-level batch JSON uses index 0. No progress text is written to stdout. Success exits 0. Paths can contain spaces; `--` ends option parsing. Options may precede file arguments. Duplicate/unknown options and extra/missing arguments are refused. All reads use the existing bounded v3 loader (raw JSON and native PDF containers; existing v1/v2 migrations still apply). Outputs use unique sibling temp files and rename only after a complete, synced write; validation never touches the destination. An existing `--out` file is replaced after a successful write. `apply` refuses an output resolving to its input unless `--in-place` is present.

## Commands and result shapes

### `describe <board.vrs> [--detail <id>]`

Summary result:

```json
{"name":"Board","description":"A logo","tags":["client"],"artboards":[{"x":0,"y":0,"w":100,"h":100,"name":"Artboard 1","bleed":0,"page_color":[1,1,1,1],"clip":true,"hidden":false,"locked":false}],"element_count":2,"elements":[{"id":"node:1","kind":"layer","name":"Layer 1","bounds":[0,0,60,60],"fill":null,"stroke":null,"hidden":false,"locked":false,"parent":null},{"id":"path:10","kind":"path","name":null,"bounds":[0,0,60,60],"fill":[0.1,0.4,0.9,1],"stroke":{"paint":null,"width":1},"opacity":1,"hidden":false,"locked":false,"parent":"node:1"}]}
```

Elements include paths, groups and layers; artboards are listed separately. `element_count` counts those element rows, not anchors or artboards. Paths and nodes occupy separate id namespaces in v3, so public ids use `path:N` and `node:N`. Path-leaf nodes are represented by their paths rather than duplicate rows. Ordering follows the tree in back-to-front stacking (paint) order, with each container before its descendants, including nested groups. A null path name means the existing automatic name; the board name is the stored metadata (empty means the app uses its usual file-name fallback).

Bounds are world-space outline bounds `[min_x,min_y,max_x,max_y]` (no stroke inflation); empty geometry has null bounds. Fill and stroke paint are null for none, or four RGBA floats in 0..1. Container paint is null. All numbers in describe summary/detail JSON are rounded to three decimals, consistently with the SVG exporter; persisted geometry is unchanged. Path hidden/locked flags are effective through ancestors/artboards; container flags are the node's own flags.

`--detail path:10` returns that element alone, adding `geometry` (the complete serialized Path: anchors, handles, holes, appearance and flags), `node` (its complete leaf Node) and `world_transform` (`rot`, `piv`). `--detail node:15` adds the complete Node in `geometry`, including child order, local transform and clipping role/mask. Geometry is in the persisted local coordinate space; bounds and world_transform explain its world placement. Unknown ids fail.

### `snapshot <board.vrs> --out x.png [--size 400]`

```json
{"out":"x.png","width":400,"height":400,"bytes":12345}
```

Deterministic CPU PNG, square dimensions, size defaults to 400 and must be 1..2048. Fits visible content/artboards to the existing thumbnail well, with padding and the existing dotted background. Shares the app's clipping, opacity and knockout raster behavior. Memory grows with group depth: each isolated/clip group allocates a full RGBA pixmap (16 MiB at 2048×2048), in addition to the base image. No new visual design or application UI is introduced.

### `export-pdf <board.vrs> --out x.pdf`

```json
{"out":"x.pdf","bytes":12345}
```

Uses `plan_pdf_export` and `export_pdf_bytes`, matching desktop File ▸ Export. Produces a model-free deliverable with no embedded editable model or private metadata. The default scope is all visible artboards, or artwork bounds for boardless documents; empty boardless documents have nothing to export.

### `save-as <board.vrs> --out x.vrs`

Returns `{"out":"x.vrs","bytes":12345}`. Uses `write_pdf_checked` to save the editable native v3 PDF container, including the model, with existing save validation.

### `apply <board.vrs> --batch commands.json [--out new.vrs] [--in-place]`

```json
{"out":"new.vrs","commands":4,"changed":true}
```

Supply `--out` for a separate output. If its canonicalized path equals the input (including symlink aliases), replacement requires explicit `--in-place`. `--in-place` alone replaces the input; if also supplying `--out`, it must resolve to that same input. In-place writes replace the canonical input and preserve symlink aliases.

The batch file is a versioned Bridge envelope containing `EditCommand` values, at most 1 MiB. `api` is required; major 0 is supported (current version `0.1`), other majors and malformed versions are refused before editing:

```json
{"api":"0.1","commands":[
  {"SetBoardName":"New name"},
  {"RenamePath":{"path":10,"name":"Logo"}},
  {"SelectPaths":[10]},
  {"SetOpacity":0.75}
]}
```

Variant and payload field spellings are explicitly pinned with `#[serde(rename)]` in **one serde enum table**, `varos-core/src/command.rs`; no second JSON-to-command name mapping exists. Struct variants are objects, one-value variants wrap their value, and unit variants are strings (e.g. `"AddArtboard"`). Unknown verbs and unknown struct fields are rejected. Numeric ids in edit payloads remain native core ids: `SelectPaths` takes path ids, node operations take node ids. `SelectPaths` selects precisely the supplied paths and does not widen selection to groups. Artboard operations take zero-based indices, which are unstable across `DeleteArtboard` (later indices shift). Both limitations are flagged for ADR-0009; callers must refresh describe results after structural edits. Payload enum spellings (paint, alignment, distribution, boolean, arrange and drop positions) are also pinned with serde renames.

Supported checked document commands:

| Command | Payload inside the variant object |
|---|---|
| AddShape | `{ "kind":"Rect", "bounds":[0,0,100,80], "parent":null, "fill":[1,0,0,1], "stroke":null, "stroke_width":0, "opacity":1, "name":null }`; kind Rect or Ellipse; paint uses RGBA arrays, not API 1.0 hex strings |
| SelectPaths / SelectAnchors | array of path / anchor ids (empty clears selection) |
| SetBoardName / SetBoardDescription | string |
| SetBoardTags | array of strings; existing cleaning/deduplication rules apply |
| RenamePath / RenameNode | `{ "path":10, "name":"Logo" }` / `{ "node":15, "name":"Logo" }` |
| ToggleNodeHidden / ToggleNodeLocked | node id |
| SetObjectBounds | `{ "x":0, "y":0, "width":100, "height":100, "anchor_x":0, "anchor_y":0 }`; x/y/width/height may be null/omitted |
| SetObjectRotation / SetOpacity / SetStrokeWidth | degrees / 0..1 / non-negative number |
| ApplyPaint | `{ "target":"Fill", "color":[1,0,0,1] }`; target Fill or Stroke, null color removes paint |
| SetClipExempt / Flip | bool; Flip true = horizontal |
| Align | `{ "mode":"Left", "target":"Auto" }`; modes Left/CenterH/Right/Top/Middle/Bottom; targets Auto/Selection/Artboard |
| Distribute | Horizontal or Vertical |
| Boolean | Unite/MinusFront/Intersect/Exclude |
| Arrange | Front/Forward/Backward/Back |
| Nudge | `{ "x":1, "y":0 }`; SelectAnchors first (objects use SetObjectBounds) |
| Paste | `{ "offset":[10,10] }`; null/omitted = in place |
| MoveLayer / DuplicateMoveLayer | `{ "sources":[13], "target":1, "position":"Into" }`; positions Before/Into/After |
| MoveLayerToBoard | `{ "sources":[13], "source_board":0, "target_board":1 }`; source_board may be null/omitted |
| SetActiveArtboard / ToggleArtboardClip / ToggleArtboardHidden / ToggleArtboardLocked / OrientArtboard / DuplicateArtboard / DeleteArtboard | artboard index |
| SetArtboardRect | `{ "index":0, "x":0, "y":0, "width":100, "height":100 }`; coordinates/dimensions may be null/omitted |
| RenameArtboard | `{ "index":0, "name":"Page" }` |
| SetArtboardColor | `{ "index":0, "color":[1,1,1,1] }`; null color = transparent |
| SetArtboardCount | integer 1..1000 |
| SetMoveArtWithArtboard | bool |

Unit commands: `"GroupSelection"`, `"UngroupSelection"`, `"DeleteSelected"`, `"DeleteLayerSelection"`, `"SwapColors"`, `"DefaultPaint"`, `"Copy"`, `"Cut"`, `"AddArtboard"`, `"CycleUnits"`.

`AddShape` is accepted by legacy API 0.x `parse_batch` through the same pinned `EditCommand` serde table and checked core allocation/staging path. Its required fields are kind, bounds, stroke_width and opacity; parent/fill/stroke/name may be omitted or null. Bounds must be positive and finite, and paint values valid; null parent uses the active layer. It does not select the new path or return its allocated ID in the CLI apply receipt; refresh `describe` after saving. Both null paints are valid in this low-level provisional API, whereas attached API 1.0 `add_shape` refuses them. We document the existing acceptance rather than adding a second version-specific verb filter; API 0.x deliberately exposes the checked core command table.

Selection-target commands require a selection. Paths/anchors must exist and be visible/unlocked. Coordinates must be finite, dimensions positive, RGBA/opacity bounded, structural moves legal, and the resulting document must pass structure and semantic validation after **each** entry. Allocation near id exhaustion is refused before mutation. Clipboard state is the core's in-memory clipboard, never the OS clipboard; CLI batches start empty and can Copy/Cut then Paste.

Interactive picker/guide/TransformAgain commands are refused because they require gesture state. Undo/Redo inside a batch are refused. Preference-only commands (SetRulerOrigin, SetSnapConfig, ToggleSnapping, ToggleGuidesLocked, ToggleSmartGuides) are refused because normal core undo deliberately preserves those preferences. Their refusal is an indexed error, not a silent no-op.

The batch runs in a staging Editor. Any failed entry discards the entire batch, preserving the caller's document, selection, revision, prior undo and redo, and the output file. A successful changed batch becomes **one undo step** in the live core Editor; empty/no-op batches add none. Selection/clipboard-only changes do not count as document changes. A standalone CLI writes the resulting document only: **v3 does not persist undo history**, so reopening the file starts the usual fresh history. Live desktop attachment and transporting that one undo step to a running window remain future Bridge work.

### `new --preset <name> --out x.vrs`

```json
{"out":"x.vrs","preset":"square","artboards":[...]}
```

Case-insensitive names: free (zero artboards), square, portrait, story, a4. Uses the existing core presets unchanged, writes a checked native v3 PDF container. Unknown names fail.

### `diff a.vrs b.vrs`

```json
{"added":["path:20"],"removed":["path:10"],"changed":[{"id":"node:15","summary":"changed bounds, geometry"}],"document_changes":["name","artboards"]}
```

Element arrays follow back-to-front tree paint order (removed uses the old tree; added/changed use the new tree), change summaries are one line listing changed fields. Full geometry and nodes are compared internally, so handle/hole/transform/mask/parent/child-order changes count even when compact bounds are identical. Changes to board metadata, artboards and other document-level fields appear in `document_changes`. No full geometry is printed by diff. Empty arrays mean no differences in that category.

## Verification

Integration tests execute the actual CLI against all four frozen v3 raw-JSON/PDF fixtures, without modifying them. Coverage includes all seven verbs, detail geometry, deterministic PNG dimensions/bytes, model-free PDF privacy checks, editable save-as reopening, canonical input protection, version refusal, golden nested-group JSON, preset creation, indexed errors, preservation of source/destination on failure, and core batch undo/redo/rollback/no-op behavior. No test constructs a GPU Renderer or EventLoop. Run the workspace's Mac tests, Mac/Windows-target clippy and fmt gates from `varos/`.

Slice 4E/4D headless commands: `Pathfinder` takes `"divide"`, `"trim"`, `"merge"`, `"crop"`,
`"outline"`, or `"minus_back"`; `ShapeBuilder` takes `{ "points": [[x,y],...], "delete": false }`;
`Scissors` takes `{ "path": N, "segment": 0, "t": 0.5 }`; `Knife` takes `{ "points": [...] }`;
`Eraser` takes `{ "points": [...], "radius": 8 }`; `DivideObjectsBelow` is a unit command.
Use `SelectPaths` first in the existing `apply --batch` API 0.1 envelope. Attached `bridge edit`
exposes corresponding snake-case verbs with explicit `ids` under API 1.2 only. Construction operations
flatten curves; Scissors retains cubic handles. UI and fixed interactive eraser radius are provisional.

Lane B, API 1.2: `apply` batches accept `Colour` (paint, swatches, reduce, recolor) after `SelectPaths`.
Paint is null, an RGBA array, or `{"type":"gradient","value":...}` / `{"type":"swatch_ref","value":{"id":N}}`.
Bridge's discoverable `colour` verb exposes the same command under `command` with explicit `ids`.
`palette-import FILE PALETTE OUT.vrs` imports GPL/ASE/native JSON; `palette-export FILE OUT.gpl|ase|json`
creates an exclusive destination. GPL/ASE refuse gradient/alpha loss; GPL also refuses global-linkage loss.
Use native JSON for lossless palettes. Palette input is capped at 4 MiB; document table at 4096 entries.

Lane C: `colour-management FILE COMMAND.json OUT.vrs` sets `{"action":"mode","mode":"Cmyk"}`
or a targeted paint envelope `{"ids":[1],"command":{"action":"paint","target":"Fill",
"colour":{"colour":{"model":"cmyk","c":0.2,"m":0.3,"y":0.4,"k":0.1},"alpha":1}}}`.
Profiles use `{"action":"profile","profile":{"name":"Printer","data":"ICC_BYTES_IN_HEX"}}`.
Proof/overprint are view-only Bridge/desktop commands; they do not alter saved artwork.
