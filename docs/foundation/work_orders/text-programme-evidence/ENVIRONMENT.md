# Text programme evidence environment

2026-10-09, feat/text-programme, base 7b48f2c; no commit/push/GUI/install.
Host: aarch64-apple-darwin, rustc 1.98.1 (48a229cea 2026-09-01), LLVM 22.1.8.
CPU model/RAM query denied by sandbox; no hardware-specific speed claim.
All dependency resolution offline, compilation -j 2, from varos/.
Fonts: compile-time byte fixtures under crates/varos-text/assets/fonts; checksums
and OFL notices there. Noto mixed v2، segment explicitly falls back to Plex.

Programme matrix: 8 real strings × 2 Arabic font configurations × 3 sizes
(12/48/200) × 3 slack amounts (0.3/1/3 em) × 4 kashida policies = 576.
Legal break matrix: 10 strings × 2 composers × 3 base directions × 3 widths = 180.
Grapheme coverage: 10 strings × 3 directions with wrapped Balanced justification.
Outline fixture: 3 Arabic/mixed strings × 3 sizes, each glyph independently
rasterized with NonZero, exact bytes and absolute bounds compared after conversion.
The safe-flag oracle also extends the existing language/script/cache sweep.
No fixture skips on missing fonts; include_bytes! or metadata validation fails.

Measured every-line fixture (Plex fallback/Inter; 24 px, width 180 px):
السلام عليكم ورحمة الله، هذا نص عربي طويل لاختبار توزيع الكلمات على الأسطر بشكل متوازن وجميل.
Source byte ranges / advances: 0..24 122.688; 24..53 142.992;
53..76 140.568; 76..102 111.888; 102..124 116.184;
124..146 124.920; 146..170 132.528 (tolerance 0.001 px).

Timing example: programme_budget, release, 41 full-layout samples; 200 sequential
insert/delete edits cycling start/middle/end, including clone/edit/result drop.
Seed: السَّلَامُ عليكم، Logo شعار 12.50 موعدنا؟ ; exactly 10k/100k Unicode scalars.
Incremental final output is checked against cold layout. Existing edit regression
sweeps compare each intermediate edit against cold layout outside this timing run.
Composer timing is separate (930-scalar paragraph); no incremental composer cache.
This new marked-Arabic seed is a stress extension, not an apples-to-apples speedup
against the original P1 Logo شعار 12 corpus. Timings are engine-only, not app latency.
WASM check is build-only: no runtime parity or owner visual acceptance claimed.
