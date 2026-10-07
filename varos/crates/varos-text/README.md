# varos-text

Pure, byte-fed layout leaf: no Varos, UI, GPU, filesystem or discovery dependencies.
Construct `FontFace` values from immutable `Arc<[u8]>` bytes with the font's exact
family and weight (400/500/600), then a `FontSet` and ordered `FallbackPolicy`.
`Engine::new` rejects invalid faces, collections and metadata mismatches.
`FaceId` is an index in that snapshot; `Script` parses an ISO 15924 tag.
COSMIC/HarfRust types remain internal. Source and caret offsets are UTF-8 bytes.

`Engine::layout` returns glyphs, levels, logical ranges, affinity carets and ink
bounds. `outlines::glyph_outline` emits exact cubic Y-down geometry with native
winding. `elide`, visual/word/home/end caret movement and cluster selection
rectangles are headless helpers; the host owns input, IME, atlas and rendering.
`LabelCache::new(&engine)` binds to its immutable font/fallback snapshot and caps
retained payload at 4,096 entries / 4 MiB; expiration runs at most once per frame from `layout(frame)`.
The host can also call `expire(frame)` on frames without layout calls.
Payload excludes HashMap/allocator overhead and caller-held Arcs. Engine shaping
and incremental paragraph caches have separate limits inherited from P1c.

`incremental::Incremental::new(fonts, byte_limit)` promotes P1c's convergent layout
as prepared T1 work, pending owner acceptance of ADR-0012. Its 1 MiB cancellation, temporary-memory and hostile-font
bounds remain open; promotion does not authorize canvas P2 or T4.
Tests supply bundled fonts through `include_bytes!`; production embeds none.
The spike remains standalone for historical proofs/stencil/export/parity runners.

Single short budget run, from `varos/`:
`cargo run --offline -j 2 --release -p varos-text --example ui_budget`
The warm measurement includes lookup and CPU quad positions, not a textured mesh.
