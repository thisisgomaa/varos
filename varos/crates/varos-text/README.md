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

## Paragraph programme (headless, opt-in)

`Engine::compose(&Request, &composer::ParagraphOptions)` adds paragraph-level
Greedy/EveryLine composition, seven alignment modes and optional
`Kashida::{Off,Minimal,Balanced,Display}`. Original Unicode source is immutable.
EveryLine uses bounded total-fit/fitness scoring with exact COSMIC edge reshapes;
resource exhaustion is an error, never an undocumented greedy fallback.
The ordinary/incremental `layout` APIs retain their previous policy and cache keys.

Kashida retains HarfRust SAFE_TO_INSERT_TATWEEL through the vendored ShapeGlyph,
then applies BStudio joining/mark/lam-alef guards, preference order, per-word caps,
a 64-candidate reshape budget and original↔working byte maps. Minimal allocates
up to 25% of slack and one insertion/word; Balanced 60% and two; Display 100% and
four. Remaining slack goes to ordinary internal spaces. An indivisible or
unstretchable line reports `Issue::JustificationResidual`; widths are never faked
by scaling letters or numbers. `KashidaInserted` records virtual source positions.
This conservative Arabic table does not claim complete Persian/Urdu or ALReq
coverage, stylistic jalt cycling, or owner-approved Arabic visual quality.

`LineHeight` includes font ascender/descender/lineGap and actual outline ink;
Arabic minimum is 130%, default body request is 150%. Mixed faces share a baseline.
`FramePolicy::split` supplies widow/orphan decisions for a future frame allocator;
there is no frame/model integration. `Engine::compose_with_hyphenation` accepts an
optional `HyphenationProvider`; both composers validate word-local grapheme breaks
and measure exact edge shapes including a virtual hyphen. The glyph maps to the
preceding source grapheme; `Issue::DiscretionaryHyphen` records the break byte.
Copy/source bytes stay unchanged. No dictionary or heuristic is bundled.

`paths::layout_paths` emits exact anchor/control-point geometry, one NonZero
compound item per glyph. Hosts can map p/hin/hout to core anchors when T4 opens;
they must carry NonZero rather than discard it into the existing EvenOdd model.
No text is destructively converted unless a future explicit Create Outlines
command is invoked. Variable-axis shaping/outlining remains P7 work.

Tests include_bytes! fixtures under assets/fonts; missing files are hard failures.
`cargo run --offline --release -p varos-text -j 2 --example programme_budget`
measures 10k/100k full and 200 sequential edits plus opt-in paragraph composition.
See the worktree REPORT.md for actually executed gates and measured limitations.
