> **Status:** current — work order (charter §3 level 4), derived from docs/specs/DOCUMENT_FILE_SYSTEM.md; owner decisions D1–D3 recorded 2026-09-24.
> Amended 2026-09-24 after the independent plan review (see reviews/DFS_S5_FORMAT_V2.review.md); P1/P2 applied, "Needs Ahmed" items carry their default.
# DFS S5 — `.vrs` format version 2, migration and bounded validation

Date: 2026-09-24 · Base: `ecf67f5` on `claude/sweet-cerf-1sg30t` · Scope: `varos-core` (model/format) + `varos-pdf` (container read side) + docs. **No `varos-app` edits.** Parallel-safe with S1, because S5 does not touch the app loop and keeps every existing public signature the app uses.

## 1. Goal & acceptance

**Goal (spec §2 "Format versions…", §5 row S5, §6 D3 / "Validation effort").** Every new save is **format 2**, in the JSON wrapper and in the PDF catalog. Files newer than 2, and files with a missing, zero, invalid or mismatched version, are **refused before typed `Document` decoding**, with a readable reason. Old v1 files (raw JSON or PDF, including ones with `role`/`mask_child`/`xform`) stay readable through an explicit, pure v1→v2 migration. Loading runs one bounded pipeline: byte cap → bounded PDF parse → version gate → bounded JSON decode → structural precheck → migration → semantic validation. Nothing reaches `sync_tree` unvalidated. Save runs the same checks, so this build cannot write a file it would refuse to open. Deliverables: the code, the fixtures, `docs/reference/VRS_FORMAT.md`, and the companion ADR `docs/adr/ADR-0008-vrs-format-versioning.md`. **The ADR must be Accepted by Ahmed before any S5 code reaches `main`.**

**Headless tests prove the following** (no Renderer, no EventLoop):
- (a) New writers emit `"varos":2` and `/VAROS_SchemaVersion 2`.
- (b) Every frozen v1 fixture (legacy raw, pre-artboards, legacy groups, pre-Paint, boardless, masked+rotated raw and PDF) loads, migrates and obeys the **charter round-trip law**: load → save A → reload equals → save B, with A == B (`FOUNDATION_CHARTER.md:45`).
- (c) Newer, zero, missing, mismatched, unknown-field, cyclic, oversized, too-deep, id-overflow and non-finite inputs are refused with typed errors. Cyclic input does not hang or overflow the stack.
- (d) An **old-reader harness** proves that the pre-change reader logic refuses v2 before any typed decode.
- (e) Limits are enforced inside the PDF read path (no unbounded inflation).

**Ahmed's hand test (Mac, batched after merge):**
0. **Before merge to `main`, a precondition, not a batched item:** `VAROS_CORPUS_DIR=~/Documents cargo test -p varos-pdf --test corpus_check -- --ignored --nocapture`. Send the output. Any refusal of one of your own files blocks the merge.
1. Open the old fixtures (`varos/crates/varos-core/tests/fixtures/*.vrs` and `varos/crates/varos-pdf/tests/fixtures/v1_*`), including the populated masked v1 file. Save, then reopen: same editable content, masks still clip, rotation kept.
2. Keep document A dirty, then try `v3_future.vrs`, `cycle_nodes.vrs` and an oversized file (`mkfile -n 300m ~/Desktop/big.vrs`). Each gives a readable refusal, and A, its history and its path stay untouched.
3. Open a v2 file with a pre-S5 build (`main` today). It refuses with "saved by a newer Varos (v2)".
4. Re-save a v2 `.vrs` in macOS Preview and reopen it in Varos. Record whether it opens or is refused with the "re-saved by another app" reason (see Risk R2).

## 2. Current-code findings (evidence read at `ecf67f5`)

**Where the version lives and what "v1" means**
- `varos-core/src/file.rs:12` has `VRS_VERSION: u32 = 1`. Its comment at `:11` says that only wrapper changes bump it ("model evolution is handled by serde defaults").
- The envelope is `struct VrsFile { varos: u32, doc: Document }` (`:14-18`). **The version field already exists; it is the key `varos`.**
- The PDF catalog repeats it as `/VAROS_SchemaVersion` (`varos-pdf/src/lib.rs:266`). Nothing reads that key back.
- "v1" is therefore *a family of eras* that all carry `1`:
  - raw JSON (first slice);
  - pre-artboards (no `artboards` key; `legacy_artboards` default, `model.rs:397-402`, `:525`);
  - legacy group registry (`groups`/`group_of`, `model.rs:229-236`, `:502-506`);
  - pre-Paint (`Option<Rgba>` shape, hand-written serde at `model.rs:142-171`; same JSON);
  - pre-tree (no `nodes`/`roots`);
  - pre-mask/pre-xform (`role`/`mask_child`/`xform` absent);
  - the current writer (all present).
- `GroupRole`'s comment at `model.rs:269-271` says "additive, no `.vrs` format bump". `role`/`mask_child` are defaulted (`:319-329`), and `xform` is skipped when it is the identity (`:317`). **This is the hole.** A same-number old reader drops unknown mask keys and re-saves data loss.

**The current load path**
- `doc_from_blob` (`file.rs:27-40`) first does a header-only parse (`VrsHead{varos}`) and refuses `varos > 1` before the typed decode (`:33-35`). That is good. But it accepts `0`, and a missing key surfaces as a raw serde "missing field" error.
- It then decodes the typed `Document` and **calls `sync_tree()` on unvalidated input** (`:38`). `Editor::replace_doc` calls `sync_tree` again (`editor.rs:3108-3110`).
- Raw load does `read_to_string` with no cap (`file.rs:59-61`). PDF load does `std::fs::read` with no cap and sniffs `%PDF-` at byte 0 (`varos-pdf/src/lib.rs:36-44`).

**How serde treats unknown data**
- `deny_unknown_fields` appears nowhere in the workspace (grep), so unknown keys are **silently dropped** at every level.
- Unknown enum variants (`NodeKind`, `GroupRole`, `Unit`) already fail the decode.
- serde_json's default **recursion limit of 128** is active: `cargo tree --workspace -e features -i serde_json` shows only `default`/`std`, no `unbounded_depth`.
- A non-finite `f32` serializes as `null` and cannot be decoded back. So today a NaN coordinate produces a file that will not reopen, which is why save-side validation is needed.
- Every key used by the three frozen fixtures is known to the current model (checked with a key-walk script). Adding `deny_unknown_fields` does not break them.

**Crash, hang and overflow hazards on hostile input** (all reachable today via `sync_tree`, or at render time)
- `collect_paths` recurses without a cycle guard (`model.rs:1049-1058`).
- `attach_front` recurses up legacy parents (`:1449-1458`), so a legacy `groups[].parent` cycle recurses forever.
- `is_mask_source` runs an unbounded `while let` up parents (`:643-651`).
- `node()` is a linear scan (`:1004-1006`), so `sync_tree` is roughly O(n²) at 100k nodes.
- `nid()` does `self.ids += 1` unchecked (`:588-591`); `ids` is persisted (`:516`). It panics in debug and wraps in release, producing duplicate ids.
- `sync_tree` silently **demotes invalid clips** (`:1638-1654`), **heals nested live xforms** (`:1657-1666`), prunes dangling leaves and empty groups, and adopts orphans (`:1556-1605`). The existing test `masks.rs::sync_tree_demotes_a_clip_whose_mask_was_removed` pins this *runtime* behaviour, which S5 keeps. S5 refuses such input *on load* instead of repairing authored mask semantics.

**PDF read (`varos-pdf/src/lib.rs:343-407`, lopdf 0.43.0)**
- It calls `lopdf::Document::load_mem` with no options.
- lopdf eagerly inflates cross-reference streams while parsing the xref (`parser/mod.rs` → `decode_xref_stream`).
- It also inflates every `/Type /ObjStm` stream (`reader.rs:987-989` → `object_stream.rs:42`), using `read_to_end` with no cap (`object.rs:849-866`).
- The encrypted-load path inflates object streams while ignoring any filter (`reader.rs:839-924`).
- **For unencrypted input, `LoadOptions.filter` runs before ObjStm inflation** (`reader.rs:983-989`). That is the hook S5 uses.
- `dereference` is bounded (`document.rs:144-159`, `DEREF_LIMIT`).
- Varos's own code has further problems:
  - `decompressed_content()` is unbounded and silently falls back to the raw bytes on a decode error (`lib.rs:350`).
  - The recursive `/Kids` walk has no depth limit and no visited set (`:359-383`).
  - The fallback accepts the **first UTF-8 embedded file of any name** (`:394-405`).
- The writer embeds the model **uncompressed** (`:252`), uses a classic xref and no object streams. `flate2 1.1.9` is already in `Cargo.lock` (via lopdf/usvg).

**Fixtures and tests**
- `varos-core/tests/fixtures/` holds only 3 raw v1 JSON files. There are no PDF fixtures and no masked, rotated, boardless, newer, cycle or oversize fixtures.
- `golden.rs:25-38` implements the round-trip law on the JSON blob path only.
- `vrs.rs:41-48` checks "newer" using `"doc":{}`. That body would fail a typed decode anyway, so only the error message proves the header check ran first.
- The container round-trips live in `varos-pdf/tests/container.rs`.

**App and other work orders**
- The app calls only `varos_pdf::save_vrs` (`main.rs:587`) and `varos_pdf::load_vrs` (`main.rs:637`), each with `Result<_, String>`. On error it shows `Open failed: {e}`, and it replaces the document only on `Ok`.
- S3's work order snapshots through `doc_to_blob`/`doc_from_blob`, with a 32 MiB cap.
- S6's work order moves writing into `varos-pdf/src/write.rs`, leaves the read side of `lib.rs` to S5, and expects `/VAROS_SchemaVersion` → 2.
- **ADR-0008 is free as a file.** However, the MCP study (`docs/studies/2026-09-23-MCP_CONTROL_STUDY.md:126,146`) and the Online study (`…ONLINE_AND_MAC_STUDY.md:194`) also *propose* that number.

## 3. Design

**What v2 is (honest minimum): no schema change.** v2 is the current `Document` serde model with the stamp raised to `2`. What is new is the *reader contract*:
- a strict version gate;
- required keys where a serde default is a **legacy** reinterpretation;
- unknown keys fail closed;
- declared limits and semantic validity.

The wrapper key stays **`varos`**. It is the `format_version` field. Renaming it would make pre-S5 readers fail with "missing field `varos`" instead of their readable "newer Varos" message. The Rust constant is renamed to `FORMAT_VERSION`, and `VRS_VERSION` stays as an alias. UI preferences that are persisted today (`snap`, `ruler_origin`, `guides_locked`, `move_art_with_ab`, `active`, `active_layer`) stay in the schema. S1 excludes them from the dirty comparison. Moving them out would be a v3 change.

**Load pipeline** (core function `format::decode_model`; the PDF prefix lives in `varos-pdf`):
1. **Bytes.** File ≤ `max_file_bytes`: check `metadata().len()`, then read through `take(max+1)`.
2. **PDF.** Pre-gate, then bounded lopdf parse, then the model stream (§3 PDF), giving `(model_json, catalog_version)`.
3. **JSON size.** ≤ `max_model_bytes`. Depth: serde_json's built-in 128-level limit; no own scanner (it is already active — `cargo tree -i serde_json` shows only `default`/`std`, no `unbounded_depth` — and skipped unknown content goes through its iterative, heap-stacked `ignore_value`, so deep junk cannot overflow the stack; the model's own typed depth is about 6).
4. **Version gate.** A header-only parse of `varos`:
   - missing → `MissingVersion`;
   - not a positive integer that fits in u32, or `0` → `InvalidVersion`;
   - `> 2` → `NewerVersion`;
   - catalog version present and `≠` the JSON version → `VersionMismatch`.
   All of this happens **before any typed decode**.
5. **Typed decode.** `VrsFile` and every persisted struct get `#[serde(deny_unknown_fields)]`, so a serde error becomes `Malformed(msg)`. No Varos writer ever omits a legacy-defaulted key (none of them is `skip_serializing`), so there is no separate v2 required-key pass — it would cost a second full JSON walk over up to 32 MiB and would wrongly make the legacy `groups`/`group_of` keys mandatory in v2, freezing keys the format wants retired. A v1 file without `artboards` still gets the legacy single non-clipping board; an explicit `[]` stays boardless in both versions.
6. **Structural precheck** (`format/structure.rs`, iterative, HashMap-indexed; must run before any tree walk):
   - counts against the limits: nodes, paths, total anchors (outer + holes), artboards, legacy groups;
   - ids unique within each kind; cross-kind reuse is legal (existing tests and older files already reuse ids across kinds, e.g. path 1 / anchor 1 / Layer node 1 coexisting; R10);
   - every `NodeKind::Path(pid)` points to an existing path; no dangling parent or child ids;
   - `parent` agrees with the `children` membership, and each node has exactly one owner;
   - no cycles, including legacy `groups[].parent` cycles;
   - tree depth ≤ `max_tree_depth`;
   - id headroom: `max_used = max(all path/node/anchor/legacy-group ids)`, and `ids.max(max_used).checked_add(1)` must succeed, else `IdExhausted`.
7. **Migration** (`format/migrate.rs`: sequential table `[(1, migrate_v1_to_v2)]`, pure, deterministic):
   - Run `sync_tree()`. This is safe now, because the tree is acyclic and depth-bounded. It performs only the documented v1 normalizations: registry → tree with order preserved, adopting tree-less paths, pruning empty groups, healing nested live xforms.
   - Before and after that call, compare the `(node, role, mask_child)` triples. **Any demotion means the input is refused (`Invalid::BadMask`)** and is never repaired.
   - Set `ids = ids.max(max_used)`.
   - v2 input skips this step.
8. **Semantic validation** (`format/validate.rs`, version-agnostic, applied to the canonical v2 state):
   - masks, parentage and kinds;
   - finiteness and ranges;
   - the legacy registry is empty;
   - no nested live xform, no empty group, no orphan path;
   - reserved `MaskAlpha`/`MaskLuma` are refused (never written, so accepting them would reinterpret them later).
9. **Canonical check for v2.** `sync_tree()` must leave `nodes`, `roots`, the path set and every role/`mask_child` unchanged. Only these may change: the storage order of `paths`, `active_layer`, and a clamped `active`. Anything else → `Invalid::NotCanonical`.

   The result is `Loaded { doc, source_version, migrated }`. The original bytes are never modified.

**Save:** `encode_model` runs `check_structure` on `doc`. It then runs the step-7 normalizer on a clone (`sync_tree`; a clip demotion ⇒ `SaveRefused(Invalid::BadMask)`), then `validate` and the size cap, and serializes the normalized clone. The editor's document is never mutated. `doc_to_blob` delegates to it, so `write_pdf`, raw `save_vrs` and S3's snapshots all refuse invalid documents with a readable `SaveRefused`, and this build cannot write a file it would then refuse to reopen (step 9's canonical check on load is now matched on save). The editor keeps the document dirty in memory.

**PDF side** (`varos-pdf/src/read.rs`, new; `lib.rs` read side only):
1. **Pre-gate on raw bytes, before lopdf:**
   - Find the last `startxref` in the final 1 KiB. The offset must point at the classic `xref` keyword; otherwise refuse with `UnsupportedPdf("compressed cross-reference")`.
   - Sum the subsection counts: ≤ `max_pdf_objects`, overflow-safe.
   - Scan the trailer dictionary (bounded to 64 KiB) for delimiter-aware `/Prev`, `/XRefStm` and `/Encrypt`. Refuse with the reason "incremental update", "hybrid xref" or "encrypted". Varos never writes any of these.
2. **lopdf load:** `load_mem_with_options` with `LoadOptions { filter: Some(drop_object_streams), ..Default::default() }`. The filter is a plain `fn` that returns `None` for any stream whose dict has `/Type /ObjStm`, so object streams are never inflated. Then check that `objects.len() ≤ max_pdf_objects`.
3. **Finding the model:**
   - Prefer catalog `/VAROS_Model`.
   - Otherwise walk `/Names/EmbeddedFiles` **iteratively**, with depth ≤ `max_pdf_depth`, a visited `ObjectId` set and a node budget. Accept **only** the name-tree key `model.varos.json`.
   - If there is none → `NoEmbeddedModel`.
4. **Decoding the model stream:** no `/Filter` ⇒ raw content (≤ `max_model_bytes`); any filter ⇒ `UnsupportedPdf("model encoding")`. No `flate2` dependency: Varos writes the model unfiltered (`lib.rs:252`), and an app that re-compresses it will almost certainly also write object/xref streams, which the pre-gate already refuses — so decompression support buys nothing acceptance needs. Revisit only if hand test 4 finds a Preview output that passes the pre-gate with a Flate model (R7's own default).
5. **Catalog version:** `/VAROS_SchemaVersion` must be an integer in `1..=u32::MAX` if present (else `MalformedPdf`/`InvalidVersion`). `None` if absent.

**Public API** (spelled out exactly; S5-B lands it and the others code against it):
```rust
// varos-core/src/format/mod.rs   (lib.rs: `pub mod format;`)
pub const FORMAT_VERSION: u32 = 2;      // what this build writes
pub const MIN_READ_VERSION: u32 = 1;    // oldest readable
pub use limits::{Limits, LimitKind}; pub use error::{LoadError, Invalid, SaveRefused};
pub struct Loaded { pub doc: Document, pub source_version: u32, pub migrated: bool }
impl Loaded { pub fn notice(&self) -> Option<&'static str> } // migrated ⇒ "Opened an older file. Saving will update its format."
pub fn peek_version(json: &[u8]) -> Result<u32, LoadError>;
pub fn decode_model(json: &[u8], container_version: Option<u32>, limits: &Limits) -> Result<Loaded, LoadError>;
pub fn encode_model(doc: &Document, limits: &Limits) -> Result<String, SaveRefused>;
// format/limits.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits { pub max_file_bytes: u64 /*256 MiB*/, pub max_model_bytes: usize /*32 MiB*/,
  pub max_pdf_objects: usize /*100_000*/,
  pub max_decoded_stream_bytes: usize /*64 MiB*/, pub max_pdf_depth: usize /*64*/,
  pub max_nodes: usize /*100_000*/, pub max_paths: usize /*100_000*/, pub max_anchors: usize /*1_000_000*/,
  pub max_artboards: usize /*1_000*/, pub max_tree_depth: usize /*64*/ } // no max_json_depth: serde_json's built-in 128-level limit already applies
impl Limits { pub const DEFAULT: Limits = /* the numbers above */; } impl Default for Limits { /* DEFAULT */ }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind { FileBytes, ModelBytes, PdfObjects, DecodedStreams, PdfDepth, Nodes, Paths, Anchors, Artboards, TreeDepth }
// format/error.rs  (Display = the user-readable reason; std::error::Error)
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
  Io(String), TooLarge { limit: LimitKind, found: u64, max: u64 }, NotAVarosFile, NoEmbeddedModel,
  UnsupportedPdf(String), MalformedPdf(String), Malformed(String), MissingVersion, InvalidVersion(String),
  NewerVersion { found: u32, supported: u32 }, VersionMismatch { container: u32, model: u32 },
  Invalid(Invalid), MigrationFailed { from: u32, reason: String } }
#[derive(Clone, Debug, PartialEq)]
pub enum Invalid {
  DuplicateId { kind: &'static str, id: u32 }, Dangling { from: &'static str, id: u32, missing: u32 },
  Cycle { node: u32 }, BadParentage { node: u32, reason: &'static str }, BadMask { group: u32, reason: &'static str },
  NonFinite { what: String }, OutOfRange { what: String, value: f64 }, IdExhausted,
  LegacyInV2 { what: &'static str }, NotCanonical { what: &'static str } }
pub struct SaveRefused(pub LoadError); // Display: "This document can't be saved: {reason}. It is still open."
// format/structure.rs   pub fn check_structure(doc: &Document, limits: &Limits) -> Result<(), LoadError>;
// format/migrate.rs     pub fn migrate_v1_to_v2(doc: Document, limits: &Limits) -> Result<Document, LoadError>;
// format/validate.rs    pub fn validate(doc: &Document, limits: &Limits) -> Result<(), Invalid>;   // never mutates
// varos-core/src/file.rs (signatures UNCHANGED): doc_to_blob → encode_model(DEFAULT); doc_from_blob → decode_model(.., None, DEFAULT).doc;
//   load_vrs(path) bounded read; `pub const VRS_VERSION: u32 = format::FORMAT_VERSION;`
// varos-pdf/src/read.rs
pub struct Container { pub model_json: Vec<u8>, pub catalog_version: Option<u32> }
pub fn read_container(pdf: &[u8], limits: &Limits) -> Result<Container, LoadError>;
// varos-pdf/src/lib.rs
pub fn load_vrs_bytes(bytes: &[u8], limits: &Limits) -> Result<Loaded, LoadError>; // %PDF- sniff at 0, else raw JSON
pub fn load_vrs_checked(path: &Path, limits: &Limits) -> Result<Loaded, LoadError>;
pub fn load_vrs(path: &Path) -> Result<Document, String>; // UNCHANGED signature: checked(DEFAULT).doc, err.to_string()
```
**Display copy** (spec §4): `NewerVersion` → "This file needs a newer Varos. It uses file format {found}; this build supports up to {supported}. Update Varos to open it. The file has not been changed." `TooLarge` → "The {editable model|file|…} exceeds the {32 MiB|100,000 nodes|…} limit". `NoEmbeddedModel` → "This PDF has no editable Varos document. Open the original .vrs file. Importing other PDFs is not available yet." Each keeps the word "newer" etc., so the existing `vrs.rs` assertions hold. Dialog titles and the migration notice are wired by S1's open pipeline; S5 only exposes them.

## 4. Pieces

**S5-A — Companion ADR-0008 (Proposed)** · `opus` · S · no dependencies · **merge first**
- Owns: `docs/adr/ADR-0008-vrs-format-versioning.md` (new, following `ADR-0000-template.md`). Must not touch other ADRs, STATUS or code.
- Contents:
  - **Context:** ADR-0004 defers migrations and validation; the hole at `file.rs:11-12` and `model.rs:269-271,319-329`; D3 of 2026-09-24.
  - **Decision:**
    - integer format version in the wrapper `varos` plus `/VAROS_SchemaVersion`; the container generation (ADR-0003) is distinct;
    - the bump rule, stated simply: any change to what the writer can emit raises the format number; reader-only relaxations do not (no "needs proof" case, since `deny_unknown_fields` already makes every writer-side addition non-additive-safe);
    - refusal of newer, missing, zero and mismatched versions before typed decode;
    - no view-only fallback or downgrade save;
    - sequential pure migrations, each with old and new fixtures plus a rejection fixture;
    - unknown fields and enum variants fail closed;
    - declared limits, with save-side symmetry;
    - `VRS_FORMAT.md` as the wire contract;
    - a pointer that the deliverable PDF omits the model (S6).
  - **Deferred:** a plugin/introspectable schema, stable property ids, downgrade.
  - **Consequences:** old builds refuse new files, even when the new feature is unused in that file; bounded third-party PDF support.
  - **Status:** `Proposed`.
- Also state that it complements, and does not amend, ADR-0003/0004, and note the number collision with the MCP and Online studies.

**S5-B — Core format spine: v2, gate, limits, errors, structure, migration, encode** · `opus` · M · depends on A only for the merge to `main` · **lands before C and D**
- Creates: `varos-core/src/format/{mod,limits,error,structure,migrate}.rs`, plus `format/validate.rs` as a **stub** (`Ok(())`, header `// OWNED BY S5-C`), and `varos-core/tests/format_v2.rs`.
- Modifies:
  - `varos-core/src/lib.rs` (+`pub mod format;`);
  - `varos-core/src/file.rs` (delegation, bounded `load_vrs`, `VRS_VERSION` alias);
  - `varos-core/src/model.rs`: **attributes and comments only**. Add `#[serde(deny_unknown_fields)]` on `Anchor, Xform, Path, Group, Node, Artboard, SnapConfig, Guide, Document`, and `units.rs::DocUnits`; fix the `GroupRole` comment ("no bump" → "v2, see ADR-0008").
- Also owns, conditionally: `varos-pdf/tests/fixtures/native_demo.pdf` and `native_rich.pdf` — **only** if S6-A merges before B, and only to regenerate them for the version bump (2-byte diff each, proved with `cmp -l`; see §5 hot spots).
- Must not touch: `editor.rs`, `command.rs`, `varos-pdf` source, `varos-app`, other fixtures, `golden.rs`.
- Steps: implement pipeline steps 3–7 and step 9 of §3; wire `doc_to_blob`/`doc_from_blob`; run the full workspace suite (existing tests must stay green unchanged, except messages that intentionally change).
- Tests (`format_v2.rs`):
  - `new_saves_write_format_2`
  - `v1_blob_migrates_and_reports_notice`
  - `v2_blob_loads_without_migration`
  - `newer_version_refused_before_typed_decode` (body `"doc":42` ⇒ `NewerVersion`, not `Malformed`)
  - `missing_zero_negative_fractional_huge_versions_refused`
  - `unknown_field_fails_closed_envelope_document_node`
  - `v1_missing_artboards_gets_legacy_board`
  - `explicit_empty_artboards_stays_boardless_v1_and_v2`
  - `container_version_mismatch_refused`
  - `json_depth_over_limit_refused`
  - `model_bytes_over_limit_refused` (tiny `Limits`)
  - `node_cycle_refused_without_hang`, `legacy_group_parent_cycle_refused_without_hang`
  - `duplicate_ids_refused`, `dangling_leaf_refused`
  - `tree_depth_over_limit_refused`, `counts_over_limits_refused`
  - `id_exhaustion_refused`, `stale_ids_counter_raised`
  - `v1_invalid_clip_refused_not_demoted`
  - `v1_masks_and_xform_preserved_through_migration`
  - `sync_tree_is_noop_on_canonical_v2`
  - `save_refuses_non_finite_and_leaves_doc_unchanged`
  - `every_saved_doc_reopens` (for every document built in `format_v2.rs`/`format_validate.rs`: `decode_model(encode_model(d)?)` is `Ok`)
  - `limits_default_values_match_documented_numbers`
  - `#[ignore] decode_timing_at_caps` (prints load **and encode** time at 10k, 50k and 100k nodes — S3 pays the encode cost every 30 s; the numbers go in the PR, see R4).

**S5-C — Semantic validator** · `opus` · M · branches from B's merge (or codes against §3 API from base and rebases) · parallel with D
- Owns: `varos-core/src/format/validate.rs` (replaces B's stub wholesale) and `varos-core/tests/format_validate.rs`. Must not touch other `format/*` files, `model.rs`, or `varos-pdf`.
- Rules are trimmed to what the user actually drew, not what the save-side normalizer (§3 step 7, now also run on save per F1) already guarantees: "legacy registry empty; no nested live xform; no empty group; no orphan path" are `sync_tree` invariants produced by that normalizer, so re-checking them here refuses nothing new on load and, on save, they were the rules most likely to refuse a user's work for no benefit (R5). (Every remaining rule must cite the code that maintains it in the live editor; a rule the editor cannot guarantee is dropped and listed in the PR, never tightened silently.)
  - **Kinds and parentage:** roots are Layers with `parent: None`; Path leaves have no children; nesting of Layer/Group matches what `model.rs` actually permits; every path has exactly one leaf.
  - **Masks:** `Clip` only on Group nodes; `mask_child` is `Some` iff `Clip`, and it is a direct child that exists; `MaskAlpha`/`MaskLuma` refused.
  - **Finite:** every `f32` (anchor `p/hin/hout`, holes, `stroke_width`, `opacity`, paint channels, `xform.rot/piv`, artboard `x/y/w/h/bleed/page_color`, guides, `ruler_origin`, snap floats, `units.ppi`). `Invalid::NonFinite { what }` names the object (path name or id), so a refused save tells Ahmed which shape to fix.
  - **Ranges:** `opacity` and color channels ∈ [0,1]; `stroke_width` ≥ 0; artboard `w`,`h` ≥ 0 (the spec says "negative sizes"; `> 0` has no cited editor invariant) and `bleed` ≥ 0; `ppi` > 0; `snap.candidate_max` bounded.
  - **Complexity:** O(n) with HashMaps.
- Tests:
  - `valid_editor_documents_pass` (fresh, grouped, `clip_group`, rotated via `set_node_xform`, boardless, multi-board)
  - one `*_refused` test per rule, table-driven for finiteness over every float field
  - `validate_is_linear` (`#[ignore]` timing)

**S5-D — Bounded PDF container reader** · `opus` · M/L · branches from B's merge · parallel with C
- Owns:
  - `varos-pdf/src/read.rs` (new: pre-gate, filter, iterative name-tree walk, bounded decode);
  - `varos-pdf/src/lib.rs` (**read side only**: remove `extract_model`, add `load_vrs_bytes`/`load_vrs_checked`, rewire `load_vrs`);
  - `varos-pdf/tests/container_bounds.rs`.
- Must not touch: `write_pdf`/page code (S6), `varos-core`, `varos-app`.
- Tests:
  - `v2_native_pdf_round_trips_and_catalog_says_2`
  - `catalog_json_version_mismatch_refused` (the catalog is rewritten with lopdf)
  - `plain_pdf_is_no_embedded_model`
  - `resaved_with_object_streams_refused`, `xref_stream_refused` (lopdf `SaveOptions{use_object_streams/use_xref_streams}`)
  - `incremental_update_refused`, `encrypted_trailer_refused`
  - `xref_count_over_limit_refused`
  - `filtered_model_refused` (any `/Filter`, including Flate, is `UnsupportedPdf("model encoding")` — no `flate2` dependency)
  - `name_tree_kids_cycle_terminates`, `name_tree_depth_over_limit_refused`
  - `fallback_accepts_only_model_varos_json`
  - `normal_streams_not_inflated_by_parse` (content length equals the compressed length after load)
  - `object_streams_dropped_by_filter`
  - `file_over_cap_refused_before_read`
  - `load_vrs_string_api_unchanged`
  - `#[ignore] lopdf_peak_memory_on_large_array` (measure the amplification, see R4)

**S5-E — Frozen fixtures, old-reader harness, golden law, `VRS_FORMAT.md`** · `sonnet` · M · **starts at base `ecf67f5` immediately** (the v1 fixtures must come from the pre-bump writer); finishes after C and D merge
- Owns:
  - new files in `varos-core/tests/fixtures/` and `varos-pdf/tests/fixtures/`;
  - `varos-core/tests/golden.rs` (extend the list);
  - `varos-pdf/tests/golden_pdf.rs`, `varos-pdf/tests/old_reader_harness.rs`;
  - `varos-pdf/tests/corpus_check.rs` — `#[ignore]`, reads `VAROS_CORPUS_DIR`, prints OK or the refusal reason for each file found there, and never writes; it is the harness behind hand test 0.
  - `docs/reference/VRS_FORMAT.md`.
- Must not touch any `src/`.
- Fixture generation at base, with a one-off throwaway test that is not committed:
  - PDF: `v1_masked_rotated_pdf.vrs` (2 boards, a clip group, one rotated unit), `v1_boardless_pdf.vrs`;
  - raw: `v1_masked_rotated.vrs`, `v1_boardless.vrs`.
- Hand-written:
  - `v1_unknown_field.vrs`, `v3_future.vrs`, `v0_zero.vrs`, `no_version.vrs`;
  - `cycle_nodes.vrs`, `cycle_legacy_groups.vrs`, `dup_ids.vrs`;
  - `id_overflow.vrs` (id `4294967295`), `float_overflow.vrs` (`1e39`).
- After B: `v2_masked_rotated.vrs` and `v2_masked_rotated_pdf.vrs`, frozen as the v2 goldens for a future v3.
- Oversized and deep inputs are generated in-test with tiny `Limits`; never commit big binaries. Provenance (base sha, generator snippet) is recorded in `VRS_FORMAT.md` §Fixtures.
- **Old-reader harness (shrunk to what it actually proves — every build able to save has always had the header-first gate, from `7a5b3c8:file.rs:38` through `ecf67f5:file.rs:33`, and every Varos PDF has written `/VAROS_Model` since `b06194a:lib.rs:194`; a generic copy with a `Tripwire` counter would only prove `2 > 1` again):**
  - `old_reader_harness.rs` holds a **≤15-line verbatim copy** of `ecf67f5:file.rs:28-35` (the typed decode comes after the gate, so the gate returns first by construction) and of the old `/VAROS_Model` lookup (`ecf67f5:lib.rs:343-356`). No generics, no `Tripwire` counter, no `sync_tree` closure. The reviewer diffs it with `git show ecf67f5:<path>`.
  - Tests: it asserts that the v2 JSON and PDF fixtures give "newer Varos (v2)" — `old_reader_refuses_v2_json_before_decode` and `old_reader_refuses_v2_pdf_before_decode`.
  - Both refusal tests **fail at base and pass after B**. That is the evidence.
  - Honesty note in the file: this proves the frozen *logic*. The old *binary* is covered by Ahmed's hand test 3.
- **Golden law:**
  - Extend `golden.rs` to every JSON fixture.
  - `golden_pdf.rs`: load the PDF fixtures via `load_vrs_bytes` → `write_pdf` A → reload equal → B, with `A == B`. If the pdf-writer output proves non-deterministic, compare the embedded model bytes and record why.
  - Refusal fixtures assert their exact `LoadError` variant.
- **`VRS_FORMAT.md`** (status `current`, level 4 reference):
  - the JSON envelope; the PDF keys (`/VAROS_Model`, `/VAROS_SchemaVersion`, the EmbeddedFiles name `model.varos.json`, `/AF` Source);
  - the version table and bump rule;
  - migrations v1→v2 with each normalization;
  - the v2 required keys and the unknown-field policy;
  - the limits table with numbers;
  - the refusal list with user copy;
  - the supported PDF profile (classic xref, no ObjStm/XRefStm/Prev/Encrypt, model raw or single Flate);
  - examples; the fixture index with provenance.

## 5. Merge order and conflict hot-spots

**Order:**
1. **A.** Docs only, into the work branch. Ahmed accepts it (edits Status → `Accepted — product owner, <date>`).
2. **B.**
3. **C ∥ D.** Disjoint files.
4. **E.** Rebase on C+D and run every fixture.
5. Moderator: gates (`cargo test --workspace -j 4`, clippy `--all-targets -D warnings` for host + `x86_64-pc-windows-msvc` + `aarch64-apple-darwin`, `fmt --check`), independent review, then STATUS/GATE_LOG. **Nothing from B–E reaches `main` while ADR-0008 is `Proposed`.**

**Hot-spots:**
- `varos-core/src/model.rs`: B adds attribute lines only; S1 may add content-checkpoint code. Textual merge, keep both.
- `varos-core/src/lib.rs`: B's one line vs any S1 module line.
- `format/validate.rs`: add/add between B's stub and C. **Take C's file wholesale.**
- `varos-pdf/src/lib.rs`: D edits the read side. If S6-A (writer → `write.rs`) lands first, D rebases onto it; if D lands first, S6-A keeps D's read side. S6-A's `write.rs:12,283` imports `VRS_VERSION` and its `export_pdf.rs:136-138` compares `write_pdf` byte-for-byte against `native_demo.pdf` **and** `native_rich.pdf` — B's own bump breaks that byte-identity, and S6-A is being built now. So: if S6-A is merged first, **B** regenerates both `varos-pdf/tests/fixtures/native_demo.pdf` and `native_rich.pdf` itself (added to B's owned files), and proves with `cmp -l` that each file differs in exactly 2 bytes (`"varos":1→2` in the blob, `/VAROS_SchemaVersion 1→2`; both are the same length, so no offsets move). Any other difference stops B.
- `file.rs`: B edits it now; S3 later moves `write_atomic`. B must not touch `write_atomic`.

## 6. Risks / open questions (each with a default so execution never blocks)

- **R1 — ADR acceptance is an owner act.** Default: A merges as `Proposed`, and code pieces may be built and merged on the work branch, but not to `main`, until Ahmed marks it Accepted. On the number collision (MCP and Online studies also *propose* "ADR-0008"): this ADR takes 0008 because it is the first filed, and the studies renumber when they are drafted. The moderator notes this in STATUS.
- **R2 — Some third-party re-saved PDFs will now be refused** (object/xref streams, incremental updates, encryption). Today they may open unboundedly. Default: fail closed with the reason "This file was re-saved by another app in a form Varos can't read safely yet. Open the original .vrs." Bounded ObjStm support is a later F7 follow-up. It is unknown whether macOS Preview re-saves this way; hand test 4 measures it.
- **R3 — Real v1 user files may carry retired or unknown keys, dangling leaves or invalid clips,** which are now refused under fail-closed. A full-history audit narrows this: the first build that could save `.vrs` is `7a5b3c8` (2026-07-02), no earlier app code wrote files; since then no persisted field has been removed or renamed in `model.rs`/`units.rs` (`d556acf` kept the `fill`/`stroke` keys and only changed their type; `0f3c629` was a fmt reflow that re-added every field it removed); the singular `artboard` key was removed in `d1e5f80`, before any save existed. So `deny_unknown_fields` on v1 is safe by construction — the remaining risk is *structural*: broken clips, dangling leaves, out-of-range values. Default: refuse with the precise reason. Before any merge to `main`, run `#[ignore] corpus_check` (`varos-pdf/tests/corpus_check.rs`, reads `VAROS_CORPUS_DIR`) over Ahmed's own `.vrs` files — this is hand test 0, required, not optional. A legitimately retired key becomes an explicit, documented allowlist field (`#[serde(default, skip_serializing)]` + `IgnoredAny`), never blanket tolerance.
- **R4 — Declared limits versus measured cost.** `sync_tree` is about O(n²) (`model.rs:1004`), and lopdf may amplify memory on huge array objects. Default: B and D report the measured time and peak memory at the caps. If loading at the caps takes more than about 2 s or 8× the file size, **lower the default numbers** in `limits.rs` and `VRS_FORMAT.md`. Optimizing `sync_tree` is out of S5.
- **R5 — Save-side refusal could block saving work the editor produced** if the validator is stricter than the editor. Default: only rules backed by a cited editor invariant (C's rule); a refused save leaves the document dirty with the reason, and S3 recovery still holds it in memory; `valid_editor_documents_pass` covers the flows the tests exercise.
- **R6 — The harness is frozen logic, not the old binary.** Builds older than the header-first check (if any exist) cannot be protected retroactively. v1 files already damaged by an old re-save cannot be reconstructed (spec §2). Default: state this in `VRS_FORMAT.md`, and rely on hand test 3 for the binary.
- **R7 — No `flate2` dependency.** Decided: D keeps the pre-gate and the ObjStm filter (the minimum lopdf needs) but does not add `flate2`; a Flate-encoded model is refused as "unsupported encoding" instead. Varos writes the model unfiltered, and any re-compressing app is already caught by the pre-gate's object/xref-stream refusal. Revisit only if hand test 4 finds a Preview-resaved file that needs it.
- **R8 — App messaging.** S5 keeps `load_vrs`/`save_vrs` returning `String`. The exact dialog titles and the "Opened an older file…" notice are wired by S1's open pipeline, or a small integration piece after S1, consuming `Loaded`/`LoadError`. For S5, the hand test sees "Open failed: {reason}".
- **R9 — Day cap.** D (bounded parsing) may exceed S5's one-day cap (spec §6). Default: reveal at entry. The release gate stays closed until bounded parsing is real, and checks are never moved after live replacement to meet the deadline.
- **R10 — Anchor-id uniqueness is not enforced.** Tests and older files reuse ids across kinds (e.g. `vrs.rs:21-31`). Anchor ids only feed `max_used`. Documented as a non-invariant.
- **R11 — F7 overlap.** S5 delivers the charter's "hostile-file hardening" items (recursion/cycle guards, decompression cap, id re-derivation, `FOUNDATION_CHARTER.md:113`) for the load path. The moderator records this against F7 in STATUS rather than duplicating the work.

## 7. Out of scope

- Any `varos-app` change: dialogs, the AppCommand open pipeline, the migration notice UI (S1).
- Atomic or durable writes, and moving `write_atomic` out of core (S3).
- Recovery snapshots (S3; they inherit the validation through `doc_to_blob`/`doc_from_blob`).
- Deliverable PDF without the model, export modes, and `write.rs` (S6).
- Schema changes of any kind: moving UI preferences out of `Document`, new fields, soft masks. Any of these requires v3 and a new migration.
- A downgrade-save or view-only mode.
- Foreign PDF/AI/SVG import.
- Bounded support for object streams and xref streams.
- Optimizing `sync_tree` or `node()`.
- An editor-side `nid()` overflow guard at runtime.
- A plugin or introspectable schema.
- Amending ADR-0003/0004, charter flags, and STATUS/GATE_LOG edits (the moderator's job).

## Needs Ahmed (from review)
1. **Accept ADR-0008** after the review's edits are applied (they are, in this amendment). **Working assumption: accept.**
2. **An old file whose clipping mask is already broken** (the mask shape is no longer inside its group). Refuse to open it, or open it with the mask released and a notice? **Working assumption: refuse — unless hand test 0's corpus check finds one of Ahmed's own files like this, in which case open it with a notice instead.**
3. **Every new saved feature raises the format number** (an older Varos then cannot open files from a newer one, even ones that don't use the new feature). **Working assumption: yes** — simple and honest (see ADR-0008 rule 3 and Consequences).
4. **Run the one corpus command on the Mac before S5 reaches `main`.** **Working assumption: required** — see hand test 0.
