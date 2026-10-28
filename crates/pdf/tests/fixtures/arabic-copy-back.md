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
- `aat-*.pdf`: the same two cases for installed morx fonts (Geeza Pro, Al Nile,
  Baghdad, Damascus, KufiStandardGK, Diwan Thuluth). The test checks actual morx
  support, no fallback in the Force case, and actual virtual elongation. It reports
  how many fonts ran; none were available in the Linux cloud environment (all 557
  Fontconfig font files were inspected; none contained a morx/mort table).

Both the crate reader and Poppler must return the authored string. The Poppler
checks run in default and `-layout` modes, stripping only Poppler's bidi embedding
controls and page/line terminators, never internal spaces or tatweels. Poppler is
optional in ordinary crate builds; `REQUIRE_PDFTOTEXT=1` makes absence an error.
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
