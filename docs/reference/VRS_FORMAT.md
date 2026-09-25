> **Status:** current — the `.vrs` wire contract, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3. Owned by DFS S5-E (`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md`); the version, limit and migration numbers below are filled in as S5-B/C/D land.

# `.vrs` file format — wire contract

`.vrs` is Varos's document format: a versioned JSON model of `varos-core::model::Document`, carried
either as a raw JSON file or embedded inside a valid PDF container (the `.ai` pattern). This document
is the exact contract a reader or writer must honour — envelope, container, versioning, limits and
refusal copy. The decision record behind it is `docs/adr/ADR-0008-vrs-format-versioning.md`; read that
for *why*, this for *what byte, what key, what number*.

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
{"varos": 1, "doc": { /* the full Document, see varos-core/src/model.rs */ }}
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

Today's build (pre-S5-B) already does a version-first, header-only parse before the typed decode
(`varos-core/src/file.rs::doc_from_blob`) — that is the hole ADR-0008 closes: it accepts `0`, reports
a missing version as a raw serde error, and never reads `/VAROS_SchemaVersion` back. S5-B replaces this
with `varos_core::format::decode_model`, implementing every bullet above precisely.

## 5. Version table

| version | status | written by | notes |
|---|---|---|---|
| 1 | **current** (every build up to and including this one) | every `.vrs`-capable build since `7a5b3c8` (2026-07-02) | a *family* of eras, all stamped `1` — raw JSON, pre-artboards, legacy group registry, pre-`Paint` enum, pre-tree, and (the hole) masks/rotation added under this same number. See ADR-0008 §Context. |
| 2 | **planned** — lands with S5-B | this build, once S5-B merges | same `Document` shape as 1; the reader contract tightens (§6, §9). No schema change — see ADR-0008 §"v2 is the same model with a stricter reader". |

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

**What migration refuses instead of repairing:** if `sync_tree`'s before/after comparison shows a clip
group's `(role, mask_child)` was *demoted* (the pre-S5 runtime repair for a mask whose shape was
removed — `masks.rs::sync_tree_demotes_a_clip_whose_mask_was_removed` pins that this still happens at
**runtime**), the v1→v2 **load** is refused instead: migration never repairs authored mask meaning.

A migrated file opens with the notice **"Opened an older file. Saving will update its format."** The
original bytes on disk are never touched until the user explicitly saves.

`migrate_v1_to_v2` itself, and the sequential migration table structure it sits in
(`format/migrate.rs`), are S5-B's deliverable — this table is the contract that implementation must
match.

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

Starting numbers proposed in ADR-0008 §6 ("Bounded load, symmetric save"); S5-B made them the enforced
`varos_core::format::Limits::DEFAULT` (`varos-core/src/format/limits.rs`, the one place they are
declared) and lowered two after measuring (ADR-0008 R4: lowered after measurement, never raised silently).

| limit | proposed (ADR-0008 §6) | enforced (`Limits::DEFAULT`) |
|---|---|---|
| file bytes | 256 MiB | 256 MiB |
| JSON model bytes | 32 MiB | 32 MiB |
| JSON nesting depth | 128 (serde_json's built-in limit — no separate scanner) | 128 (serde_json built-in; no `Limits` field) |
| PDF objects | 100,000 | 100,000 (enforced by S5-D's reader) |
| decoded PDF stream bytes | 64 MiB | 64 MiB (enforced by S5-D's reader) |
| PDF name-tree depth | 64 | 64 (enforced by S5-D's reader) |
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

Over-limit input is refused with the number that was exceeded, never silently truncated.

## 9. Refusal messages (user-facing copy, ADR-0008 §4 / spec §4)

| condition | copy |
|---|---|
| newer format than this build supports | "This file needs a newer Varos. It uses file format {found}; this build supports up to {supported}. Update Varos to open it. The file has not been changed." |
| over any limit | "The {editable model \| file \| …} exceeds the {32 MiB \| 40,000 \| …} limit (found: {n})." |
| PDF with no embedded model | "This PDF has no editable Varos document. Open the original .vrs file. Importing other PDFs is not available yet." |
| a re-saved PDF using object/xref streams, incremental updates or encryption | "This file was re-saved by another app in a form Varos can't read safely yet. Open the original .vrs." (ADR-0008 R2 — not yet enforced; today's reader has no such gate.) |
| a save that would violate its own limits or validity | "This document can't be saved: {reason}. It is still open." — the document stays dirty in memory; nothing on disk changes. |

Today's build (pre-S5-B) only has the "newer Varos (v{n}) — please update" message
(`varos-core/src/file.rs:34`); the rest of this table is S5-B/C/D's deliverable, worded here so every
piece implements the same copy.

## 10. Supported PDF profile (S5-D's deliverable)

Once S5-D lands, a `.vrs` PDF container must be: a classic (non-compressed) cross-reference table, no
`/Prev`/`/XRefStm` (no incremental updates, no hybrid xref), no `/Encrypt`, and the model stream either
raw or refused as `UnsupportedPdf`. Varos never writes any of the refused shapes itself; the risk is
third-party re-saves (ADR-0008 §Consequences, R2/R7). **Until S5-D merges, PDF reading is unbounded**
(`varos-pdf/src/lib.rs::extract_model` calls `lopdf::Document::load_mem` with no options and no byte
cap) — this is a known, tracked gap, not yet closed by this piece.

## 11. Examples

A minimal raw JSON `.vrs` (format 1, today):

```json
{"varos":1,"doc":{"paths":[{"id":1,"anchors":[{"id":1,"p":[0.0,0.0],"hin":null,"hout":null,"smooth":false},{"id":2,"p":[80.0,0.0],"hin":null,"hout":null,"smooth":false},{"id":3,"p":[40.0,60.0],"hin":null,"hout":null,"smooth":false}],"closed":true,"fill":[0.2,0.7,0.3,1.0],"stroke":[0.0,0.0,0.0,1.0],"stroke_width":2.0,"holes":[],"opacity":1.0,"hidden":false,"locked":false,"name":null}],"groups":[],"group_of":{},"nodes":[{"id":1,"kind":"Layer","name":"Layer 1","parent":null,"children":[4],"hidden":false,"locked":false,"color":null,"clip_exempt":false,"role":"Normal"},{"id":4,"kind":{"Path":1},"name":"","parent":1,"children":[],"hidden":false,"locked":false,"color":null,"clip_exempt":false,"role":"Normal"}],"roots":[1],"active_layer":1,"ids":4,"units":{"ppi":72.0,"display":"Px"},"artboards":[{"x":0.0,"y":0.0,"w":1080.0,"h":1080.0,"name":"Artboard 1","bleed":0.0,"page_color":[1.0,1.0,1.0,1.0],"clip":true,"hidden":false,"locked":false}],"active":0,"move_art_with_ab":true,"snap":{"...":"…SnapConfig fields…"},"ruler_origin":[0.0,0.0],"guides":[],"guides_locked":false}}
```

(this is `varos-core/tests/fixtures/v1/v1_plain.vrs`, byte-identical, `snap` elided here for length.)

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

### Future v2 goldens

Once S5-B lands and format 2 exists, `v2_masked_rotated.vrs` / `v2_masked_rotated_pdf.vrs` get frozen
the same way, as the goldens a future v3 migration will be proved against.

## 13. Old-reader harness

`varos-pdf/tests/old_reader_harness.rs` carries a ≤15-line verbatim copy of the version gate at
`ecf67f5:varos/crates/varos-core/src/file.rs:28-35` (diff it against `git show
ecf67f5:varos/crates/varos-core/src/file.rs` to check it is still exact) and two tests,
`old_reader_refuses_v2_json_before_decode` / `old_reader_refuses_v2_pdf_before_decode`, that feed it
*this build's own* current writer output. They are `#[ignore]`d today with the reason "fails until
S5-B raises the write version past 1" — run them with `--include-ignored` to see them fail now; once
S5-B raises `FORMAT_VERSION`/`VRS_VERSION` to 2 they start passing, and that pass is exactly the proof
that a pre-S5 build refuses a v2 file before any typed decode. This proves the frozen *logic*, not the
old *binary* — the binary is covered by Ahmed's hand test 3
(`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md` §1).

## 14. Corpus check — Ahmed's hand test 0

`varos-pdf/tests/corpus_check.rs` is `#[ignore]`d (it needs real files, not a CI fixture) and reads
`VAROS_CORPUS_DIR`; unset, it prints a note and returns without failing. Pointed at a real folder, it
walks every `.vrs`/`.json` file recursively and prints `OK <path>` or `REFUSED <path> — <reason>` for
each, never writing anything:

```
VAROS_CORPUS_DIR=~/Documents CARGO_TARGET_DIR=/home/user/varos/target-s5 \
  cargo test -p varos-pdf --test corpus_check -- --ignored --nocapture
```

This is the required precondition before any S5 code reaches `main` (DFS_S5_FORMAT_V2.md §1, hand test
0): any refusal of one of Ahmed's own `.vrs` files blocks the merge until it is understood (ADR-0008
§Consequences, R3).
