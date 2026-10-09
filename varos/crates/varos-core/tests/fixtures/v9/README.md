Frozen mixed format-9 fixture (integration w2, 2026-10-09). One document carries every wave-2
model addition: a placed embedded image (`images`/`assets`, format 6), a gradient fill and a
global swatch (`swatches`, format 7), an editable text box (`text_boxes`, format 8) and Live
Corners (`paths[].corners`, format 9). `mixed.json` pins the combined `Document` key order
(`images, assets, swatches, text_boxes, paths, …`); `mixed.vrs` is the native container written by
`varos_pdf::images::write_vrs`. Generated once by
`cargo run --offline -j 3 -p varos-pdf --example freeze_mixed_v9`; tests never regenerate it.
Verified by `varos-pdf/tests/format_v9.rs`. `SHA256SUMS` covers both files
(`shasum -a 256 -c SHA256SUMS` from this folder).
