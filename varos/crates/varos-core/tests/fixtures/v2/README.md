# Frozen S5-E compatibility fixtures — 2026-09-27

These bytes were frozen once from the v2 writer at `6b6f41e48bf02313ae6aba886cf811070b8bd22e` (S5-D),
using locked lopdf 0.43.0. Never regenerate them with a newer writer. Add a new era instead.
The temporary Rust integration generator was removed after creating the files; tests only read
these committed bytes. `SHA256SUMS` records the initial bytes, including the PDF containers.
Verify from this folder with `shasum -a 256 -c SHA256SUMS`.

| Files | Construction and meaning |
|---|---|
| `v2_masked_rotated.vrs`, `v2_masked_rotated_pdf.vrs` | Load the unchanged `v1/v1_masked.vrs`; set Clip node 15's Xform to `rot: 0.5, piv: [30,30]` with `set_node_xform`. Replace artboards with two default-based 100×100 boards: first at (0,0), second at (120,0) named `لوحة ثانية`. Serialize through `doc_to_blob` and `write_pdf`. Retains donut path 10, its four-anchor hole, mask path 11 / node 14, children [14,13]. |
| `v2_boardless.vrs`, `v2_boardless_pdf.vrs` | Load unchanged `v1/v1_boardless.vrs` through migration, then serialize with the same two APIs. Explicit empty artboards must survive. |
| `v1_broken_mask.vrs` | Synthetic **v1** repair example frozen with this S5-E cohort, not an original historical writer output. Copy the masked/rotated raw fixture, stamp `varos:1` and set node 15's `mask_child` to nonexistent 999, using compact UTF-8 JSON. Only the Clip role/reference may be released, with a notice; original bytes remain unchanged. |

`varos-core/tests/golden.rs` includes all three raw examples in the full round-trip law.
`varos-pdf/tests/frozen_v2.rs` checks exact raw/PDF twins, authored mask/hole/rotation/board
values, byte identity against frozen v2, stable subsequent saves, repair notice and limits.
`old_reader_harness.rs` checks fresh and frozen v2 refusal using the old header gate.
These are headless checks; they do not establish rendered appearance or old-application behavior.

Fixture-local `.gitattributes` disables line-ending conversion for every `.vrs` and marks
PDF containers binary. Their xref offsets and byte identity depend on preserving exact whitespace.
