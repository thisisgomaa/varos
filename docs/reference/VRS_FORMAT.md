> **Status:** current — the `.vrs` wire contract, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3. Owned by DFS S5-E (`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md`); version, limits and migration behavior are recorded below; work-branch acceptance is tracked separately.

# `.vrs` file format — wire contract

`.vrs` is Varos's document format: a versioned JSON model of `varos-core::model::Document`, carried
either as a raw JSON file or embedded inside a valid PDF container (the `.ai` pattern). This document records the accepted target contract and the implementation status below. A pending check is not a current runtime guarantee. The contract covers the envelope, container, versioning, limits and
refusal copy. The decision record behind it is `docs/adr/ADR-0008-vrs-format-versioning.md`; read that
for *why*, this for *what byte, what key, what number*.

## Lane A format 10 (wave-3 worktree, integration pending)

The writer is 10 (`APPEARANCE_VERSION = 10`); earlier era constants remain unchanged.
`migrate_v9_to_v10` is a named pure identity migration: absent keys keep earlier semantics.
`Path.stack` is optional (empty is omitted). Nonempty stacks contain exactly one base fill and
one base stroke, plus owned fill/stroke entries with paint, Normal blend, visibility and opacity.
Extra strokes own width in points and StrokeStyle. `Node.look` is optional on containers and
stores opacity/isolate. `GroupRole::MaskAlpha` uses the existing group `mask_child` form;
several masks are nested groups. Stack, look and MaskAlpha are refused in every stamp below 10.
Frozen JSON/PDF/SVG and refusal artifacts: `varos-core/tests/fixtures/v10/`, with SHA256SUMS.
Plain v9 model bytes remain identical after replacing only the format stamp. Original v5–v9
fixtures are retained; new v11 copies serve future-format refusals for this writer.
Bridge 1.2 discovers appearance/mask through list_verbs/schema; legacy fixtures stay frozen.
The integrator rechains this step with the later optional wave-3 keys.

## Wave-2 formats 6–9 (stamped 2026-10-09, `integ/w2`) — base writer **9**

Final numbers, binding merge order: **6** images · **7** gradients + swatches · **8** editable text ·
**9** Live Corners + the optional Quick Look preview (one bump). At wave-2 integration, `FORMAT_VERSION = CORNERS_VERSION = 9`;
`IMAGE_VERSION = 6`, `GRADIENT_VERSION = 7`, `TEXT_FORMAT_VERSION = 8`, `PREVIEW_FORMAT_VERSION = 9`
(pinned literally by `varos-core/tests/format_pin.rs`). JSON `varos` and PDF `/VAROS_SchemaVersion`
always agree. `MIGRATIONS` is the contiguous named pure chain `migrate_v5_to_v6` (images, identity),
`migrate_v6_to_v7` (gradients, identity, no validation), `migrate_v7_to_v8` (text,
`text_format::migrate_to_text_boxes`), `migrate_v8_to_v9` (corners + preview, identity); Bridge 1.2
`readable_vrs` is derived from that table (1–9 at wave-2 integration, 1–10 in Lane A). Each era's keys are refused under an older stamp
before typed decode: images (<6), gradient paints/swatches (<7), text (<8), corners (<9) and the
PDF-catalog preview keys (<9). Every new key is optional with a default, so plain documents keep
their bytes apart from the stamp. `Document` key order (frozen by `fixtures/v9/mixed.json`):
`name, description, tags, images, assets, raster_effects_ppi, swatches, text_boxes, paths, …`.
Each wave-2 lane's historical refused-future fixture is format **10** and remains frozen; active future gates now use separate v11 copies. Old-reader gates v4–v8 are frozen in
`varos-pdf/tests/{old_reader_harness,format_v9}.rs` (raw JSON and PDF container).

### Format 8 — editable text

New optional `doc.text_boxes` stores TextBox records; each has `id`, `box_kind` (`Point` or
`{"Area":[x,y,width,height]}`), baseline `frame:[x,y]`, `runs:[{text,style}]`, and `para`.
`style` contains `font:{family,weight,hash}` (exact lowercase SHA-256 bytes identity), `size`,
`letter_spacing`, and RGBA `fill`. `para` contains `align` (Left/Centre/Right/Justify),
`direction` (Auto/Ltr/Rtl), `kashida` (Off/Minimal/Balanced/Display), and `line_height`.
A leaf's new `NodeKind` is `{"Text":text_id}`. Every text record has exactly one childless leaf.
No font bytes, machine paths, layout caches, or generated glyph paths are persisted.

Load/save/edit enforce 4,096 boxes, 4,096 runs per box, 1 MiB source per box and 8 MiB total;
finite frames, positive area dimensions, font size 0.1–4,096 pt, leading 1.3–20 em, and zero Arabic
tracking. Runtime shaping additionally refuses frames above 64 KiB or 16,384 glyphs, and bounds
outline vertices. Unknown/missing font snapshots fail explicitly when rendering/exporting.
Omitted text storage remains omitted for plain-path files. Declaring a pre-text version while
carrying either text key is refused. Frozen `fixtures/text_next/` includes mixed source and
legacy-key, Arabic-tracking, future-JSON/PDF refusal cases, with SHA256SUMS (restamped 2026-10-09:
`mixed.json` and `refuse_arabic_tracking.json` → 8; `refuse_newer.json`/`.pdf` → 10; `refuse_text_in_v5.json`
stays 5; new `refuse_text_in_v7.json`).
The PDF tests exercise the frozen v5 reader gate against current text output before typed decode.
Native files retain editable source; PDF/SVG deliverables report **text exported as outlines**.

## Implementation status — Lane F preview (2026-10-09, merged)

The Lane F branch stamped a provisional 6; integration folded its container-only preview keys into
format **9** (see "Wave-2 formats 6–9" above and the preview section at the end of this contract).

## Implementation status — Lane H format 5 (2026-10-09)

The worktree writer stamps JSON `varos:5` and PDF `/VAROS_SchemaVersion 5`. First merged writer takes v5;
this branch is not merged or installed. Phase 2 adds only optional `Path.stroke_style`. Default paths omit it.

## Implementation status — 2026-10-07 (format 4)

The writer emits **v4** (stable artboard ids: `doc.artboards[].id` — Bridge slice 3, ADR-0008
amendment 2026-10-07). It reads v3 through the v3→v4 migration (§6c), v2 through v2→v3→v4 and v1
through all three steps. Everything below about v2/v3 strictness still holds; v3 is now an older format
and opens with the migration notice. Work-branch status (`feat/bridge-slice3-artboards`): implemented
with headless tests; independent review and owner hand test pending.

## Implementation status — 2026-10-04 (format 3)

The writer emits **v3** (board metadata: `doc.name`, `doc.description`, `doc.tags` — Start v2 lane
L2, ADR-0008 amendment 2026-10-04). It reads v2 through the v2→v3 migration (§6b) and v1 through
v1→v2→v3. Everything below about v2 strictness still holds; a v2 file is now an older format and
opens with the migration notice. Work-branch status: implemented with headless tests; independent
review and owner hand test pending.

## Implementation status — 2026-09-27

The writer emits **v2**; it reads v1 through migration. ADR-0008 is accepted. S5-B and the v1 fixture work are merged; S5-C/D and the automated S5-E fixture/harness slice are implemented on the work branch with independent review pending; real-application acceptance remains open. The owner reports no personal documents; the scoped scan found only fixtures.

| Boundary | Enforced today | Pending |
|---|---|---|
| Core JSON (`format::decode_model`) | Model byte/depth caps, version-first gate, strict fields, structural/count checks, migration/canonical checks. Work branch adds semantic kinds/masks/ranges, all persisted floats and authored checks before normalization; save retains the decode-backstop | Batched independent review; C is not merged. |
| Container version | Work branch extracts `/VAROS_SchemaVersion`, validates its type/range and supplies it to the core version gate before model decoding | Batched independent review. |
| App file loading | Work branch routes both compatibility wrappers through checked bounded file/byte APIs; preserves migration notices | Real-window acceptance; no personal documents reported/found in the scoped scan. |
| PDF parsing | Work branch enforces the strict native profile below: preflight before lopdf, no stream inflation, bounded name-tree traversal, exact fallback filename | Independent review; third-party Preview re-save acceptance unverified. |
| Legacy broken mask | Work branch: recoverable v1 Group clip references released in memory, with notice retained through PDF/disk/lifecycle Open; current v2/save stay strict | Real-window verification and batched independent review; no on-disk repair during Open. |

Source: `varos-core/src/format/{mod,validate,migrate,limits}.rs`, `varos-pdf/src/lib.rs`, `varos-app/src/file_ports.rs`. Work ownership: [S5](../foundation/work_orders/DFS_S5_FORMAT_V2.md). Sections below distinguish target rules from current enforcement.

## 1. Container

A `.vrs` file is one of two things, sniffed by its first bytes:

- **Raw JSON** — the file *is* the envelope (§2), UTF-8 text, no PDF wrapper. This is the original,
  simplest form and stays readable forever.
- **PDF container** (`varos-pdf`) — a valid PDF that any viewer renders (one page per visible
  artboard), with the exact same JSON envelope embedded as an associated file. Readers prefer the
  catalog's private `/VAROS_Model` key; if that is missing they fall back to walking
  `/Names/EmbeddedFiles` for the name-tree entry `model.varos.json`. Detection: the file starts with
  `%PDF-` → PDF container; otherwise → raw JSON.

Both forms carry the identical envelope bytes for a given save — the container choice never changes
what a file means (ADR-0003, ADR-0008 §1).

## 2. The JSON envelope

```json
{"varos": 3, "doc": { "name": "…", "description": "…", "tags": ["…"], /* the rest of the Document, see varos-core/src/model.rs */ }}
```

| key | meaning |
|---|---|
| `varos` | the **format version** — an unsigned integer. This is the field ADR-0008 governs. Renaming it was considered and rejected (ADR-0008, Alternatives): a pre-S5 reader without this exact key fails with a raw "missing field" error instead of its readable "newer Varos" message. |
| `doc` | the persisted `Document` — every field in `varos-core/src/model.rs`'s `Document` struct, via `#[derive(Serialize, Deserialize)]` (plus a hand-written shape for `Paint`, kept identical to the old bare `Option<Rgba>` array/`null` form — see `serde_roundtrip.rs::paint_reads_and_writes_the_legacy_option_rgba_shape`). |

Rust-side, the wrapper is `varos_core::file::VrsFile { varos: u32, doc: Document }`
(`varos-core/src/file.rs`). The constant naming it is `VRS_VERSION`; ADR-0008 §"v2 is the same model
with a stricter reader" renames the *constant* to `FORMAT_VERSION` when S5-B lands (`MIN_READ_VERSION`
alongside it) — the wire key `varos` itself never changes name.

## 3. PDF keys

Every Varos-written PDF container carries, in addition to the ordinary page tree:

| key | where | meaning |
|---|---|---|
| `/VAROS_Preview` + `/VAROS_PreviewVersion` | catalog, optional in next format | PNG stream reference + revision 1; bounded and refused under older stamps (next-preview section). |
| `/VAROS_Model` | catalog | indirect reference to the embedded-file stream holding the JSON envelope bytes, unfiltered (§2). The fast read path. |
| `/VAROS_SchemaVersion` | catalog | the same integer as the envelope's `varos` key, repeated so a reader can check the container's own claim without first decoding the model stream. |
| `EmbeddedFiles` name tree, key `model.varos.json` | `/Names/EmbeddedFiles` | the same stream, reachable by name — the fallback path, and the one most likely to survive a third-party re-save that drops private catalog keys. |
| `/AF` (AFRelationship = `Source`) | catalog | marks the embedded file as the document's *source*, per the PDF/A-3 associated-file convention (the same pattern Illustrator uses for `.ai`-in-`.pdf`). |

The writer embeds the model **unfiltered** (no `/Filter`) with a classic (non-compressed) cross-
reference table — see §7 for why bounded PDF reading depends on this.

## 4. Version rules (ADR-0008 §"The bump rule", §"Refusal policy")

**The bump rule.** Any change to what the writer can emit — a new, removed or renamed key, a new enum
variant, a changed type, unit or default — raises `varos`/`FORMAT_VERSION`. Reader-only relaxations
(accepting input no writer emits yet) do not. Every bump ships, before its writer ships: a named pure
migration, old and new fixtures, a rejection fixture, and an update to this file.

**Refusal policy — all of this before any typed `Document` decode:**

- **Newer than this build supports** → refused. No view-only fallback, no editable guess.
- **Missing `varos` key** → refused.
- **Zero, negative, fractional, or too large to fit a `u32`** → refused as invalid.
- **Catalog `/VAROS_SchemaVersion` present and different from the envelope's `varos`** → refused as a
  mismatch. A *missing* catalog key (raw JSON files carry no catalog at all) is not a mismatch.
- A refusal never installs a partial document and never modifies the file on disk.

S5-B now supplies the version-first gate through `format::decode_model`, including missing/invalid/newer versions. Container mismatch checks work only when the caller supplies the catalog version; the current app PDF reader does not yet do so (S5-D).

## 5. Version table

| version | status | written by | notes |
|---|---|---|---|
| 1 | **legacy, readable through migration** | every `.vrs`-capable build since `7a5b3c8` (2026-07-02) | a *family* of eras, all stamped `1` — raw JSON, pre-artboards, legacy group registry, pre-`Paint` enum, pre-tree, and (the hole) masks/rotation added under this same number. See ADR-0008 §Context. |
| 2 | **legacy, readable through migration** (since 2026-10-04) | S5-B builds up to `f21c20e` | same `Document` shape as 1; the reader contract tightens (§6, §9). No schema change — see ADR-0008 §"v2 is the same model with a stricter reader". |
| 3 | **legacy, readable through migration** (since 2026-10-07) | builds up to `a5f687b` | v2 plus three `doc` keys: `name` (string), `description` (string), `tags` (array of strings) — board metadata, bounded (§6b). A v1/v2 file carrying any of them is refused. |
| 4 | **legacy, readable through identity migration** | builds through base `7b48f2c` | v3 plus one key on every artboard: `id` (u32 > 0, unique among artboards, from the document id counter) — stable artboard identity (§6c). `active` must name an artboard (or be 0 on a free canvas). A v1/v2/v3 file carrying an artboard `id` is refused. |

| 5 | **legacy, readable through identity migration** | `feat/p2-stroke` builds through main `c852a55` | optional `doc.paths[].stroke_style`; exact keys and validation in §6d; default authored doc bytes unchanged. |
| 6 | **legacy, readable through identity migration** (stamped 2026-10-09) | `integ/w2` intermediate | images: optional `doc.images`, `doc.assets`, `doc.raster_effects_ppi`, `NodeKind::Image`; binary resources in the PDF container ([ADR-0008 images amendment](../adr/ADR-0008-amendment-next-images.md)). |
| 7 | **legacy, readable through identity migration** (stamped 2026-10-09) | `integ/w2` intermediate | gradients: tagged `fill`/`stroke` gradient and `swatch_ref` paints, optional `doc.swatches` ([gradients amendment](../adr/ADR-0008-amendment-next-gradients.md)). |
| 8 | **legacy, readable through migration** (STAMPED 2026-10-09; shipped by stage 1) | `integ/w2` stage 1 | editable text: optional `doc.text_boxes`, `NodeKind::Text` (section "Format 8"). |
| 9 | **current writer** (stamped 2026-10-09) | `integ/w2` | Live Corners: optional `doc.paths[].corners` ([live corners amendment](../adr/ADR-0008-live-corners-next-writer.md)); optional PDF-catalog `/VAROS_Preview` + `/VAROS_PreviewVersion` (Lane F, container-only). |

## 6. Migration v1 → v2

Migration is pure, deterministic, and runs **once**, only on a v1 input (a v2 file skips it entirely).
It performs *only* the normalizations `Document::sync_tree` already documents:

| step | what it does |
|---|---|
| registry → tree | the legacy `groups`/`group_of` flat registry becomes the `nodes`/`roots` tree, order preserved |
| adopt tree-less paths | a path with no owning node is wrapped into a leaf under the active layer (or `Layer 1`), z-order kept |
| prune empty groups | a `Group` node left with no children after the above is dropped |
| heal nested live transforms | a non-identity `Node.xform` nested under another non-identity `xform` is flattened to one |
| raise the id counter | `ids = ids.max(max_used)`, `max_used` = the highest path/node/anchor/legacy-group id seen, via a checked `+1` (`IdExhausted` if it would overflow) |

**Owner-approved v1 exception (implemented on the S5-C work branch):** before structural
checking or allocating IDs, a Group with role Clip whose mask is missing or is not a direct child
becomes Normal with no mask_child. This pass changes no paths or geometry. All other structural
references, counts, kinds, reserved roles and numerical values remain subject to strict checks.
A stale mask ID can never bind to a node newly allocated by normalization. Current v2 input and
Save do not use this exception; their invalid clips are refused.

`Loaded::released_legacy_masks` retains this outcome. `Loaded::notice()` describes the release,
that remaining artwork was kept and that the original was not changed. The app uses the additive
notice-retaining store method and shows the notice only after successful Open; duplicate opens do
not re-notify. Ordinary v1 migration keeps the existing “Opened an older file…” notice. Legacy
Document-only library wrappers remain compatible and cannot carry notices; UI callers use the
notice-retaining API. No bytes on disk change until an explicit Save.

Semantic checks run before normalization to prevent invalid authored values disappearing when a
nested transform is healed or a group pruned, and again on the canonical result. Every persisted
float is checked, including hole handles and node colors; opacity/colors are in [0,1], stroke width
and artboard width/height/bleed are nonnegative, ppi is positive. Path leaves have no children or
duplicate owners; Layers cannot nest in Groups; Clip only belongs to Group and requires a direct
mask child; Normal has no mask_child and reserved soft masks are refused. Root-level paths/groups
remain legal because the live model supports them. candidate_max has no invented upper cap (no
current editor invariant or active consumer). Canonical-only invariants retain their existing gate.

## 6b. Migration v2 → v3 (2026-10-04, board metadata)

`format::migrate_v2_to_v3` — pure, deterministic, runs once on a v2 input after the v2 canonical
check (so a v2 file keeps exactly the strictness it had when v2 was current). A v2 file has no board
keys, so the typed decode already defaulted them; the step changes nothing else:

| key | migrated value | meaning |
|---|---|---|
| `name` | `""` | "use the file stem" — `varos_core::board::display_name`: board name, else file stem, else `Untitled-N` |
| `description` | `""` | no description |
| `tags` | `[]` | no tags |

A v1 file runs v1→v2 (§6) then v2→v3. The migrated file opens with the ordinary migration notice;
no bytes change until the user saves (which writes v3).

**Bounds (validated on load, on save and on edit — `varos_core::board`):** name ≤ 120 characters,
description ≤ 500 characters, ≤ 16 tags of ≤ 32 characters each (characters = Unicode scalar
values; Arabic and Latin share the budget); no control characters (line breaks, tabs, NUL…) in any of
them. Stored tags are clean: edge-trimmed (whitespace and invisible direction marks), non-empty, and
unique case-insensitively, order kept — the editor normalizes typed tags (first spelling kept) before
storing, so a stored list that breaks this was not written by Varos and is refused. Text is stored as
UTF-8; NFC normalization is not required.

**Text rules (product behaviour, 2026-10-04 review).** Bounds count Unicode scalar values, not
grapheme clusters (a family emoji `👨‍👩‍👧` is 5 scalars toward a tag's 32). Control characters
(general category Cc) are refused. Format characters are allowed INSIDE text — ZWNJ/ZWJ
(U+200C/U+200D, which Arabic and Persian need for correct joining) and the bidi marks, embeddings and
isolates — and are trimmed only at the edges, together with whitespace. Tags compare through one fold,
`varos_core::board::fold` (NFC, then upper-then-lower full case fold, so `Straße` = `STRASSE` and
`σς` = `ΣΣ`; Arabic is unchanged): dedupe on edit, the duplicate check on load, and Start's tag filter,
tag counts and search all use it. Edits go through the checked `Editor::try_set_board_*`, which returns
the plain-English reason and changes nothing when input breaks a rule.

**Older formats must not carry the board keys.** The typed decoder would default the three keys, so a
keys-only scan (`format::refuse_newer_keys`, v1/v2 input only) runs right after the version gate and
BEFORE any typed decode: it reads the top-level keys of `doc`, skipping every value, and refuses a file
that claims format 1 or 2 but has `name`, `description` or `tags` — whatever the value (`42`, `null`, a
nested object) — with `Invalid::FieldNotInFormat`, the same fail-closed rule as any unknown field. A v3 file may omit them (reader relaxation: they
default to empty); this build's writer always emits all three.

## 6c. Migration v3 → v4 (2026-10-07, artboard ids)

`format::migrate_v3_to_v4` — pure, deterministic, runs once on a v3 input after the v3 canonical
check. A v3 file has no artboard ids (refused below), so every artboard decoded with id 0. The step:

| field | migrated value |
|---|---|
| `artboards[i].id` | `ids + 1`, `ids + 2`, … in artboard order, where `ids` is the document id counter after the v3 canonical pass (raised to cover every id in use) |
| `ids` | raised by the number of artboards |
| `active` | clamped into range (`min(active, len − 1)`, or 0 on a free canvas); a v3 reader already clamped on read, so the active page is the same |

Nothing else changes. v1/v2 files run their earlier steps first. The migrated file opens with the
ordinary migration notice; no bytes change until the user saves (which writes v4). The frozen
`v3/v3_board_meta` (counter 15, two pages) becomes ids 16 and 17, counter 17.

**Format-4 checks.** Structure: artboard ids are unique among artboards (`Invalid::DuplicateId { kind:
"artboard" }`); cross-kind reuse stays legal (a path, a node and an artboard may share a number), and
the id-headroom check counts one id per artboard still to be assigned. Validation: every artboard has
an id (`Invalid::MissingArtboardId { index }`; the typed decode defaults a missing `id` to 0) and
`active < max(len, 1)` (`Invalid::ActiveArtboardOutOfRange`). A v2/v3 file is validated without these
two checks, before its migration. Save runs the same checks after the save-side normalizer, which gives
an artboard built in memory without an id (`0`) a fresh one and clamps a stale `active` on the clone —
the forms `Editor::commit` already keeps. A duplicate non-zero id is refused on save, never renumbered.

**Older formats must not carry the artboard id.** The keys-only scan (`format::refuse_newer_keys`) now
runs for v1–v3 input and also reads the keys of each artboard object (values skipped): an artboard
`id` in a file that claims format 1, 2 or 3 — any value, `0`, `null` or a nested object — is refused
with `Invalid::FieldNotInFormat { field: "artboard id" }` before typed decoding. A board key in a v1/v2
file is still reported first.

**Live identity.** The editor allocates artboard ids from the same counter and lifetime high-water
mark as paths and nodes: every commit gives a new or duplicated page a fresh id (`Document::
assign_artboard_ids` — a copy inserted after its source gets the new id), undo restores the same page
with the same id, and a page created after an undo never reuses a removed page's id. Indices stay
internal (export planning, Layers artboard sections, `path_boards`/`node_boards`, the `active`
preference, `ExportScope::ActiveArtboard`); the Bridge and `varos-cli export-pdf --artboard` speak ids.

## 6d. Migration v4 → v5 and exact stored style

`format::migrate_v4_to_v5(Document, &Limits)` is a pure identity step, registered after v3→v4.
Formats 1–4 reject presence of `stroke_style`, including `{}` and null, before typed decoding:
`Invalid::FieldNotInFormat { field: "stroke_style", version }`. Format 6 is refused before typed decode.
Existing v4 files retain disk bytes on open; Save updates only the wrapper/catalog and container offsets
for an unstyled document. Every default nested field is omitted; all three new structs deny unknown fields.

| Key | Exact wire values / type | Default | Limits |
|---|---|---|---|
| cap | Butt, Round, Square | Round | known enum only |
| join | Miter, Round, Bevel | Round | known enum only |
| miter_limit | f32 | 10 | finite 1–1000 |
| dash | array of f32 | [] | 0/2/4/6 entries; finite 0–1,000,000; positives ≥0.0001; each pair positive sum |
| dash_phase | f32 | 0 | finite absolute value ≤1,000,000 |
| align_dashes_to_corners | bool | false | strict boolean |
| align | Center, Inside, Outside | Center | known enum only |
| arrows.start/end | nullable enum | null | 29 names below; omitted on save when absent |
| arrows.scale_start/scale_end | f32 | 1 | finite 0.01–100 |
| arrows.align | Tip, Extend | Tip | known enum only |

Arrow enum: Triangle, TriangleOpen, Circle, CircleOpen, Square, SquareOpen, Bar, Diamond, Arrow,
ArrowOpen, Barbed, HalfArrowLeft, HalfArrowRight, Concave, DoubleBar, Feather, DotOnBar, Chevron,
DoubleArrow, Target, Star, Cross, Plus, Hexagon, HexagonOpen, Tag, TagOpen, HalfCircle, Drop.
Null style/arrows, unknown fields/enums, odd dash arrays and zero-sum pairs fail closed. Shared validation
runs on load, edit and save. Geometry overflow/work-budget errors identify the path and refuse output.


## 7. v2 required keys and the unknown-field policy

Format 2 adds no field. What changes is strictness:

- Every persisted struct (`VrsFile`, `Anchor`, `Xform`, `Path`, `Group`, `Node`, `Artboard`,
  `SnapConfig`, `Guide`, `Document`, `DocUnits`) gets `#[serde(deny_unknown_fields)]`. An unknown key
  at any level fails the decode closed, instead of today's silent drop — the exact hole ADR-0008 names
  (masks/rotation were added as defaulted fields "no `.vrs` format bump", so a same-numbered old reader
  could open a masked file, lose the masks, and save the loss).
- No Varos writer omits a legacy-defaulted key, so v1 files need **no separate "v2 required key" pass**
  — a v1 file without `artboards` still gets the legacy single non-clipping board (the `#[serde(default)]`
  path); an explicit `artboards: []` stays boardless in both v1 and v2.
- Unknown enum variants (`NodeKind`, `GroupRole`, `Unit`) already fail the decode today and keep doing so.

*(to be filled by S5-B: the exact attribute placement diff, and confirmation that all three frozen
pre-artboards/legacy-group/pre-Paint fixtures still decode under `deny_unknown_fields` — ADR-0008 §R3
argues this is safe by construction from a full field-history audit, but S5-B is where that gets
proved by a passing test, not just argued.)*

## 8. Limits

Starting numbers proposed in ADR-0008 §6 ("Bounded load, symmetric save"); S5-B declared them in
`varos_core::format::Limits::DEFAULT` (`varos-core/src/format/limits.rs`, the one place they are
declared) and lowered two after measuring (ADR-0008 R4: lowered after measurement, never raised silently).

| limit | proposed (ADR-0008 §6) | declared value / actual enforcement |
|---|---|---|
| file bytes | 256 MiB | 256 MiB on checked disk reads and byte input, including the app wrappers on this branch. |
| JSON model bytes | 32 MiB | 32 MiB |
| JSON nesting depth | 128 (serde_json's built-in limit — no separate scanner) | 128 (serde_json built-in; no `Limits` field) |
| PDF objects | 100,000 | 100,000 — xref entries including free slots, loaded objects and name-tree visit budget |
| decoded PDF stream bytes | 64 MiB | 64 MiB — raw extracted model budget; filtered models refused, no inflation performed |
| PDF name-tree depth | 64 | 64 — name-tree depth and preflight direct-container/string nesting |
| nodes (legacy registry groups count here too) | 100,000 | **40,000** |
| paths | 100,000 | **40,000** |
| anchors (outer + holes, total) | 1,000,000 | 1,000,000 — in practice never reached: the 32 MiB model cap binds first, at about 350,000 anchors (~92 bytes per anchor with handles) |
| artboards | 1,000 | 1,000 |
| tree depth (a root Layer is level 1) | 64 | 64 |

Why 40,000 (2026-09-24, S5-B measurement, release build, cloud Linux): loading is dominated by
`sync_tree`, which is roughly quadratic. 40,000 one-anchor paths (≈15 MiB model) took ≈1.3 s to load and
≈1.25 s to save; 50,000 took ≈2.8 s / 2.4 s; 100,000 did not fit under the 32 MiB model cap. The work
order's bar is "about 2 s at the cap" (R4). Optimising `sync_tree` is out of S5; raising the number
afterwards needs the owner.

Limits refuse rather than truncate. The PDF preflight also limits xref lines to 64 bytes, the trailer to 64 KiB, and total direct-object tokens to 16 × max_pdf_objects (1.6 million by default), excluding stream contents. This derived complexity budget prevents a single array from bypassing the indirect-object count; it is not a new serialized field.

## 9. Refusal messages (user-facing copy, ADR-0008 §4 / spec §4)

| condition | copy |
|---|---|
| newer format than this build supports | "This file needs a newer Varos. It uses file format {found}; this build supports up to {supported}. Update Varos to open it. The file has not been changed." |
| over any limit | "The {editable model \| file \| …} exceeds the {32 MiB \| 40,000 \| …} limit (found: {n})." |
| PDF with no embedded model | "This PDF has no editable Varos document. Open the original .vrs file. Importing other PDFs is not available yet." |
| a re-saved PDF using object/xref streams, incremental updates or encryption | "This file was re-saved by another app in a form Varos can't read safely yet. Open the original .vrs." (ADR-0008 R2 — not yet enforced; today's reader has no such gate.) |
| a save that would violate its own limits or validity | "This document can't be saved: {reason}. It is still open." — the document stays dirty in memory; nothing on disk changes. |
| board metadata over a bound / control character / unclean tag (v3) | "This document is damaged: the board name is 121 characters long; the limit is 120." (and the matching sentence per field; on save the same reason inside the save sentence) |
| a v1/v2 file carrying a board key | "This document is damaged: it has a board name, which format 2 files cannot contain, so it may have been edited by another app." |

Core typed errors now exist in `format/error.rs`. Semantic refusals are implemented on the S5-C branch; PDF profile-specific refusals are implemented on the S5-D branch; app `file_ports.rs` also translates errors to plain messages. The table describes the shared target copy, not proof that each app path emits it today.

## 10. Supported PDF profile (S5-D work branch)

Native Varos PDFs use a classic cross-reference table, direct stream lengths and an unfiltered
editable model. The reader checks the actual xref rows/IDs/offsets/object boundaries, not just the
advertised count, and requires one unambiguous terminal startxref/EOF. Before lopdf it rejects
compressed cross-references, Prev (incremental updates), XRefStm (hybrid references), Encrypt and
oversized/over-complex structures. Escaped PDF names are decoded; strings and comments are not
mistaken for forbidden keys. Indirect stream Length and extra material between referenced objects
are outside this native profile. Accepted direct stream extents cannot overlap another object.

lopdf uses a filter that drops ObjStm before inflation. Ordinary filtered page streams remain
compressed and are not rendered/decompressed during loading. The model stream must have no Filter
key (including an empty array or null); no fallback from a failed decoder to raw bytes is allowed.
Its bytes obey both max_model_bytes and max_decoded_stream_bytes before JSON decoding.

The private VAROS_Model reference wins; an invalid private reference is an error, not permission to
try another candidate. If absent, iterative Names/EmbeddedFiles traversal checks depth, visited
references and a node/entry budget, accepts exactly model.varos.json and refuses duplicate matches.
Catalog version is optional for legacy compatibility; when present it must be an integer in
1..=u32::MAX and match the model version through the core gate. Every existing load wrapper uses
this reader; the notice-retaining wrapper continues to surface S5-C migration outcomes.

All eight frozen v1 native PDFs and current writer output pass the checked path. This does not
establish support for Preview/other third-party re-saves: those may be refused with the unsupported
profile reason. No PDF import or new compression dependency is introduced. Independent review,
Preview hand checks remain pending before S5 release acceptance; the owner reports no personal corpus.

## 11. Examples

An abbreviated legacy raw JSON `.vrs` example (format 1; current saves write format 2):

```json
{"varos":1,"doc":{"paths":[{"id":1,"anchors":[{"id":1,"p":[0.0,0.0],"hin":null,"hout":null,"smooth":false},{"id":2,"p":[80.0,0.0],"hin":null,"hout":null,"smooth":false},{"id":3,"p":[40.0,60.0],"hin":null,"hout":null,"smooth":false}],"closed":true,"fill":[0.2,0.7,0.3,1.0],"stroke":[0.0,0.0,0.0,1.0],"stroke_width":2.0,"holes":[],"opacity":1.0,"hidden":false,"locked":false,"name":null}],"groups":[],"group_of":{},"nodes":[{"id":1,"kind":"Layer","name":"Layer 1","parent":null,"children":[4],"hidden":false,"locked":false,"color":null,"clip_exempt":false,"role":"Normal"},{"id":4,"kind":{"Path":1},"name":"","parent":1,"children":[],"hidden":false,"locked":false,"color":null,"clip_exempt":false,"role":"Normal"}],"roots":[1],"active_layer":1,"ids":4,"units":{"ppi":72.0,"display":"Px"},"artboards":[{"x":0.0,"y":0.0,"w":1080.0,"h":1080.0,"name":"Artboard 1","bleed":0.0,"page_color":[1.0,1.0,1.0,1.0],"clip":true,"hidden":false,"locked":false}],"active":0,"move_art_with_ab":true,"snap":{"...":"…SnapConfig fields…"},"ruler_origin":[0.0,0.0],"guides":[],"guides_locked":false}}
```

(Based on `varos-core/tests/fixtures/v1/v1_plain.vrs`; `snap` is elided, so this illustration is not a loadable or byte-identical fixture. Use the actual fixture for tests.)

A clip group's node shape (inside `nodes`, from `v1_masked.vrs`):

```json
{"id":15,"kind":"Group","name":"Group 15","parent":1,"children":[14,13],"hidden":false,"locked":false,
 "color":null,"clip_exempt":false,"role":"Clip","mask_child":14}
```

A rotated leaf's node shape (from `v1_rotated.vrs`):

```json
{"id":5,"kind":{"Path":1},"name":"","parent":1,"children":[],"hidden":false,"locked":false,
 "color":null,"clip_exempt":false,"xform":{"rot":0.6,"piv":[20.0,10.0]},"role":"Normal"}
```

`mask_child` is only present when `role` is `"Clip"` (`#[serde(skip_serializing_if)]` on `None`);
`xform` is only present when it is not the identity transform.

## 12. Fixture index

### The pre-existing golden fixtures (`varos-core/tests/fixtures/`)

| file | era |
|---|---|
| `ancient_pre_artboards.vrs` | pre-artboards, legacy page-bleed behaviour |
| `legacy_groups.vrs` | the flat `groups`/`group_of` registry, pre-tree |
| `pre_paint_enum.vrs` | `fill`/`stroke` as a bare `Option<Rgba>`, pre-`Paint` enum |

### The frozen v1 corpus (`varos-core/tests/fixtures/v1/`, DFS S5-E)

Full provenance in `varos-core/tests/fixtures/v1/README.md`. Every scenario below exists as both a raw
JSON file (`v1_<name>.vrs`) and a PDF container (`v1_<name>_pdf.vrs`), generated by the pre-S5,
format-1 writer at commit `ecf67f5` and never regenerated:

| name | exercises |
|---|---|
| `plain` | one filled+stroked path, one clipping artboard — the ordinary save |
| `boardless` | explicit empty `artboards` — a free canvas |
| `masked` | a path with a hole, clipped by another path (`GroupRole::Clip` + `mask_child` together) |
| `rotated` | a leaf node's live `Node.xform` (non-identity rotation, A7) |
| `translucent` | fill/stroke alpha < 1 plus a reduced path `opacity` |
| `two_artboards` | a clipping white page and a non-clipping transparent page |
| `guides_snap` | non-default ruler guides, `ruler_origin`, `guides_locked`, `SnapConfig` |
| `unicode_arabic` | Arabic (RTL) text in a path name and an artboard name |

Round-trip proof: `varos-core/tests/golden.rs::frozen_v1_corpus_round_trips_on_disk_by_content` (raw)
and `varos-pdf/tests/golden_pdf.rs::frozen_v1_pdf_corpus_round_trips_on_disk_by_content` (PDF), both
via `Document::content_eq` — load → save → reload must preserve every authored-content field. A third
test, `raw_and_pdf_twins_load_to_the_same_content`, checks the two containers of the same scenario
agree.

### Frozen v3 corpus (2026-10-04)

`varos-core/tests/fixtures/v3/README.md` records the first v3 writer, construction and SHA256SUMS:
`v3_board_meta` (the v2 masked/rotated board migrated, then named `شعار المقهى — Café logo` with an
Arabic+Latin description and tags `client`, `عربي`, `logo`) and `v3_boardless` (the v2 boardless
file migrated: what a v2 file saves as), each as raw + PDF twins. Four refusal inputs were appended
to `fixtures/refused/` (README addendum): `v4_future`, `future_v4_pdf` (NewerVersion 4/3),
`v2_board_name` (FieldNotInFormat) and `board_duplicate_tag` (Board DuplicateTag). Since v3 is
current, the frozen `v3_future` / `future_pdf` now pass the gate and fail the typed decode
(Malformed); their bytes are unchanged. The native writer byte fixtures `varos-pdf/tests/fixtures/
native_{demo,rich}.pdf` were re-blessed once for the bump (model stream + version + offsets only).

### Frozen v4 corpus (2026-10-07)

`varos-core/tests/fixtures/v4/README.md` records the first v4 writer, construction and SHA256SUMS:
`v4_board_meta` (the frozen `v3_board_meta` migrated: artboard ids 16 and 17) and `v4_boardless` (the
frozen `v3_boardless` migrated: no pages, so no ids), each as raw + PDF twins. Six refusal inputs were
appended to `fixtures/refused/` (README format-4 addendum): `v5_future`, `future_v5_pdf` (NewerVersion
5/4), `v3_artboard_id` (FieldNotInFormat "artboard id", 3), `artboard_duplicate_id` (DuplicateId
artboard 16), `artboard_missing_id` (MissingArtboardId 0) and `active_out_of_range`
(ActiveArtboardOutOfRange 2/2). Since v4 is current, the frozen `v4_future` / `future_v4_pdf` now pass
the gate and fail the typed decode (Malformed); their bytes are unchanged. The native writer byte
fixtures `varos-pdf/tests/fixtures/native_{demo,rich}.pdf` were re-blessed once for the bump (model
stream + version + offsets only; page content streams are byte-identical).

### Frozen v2 and refusal corpus (S5-E)

`varos-core/tests/fixtures/v2/README.md` records the writer baseline (`6b6f41e`), exact
construction and SHA256SUMS for masked+rotated and boardless raw/PDF twins. A synthetic
v1 broken-mask sample in the same cohort proves notice-bearing in-memory repair.
`fixtures/refused/README.md` records eighteen immutable refusal inputs and expected typed
reasons. Existing v1 and ancient fixtures remain unchanged; tests never regenerate them.

Core golden tests apply the full round-trip law to the raw fixtures. PDF golden tests now
check complete Document equality and A==B for all eight frozen v1 PDFs. `frozen_v2.rs`
checks raw/PDF equivalence, exact frozen v2 bytes, stable saves, mask/hole/rotation/board
values, legacy repair notices, typed refusals through bytes and disk, and lowered limits.
Large caps use small fixtures plus explicit Limits; no huge fixture is committed.

## 13. Old-reader harness

`varos-pdf/tests/old_reader_harness.rs` preserves the version gate from
`ecf67f5:varos/crates/varos-core/src/file.rs:28-35`, replacing the old `VRS_VERSION` constant
with literal 1. This is a frozen adaptation, not a verbatim copy of an old executable.
Current lopdf is only test plumbing for extracting the PDF model. Fresh and frozen v2 raw/PDF
outputs must receive the exact old newer-version refusal; a frozen v1 control must pass the gate.

```bash
cargo test --locked -p varos-pdf --test old_reader_harness
# 5 passed, 0 failed; included in the default workspace suite
```

Format 3 adds a second frozen gate, `v2_gate`: the format-2 reader's `peek_version` from
`f21c20e:varos/crates/varos-core/src/format/mod.rs` with `FORMAT_VERSION` = 2 and its `NewerVersion`
text inlined. Fresh and frozen v3 raw/PDF output must receive exactly "This file needs a newer Varos.
It uses file format 3; this build supports up to 2. …"; frozen v1 and v2 headers pass it (5 tests).

Format 4 adds a third frozen gate, `v3_gate`: the format-3 reader's `peek_version` from
`a5f687b:varos/crates/varos-core/src/format/mod.rs` with `FORMAT_VERSION` = 3. Fresh and frozen v4
raw/PDF output must receive exactly "This file needs a newer Varos. It uses file format 4; this build
supports up to 3. …"; frozen v1, v2 and v3 headers pass it. The v1 and v2 gates refuse v4 too (8 tests).

These tests prove the old gate logic, not execution of an old application binary. Personal
files, real-window behavior, an actual pre-S5 build and Preview re-saving remain unverified.

## 14. Corpus check — Ahmed's hand test 0

`varos-pdf/tests/corpus_check.rs` has one ignored manual entry point and ordinary synthetic
harness tests. The explicit manual run requires `VAROS_CORPUS_DIR` to name a real, dedicated
personal-document directory. It reads every regular `.vrs`/`.json` file recursively, with
case-insensitive extensions. It reports/skips symlinks without following them, prints load
notices and refusal reasons, and never writes files. Do not point it at source/configuration
folders or the intentionally invalid test corpus.

```bash
# Run from varos/; substitute the actual personal-document directory.
VAROS_CORPUS_DIR="/path/to/personal-varos-documents" \
  cargo test --locked -p varos-pdf --test corpus_check -- --ignored --nocapture
```

Unset configuration, a missing/unreadable/empty folder or any refused document makes the
explicit run fail. An ordinary CI run skips this entry point and cannot establish personal
acceptance. Any refusal of a personal file blocks S5 merge until understood/resolved. On 2026-09-27 the owner said there are no saved personal files. Read-only filename scans of
the home directory (excluding Library/build/cache trees), iCloud/CloudStorage and Varos support
locations found only project fixtures, with no scan errors. The personal-file run is therefore
not applicable to the currently available corpus, not a passing manual test. Run it if personal
documents are found later (DFS_S5_FORMAT_V2.md §1; ADR-0008 §Consequences, R3).

### Format 5 fixtures and refusals (§§7–9, 12–13)

`varos-core/tests/fixtures/v5/INDEX` indexes canonical JSON, native PDF and SVG goldens for plain,
cap/join, 1/2/3 dash pairs and phase signs, corner fitting, alignments and all 29 arrow kinds.
`stroke_style.rs` covers load/edit/save validation, undo, notes, coverage and v4 doc byte identity.
`varos-pdf/tests/stroke_v5.rs` verifies round trips and frozen outputs; frozen v4 tests retain their original
SHA256 corpus and compare doc subtree/page content bytes across the bump.
`refused/future_v6.json` and `future_v6.pdf` freeze matched version-6 stamps with undecodable models.
The old `v5_future.vrs`/`future_v5_pdf.vrs` remain archived inputs to the frozen v4 reader gate;
the current reader accepts their version then refuses their invalid model. The old-reader harness adds
the base-7b48f2c v4 gate; plain v5 JSON/PDF and `doc:42` demonstrate refusal before typed decode.
Native save retains editable centrelines/styles; export baking never overwrites authored paths.

### Format 6: images (lane w2-images; provisional 6 confirmed at integration)

See [the image amendment](../adr/ADR-0008-amendment-next-images.md). Provisional version 6 adds `images`, `assets`, `raster_effects_ppi` and `NodeKind::Image(id)`. Defaults are omitted; the named v5 migration preserves old payloads. Original and proxy binary streams are siblings of the embedded model attachment, joined through immutable asset keys, never JSON pixel arrays. The integrator renumbers this writer if merge order changes.

### Format 7: gradient paints and document swatches (lane B; renumbered 6 → 7 at integration)

The writer in `feat/w2-gradients` emits `NEXT_GRADIENT_VERSION` (provisional 6 against the
format-5 base; the moderator renumbers after the image lane). The normative keys, limits,
identity migration, backward refusal, renderer/export behavior and new fixture locations are
in [ADR-0008 next-gradients amendment](../adr/ADR-0008-amendment-next-gradients.md).
`doc.swatches` is omitted when empty; null and solid array paints retain their original bytes.
Tagged gradient/reference paints are refused under earlier version stamps before typed decoding.
Frozen v4/v5 fixture files are unchanged. Whole container version stamps advance on Save.

<!-- Lane F fix round: next-preview format; moderator renumbers in merge order -->
### Format 9 container key: optional Quick Look preview (Lane F; renumbered 6 → 9 at integration)

This lane's next writer uses provisional format **6**, after stroke's 5. The integrator must renumber
this step after images, gradients, text and Live Corners in the actual merge order; this lane owns no
image/blob schema. Both JSON `varos` and PDF `/VAROS_SchemaVersion` advance together for all new saves.
`migrate_v5_to_next_preview(Document, &Limits)` is a named pure identity migration: preview metadata
is container-only, with no authored-content change. Existing v1–v5 fixtures are unchanged.

The PDF catalog optionally adds `/VAROS_Preview` (reference to an unfiltered EmbeddedFile PNG stream,
maximum 2 MiB) and `/VAROS_PreviewVersion 1`. Both keys must be present together. The normal native loader
refuses these keys with an older or absent schema stamp, malformed references, future preview revisions,
filtered or oversized streams, and invalid PNG signatures. A previous v5 reader refuses the new format
before typed decoding. Missing previews remain valid in the next format; no downgrade writer is offered.

Frozen fixtures in `varos-pdf/fixtures/quicklook`: `next-preview.vrs` (restamped 9), `refuse-old-stamp.vrs`
(5), `refuse-v8-stamp.vrs` (8), `future-v10.json` and `future-v10.pdf`; `fix_round_tests` checks content preservation and the previous
reader gate. Historical v5 visual goldens compare unchanged output after normalizing only the two
single-digit format stamps. Their original files and SHA256SUMS remain untouched.

Save validates cached PNGs using bounded reads (2 MiB plus a sentinel byte), strict 544 × 246 dimension
limits and an 8 MiB decoder budget before decoding, and reuses those validated bytes. Cache writes are
best-effort; missing/corrupt/unwritable cache falls back to an in-memory thumbnail. The signed macOS
Quick Look extension and blob-aware integration remain moderator work, requiring native acceptance.
<!-- End Lane F fix round -->

<!-- ---- Lane B w3-effects ---- -->
## Provisional wave-3 v11 — Phase 10 live effects
Writer: 11. Previous era: 10 (Lane A appearance; identity reservation in this standalone worktree).
Pure step: migrate_v10_to_v11, typed defaults only. Integrator rechains v9→v10→v11.
Path.effects: ordered tagged offset/zig_zag/transform/warp recipes, empty list omitted.
StrokeStyle.width_profile: optional {points:[[length_fraction,left_factor,right_factor],…]}, None omitted.
Seven presets are canonical point arrays; custom points have the same representation.
Reader key gates refuse both new keys in eras 1–10, including empty/null values.
Frozen JSON/refusal corpus: crates/varos-core/tests/fixtures/w3-effects, SHA256SUMS.
Export resolves independent copies; editable embedded models retain the authored stack.
No change to historical fixtures; historical malformed-v10 assertions now test typed refusal.
<!-- ---- end Lane B w3-effects ---- -->
<!-- ---- w3-cmyk ---- -->
### Format 12: explicit colour sources (Lane C)

`COLOUR_VERSION = 12`. `Document.colour_mode` defaults to `Rgb` and is omitted for RGB;
`output_profile` is optional `{name,data}` with bounded ICC metadata encoded as hex.
Existing null/solid-array paints retain their exact JSON bytes. New paints are
`{"type":"managed","value":{"colour":{"model":"rgb|cmyk|gray|spot",...},"alpha":1}}`.
Source channels/tint are finite 0..1; named spot inks require a consistent CMYK alternate.
Swatches retain the managed paint and existing global identity. See proposed ADR-0017.

`migrate_v11_to_v12` is a named pure identity migration; missing keys are RGB defaults.
This worktree uses reserved identity v9→v10→v11 steps; the integrator must replace these
with the other wave-3 lane migrations before merging. Pre-v12 colour keys/managed tags
are refused before typed decoding; unknown future stamps are refused. Frozen v12 fixtures
live in `varos-core/tests/fixtures/v12` and PDF operator goldens in `varos-pdf/tests/fixtures/v12`.
Historical fixtures remain unchanged. RGB authored bodies/page operators are identical;
container/model stamps and dependent PDF offsets/lengths necessarily advance.
<!-- ---- end w3-cmyk ---- -->
