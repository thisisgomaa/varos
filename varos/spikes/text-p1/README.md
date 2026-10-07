# Text P1b/P1c — headless, excluded workspace

This is an Amendment-1 experiment, **not product code or P2 approval**. Run every
Cargo command offline from this directory. No production renderer/model changes.

```sh
cargo --offline test --release --no-fail-fast
cargo --offline clippy --all-targets -- -D warnings
cargo --offline build --release
./target/release/varos-text-spike proofs /private/tmp/text-p1b
./target/release/varos-text-spike edits 10000 long middle          # optional 5th arg: motif|words|arabic
./target/release/varos-text-spike corpus-dump /private/tmp/text-p1c  # golden layouts, cold + cached path
./target/release/varos-text-spike cap-probe                          # 1 MiB slice/cache probe
python3 scripts/benchmarks.py /private/tmp/text-p1b                  # VAROS_SPIKE_BIN=… for another build
cargo --offline build --release --example control_bench
python3 scripts/control_benchmarks.py /private/tmp/text-p1b          # VAROS_CONTROL_BIN=… likewise
python3 scripts/harness.py /private/tmp/text-p1b/harnesses
python3 scripts/checks.py /private/tmp/text-p1b
```

The native suite is green, including legal wrapping and native NonZero. There are
no ignored or expected-failure acceptance tests. Architectural/resource/review
exit gates remain open; green unit tests do not authorize P2. `proofs` saves every
artifact and returns failure if the unchanged alpha limits fail. Failed conversion
and single-sample NonZero measurements remain historical, **not acceptance**.
See [P1b report](../../../docs/foundation/work_orders/TEXT_P1B_RESULTS.md).

## Patch and engine contracts

- COSMIC 0.19.0 is copied intact from the local registry into `vendor/cosmic-text` (gitignored; rebuild it with `vendor/reproduce.sh`, which copies the pristine registry source and applies the patch).
  `vendor/BASE.md`, `pristine.sha256`, and `cosmic-text-0.19.0-p1b.patch` record the
  base, archive/directory hashes, and complete diff. Licences remain with it.
- `std` + `shape-run-cache`, defaults off. `system-discovery` separately enables
  locale, memmap and automatic discovery; upstream defaults opt in. Byte-fed mode
  rejects file sources even if resvg unifies fontdb fs/memmap features.
- Borrowed/owned attributes carry language/script; compatibility, cache identity,
  fallback buffers and plans preserve them. Reused buffers reset properties.
  Context includes five surrounding scalars, matching HarfRust 0.5.2's bound.
  The adapter itemizes strong scripts once per paragraph; Common/Inherited
  graphemes inherit the preceding strong script, or the first following one at
  paragraph start. Explicit overrides stay local. This policy is fixture-tested,
  not a full Script_Extensions/paired-punctuation conformance claim. Paint does not split shaping. Direct HarfRust is a **test oracle**.
- Requests support global language/script and `language_runs`. Language syntax is
  a bounded BCP-47 subset (primary 2–8 ASCII letters, following 1–8 alphanumerics,
  max 63 bytes); grandfathered/private-use-only forms are not implemented.
- Pinned Plex has a real Urdu `locl`: GSUB arab/URD lookup 6 maps uni06F6→uni0666.
  Inter/Plex hashes and OFL notices remain unchanged. The vendored OFL Noto Sans
  Arabic fixture independently tests real Arabic font-boundary context.
- Full-paragraph UAX #14 opportunities define indivisible fitting units across
  bidi/style/font fragments. Logical fitting backtracks to a legal opportunity,
  overflows whole units, and preserves source ranges including hard-break bytes.
  The legacy unbounded Word workaround stays; no emergency glyph breaks.
  HarfRust unsafe-to-break flags and conservative non-space boundaries trigger
  reshaping with context clipped to the actual line, then forward/backward refit.
  Legal opportunities inside old clusters remain available through reshaping.
  ZWSP Arabic fixtures demonstrate real form changes and agree with direct shaping.
  Cluster-order reversal preserves mark/base order, checked by positioned oracles.
- Per-line UBA L1 uses a line-local snapshot of upstream resolved classes/levels,
  avoiding repeated full-paragraph clones. L2 still reorders whole clusters.
- The incremental prototype caches paragraphs with stable identities/revisions.
  Font bytes are compile-time immutable. Unchanged paragraphs are not reshaped;
  paint does not invalidate. **P1c (`src/converge.rs`)**: each paragraph keeps a
  text-revision *analysis* (full-paragraph UBA classes/levels, grapheme boundaries,
  scripts, UAX #14 opportunities, cut-segment attributes, legal fitting units with
  shaped glyphs) that survives width/alignment changes, plus fitted lines. An edit
  recomputes the O(n) Unicode analyses exactly, diffs them against the previous
  revision (levels may change anywhere), and reshapes only level runs touching the
  difference ± HarfRust's five-scalar context, closed over segment/run/unit
  boundaries. Fitting (a port of the patched `layout_with_breaks`) restarts at the
  first line whose inputs or one-unit look-ahead touch that window and stops when a
  new line start equals a cached one beyond it; later lines are reused shifted.
  Base-direction flips, settings changes and unmappable style/language ranges
  rebuild the analysis. Several UBA paragraphs inside one UAX #14 paragraph fall back
  to the unchanged cold engine. Cached output equals the cold engine (randomised
  edit tests, corpus goldens); counters (`Counters::work`) prove runs/lines reused.
  A periodic text can legitimately cascade line breaks to the paragraph end.
  Cached paragraph analyses/lines are not mutated before commit, so cancellation/
  refusal preserve the published source and cache; content-keyed memo caches (attribute
  interner, span/shape-run caches, font system) are still written, which is benign.
  `cancel` is polled inside analysis, shaping, segment building,
  per fitted line, output and merge; labelled poll stretches double as a stage
  profile. Cache counters are retained-payload estimates; RSS is separate.
- A separate resolved-bidi-span cache reuses shapes with identical text, attributes,
  direction and five-scalar surrounding context. It does not split arbitrary words.
  Shape-run and adapter-span caches each cap at 4,096 entries / 8MiB payload;
  outline cache ≤16,384 entries; paragraph cache ≤32MiB payload; text cap 1MiB.
  Fixed Unicode coverage bitsets avoid per-character tree lookup. Counters include
  their storage. A 100k-scalar motif paragraph's P1c state is ≈26MB, so paragraphs
  well above ~120k scalars exceed the 32MiB bound and relayout cold every edit.
  Transient heap per edit is measured (counting allocator in the benchmark binary),
  not capped. Hostile-font work is not addressed: fonts are fixed compile-time bytes.

## NonZero and runtime evidence

`proof::nonzero_coverage` uses tiny-skia Winding independently per glyph, combines
subpixel coverage by max, box-filters 4×4 samples to each output pixel, and applies
alpha once. Opposite font orientations cannot cancel. Exact-cubic vs 0.005pt-flat
Winding now passes the original mean ≤1 / zero pixels above 32 limits at 2×.
The original single-sample renderer is retained for diagnostics; tightening its
flatten tolerance alone did not fix its AA discrepancy. Intermediate rasters cap
at 64M pixels (4M output pixels with this sampling policy); refusal is explicit.
The CPU stencil model has separate winding storage, increment/decrement, cover,
clear, preserved clip bits and explicit refusal before unsafe ±256 winding.
This is neither GPU pixel proof nor a complete production coverage design.

PDF (`f`) and SVG (`fill-rule="nonzero"`) prototypes retain independent exact cubic
glyph paths. Resvg and headless CoreGraphics render them; their alpha comparison
uses the same 4×4 sampling/filter and fixed tolerance. Render at 8× using the
optional scale argument to `scripts/render_pdf.swift` / `scripts/render_svg.rs`,
then use `varos-text-spike downsample-4x input.png output.png` for 2× output.
This headless quality prototype has measured extra CPU cost, not a GPU or
interactive draw guarantee. See measurements for limits and preserved failures.

`parity::fixtures` runs the recorded native/WASM configurations with the same font bytes.
Build the WASM cdylib with `cargo --offline build --release --lib --target
wasm32-unknown-unknown`, then run `scripts/parity.mjs` using Node. Compare with
`scripts/compare_parity.py`: exact identities, positions ≤0.001pt, alpha mean ≤1
and no delta >32 at 2x. No browser/GUI or host imports are required.

Original P1's 10 rows / 23 strings and 41-sample benchmark are retained. The P1 raw
414 configurations / 611 line comparisons are copied/checksummed in the P1b evidence
folder before new output. Updated wrapping can change line count, not input identity.
Unicode remains bidi 16 / linebreak 15 / segmentation 17, not unified conformance.

## T1 promotion (2026-10-07)

The evidence harness stays standalone on its original adapter and patched vendor.
Production shaping, convergence/incremental layout, caret maps and exact cubic
outlines now live in `../../crates/varos-text`; migrated tests use byte-fed fixture
FontSets. Proof/stencil/export/parity scripts and benchmarks stay here. Production
changes must be tested in the main workspace; spike results do not certify them.
Incremental code is prepared for T1, pending owner acceptance of ADR-0012;
the owner verbally selected “option 2 = path B” on 2026-10-07, but acceptance
is not recorded and no T1 work-order file exists. The T4 deferral is not formally overridden;
its 1 MiB cancellation, temporary-memory and hostile-font limits remain open.
