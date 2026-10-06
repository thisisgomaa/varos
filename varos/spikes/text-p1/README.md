# Text P1 — throwaway engine spike

**Not product code. Do not promote to P2.** This crate measures the accepted
ADR-0010 candidate and deliberately reports unsupported behavior. It introduces
no TextBox schema, app renderer, UI, system-font scan or production text editor.

## Reproduce (from `varos/`, offline only)

```sh
cd spikes/text-p1 && cargo test --offline
cd spikes/text-p1 && cargo build --offline --release
cargo tree --offline --workspace -e features -i cosmic-text
./target/release/varos-text-spike proofs /private/tmp/text-p1
python3 spikes/text-p1/scripts/measure.py ./target/release/varos-text-spike 1000
python3 spikes/text-p1/scripts/measure.py ./target/release/varos-text-spike 10000
python3 spikes/text-p1/scripts/measure.py ./target/release/varos-text-spike 100000
```

The proof command writes all six sheets, metrics and diffs **then returns a
failure** if the declared winding tolerance is exceeded. The acceptance suite currently has two red tests (legal breaks and winding);
they are not ignored or annotated as expected failures. See
[`TEXT_P1_RESULTS.md`](../../../docs/foundation/work_orders/TEXT_P1_RESULTS.md)
and the generated `measurements.md` for the actual verdict.

## Boundary and adapters

- COSMIC 0.19.0 defaults off / `no_std`; bundled bytes into fontdb 0.23.0 only.
  Exact direct pins and the workspace lockfile are the reproducibility contract.
  `FontSystem::new_with_locale_and_db` sets a fixed locale; this **does not** set
  the HarfRust language. Non-`und` requests return `UnsupportedLanguage`.
- Explicit paragraph base direction uses unicode-bidi plus public COSMIC
  `ShapeSpan`/`ShapeLine` APIs. No controls are inserted into stored source.
  Per-line UBA L1/L2 reordering moves whole shaped clusters, preserving offsets.
  Point text uses `Wrap::Word` with no width: COSMIC's `Wrap::None` path dropped
  an opposing-direction numeric run in the initial probe.
- Fallback resolves a complete whitespace-delimited segment when possible,
  including punctuation. It preserves base/mark and emoji ZWJ graphemes. True
  typographic style boundaries can divide a segment. Unsupported coverage is an
  explicit byte-range diagnostic; monochrome .notdef remains visible.
- Paint never becomes a shaping boundary. Size/face affect shaping; baseline
  shift affects placement and line extents. Native OpenType features are passed
  through; the spike refuses disabling required joining features.
- Word wrapping uses legal engine break opportunities; unbreakable words can
  overflow, with a diagnostic. Intra-word Arabic line-edge reshaping is unproven.
- Carets partition ligature advance at grapheme boundaries (no GDEF extraction).
  A hit returns **all coincident candidates**. Affinity/traversal identity must
  survive host hit testing; x alone cannot identify coincident bidi/control stops.
  This is not an Illustrator-validated editing policy.
- Skrifa 0.42.1 produces unhinted exact cubics in Y-down points. Ink bounds are
  conservative control bounds cached by face/glyph/size, separate from line boxes.
  i_overlay 7.0.2 resolves flattened nonzero regions into even-odd contours.
  Conversion stays in the spike: no core/PDF fill semantics were changed.

## What is tested / what is not

Ten named corpus tests execute decoded strings at 12/48/200 pt with point text
and 120/600 pt widths. Additional tests cover mixed sizes/baseline shift,
rotation, invalid fonts/metrics/styles, byte limits, overlapping contours,
feature control and missing-font reporting. The proof matrix also runs both
requested faces. Dumps retain glyph IDs, advances, offsets, byte ranges, bidi
levels, carets and ink bounds. Images are CPU artifacts, not GUI screenshots.

The tiny style-edit model tests logical replacement/undo and refuses edits that
merge grapheme/style boundaries. It is not a production edit engine. Broader
IME, arbitrary font resources, cancellation, bounded caches, hostile fonts,
variable/CFF fonts, a contrasting Arabic fixture, PDF equivalence and WASM
runtime parity remain unproved. WASM check: **not run (target not installed)**.

Fonts use `include_bytes!` paths into `varos-app/assets/fonts`; nothing is copied.
Read that directory's README/manifest and original OFL notices. COSMIC and
Skrifa currently resolve different Skrifa/read-fonts versions; the feature dump
records that cost. Unicode data: bidi 16.0, linebreak 15.0, segmentation 17.0;
this is a recorded mismatch, not a single-version conformance claim.
