# Arabic copy-back fixtures

`../arabic_copy_back.rs` generates these PDFs from authored text and the existing
OFL Noto Sans Arabic fixture in `crates/text-arabic/fixtures`. No new font binaries
or reader-generated PDFs are checked in.

```sh
# Poppler's pdftotext and pdftoppm must be on PATH (Debian/Ubuntu: poppler-utils).
REQUIRE_PDFTOTEXT=1 ARABIC_PDF_FIXTURES=/tmp/arabic-pdf \
  cargo test -p vectorcraft-pdf --test arabic_copy_back -- --nocapture
pdftotext /tmp/arabic-pdf/ligature.pdf -
pdftotext /tmp/arabic-pdf/force-stroked.pdf -
```

- `ligature.pdf`: `بسم الله الرحمن الرحيم`, including a multi-codepoint Allah glyph.
- `force-stroked.pdf`: `بسم الرحمن الرحيم`, Force 100, 0.25 pt character stroke.
- `modes-*.pdf`: Off/Auto/Force at strengths 0/50/100, with both the Allah ligature
  and an explicitly authored U+0640 in `بسـم`. The authored tatweel must survive;
  virtual elongation must never appear in the searchable text.
- `force-unstroked.pdf`: the same logical copy-back with a fill and no stroke.
- `digits.pdf`, `repeated.pdf`, `latin.pdf`: authored digits drawn as Hindi digits,
  repeated ligatures and unchanged Latin text.
- `auto-area.pdf`: justified area type (not point type), Auto at strength 100, wide
  enough that virtual kashida is inserted. Copy-back is the authored string; the
  tatweels are not in it. The test checks that non-space glyph advances grew.
- `mixed.pdf`: one paragraph, `مرحبا (Office), 2026. سنة ٠١٢٣`, Arabic in Noto Sans
  Arabic and the Latin run (parentheses, comma, period, ASCII digits) in Source Sans 3,
  then Arabic-Indic digits.
- `persian-zwnj.pdf`: `کتاب‌ها بالا` (ZWNJ, and a lam-alef ligature) in Noto Sans Arabic.
- Hebrew niqqud is not generated here. None of the committed fixture fonts (Noto Sans
  Arabic, IBM Plex Sans Arabic, Source Sans 3, Source Serif 4, Inter, JetBrains Mono)
  cover Hebrew. The test skips that case and says so; it does not use an installed
  system font.
- `latin-frozen.pdf`: `Hello, world. (2026)` in Source Sans 3, creation date pinned to
  2020-01-01 UTC. The writer has no switch for the Arabic mapping. The test locks the
  PDF's sha256. On failure it prints the new digest; paste it into `LATIN_ONLY_SHA256`
  in `arabic_copy_back.rs` and rerun:

```sh
CARGO_TARGET_DIR="~/Documents/AI workspace/lanes/_target-shared" \
  cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \
  --test-threads=2 latin_only_pdf_bytes_stay_frozen --nocapture
```
- `aat-*.pdf`: the same two cases for installed morx fonts (Geeza Pro, Al Nile,
  Baghdad, Damascus, KufiStandardGK, Diwan Thuluth). The test checks actual morx
  support, no fallback in the Force case, and actual virtual elongation. It reports
  how many fonts ran; none were available in the Linux cloud environment (all 557
  Fontconfig font files were inspected; none contained a morx/mort table). A face
  whose shaped outlines the reader will not accept as that face (it warns that the
  glyphs differ from the installed font and keeps paths) is skipped with a visible
  note: the PDF still has a text layer, but this reader does not return it.

Both the crate reader and Poppler must return the authored string. The Poppler
checks run in default and `-layout` modes, stripping only Poppler's bidi embedding
controls and page/line terminators, never internal spaces or tatweels. Poppler is
optional in ordinary crate builds: `which pdftotext` decides. When it is absent the
external assertions are skipped and the test prints `SKIP pdftotext`; they are not
reported as passed. `REQUIRE_PDFTOTEXT=1` makes absence an error.
The rendering test also compares Poppler pixels against an outline-only export:
adding the searchable layer must leave the painted stroked word identical.

## Root causes and measured before/after

Base: `aa877ed7` (main), Poppler 25.03.0. The original fixture's x=500 placed part
of its text outside the 600 pt page; both before and after were measured at x=50.

The writer emitted glyphs in visual order, but multi-codepoint RTL mappings were
still logical inside each glyph. Applying bidi to the extracted line reversed the
ligature's letters a second time. The writer now supplies visual codepoint order
inside RTL clusters as well, for both ToUnicode and the backend's ActualText
fallback. Glyph IDs, positions and painted outlines stay the same.

The unioned Force outlines retain glyph metadata, but virtual tatweels have no
source characters and are omitted from hidden text. PDF font widths still covered
only the original letter, so the jump across the omitted tatweels looked like a
word gap. Only the preceding hidden glyph's horizontal matrix is expanded to cover
that advance; the outline, stroke and authored text are unchanged.

`pdftotext FILE -` below has only bidi controls and trailing page whitespace removed;
`\n` denotes an actual newline, including the spurious paragraph breaks before the fix:

| Fixture | Before | After |
| --- | --- | --- |
| Ligature | `بسم هللا الرحمن الرحيم` | `بسم الله الرحمن الرحيم` |
| Force stroke | `بس\n\nم الرحم\n\nن الرحي\n\nم` | `بسم الرحمن الرحيم` |

The crate reader before: `بسم هللا الرحمن الرحيم` and `بس م الرحم ن الرحي م`.
After: exactly the authored strings. Poppler raster bytes at 1200 px matched before
and after for each fixture. Preview and Acrobat were not run in this Linux environment.

## Separate pre-existing limitation

An exploratory fully vocalized case, `بِسْمِ الرَّحْمَنِ الرَّحِيمِ`, still has duplicated
base/mark clusters in Poppler. The same case was reproduced against the unmodified
`aa877ed7` writer. That many-glyph/mark extraction issue is separate from the two
regressions above and is not claimed fixed here.
