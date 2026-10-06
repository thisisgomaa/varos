# Text P1 results — amend ADR before P2 (2026-10-06; spike relocated 2026-10-07)

The throwaway spike now runs offline and produces real layouts, proofs and timings.
**Do not proceed to P2:** language, legal wrapping, winding and workspace integration fail.
No production model/UI changes, engine fork, commit, push, GUI launch or installation.

## Hard gates

| Gate | Result | Evidence / boundary |
|---|---|---|
| Pinned byte-fed engine/fonts | PASS | COSMIC 0.19.0, fontdb 0.23.0, outline Skrifa 0.42.1; original Inter/Plex include_bytes!, hashes verified. |
| Explicit LTR/RTL/empty direction | PASS | Public ShapeSpan + explicit UBA base level; source unchanged; `explicit_direction` test. |
| Language | FAIL | No public COSMIC language-to-HarfRust input; requests return UnsupportedLanguage. Fixed FontSystem locale is not shaping language. |
| OpenType controls | PASS | Native feature API, kerning toggle test; required joining-feature disable refused. |
| Paint/font context | PASS / UNKNOWN | Paint split preserves glyphs; Inter Arabic request falls back to Plex deterministically. Real contrasting-Arabic-face context remains unproved. |
| Shape/metrics/marks/byte maps | PASS | Ten corpus-row tests, 12/48/200 pt x point/120/600 pt; finite metrics, legal clusters, grapheme integrity, source/edit undo. Conservative ink bounds. |
| Per-line bidi order/carets | PASS / UNKNOWN | UBA oracle across Auto/LTR/RTL and widths passes after cluster reorder adapter. Set-valued hit/affinity round-trips pass; host/Illustrator editing unproved. |
| Legal wrapping / line edges | FAIL | 8 forbidden break positions in price/isolate cases; `legal_line_breaks_gate`. Whole-word Arabic joins pass; intra-word reshaping unproved. |
| Whole-cluster fallback/unsupported | PASS | Pinned-font coverage, missing Plex, Latin in Plex and emoji ZWJ tests; unsupported ranges reported, .notdef retained. Not arbitrary-font validation. |
| Cubic Y-down outlines | PASS | Skrifa unhinted outlines, exact quadratic-to-cubic conversion, bounds/rotation/mixed size/baseline-shift tests. |
| Nonzero → even-odd winding | FAIL | 425/611 line comparisons exceed declared raster tolerance; max alpha delta 64/255, worst mean 0.574120/255. Zero fully opaque/transparent flips; PDF parity unproved. |
| Proof sheets / Arabic quality | PASS / UNKNOWN | Six readable CPU PNGs @2x generated and inspected; Ahmed's review pending. |
| Limits/malformed fonts | PASS / UNKNOWN | Invalid metrics/styles/font and >1 MiB text refusal tested; hostile-font work bounds/cancellation and bounded caches unproved. |
| Contrasting Arabic fixture | UNKNOWN | No additional licensed font acquired/audited. |
| Standalone headless feature graph | PASS | No UI/GPU deps; cosmic no_std only, std/sys-locale/Swash off. Dependency checker + 8 tests pass. |
| Workspace feature integration | FAIL (parked) | App resvg → usvg enables fontdb fs/memmap; COSMIC no_std fails E0004 at font/mod.rs:125 when the spike shares the workspace. Moderator 2026-10-07: the spike now lives in its OWN workspace (`varos/spikes/text-p1`, excluded from the main one) so main stays green; the feature-unification conflict is a real P2 planning item (separate fontdb feature set or a different font-source seam). |
| Native/WASM equivalence | PASS / UNKNOWN | Moderator 2026-10-07: `cargo check --lib --target wasm32-unknown-unknown` **passes** unchanged (no native deps in the engine graph); no runtime/pixel comparison yet. |
| Warm-edit target / measured size | FAIL | 10k scalar p50/p95 17.300/17.503 ms > suggested 16 ms; release bin 4,703,696 bytes. |

## Measurements and artifacts

Root: `/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/text-p1/`.
`measurements.md` contains methods, raw-log links/filenames and all unmeasured ADR items.
41 warm full-paragraph relayout samples per workload; process-cold fonts/layout, release defaults:
1k p50/p95 0.699/0.721 ms; 10k 17.300/17.503 ms; 100k 1837.702/2103.614 ms.
Peak process RSS: see per-workload logs; this is not an isolated heap/cache measurement.
`{shaping,marks,bidi,wrapping,controls,fallback}@2x.png`, `corpus-layouts.txt` (414 baseline configurations),
`winding.tsv` (611 lines), 12 failed image pairs and `artifact-inventory.json` preserve evidence.
Unicode tables differ: bidi 16.0 / linebreak 15.0 / segmentation 17.0. No unified-conformance claim.

## Required verification (all Cargo commands offline, from varos/)

- `cargo test --offline -p varos-text-spike`: **FAIL, 13 passed / 2 failed** (legal breaks, winding); no ignored/expected-failure masking.
- `cargo test --offline --workspace -j 4`: **FAIL to compile** while the spike was a workspace member (E0004 above); after moving it to `varos/spikes/text-p1` the main workspace gates pass again (moderator run 2026-10-07).
- `cargo clippy --offline --workspace --all-targets -- -D warnings`: **FAIL** at the time (raster tests lacked the resvg dev-dependency — fixed on main in `5e3c29c`); passes now.
- `cargo clippy --offline --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: passes after the same fix.
- `cargo --offline fmt --all --check`: **PASS**. Isolated spike native + Windows clippy: **PASS**.
- Release build, dependency-direction checker/8 tests, `git diff --check`: **PASS**. `proofs` exits 1 after saving artifacts because winding fails.
- Raster clippy failure reproduced without the spike; logs and exact exits are in `gate-exits.json` and named logs.

## Recommendation / implementation

Amend ADR-0010 before P2: resolve language/context API and fontdb feature-unification compatibility;
fix legal line breaks, validate winding/PDF parity, then repeat performance and WASM/owner gates.
Supported adapters are in `src/engine.rs` (explicit levels, per-line L1/L2, word fallback, caret map);
`src/outlines.rs` and `src/proof.rs` hold the cubic/region conversion and strict CPU diff.
Point text uses unbounded Word mode to avoid a dropped opposing run in COSMIC Wrap::None.
No claim that the stack is production-ready; no alternate shaper or open-ended fork was introduced.
