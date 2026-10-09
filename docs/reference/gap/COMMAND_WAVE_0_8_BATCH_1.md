# Slice 0.8 command wiring (2026-10-09)

Worktree `p0-commands`, base `f2066f1`. No format bump. No commit, push,
GUI launch, install or native acceptance in this lane.

## Evidence and implemented routes

Paths below are relative to `varos/crates/`.

| Behaviour | Inventory evidence | Implementation |
|---|---|---|
| Select All / Deselect / Reselect / Inverse / Next Above-Below / Artboard / six Same modes | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:26-27` | `varos-core/src/command_wave.rs`, `varos-app/src/menus/select.rs` |
| Lock / Hide Selection, Unlock / Show All | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:43` | `EditCommand::Object`, native shortcuts + Windows command submenu |
| Six distribution modes, fixed spacing, key object | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:37-39` | core unit-aware distribution; `ui/panels/align.rs`; last clicked selected path is the key; azure outline |
| Join, Average, Reverse, Add Anchors, Clean Up | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:79-84` | core command boundary; Object/Path rows; Average = both coordinates |
| Compound Make / Release | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:73` | existing `Path.holes`, anchor IDs preserved on release |
| New Layer / New Sublayer / Send to Current Layer | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:116,166` | Object/Layers rows; footer New Layer, Option-click creates sublayer |
| Paste Remembers Layers | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:121,166` | persisted app preference, source layer-name ancestry retained/recreated on paste |
| Group Selection / Lasso | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:23-24` | Option-Direct climbs one group; freehand Q selects anchors, or objects when an object selection is active; Shift adds |
| Anchor Point / Add / Delete tools | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:59-60` | Shift-C / + / - and eleven rail entries in `ui/rail.rs`; deterministic anchor commands |
| Expand Transform | `GAP_2_TOOLS_PAINT_LAYERS_APP.md:77` | existing bake helper through Object command |
| Shortcut parity data | requested catalogue lift | `varos-app/src/shortcuts/parity.rs`; runtime refuses unlisted document chords; removes non-Illustrator Command-Y Redo alias |

Bridge API 1.2 opt-in: `select.mode` (including `{same: ...}` and
`{key_object: path_id}`); `edit` verbs `object`, `distribute_mode`,
`distribute_spacing`, `insert_anchor`, `delete_anchor`, `anchor_type`.
`select.lasso` carries world polygon points and object/additive flags;
`select.paste_remembers_layers` sets the app preference without clearing selection. Global actions require an empty `ids` array.
Average accepts explicit path targets plus numeric `anchors` belonging to them.
Align uses `target: "key_object"` under 1.2. Repeated schema definitions are
interned to retain the existing tools/list byte cap. Existing Bridge and core
CLI batch/call adapters expose these same routes; no parallel command engine.

## Validation boundary

Ten missing rail/distribution glyphs were built from the locally pinned
`lucide@1.8.0` icon nodes, with provenance headers. The registry keeps the
existing icon order. No custom tool glyphs or flyouts were added.

The complete 0.8 gates passed before starting slice 0.7: 1,551 passed, zero failed,
15 ignored across 88 test targets; native and Windows clippy had zero warnings.
Independent review and owner hand testing remain pending; GUI launch and
installation are expressly outside this lane.

## Final combined gates

All commands ran offline with `-j 2` where applicable. Workspace: exit 0,
1,558 passed / 0 failed / 15 ignored across 86 unfiltered test summaries;
three additional child-process test runs passed (1,561 total pass reports).
49 new named tests: core 32, Bridge 8, app 9; existing kit/menu checks extended.
Native and x86_64-pc-windows-msvc all-target clippy: exit 0, zero warnings.
Formatting, dependency directions and diff whitespace checks: PASS.
Ratchet source byte-identical; ui.rs 826 / unchanged 843 cap.
14 tracked Bridge fixtures byte-identical; tools/list 23,993 / unchanged 24,000 bytes.
Logs and machine-readable counts: `varos/target/lane-b-gates/`.
No commit, push, merge, GUI launch or installation; independent review, native
runtime acceptance and owner hand testing remain pending.
