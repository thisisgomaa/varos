# Frozen v1 corpus (DFS S5-E)

Every file in this folder was produced by the **pre-S5, format-1 writer** at commit
`ecf67f5` ("docs: record owner decisions D1–D3 on the Document & File System spec
(2026-09-24)") on `claude/sweet-cerf-1sg30t`, using `varos_core::file::save_vrs`
(raw JSON) and `varos_pdf::save_vrs` (PDF container). At that commit `model.rs`,
`file.rs` and `varos-pdf/src/lib.rs` are byte-identical to their state at the S5
work order's base commit (`bd24627`) — no schema or writer code changed between
the two, only unrelated editor/clipboard work.

**These files are never regenerated.** They pin the exact bytes format 1 wrote for
each scenario below, so ADR-0008's v1→v2 migration and every future format change
has real, frozen input to prove it still reads old files. A new file era gets a new
fixture; existing ones are never touched or re-saved.

Generation was a throwaway test (`varos-pdf/tests/zz_gen_v1_fixtures.rs`), run once
against a temporary `git checkout ecf67f5 -- varos/crates/varos-core varos/crates/varos-pdf`
in the S5-E worktree, with its output copied here and the checkout then discarded
(`git reset --hard`) — the generator itself was never committed. See
`docs/reference/VRS_FORMAT.md` §Fixtures for the wire-contract-level description.

## Scenarios (raw JSON `v1_<name>.vrs` + PDF container `v1_<name>_pdf.vrs`)

| name | file pair | exercises |
|---|---|---|
| `plain` | `v1_plain.vrs` / `v1_plain_pdf.vrs` | one filled+stroked triangle on a single, clipping artboard — the ordinary save |
| `boardless` | `v1_boardless.vrs` / `v1_boardless_pdf.vrs` | explicit empty `artboards` (a free canvas, no page at all) |
| `masked` | `v1_masked.vrs` / `v1_masked_pdf.vrs` | a donut (outer ring + one hole) clipped by a second shape — `Path.holes` and `GroupRole::Clip`/`mask_child` together |
| `rotated` | `v1_rotated.vrs` / `v1_rotated_pdf.vrs` | a leaf path's node carries a live, non-identity `Node.xform` (A7 rotation) |
| `translucent` | `v1_translucent.vrs` / `v1_translucent_pdf.vrs` | fill alpha 0.4, stroke alpha 0.5, path `opacity` 0.7 — alpha compositing paths |
| `two_artboards` | `v1_two_artboards.vrs` / `v1_two_artboards_pdf.vrs` | a white clipping page plus a transparent, non-clipping second page |
| `guides_snap` | `v1_guides_snap.vrs` / `v1_guides_snap_pdf.vrs` | non-default ruler guides, `ruler_origin`, `guides_locked`, and a non-default `SnapConfig` |
| `unicode_arabic` | `v1_unicode_arabic.vrs` / `v1_unicode_arabic_pdf.vrs` | Arabic (RTL) text in a path name and an artboard name |

Every raw file starts `{"varos":1,…}`; every PDF container repeats the version as
the catalog's `/VAROS_SchemaVersion 1` and embeds the identical model blob as
`model.varos.json`.
