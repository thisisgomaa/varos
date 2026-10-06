# Frozen format-4 fixtures — 2026-10-07

Format 4 adds a stable `id` to every artboard (`doc.artboards[].id`; ADR-0008 amendment 2026-10-07,
`docs/reference/VRS_FORMAT.md`). These bytes were frozen once from the first v4 writer (Bridge slice 3,
branch `feat/bridge-slice3-artboards` on top of `a5f687b`), using locked lopdf 0.43.0, by a temporary
Rust integration generator in `varos-pdf/tests/` that was deleted right after it ran. Never regenerate
them with a newer writer; add a new era instead. `SHA256SUMS` records the bytes
(`shasum -a 256 -c SHA256SUMS` from this folder).

| Files | Construction and meaning |
|---|---|
| `v4_board_meta.vrs`, `v4_board_meta_pdf.vrs` | Load the unchanged `v3/v3_board_meta.vrs` (v3→v4 migration), serialize through `doc_to_blob` and `write_pdf`. The two artboards get ids **16** and **17** in artboard order (the v3 counter was 15), the counter becomes 17, `active` stays 0. Everything else equals the v3 source. |
| `v4_boardless.vrs`, `v4_boardless_pdf.vrs` | Load the unchanged `v3/v3_boardless.vrs` through the v3→v4 migration and serialize with the same two APIs: no artboards, so no ids are allocated and the counter is unchanged (5). |

`varos-pdf/tests/frozen_v2.rs` checks the raw/PDF twins are equal, byte-stable on save, that the v3
twins migrate to exactly these bytes, and the ids above. `varos-core/tests/golden.rs` includes the raw
files in the round-trip law. `old_reader_harness.rs` proves the v1-, v2- and v3-era version gates
refuse these files before any typed decode.

Fixture-local `.gitattributes` disables line-ending conversion for every `.vrs` and marks
PDF containers binary. Their xref offsets and byte identity depend on preserving exact whitespace.
