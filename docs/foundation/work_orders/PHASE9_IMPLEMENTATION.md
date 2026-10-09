> Status: implemented (provisional UI, owner design review pending). Lane F, 2026-10-09.
# Phase 9 application handoff

Preferences uses the existing kit and a typed settings specification. ⌘K opens a draft;
Apply publishes only after durable worker completion. Cancel leaves effective settings unchanged.
The recovery, autosave and Paste Remembers Layers clients use that same generation-checked writer.
Stale generations, concurrent writes and external source replacement refuse publication. A failed or
unconfirmed write retains the draft. Reconcile disk before retry explicitly refreshes source evidence;
it does not publish or retry a draft. Unsupported settings versions require Reset followed by Apply,
and original bytes are preserved before replacement. Invalid recognized v2 fields can be repaired by
explicit Apply with a unique durable evidence backup. Reads are limited to 64 KiB, including growth.

Settings v1 migration is pure and happens in memory. Existing additive v2 keys survive writes.
New settings keys: keyboard_increment_pt, default_units, gpu_preference, history_depth, language,
canvas_colour. Existing recovery_enabled, autosave_enabled, autosave_interval_seconds and
paste_remembers_layers are retained. Settings version 2 is independent of the native model version.
Nudges use points and Shift ×10 for object and anchor selections. Unsuffixed Preferences lengths are
points; px uses 72 ppi. Units apply to new blank documents, not presets or open/imported documents.
GPU hints apply at launch; Preferences shows the requested hint, launch hint and actual adapter.
System and English are shipped choices; unavailable catalog identifiers survive with English fallback.
Canvas colour changes only the pasteboard. History limits apply at settled boundaries; the draft shows
how many entries will be discarded. All four autosave/recovery combinations remain independent.

ADR-0015 remains proposed for moderator acceptance. The migration catalog projects the incumbent
menu and Illustrator parity tables, retains historical aliases, and collapses duplicate placements.
Native availability and accelerators, burger File/application rows and shortcut hints use that catalog.
The existing typed handlers remain authoritative; no arbitrary run-command endpoint exists.
API 1.2 command_index is paginated and reports enabled/reason. list_verbs reports needs_arguments
where targets are unresolved and unsupported_host for absent application services. Full schemas
remain discoverable through schema; legacy 1.0/1.1 tools/list fixtures are unchanged.
⌥⇧⌘K provides search, explicit unbind, conflict detection and reset to parity. Overrides are additive
and stored separately in shortcuts.json, with durable publication and stale-draft/source protection.
The held Space gesture is fixed and is shown as such rather than offering a nonfunctional rebind.

History is a real dockable PanelId. Committed steps have labels, timestamps and counts; Bridge batches
carry client profile/label and semantic verbs. Jump uses retained linear undo/redo positions. The first
row means the earliest retained state, not necessarily the original document after trimming.
Undo this agent's last edit refuses unless that agent owns the top step; no out-of-order undo is added.
History stays session-only and does not change .vrs equality, format keys or saved checkpoints.

Actions is a real dockable panel, with a Home sheet for loading/saving without a document.
Version 1 .vrs-actions records successful committed move, paint, opacity, stroke-width, rotation,
delete, group and ungroup commands. No-ops are omitted; unsupported committed document commands
stop recording with a reason rather than exporting an incomplete sequence. File/UI/history commands
are not action steps. Cancel discards a recording. Replay additionally supports rectangles with local
symbolic IDs, binds source targets to the current explicit selection, and publishes one atomic step.
Offline CLI: varos-cli apply input.vrs --batch sequence.vrs-actions --ids 7,12 --out output.vrs.
API 1.2 actions supports start/cancel/stop/replay/undo_mine; history_list/history_jump are separate.

Help offers docs, the shortcut list and the crash-log folder, also through the typed API 1.2 help tool.
The Quick Look fallback embeds a current CPU PNG from the thumbs cache into the PDF container.
Named pure migration: varos-pdf::quicklook::embed_preview_next. Optional catalog keys:
VAROS_Preview and VAROS_PreviewVersion (preview revision 1). The next FORMAT_VERSION is provisional 6,
with matching model-envelope/catalog stamps and pure migrate_v5_to_next_preview. No authored JSON keys
change. Frozen container/PNG/refusal fixtures cover preservation and preview/format refusals. Corrupt
or oversized thumbnail cache entries regenerate in memory; cache persistence is best-effort. Native signed extension packaging instructions are in
varos/crates/varos-app/quicklook/README.md; Finder integration is not installed or claimed tested.

All UI is provisional. Independent integration review, moderator merge and owner native tests remain
required. This lane does not commit, push, launch the GUI, install or package the macOS extension.

## Fix round — 2026-10-09

Preview keys now require the next native format (provisional 6; integrator renumbers in merge order),
with a named identity migration and frozen container/refusal inputs. Cache reads and decoding are bounded;
cache persistence is optional. The history ceiling remains 200 until SETTINGS_V2 image-memory evidence.
Reset leaves held Space and unchanged defaults with incumbent input handling. Rebound document commands
retain repeats; application commands activate once. Recording coverage lives at document commit: unsupported
pointer/creation commits refuse the entire recording visibly. Supported steps keep one selection binding;
Bridge batches validate that binding across requests before recording their committed semantic metadata.
