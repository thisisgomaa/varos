> **Status:** current — Accepted project decision, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# ADR-0008: `.vrs` format versioning, migration and bounded validation

- **Date:** 2026-09-24
- **Decision owner:** Product owner
- **Supersedes:** None (complements ADR-0003 and ADR-0004; see "Relationship" below)
- **Superseded by:** None

**ملخص بالمصري**
- كل ملف هيتحفظ من دلوقتي هيتكتب عليه «صيغة 2»، من غير ما نغيّر شكل الرسمة نفسها جوه الملف.
- لو الملف من Varos أحدث، أو رقم صيغته ناقص أو صفر أو مش متطابق، البرنامج يرفضه برسالة واضحة قبل ما يقرا الرسمة، وشغلك المفتوح يفضل زي ما هو.
- ملفاتك القديمة (صيغة 1) تفضل تتفتح؛ التحديث بيحصل في الذاكرة بس، والملف الأصلي مايتغيرش غير لما إنت تدوس حفظ.
- أي حاجة غريبة أو مكسورة أو أكبر من الحدود المعلنة تترفض بدل ما تتصلّح في السر، والحفظ بيراجع نفس الشروط عشان مانكتبش ملف مانقدرش نفتحه.
- نسخ Varos القديمة هترفض ملفات صيغة 2 برسالة «محتاج Varos أحدث»، ومفيش حفظ بصيغة قديمة؛ والموافقة على القرار ده بتاعتك إنت يا أحمد.

## Context

ADR-0004 makes the versioned Serde JSON of `varos-core` the persisted schema and defers migrations, validation metadata and compatibility guarantees to a separately reviewed ADR (`docs/adr/ADR-0004-v1-schema-policy.md`, Decision ¶3). This is that ADR, for persistence only.

The hole: `VRS_VERSION = 1`, and its comment says only wrapper changes bump it (`varos/crates/varos-core/src/file.rs:11-12`). Masks (`role`, `mask_child`) and rotation (`xform`) were added as defaulted fields under a "no `.vrs` format bump" comment (`varos/crates/varos-core/src/model.rs:266-270`, `:317-329`). Defaults protect new readers of old files, not old readers of new files. Serde drops unknown keys silently, so a same-number older reader can open a masked file, lose the masks, and save the loss. Today's loader also accepts version `0`, reports a missing version as a raw Serde error, never reads the PDF catalog's `/VAROS_SchemaVersion`, and runs `sync_tree` on unvalidated input (`file.rs:27-38`). Evidence: `docs/foundation/work_orders/DFS_S5_FORMAT_V2.md` §2.

The owner approved decision D3 on 2026-09-24: format 2 for all new saves, conservative bumps and refusals, bounded migrations, and a pure-PDF export (`docs/specs/DOCUMENT_FILE_SYSTEM.md` §1 قرار ٣, §2 "Format versions, migration and validation", §6 D3).

## Decision

**1. One integer format version, in two places.** The wrapper key `varos` is the format version. The PDF catalog repeats it as `/VAROS_SchemaVersion`. This format number is separate from the container generation chosen in ADR-0003 (PDF + embedded JSON); raising it does not change the container.

**2. v2 is the same model with a stricter reader.** Format 2 is today's `Document` Serde model with the stamp raised to 2. There is no schema change. What is new is the contract:
- a strict version check before any typed decode (rule 4);
- v1 files keep their legacy Serde defaults;
- unknown fields and unknown enum variants fail closed, at every level;
- declared size, depth, count and id limits (rule 6), and structural and semantic validity.

**3. The bump rule.** Any change to what the writer can emit — a new, removed or renamed key, a new enum variant, a changed type, unit or default — raises the format number. Reader-only relaxations (accepting input no writer emits yet) do not. Each bump ships, before its writer ships: a named pure migration, old and new fixtures, a rejection fixture, and an update to `docs/reference/VRS_FORMAT.md`.

**4. Refusal policy (all before typed `Document` decoding).**
- Newer than this build supports → refused: "This file needs a newer Varos…". No view-only fallback, no editable guess.
- Missing version → refused. Zero, negative, fractional or too large → refused as invalid.
- Catalog version present and different from the wrapper version → refused as a mismatch. A missing catalog key (raw JSON files) is not a mismatch.
- A refusal never installs a partial document and never modifies the file on disk; the open workspace stays as it was.

**5. Migration.** Migrations are sequential (v1→v2, later v2→v3…), pure and deterministic. v1→v2 performs only the documented v1 normalizations (group registry → tree with order kept, adopting tree-less paths, pruning empty groups, healing nested live transforms) and raises the id counter with a checked max+1. It never repairs authored mask meaning: a clip the old repair would demote is refused. The original bytes stay untouched until the user saves. A migrated file opens with "Opened an older file. Saving will update its format."

**6. Bounded load, symmetric save.** One pipeline: bounded file read → bounded PDF parse and decompression → version check → bounded JSON decode → structural precheck (cycles, duplicate and dangling ids, ownership, depth, counts, id headroom) → migration → semantic validation. Starting limits: 256 MiB file, 32 MiB model, 128 JSON levels, 100,000 PDF objects, 64 MiB decoded streams, 64 PDF levels, 100,000 nodes, 100,000 paths, 1,000,000 anchors, 1,000 artboards, 64 tree levels. Over-limit input is refused with the number, never truncated. Numbers may be lowered after measurement, never raised silently. Save runs the same structure, validation and size checks, so this build never writes what it would refuse to read; a refused save leaves the document open and dirty.

**7. Wire contract and export.** `docs/reference/VRS_FORMAT.md` is the exact wire contract (envelope, PDF keys, version table, migrations, required keys, limits, refusal copy, supported PDF profile, fixtures). The deliverable PDF from Export omits the editable model entirely (S6); only native `.vrs` carries it.

**Deferred:** an introspectable plugin/AI schema, stable property ids, a downgrade-save or view-only mode, bounded support for PDFs re-saved with object or xref streams, and any schema change (for example moving UI preferences out of `Document`), which would be v3.

### Alternatives considered

- **Rename the version key** (e.g. `format_version`). Rejected: pre-S5 readers would fail with "missing field `varos`" instead of their existing readable "newer Varos" message. The Rust constant is renamed (`FORMAT_VERSION`); the wire key is not.
- **Keep dropping unknown fields.** Rejected: that is the hole itself. Silent dropping turns a newer file into quiet data loss on the next save.
- **Switch to a CBOR or ZIP container.** Rejected: ADR-0003 fixes PDF + embedded JSON for V1, and a container change needs its own superseding ADR and migration. Versioning the model does not require it.

### Relationship to ADR-0003 and ADR-0004

This ADR complements both and edits neither text. ADR-0003's container stays. ADR-0004's model-is-the-schema rule stays. It narrows one reading of ADR-0004: Serde defaults remain a way for new readers to load old files, but they no longer justify skipping a format bump.

## Consequences

- An older build refuses a newer file even when the new feature is unused in it — rule 3 bumps on any writer-side change, not only ones a given file exercises. Stamping the minimum version a file actually needs is a possible later ADR, not v2.
- Old builds refuse v2 files with their existing readable message ("saved by a newer Varos (v2)"), because their header check already runs before typed decode (`file.rs:33`). Every `.vrs`-capable build since `7a5b3c8` has this check. v1 files already damaged by an old re-save cannot be rebuilt.
- No downgrade-save: a file saved by this build cannot be written for older builds.
- Fail-closed may refuse some real v1 files (retired keys, dangling leaves, invalid clips). Before any S5 merge to `main`, a corpus check runs over the owner's own `.vrs` files. A legitimately retired key becomes an explicit, documented allowlist entry, never blanket tolerance.
- Some PDFs re-saved by other apps (object streams, xref streams, incremental updates, encryption) will be refused with a readable reason.
- Every load and save pays for validation; limits must be measured at the caps and lowered if too slow.
- Enforcement: headless tests (new saves write 2; every refusal has a typed error; cyclic input terminates), an old-reader harness that proves pre-S5 reader logic refuses v2 before decode, the charter round-trip law over frozen v1 and v2 fixtures, and the owner's hand tests (`DFS_S5_FORMAT_V2.md` §1).

## Status

Accepted — product owner (Ahmed), 2026-09-24.

**Number note:** 0008 is taken by this ADR because it is filed first. The MCP study (`docs/studies/2026-09-23-MCP_CONTROL_STUDY.md`) and the Online study (`docs/studies/2026-09-23-ONLINE_AND_MAC_STUDY.md`) also propose "ADR-0008"; they take the next free numbers when their ADRs are drafted.
