# Frozen format-3 fixtures — 2026-10-04

Format 3 adds board metadata (`doc.name`, `doc.description`, `doc.tags`; ADR-0008 amendment
2026-10-04, `docs/reference/VRS_FORMAT.md`). These bytes were frozen once from the first v3 writer
(Start v2 lane L2, on top of `f21c20e`), using locked lopdf 0.43.0, by a temporary Rust integration
generator that was deleted right after it ran. Never regenerate them with a newer writer; add a new
era instead. `SHA256SUMS` records the bytes (`shasum -a 256 -c SHA256SUMS` from this folder).

| Files | Construction and meaning |
|---|---|
| `v3_board_meta.vrs`, `v3_board_meta_pdf.vrs` | Load the unchanged `v2/v2_masked_rotated.vrs` (v2→v3 migration), then set `name` = `شعار المقهى — Café logo`, `description` = `Brand mark, round two. نسخة ثانية للشعار.`, `tags` = `["client","عربي","logo"]`. Serialize through `doc_to_blob` and `write_pdf`. Arabic + Latin metadata stored as UTF-8 (no NFC step). |
| `v3_boardless.vrs`, `v3_boardless_pdf.vrs` | Load the unchanged `v2/v2_boardless.vrs` through the v2→v3 migration and serialize with the same two APIs: what a migrated v2 file saves as (empty name/description, no tags; explicit empty artboards survive). |

`varos-pdf/tests/frozen_v2.rs` checks the raw/PDF twins are equal, byte-stable on save, and that each
equals its migrated v2 source plus exactly the metadata above. `varos-core/tests/golden.rs` includes
the raw files in the round-trip law. `old_reader_harness.rs` proves both the v1-era and the v2-era
version gates refuse these files before any typed decode.

Fixture-local `.gitattributes` disables line-ending conversion for every `.vrs` and marks
PDF containers binary. Their xref offsets and byte identity depend on preserving exact whitespace.
