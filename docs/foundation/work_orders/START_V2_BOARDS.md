> **Status:** current — work order, owner-approved direction 2026-10-04 (charter §3). Design of record: `design-reference/mockups/start-v2/` (PNG mockups + NOTES).
# Start page v2 — Boards

## Owner decisions (2026-10-04)
- The engineer-built Start page was rejected ("صفر وبشعة"). **Design first**: anything visible is built from an approved mockup and must match it.
- Chosen design: direction A "Workbench", v2, in `design-reference/mockups/start-v2/` — `inter-recent.png`, `inter-empty.png`, `inter-hover.png`, `inter-list.png`. `NOTES.md` there gives the grid, type scale and tokens. Build to these images.
- **A document is a Board**: a free canvas with a **name**, a short **description** and **tags**; artboards inside are optional. "New board" (⌘N) opens a free canvas with zero artboards. Presets (Square 1080², Portrait 1080×1350, Story 1080×1920, A4 595×842 pt, Custom…) are the secondary "…or start with an artboard".
- **Typeface** (owner delegated the choice to the moderator, "اختار انت ووريني"): **Inter** Regular 400 / Medium 500 / SemiBold 600 for UI, **JetBrains Mono** Regular for numbers, sizes, paths and dates (tabular). Both OFL, bundled as static cuts. Arabic stays IBM Plex Sans Arabic behind the existing gate. IBM Plex Sans/Mono leave the UI font chain.
- The Varos **V mark** at the right end of the top bar stays as it is.

## Lanes (parallel, disjoint files; author ≠ reviewer)

**Lane status (2026-10-04): L1 Fonts DONE; local gates PASS.** Independent review and owner hand check remain; L2–L5 stay with their owners.
| Lane | What | Owns |
|---|---|---|
| L1 Fonts | Inter 400/500/600 + JetBrains Mono static cuts, manifest, licences, weight-aware text styles in `shell/tokens.rs` (`TextStyle`-like tokens: h1 30/600, h2 18/600, button 15/500, name 14/600, body 13/400, small 12/400, tag 11/500, mono 11/400), fallback chain; Plex removed from UI chain | `assets/fonts/`, `shell/fonts.rs`, `shell/tokens.rs` (type section), `tests/fonts.rs` |
| L2 Board metadata | `Document` gains `name: String`, `description: String`, `tags: Vec<String>` (bounded: name ≤ 120 chars, description ≤ 500, ≤ 16 tags of ≤ 32 chars, no control chars, tags trimmed/deduped case-insensitively); format bump + migration per ADR-0008 (older files get name = file stem at open time, empty description/tags); `EditCommand`s to edit them (undoable, dirty); recents cache carries name, description, tags, artboard count, thumbnail key so Home never parses files; presets + "New board" (zero artboards) commands | `varos-core` model/format/command, `recent_files.rs`, `start.rs` model, `app_command.rs`, `lifecycle.rs` (new-with-preset) |
| L3 Thumbnails | CPU thumbnail of a board (content bounds: artboards on the dotted canvas colour, or free artwork) rendered on a worker after a successful save, cached under the app-data dir keyed by FileKey + mtime, bounded size (e.g. 544×246 @2×), never blocks the UI, missing thumbnail → typographic placeholder | new `varos-app/src/thumbs.rs` (+ a pure rasteriser module), `storage/`, hook in the save-landed path |
| L4 Start UI | `start_ui.rs` rebuilt to the mockups: hero (New board / Open… / lede / key hints), preset panel, Recovered band, "Recent boards" head with tag filter + grid/list toggle, board cards (thumbnail well, name, date, description, tags, count, path), list table, hover / focus / "…" menu / Missing, empty state; Search field filters boards; kit components only | `start_ui.rs`, `shell/kit/` (new card/pill/filter/table controls), `tests/start_ui.rs`, `examples/` gallery |
| L5 (after L2) Board section | where the user edits name / description / tags inside the editor: the Document section of Properties when nothing is selected (kit fields, field law P2) | `ui.rs` Document section |

Interface between L2 and L4 (fixed now so both can work): the Start view consumes
`BoardCard { key, name, description: Option<String>, tags: Vec<String>, artboards: u32, path: PathBuf, modified, missing: bool, thumb: Option<ThumbKey> }` and emits the existing `StartAction`s plus `NewBoard`, `NewWithPreset(PresetId)`, `SetTagFilter(Option<String>)`, `SetView(Grid|List)`, `Search(String)`.

## Acceptance
Owner hand check: launch → the Start page matches `inter-empty.png`; with recents it matches `inter-recent.png`; New board opens a free canvas; a preset opens with that artboard; name/description/tags typed in the editor show on the card after save; list view; tag filter; Missing; Recovered. Tests per lane; gates on macOS; cross-review before merge.
