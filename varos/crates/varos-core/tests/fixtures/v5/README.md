# Format 5 stroke corpus

43 editable JSON/native PDF/SVG triples, frozen by the explicit offline `varos-pdf` example. Tests never bless fixtures. `head_library.json` freezes all 29 original head outlines and shaft insets. SHA256SUMS covers the payloads and index. Plain object serialization omits the default style; historical v4 fixtures and Bridge fixtures remain unchanged. Raster tests compare every case (including the dashed inside-aligned compound hole) to SVG; GPU tessellation tests compare coverage at a sample lattice without constructing a renderer.
