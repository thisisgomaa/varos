> **Status:** proposed amendment — 2026-10-09; pending independent review and the Phase 2 writer merge.

# ADR-0008 amendment: v5 stroke style

Decision owner: product owner; execution ordering delegated to the moderator in the approved [PLAN](../PLAN.md).
Complements [ADR-0008](ADR-0008-vrs-format-versioning.md); leaves its existing decisions and container unchanged.
This document specifies the next writer, not a claim that format 5 is implemented in this branch.

## Decision

Format 5 adds only `doc.paths[].stroke_style`, including the nested fields and enum variants defined in
[STROKE_ENGINE](../foundation/work_orders/STROKE_ENGINE.md). No new Paint variant, text, gradient, image,
appearance stack, width profile or effect key belongs in this bump. Existing fields keep their types and meanings.

| Contract | Requirement |
|---|---|
| Writer stamp | `FORMAT_VERSION = 5`; JSON `varos:5` and PDF `/VAROS_SchemaVersion 5`, including plain documents. |
| Optional key | `Path.stroke_style` uses `#[serde(default, skip_serializing_if = "StrokeStyle::is_default")]`. |
| Missing value | Round cap/join, miter limit 10, empty dash, phase 0, corner fitting false, Center, no arrows, scales 1, Tip. |
| Strictness | Unknown fields/variants at every new nesting level fail; null style fails; shared edit/load/save bounds apply. |
| Earlier formats | Reject presence of `stroke_style` in formats 1–4, even `{}` or null, as `Invalid::FieldNotInFormat`. |
| Newer files | Refuse unsupported future versions before typed decode, even if their model would otherwise fit v5. |
| Saving | Validate before writing, retain checked reopen/safe-write guarantees; refused save leaves open document dirty. |
| Downgrade | None. A style-free v5 file is still refused by a v4 reader. |

All style behaviour, including arrows, must exist in GPU, CPU, PDF, SVG, hit/cull and Bridge 1.2 before the writer ships.
Defaults protect old content; they do not waive the bump rule. UI can follow in 2.2 because headless operations expose behaviour.
The Phase 2.0 adapter alone neither changes writer output nor raises the version.

## Named pure migration

| From → to | Step | Operation |
|---|---|---|
| 4 → 5 | `format::migrate_v4_to_v5(Document, &Limits)` | Identity on the authored document; omitted styles acquire defaults during typed decode. No normalization, geometry conversion or ID allocation. |

Register the step after `migrate_v3_to_v4` in the sequential table. Files 1–3 first run their existing migrations.
Keep the version gate and keys-only format scan before typed decoding; add the style restriction to that scan.
Validate older canonical content with its existing rules before migration, then validate the resulting v5 model.
The migration is deterministic, has no IO, clock, randomness, UI or global state, and must not inspect renderer caches.
No default style is inserted into serialized JSON. No authored dash/alignment value is silently “repaired”.
Opening preserves original bytes on disk and displays the existing older-format notice; only Save updates the file.

For each frozen v4 fixture, extract the canonical serialized `doc` subtree before/after the v5 round-trip and require
byte equality, not merely JSON value equality. Width, Paint's null/array encoding, ordering and float formatting stay intact.
Only the JSON wrapper stamp changes at model-envelope level. The PDF catalog, embedded stream and container offsets change too;
whole native-file bytes cannot be identical across the bump. Preserve frozen v4 fixture files without re-blessing them.
Before the bump, 2.0 must still pass the complete existing byte-identity suite.

## Fixtures and old readers: required before the writer

All paths below are relative to `varos/crates/varos-core/tests/` unless explicitly qualified.

| Evidence | Required coverage |
|---|---|
| `fixtures/v5/` | Frozen canonical JSON and native PDF: plain/default, all caps/joins, dash pairs/phases/corners, alignments, 29 heads, holes/opacity. |
| `fixtures/v4/` | Keep original bytes; load/migrate/save tests compare the unchanged `doc`; separately compare default render output. |
| `fixtures/refused/future_v6.json` and `future_v6.pdf` | Matched v6 wrapper/catalog; refusal occurs before model decoding and preserves live workspace/disk. |
| Version refusals | Missing/zero/negative/fractional/overflow version and catalog mismatch retain existing typed errors. |
| Style refusals | `v4_stroke_style`, unknown nested field/enum, odd or zero-sum dash pairs, nonfinite/out-of-range fields, invalid nulls. Include older-version default-looking styles. |
| Symmetric validation | Every invalid style fails edit/load/save; resource failures cannot create partial output or partial transactions. |
| Old-reader harness | Freeze the v4 gate from the pre-v5 writer revision; v5 JSON/PDF, including a plain file, must return “needs a newer Varos” before decode. |

Do not “update” the frozen v4 reader to know about `StrokeStyle`. Instrument a decode sentinel or use a deliberately undecodable
v5 payload to demonstrate the ordering; a generic unknown-key failure is not proof of the version refusal gate.
Retain the earlier reader harnesses and their inputs. The current-reader future sentinel advances from v5 to v6;
archive the old v5 refusal expectation with the v4 harness rather than deleting historical compatibility evidence.
Use the fixture harness's actual extension/naming conventions if different, but retain the `future_v6` identity in its index.
Freeze matching native PDF/export goldens in the PDF suite; verify native save and pure export remain distinct.

The implementation PR must update `docs/reference/VRS_FORMAT.md` §§5, 6 (new migration subsection), 7–9 and 12–13:
version row, exact keys/defaults/enum strings/limits, migration, refusals, fixture index and old-reader evidence.
This docs-only assignment deliberately does not edit that fourth document or create fixtures, NOTICE or code.

## Number allocation and Bridge independence

**The first merged writer takes v5.** The approved schedule expects Phase 2 stroke to be that writer; a document cannot
reserve a number independently of a merged, fully supported schema. If another writer lands first, rebase this amendment,
the migration name, stamps, fixtures and future-version sentinel together before stroke merges. Never ship two v5 schemas.

This amendment resolves the historical claims in ADR-0010 (text, including ADR-0008's v4 amendment assigning it “v5”)
and `COLOR_PICKER_V3.md` §3 (“v5 gradients”): read both as **“next format bump at that writer's merge”**.
They do not share stroke's v5. PLAN currently places gradients later and text last; schedule numbers are conditional.
Moderator should propagate these cross-notes when those documents are next edited; this amendment is the scoped clarification.

Persisted format 5 is independent of Bridge API 1.2 and ADR-0011's connection protocol. Stroke DTOs require opt-in
`api:"1.2"`; API 1.0/1.1 fixtures/defaults remain frozen. No protocol version is inferred from the open file's version.

## Review boundary

The 29-kind subset needs moderator confirmation because pinned VectorCraft declares 40; see the explicit list in the work order.
No implementation, fixture generation, target compilation or owner hand test is claimed by this amendment.
