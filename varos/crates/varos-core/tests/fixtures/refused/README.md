# Frozen refusal inputs — 2026-09-27

Synthetic damaged/unsupported files, frozen once on `6b6f41e48bf02313ae6aba886cf811070b8bd22e`.
Never open these as the personal-file acceptance corpus or regenerate them in tests.
They are deliberately invalid; tests must preserve the refusal, not repair their bytes.
`SHA256SUMS` records the frozen bytes (`shasum -a 256 -c SHA256SUMS` from this folder).

Raw mutations start with `../v2/v2_masked_rotated.vrs`, using compact UTF-8 JSON.
PDF mutations start with its PDF twin, parsed and saved once with pinned lopdf 0.43.0.
All original v1 fixtures and their hashes remain untouched.

| Fixture | Exact mutation / expected refusal |
|---|---|
| `v3_future.vrs` | `{"varos":3,"doc":42}`; NewerVersion before typed Document decoding. |
| `missing_version.vrs` | `{"doc":42}`; MissingVersion. |
| `zero_version.vrs` | `{"varos":0,"doc":42}`; InvalidVersion. |
| `unknown_field.vrs` | `doc.future_feature=true`; Malformed with unknown field detail. |
| `cycle_nodes.vrs` | Node 1 parent=1, append child 1, empty roots; Cycle at 1 (one owner, so no earlier ownership error). |
| `duplicate_id.vrs` | Second path's id=10; DuplicateId(path,10). |
| `id_overflow.vrs` | doc.ids=4294967295; IdExhausted. |
| `opacity_range.vrs` | Path 10 opacity=2; OutOfRange naming path 10 / opacity. |
| `nonfinite_number.vrs` | Path 10 opacity=1e100, finite JSON f64 but non-finite as model f32; NonFinite naming path 10 / opacity. |
| `broken_mask.vrs` | Clip node 15 mask_child=999; Dangling at node 15. v2 never uses legacy repair. |
| `nonchild_mask.vrs` | Clip node 15 mask_child=1 (existing nonchild); BadMask at group 15. |
| `tree_depth.vrs` | Replace nodes with 65 nested copies of Layer 1, ids 1..65, linked parents/children; paths empty, ids=65; TreeDepth found 65 / max 64. |
| `version_mismatch.vrs` | PDF catalog version=1, model remains 2; VersionMismatch. |
| `filtered_model.vrs` | Add /Filter /FlateDecode to uncompressed model; UnsupportedPdf(model encoding), never invoke a decompressor. |
| `incremental.vrs` | Trailer /Prev 0; UnsupportedPdf before lopdf. This is a trailer-key refusal probe, not a real revision chain. |
| `xref_stream.vrs` | Save with lopdf CrossReferenceStream; UnsupportedPdf before lopdf loading. |
| `missing_model.vrs` | Remove catalog VAROS_Model and Names; NoEmbeddedModel. |
| `future_pdf.vrs` | Catalog version=3; model bytes `{"varos":3,"doc":42}`; NewerVersion. |

`v3_future.vrs` and `future_pdf.vrs` share the same NewerVersion refusal on purpose: one is the raw
JSON model, the other the PDF container, so both entry shapes prove the version gate. `incremental`
and `xref_stream` are pinned to their exact reasons ("incremental update", "compressed
cross-reference"), and `frozen_fixture_bytes_match_their_sha256sums` enforces `SHA256SUMS`
(2026-10-04 batch review fixes, A2/A3).

## Addendum — format 3 (2026-10-04, board metadata)

The eighteen files above are unchanged. Format 3 became current, so `v3_future.vrs` and
`future_pdf.vrs` are no longer "newer": both now pass the version gate and are refused by the typed
decode (`"doc":42` is not a document → Malformed). Their bytes stay frozen; only the expected
reason changed. The NewerVersion proof moved to two new files, and two format-3 refusals were added
(appended to `SHA256SUMS`; no existing line changed):

| Fixture | Exact mutation / expected refusal |
|---|---|
| `v4_future.vrs` | `{"varos":4,"doc":42}`; NewerVersion { found 4, supported 3 } before typed decoding. |
| `future_v4_pdf.vrs` | `future_pdf.vrs` with its two `3` bytes (catalog `/VAROS_SchemaVersion` and model `"varos"`) changed to `4`, same length so xref offsets hold; NewerVersion { 4, 3 }. |
| `v2_board_name.vrs` | `../v2/v2_masked_rotated.vrs` with `"name":"Logo",` inserted first in `doc`; a format-2 file carrying a format-3 key → `Invalid::FieldNotInFormat { field: "name", version: 2 }`. |
| `board_duplicate_tag.vrs` | `../v3/v3_board_meta.vrs` with tags `["client","Client"]`; `Invalid::Board(DuplicateTag "Client")` — tags are deduplicated case-insensitively, so a writer never emits this. |

`varos-pdf/tests/frozen_v2.rs` asserts typed errors through both bytes and disk APIs and
checks that originals are unchanged. Oversize cases use these small positive fixtures with
lowered Limits instead of committing huge files. Broader generated hostile-input coverage
remains in `container_bounds.rs`, `format_v2.rs` and `format_validate.rs`.

Fixture-local `.gitattributes` disables line-ending conversion for every `.vrs` and marks
PDF containers binary. Their xref offsets and byte identity depend on preserving exact whitespace.
