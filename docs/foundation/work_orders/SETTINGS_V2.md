> **Status:** proposed — Phase 9.1 contract and provisional controls; docs only, Figma choice pending.
# Settings v2 and Preferences

- Date: 2026-10-09; inspected baseline `7b48f2c`.
- Authority: [PLAN](../../PLAN.md) 9.1, 9.3, 8.4 and 1.10; [command registry proposal](../../adr/ADR-0015-command-registry.md).
- File: existing `AppLayout::settings()` → per-user `settings.json`; app-wide, outside `.vrs`, document undo and document dirty state. No native format bump.

## Baseline and scope

`varos/crates/varos-app/src/storage/settings.rs:15-22,45-84` writes version 1 with only `recovery_enabled` (default true), using the durable writer. Missing files default; corrupt files warn and preserve `.bad`; unknown versions warn and remain untouched on load. The current loader does not enforce a later-write lock for unknown versions; v2 must add one.
`varos/crates/varos-app/src/main.rs:321-350` uses fixed 1/10-point nudges. `varos-core/src/units.rs:21-28,119` supplies six document units. `varos-core/src/editor.rs:3421` caps undo at 200; `varos-render-wgpu/src/lib.rs:259-263` requests HighPerformance. These are implementation facts, not an existing Preferences system.
IDEA: `~/Documents/AI workspace/reference/artcraft/vectorcraft/crates/engine/src/cmd/prefscmds.rs:117-335` uses one preference table for labels/control kinds/bounds. Rebuild typed Varos descriptors; copy neither their broad preference inventory nor UI. No code borrowed.

## One typed specification table

Implement a `SettingSpec` table with stable key, typed accessor, default, parser/validator, unit, category, label/help keys, control kind, supported choices, apply timing and availability reason. File decoding, Preferences, reset and tests consume it. Typed storage remains authoritative; no unchecked `Value` mutation or stringly typed runtime setters.
The following is the complete proposed v2 field set, plus required integer `version: 2`. Bounds/defaults are proposed except owner-mandated autosave-on; autosave keys and range match [AUTOSAVE_TO_FILE](AUTOSAVE_TO_FILE.md).

| JSON key | Type / default | Validation | Provisional control | Application semantics |
|---|---|---|---|---|
| `keyboard_increment_pt` | number / `1.0` | finite, 0.001–1296 points inclusive | General: length field with unit suffix | Next nudge after Apply; Shift ×10; point geometry independent of zoom |
| `default_units` | string / `px` | `px`, `pt`, `pc`, `mm`, `cm`, `in` | General: Units dropdown | New blank documents only; explicit preset/template/import units win |
| `gpu_preference` | string / `high_performance` | `auto`, `low_power`, `high_performance` | Performance: Graphics processor dropdown | Next launch; preserve baseline default; effective adapter shown separately |
| `history_depth` | integer / `200` | 5–1000 | Performance: History steps number field | New editors immediately; existing editors at next settled history boundary |
| `recovery_enabled` | boolean / `true` | Boolean only | Saving: Recovery copies switch | Shared existing recovery setting/scheduler; independent of file autosave |
| `autosave_enabled` | boolean / `true` | Boolean only | Saving: Autosave to file switch | Applies when slice 1.10 writer gates are implemented; disable cancels pending attempts |
| `autosave_interval_seconds` | integer / `120` | 30–1800 inclusive | Saving: After inactivity duration field | Retained when off; change/re-enable restarts each dirty tab's idle deadline |
| `language` | string / `system` | `system` or a shipped catalog identifier; initially `en` only | Interface: Language dropdown | Next launch; System falls back to English if unavailable |
| `canvas_colour` | string / `match_ui` | `match_ui`, `white`, `#RRGGBB` | Interface: Match UI / White / Custom, opaque swatch | All canvas pasteboards after Apply; never artboard paint/export background |

Example full default file (settings version is unrelated to document/Bridge versions):

```json
{
  "version": 2,
  "keyboard_increment_pt": 1.0,
  "default_units": "px",
  "gpu_preference": "high_performance",
  "history_depth": 200,
  "recovery_enabled": true,
  "autosave_enabled": true,
  "autosave_interval_seconds": 120,
  "language": "system",
  "canvas_colour": "match_ui"
}
```

Keyboard field accepts the six core unit suffixes and converts through the existing unit parser to points. Unsuffixed input uses the displayed unit, shown beside the field; `px` uses a fixed preferences conversion of 72 ppi, stated in help, not whichever document happens to be active. Changing display units never rescales stored points. Typed Bridge `move` deltas remain unaffected.
History depth counts committed undo steps, not drag previews or Bridge suboperations; one atomic batch is one step. Trimming drops oldest undo entries, preserving nearest history; also bound redo by retaining the next replayable steps. Never trim an open transaction, change the saved checkpoint or reset revisions/ID high-water marks. Lowering the limit shows how many steps will be discarded before Apply; increasing cannot restore discarded history. Memory use must be measured with images before raising the maximum.
GPU choices are adapter-selection hints, not guarantees of a particular device or reduced heat. Auto delegates to backend default; Low power and High performance map to corresponding hints. If no requested compatible adapter exists, report actual fallback/initialization failure; no silent CPU-renderer mode or hot GPU switch. Preferences retains the requested value and shows “Restart required”.
Language lists only shipped, verified catalogs. Arabic remains unavailable until [ADR-0012](../../adr/ADR-0012-ui-text-engine.md) acceptance and its shaping/catalog/RTL gates; this schema does not approve that work. A previously valid but unavailable catalog is retained, with an English fallback notice, not overwritten on launch.
Canvas colour is view-only and resolves through existing tokens for `match_ui`; custom hex is normalized uppercase. It is not a theme editor and never changes UI tokens, transparency-grid settings or document colours.

## Provisional Preferences sheet

Entry point is the command-registry Preferences action: macOS application menu and Windows Edit/burger placement, available with no document. Present General, Saving, Performance and Interface sections with the table's controls. Keyboard Shortcuts opens 9.3 separately; it is not embedded in settings v2.
Use existing kit controls, box system, typography and spacing. This is a control/content contract; the owner chooses Figma options before visible code. No new visual design is asserted approved.
Open a draft copy with Apply, Cancel and Reset to Defaults. Reset edits the draft only. Invalid rows show inline reasons and prevent Apply; Escape/Cancel discards uncommitted changes. No live GPU, history or scheduler mutation while editing the draft. Focus, labels, keyboard traversal and error association must be testable.
Apply validates the entire draft, writes on the I/O worker through `durable::write_replace`, then publishes one settings generation on confirmed durable success. Multiple open views rebase or refuse a stale draft; serialize settings writes and detect unexpected on-disk replacement. Never let a stale recovery toggle overwrite newer preferences.
On pre-replacement failure, keep current effective settings and the draft, with retry. On `ReplacedUnconfirmed`, show that bytes may have changed but durability is uncertain; retain the draft and do not claim applied/saved or auto-retry. Reconcile disk and retry explicitly before publishing a new effective generation.
The current Document/Start recovery switch becomes another client of this same typed writer and settings generation, not a second store. Restart-only rows show requested versus effective values. Rows awaiting an implemented consumer are disabled with an honest reason; storing a default must not imply the feature works.

## Autosave and recovery contract

Use [AUTOSAVE_TO_FILE](AUTOSAVE_TO_FILE.md) for all timing, publication, disk-conflict, completion and recovery-retention rules; this spec only owns keys/validation/UI integration. A minimal 1.10 settings rollout uses these exact version-2 keys and defaults, not an incompatible second version 2. Full v2 fills absent keys later.
Recovery is the existing separate snapshot cadence (30 seconds); file autosave is default-on after 120 seconds of committed-content inactivity. All four on/off combinations are valid. Disabling recovery never implicitly disables autosave or deletes existing recovery copies; disabling autosave never disables recovery.
No autosave of Untitled/pathless/read-only documents; no dialog from a timer; no writes during gesture, field edit, transaction or Bridge batch, conflicting file/recovery jobs or unresolved external disk changes. File autosave must not erase recovery generations merely because the dirty dot clears.
Helper text distinguishes “Saves changes to the open file” from “Keeps separate recovery copies”. Interval control is disabled when autosave is off but still validated and retained. Revert reads the latest saved disk version, including autosave; it is not a return to the last manual save.

## Validation and migration

1. Bound reads (proposed 64 KiB), require a JSON object and integer version; reject duplicate keys, nonfinite numbers, wrong types and numeric strings. UI edits reject invalid values without silent clamp/coercion; validate inactive controls too.
2. Missing file: defaults, no warning; create only on explicit settings save. Version 1: parse required Boolean `recovery_enabled`, preserve true/false exactly and seed all new defaults. Migrate in memory on load; durable v2 replacement occurs on the next explicit settings change, not an unsolicited startup write.
3. Version 2: missing recognized fields get defaults (supports the 1.10 minimal rollout). For invalid recognized fields, retain source bytes, default only those fields in memory and report their names/reasons. Do not rewrite automatically; explicit repair/Apply must first preserve the original bytes in a non-overwriting backup.
4. Unknown v2 fields are retained opaquely and re-emitted unchanged within the file-size bound, never exposed as editable settings. Unknown enums are invalid except unavailable language catalogs as described above. This avoids the minimal rollout destroying later v2 fields.
5. Malformed JSON/invalid envelope or version-1 recovery type: defaults plus warning, preserve `settings.json.bad` without overwriting previous evidence. If preservation fails or `.bad` already exists, keep source untouched and require a unique preserved copy before replacement; never discard damaged evidence silently.
6. Unsupported older or newer version: defaults for this session, direction-correct warning, file untouched and **writes locked**. Explicit reset can create v2 only after preserving the old file. A load warning alone is not permission to overwrite it later.
7. Migration must be idempotent, preserve recovery choice and unknown fields, and use existing app-data resolution and atomic writer. No migration of `.vrs`, layout, recents, shortcut overrides or Bridge credentials. Old binaries are not promised safe downgrade behavior; retain a backup before intentionally downgrading settings.

## Required verification and handoff gates

- Table coverage: each key has typed parser/default/control/apply policy; full default example decodes; min/max, just-outside, wrong-type, overflow, NaN-like input, colour and catalog cases.
- Migration fixtures: missing, v1 true/false, minimal v2, full v2, unknown keys/version, duplicate keys, corrupt bytes, existing `.bad`, failed preservation and repeated migration; unknown-version writes remain blocked.
- Fault injection: temp/write/sync/rename/directory sync, concurrent draft/toggle, stale settings generation and external replacement; correct failure versus unconfirmed state.
- Behavior: nudge 1×/10× across zoom/units; defaults do not alter open/imported docs; history trims safely; GPU restart/effective hint; locale fallback; pasteboard colour leaves exports and `.vrs` bytes unchanged.
- Fake-clock and I/O acceptance from autosave spec; all four recovery/autosave combinations; idle loop remains Wait/WaitUntil without repaint churn.
- After implementation: repository gates, independent review and owner Preferences hand test after Figma choice. This document ships no settings reader, controls, autosave or language support.
