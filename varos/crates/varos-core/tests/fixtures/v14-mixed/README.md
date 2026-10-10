Frozen mixed format-14 fixture (integration w3, 2026-10-10). One document carries every wave-2 and
wave-3 model addition: a placed embedded image (format 6), a gradient fill with Live Corners
(formats 7/9), an appearance stack with an extra stroke entry (`paths[].stack`, format 10) and a
live ZigZag effect (`paths[].effects`, format 11) on the same path, a global CMYK swatch referenced
by another path (managed paint, format 12), a live mirrored Repeat node (`NodeKind::Live`, format
13), and editable text with named character/paragraph styles, an OpenType feature override and an
Area binding to a shape (`typography`, format 14). `mixed.json` pins the combined `Document` key
order (`images, assets, swatches, text_boxes, typography, paths, …`; path keys `stack, effects, id,
…`); `mixed.vrs` is the native container written by `varos_pdf::images::write_vrs`. Generated once by
`cargo run --offline -j 3 -p varos-pdf --example freeze_mixed_v14`; tests never regenerate it.
Verified by `varos-pdf/tests/format_v14.rs` (byte round trip, container rewrite, PDF resources,
frozen v9–v13 gates) and `varos-raster/tests/w3_mixed_v14.rs` (CPU pixels, SVG). `SHA256SUMS` covers
both files (`shasum -a 256 -c SHA256SUMS` from this folder).
