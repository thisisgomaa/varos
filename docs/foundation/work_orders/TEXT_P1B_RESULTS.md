# Text P1b results — another round required (2026-10-07)

Amendment 1 is owner-accepted; **do not proceed to P2** on this evidence.
Headless work stayed in the excluded spike workspace on `spike/text-p1b`.
No commit/push, GUI, application installation, production Rust/model/format/UI changes.

Evidence: [measurements and raw-log guide](/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/text-p1b/measurements.md).
[Spike README](../../../varos/spikes/text-p1/README.md) documents reproduction and limits.

| Amendment-1 exit gate | Result | Evidence / remaining boundary |
|---|---|---|
| Regression identity | PASS | Original 10 rows / 23 strings, pinned fonts and 414 configuration headers preserved. P1 raw 611-line evidence copied/checksummed; legal fitting now gives 600 lines. Original source/edit/undo/caret/fallback/control tests retained. |
| Language/script/context | PASS | ar/fa/ur/und, mixed runs, explicit/auto scripts, reset/cache alternation, paint joining, real Plex/Noto context and fallback retry match direct HarfRust. Positioned mark oracles fixed intra-cluster reversal. Real Plex Urdu locl contrast audited/shown. Bounded BCP-47/Common-script policy, not universal Unicode conformance. |
| Legal wrapping | PASS | Original gate and NBSP/isolate/neutral/mandatory/source-coverage/width tests pass. Unsafe/context-sensitive edges reshape and refit; ZWSP form changes match direct HarfRust. Eight old failures retained; stock reproducer logs nine under its Plex policy. UBA L1/L2 and whole-unit overflow verified. |
| Native NonZero contract | PASS, headless | 600/600 cubic/flat comparisons at unchanged tolerance: max delta 16, worst mean .151208, no severe pixels/interior flips. 4×4 subpixel quality renderer; old failed single-sample/conversion evidence preserved. CPU stencil/holes/overlap/alpha/clip/clear/refusal pass. All 600 PDF + 600 SVG renders pass (max 14/4). Actual GPU acceptance remains later. |
| Feature integration | PASS | Patched std + throwaway resvg 0.45.1 graph: native/Windows clippy, isolated/combined WASM builds/checks, no E0004. No sys-locale/Swash/fontconfig/UI/GPU/discovery in isolated engine; combined resvg's fs/memmap/fontconfig features explicitly recorded. |
| Performance/resources | FAIL | 10k p95 ≤5.240ms; 100k many ≤4.601ms; 100k long worst p95 64.034ms / max 102.284ms. 204 edits for every workload; cold equality/counters/eviction/cancellation pass. Convergent line reuse, width-analysis reuse, ≤8ms cancellation slices and hostile-font/temporary-memory bounds remain open. |
| Native/WASM runtime parity | PASS | Node v24.19.0, zero imports: 1,285 layouts/images plus 30 context records; exact identities/diagnostics and zero observed geometry/alpha difference. Same byte-fed fonts and fixed .001pt/alpha limits. |
| Reviewable evidence | UNKNOWN | Sources/base/diff/lockfile, raw logs, checksummed 2× sheets, full export matrix and licences saved. Sheets inspected; Ahmed's Arabic review, independent code review and production maintenance owner remain pending. |

## Verification and measured limits

- Native suite: **32 passed / 0 failed / 0 ignored** (author run). Moderator note 2026-10-07: the corpus/benchmark test was started twice on the owner's Mac and killed to cool the machine; the moderator's own re-run of the full spike suite is still pending — the main-workspace gates were re-run and are green.
- Native/Windows all-target clippy and isolated/combined WASM checks pass; exact exits in `gate-exits.json`.
- Main `tools/check_dep_directions.py` **PASS**. Separate stale HEAD unit fixtures reproduce 6 assertion/subtest failures across 2 methods (8 tests); tools were not edited.
- Original conversion **425/611 failures** and initial native single-sample **420/600 failures** remain historical diagnostics, never relabeled PASS. Tighter flattening alone failed; the new sampling renderer retains original thresholds.
- Character-edit RSS peaks at 415,334,400 bytes (includes cold oracles); retained paragraph cache peaks at 10,688,842 bytes. Caches expose bounded payload estimates, not total heap guarantees.
- Additional 204-sample width/font/language updates and all p50/p95/max samples are logged; peak control RSS 435,240,960 bytes. Not all update modes meet typing ceilings.
- Supersampled CPU draw cost is measured separately and is not an interactive-rendering solution. Whole-app delta, 100k draw/export and host edit-to-visible latency remain unmeasured.
- Shared runtime fixtures and 32 native unit tests are distinct evidence. Sizes/source hashes, legacy 41-sample comparison and preserved intermediate runs are in measurements. No crate was missing offline.

## Vendored patch inventory

Base **cosmic-text 0.19.0**: [hash manifest](../../../varos/spikes/text-p1/vendor/BASE.md), [complete patch](../../../varos/spikes/text-p1/vendor/cosmic-text-0.19.0-p1b.patch).
Pristine archive SHA-256: `be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73`.
Directory digest/per-file hashes retained; patch application reproduces vendor byte-for-byte.

| Vendor file | Added / removed | Rationale; upstream-ability |
|---|---:|---|
| `Cargo.toml` | +2 / -2 | Separate std/discovery; defaults retained; focused upstream candidate. |
| `Cargo.toml.orig` | +2 / -3 | Mirror release feature manifest; same upstream change. |
| `src/attrs.rs` | +29 / -0 | Shared language/script, owned conversion and compatibility; API review required. |
| `src/font/mod.rs` | +6 / -2 | Unified file variants compile; byte-fed mode refuses file sources; focused candidate. |
| `src/font/system.rs` | +5 / -5 | Explicit discovery gates around locale/scan/file sharing; focused candidate. |
| `src/shape.rs` | +253 / -19 | Reset/context, script/run keys, bounded attr scans, paragraph units/edge refit and mark-safe ordering; wrapping callback/API need review before upstream promotion. |
| `src/shape_run_cache.rs` | +47 / -0 | Direction/context identity, capped payload/entries; reusable candidate, eviction policy review. |

Seven vendor files, **+344 / -31**. Other work stays in spike src/tests/examples/scripts: paragraph/span caches, source validation, legal ranges/scripts, native winding/stencil/export, CPU proofs, offline harnesses and runtime/benchmark runners.
Temporary maintenance owner: unassigned; removal: upstream release or owner-reviewed replacement before production. No indefinite fork approved.

## Recommendation

**Another bounded round, not P2:** finish convergent line/analysis reuse, meet all large-text latency/resource/cancellation requirements, review the cost of supersampled CPU coverage, and obtain owner/independent review. No production renderer, format or UI integration is authorized by these proofs.

## P1c — performance-only round (2026-10-07)

Branch `spike/text-p1c`, same excluded spike workspace; no vendor (COSMIC patch), main-workspace,
GUI, commit or push changes. Raw logs stayed in the session scratchpad (not durable); every number
the decision needs is reproduced in this section. Rerun with `scripts/benchmarks.py` /
`scripts/control_benchmarks.py` (`VAROS_SPIKE_BIN` / `VAROS_CONTROL_BIN` select another build).
Hardware: arm64 Mac (warm), rustc 1.98.1, macOS 27.0.1; P1b and P1c binaries run back-to-back.
Correctness is unchanged: corpus goldens byte-identical (SHA-256
`1bc5573e…27e0`, cold and cached paths), debug suite **37 passed / 0 failed / 0 ignored**
(P1b's 32 + 5 new), clippy/fmt clean. After independent review: prefix-line reuse now requires every
unit a cached line *read* (including the unsafe-edge greedy look-ahead past its final end) to precede
the edited window, and internal splice inconsistencies rebuild the paragraph instead of failing the
edit; 2 more tests (39 total) pass in release. The 20-minute debug corpus test was not re-run after
that fix; goldens were re-checked byte-identical and the 100k single-paragraph rows re-measured
(p95 20.7 / 14.4 / 8.1 ms start/middle/end).

**Profile (before).** A P1b edit relaid the whole paragraph (≈40 ms warm + ≈7 ms wrapper):
shape-span key building/cloning 11.1, attributes 7.7, line fitting 8.0, carets 5.9, placement
2.6, bidi 2.5 ms. Shaping itself was already cached; the cost was rebuilding everything per edit.

**What changed.** `src/converge.rs` (new): a per-paragraph *analysis* keyed by text revision and
non-width settings (full-paragraph UBA, graphemes, scripts, UAX #14 units with shaped glyphs) that
survives width/alignment changes, plus cached lines. An edit recomputes the O(n) Unicode passes
exactly, diffs them with the previous revision (levels may change anywhere), reshapes only level
runs touching the difference ± five-scalar context, and re-fits lines from the first affected line
until a new line start meets a cached one beyond that window (a port of the patched fitter; reused
lines are shifted, never mutated). Base-direction flips, settings changes and unmappable
style/language ranges rebuild the analysis; multi-UBA-paragraph input falls back to the unchanged
cold engine. `cancel` is polled inside analysis, shaping, segment building, per fitted line and
output/merge. `engine.rs` was refactored into shared helpers only (goldens prove identity).

| §4 requirement | Result | Measured (final build, back-to-back with P1b binary) |
|---|---|---|
| 10k edit p95 ≤ 8 ms | PASS | worst p95 2.33 ms (P1b same run 4.37) |
| 100k many paragraphs p95 ≤ 50 / max ≤ 100 ms | PASS | worst 3.60 / 3.88 ms (P1b 4.52 / 5.38) |
| 100k single paragraph p95 ≤ 50 / max ≤ 100 ms | PASS | worst p95 20.61 / max 22.01 ms, start edit (P1b same run 46.37 / 53.30; recorded P1b 64.03 / 102.28) |
| Untouched paragraphs/lines not reshaped; cached = cold | PASS | ≤ 4 level runs shaped per 100k edit (of ≈23k); 204,000 untouched paragraphs hit; 1,600 + 80 randomized edits + every 50th benchmark sample equal a fresh cold engine; counters asserted in tests |
| Analysis reuse across width-only changes | PASS | 0 runs shaped; 100k width update p95 49.9→23.5 (long), 84.8→26.1 ms (many) |
| Cancellation slices ≤ 8 ms | PASS at 10k/100k; **FAIL at 1 MiB cap** | longest stretch 2.2 ms (100k edits), ≤ 4.3 ms (controls); 16.0 ms inside one `BidiInfo::new` call at 1 MiB |
| Temporary memory | Measured, **not capped** | peak transient heap per 100k edit ≤ 31.4 MB; retained 100k state ≈26 MB (P1b 10.4 MB) under the 32 MiB bound; a 1 MiB paragraph exceeds it and relayouts cold (≈400 ms) |
| Hostile-font work bounds | **OPEN** | not addressed; fonts are fixed compile-time bytes in the spike |

Sequential character edits (204 alternating insert/delete; p50 / p95 / max ms; gate p95 ≤ 8 at 10k,
p95 ≤ 50 and max ≤ 100 at 100k):

| Scalars | Mode | Edit | P1b binary (same session) | P1c | Gate |
|---:|---|---|---|---|---|
| 10000 | long | start | 3.967 / 4.370 / 8.808 | 1.914 / 2.327 / 4.139 | PASS |
| 10000 | long | middle | 3.958 / 4.182 / 5.266 | 1.357 / 1.415 / 2.103 | PASS |
| 10000 | long | end | 3.969 / 4.074 / 4.203 | 0.784 / 0.815 / 0.852 | PASS |
| 10000 | many | start | 0.358 / 0.393 / 0.655 | 0.331 / 0.345 / 0.368 | PASS |
| 10000 | many | middle | 0.362 / 0.381 / 0.475 | 0.357 / 0.376 / 0.418 | PASS |
| 10000 | many | end | 0.314 / 0.336 / 0.408 | 0.311 / 0.322 / 0.348 | PASS |
| 100000 | long | start | 43.154 / 43.980 / 47.257 | 20.251 / 20.607 / 22.011 | PASS |
| 100000 | long | middle | 43.223 / 44.518 / 46.754 | 14.230 / 14.520 / 15.650 | PASS |
| 100000 | long | end | 43.188 / 46.371 / 53.300 | 8.190 / 8.404 / 8.724 | PASS |
| 100000 | many | start | 4.160 / 4.468 / 5.286 | 3.463 / 3.566 / 3.883 | PASS |
| 100000 | many | middle | 4.243 / 4.520 / 5.106 | 3.446 / 3.594 / 3.713 | PASS |
| 100000 | many | end | 4.072 / 4.474 / 5.379 | 3.430 / 3.604 / 3.883 | PASS |

The P1b binary passed in this cool session but recorded p95 64.03 / max 102.28 under load: it had no
headroom. Profile after (100k middle edit, mean ms): fitting 5.9, bidi 1.8, script itemization 1.7,
validation 1.6, commit/bounds 0.6, caret merge 0.5, rest < 0.4 each. Peak transient heap per edit:
100k long ≤ 31.4 MB, 100k many 12.3 MB, 10k ≤ 3.4 MB; retained paragraph cache 10k 2.5–2.7 MB,
100k 24.8–26.5 MB. Legacy 41-sample cold benchmark (algorithm unchanged): 100k p50/p95
40.47/41.05 → 39.53/39.92 ms.

Width/font/language updates (204 each, p95 ms, P1b → P1c): 10k long width 4.99→2.52, font
5.31→4.20, language 5.12→4.18; 10k many width 10.42→2.34, font 6.81→4.45, language 9.25→4.52;
100k long width 69.07→23.55, font 47.35→46.21, language 52.71→45.54; 100k many width 84.83→26.13,
font 49.33→48.77, language 52.01→47.56. All sampled outputs equal cold recomputation.

Width/font/language updates are reported, not gated: font/language still re-analyse everything
(100k p95 45.5–48.8 ms, as P1b). The periodic gate motif makes any edit cascade line breaks to the
paragraph end (all later lines re-fit, still within ceilings); on varied text a middle edit re-fits
2–10 lines. Diagnostic beyond the gate: a 100k paragraph whose Auto direction flips on every edit
(Latin `X` typed before an Arabic first word) is a genuine full relayout, p95 75 ms on varied
words — it would fail the 100k ceiling if gated; remaining idea is direction-flip reuse of shaped
runs via COSMIC's structural reversal.

**Verdict.** The measured §4 latency ceilings, counters and cached-equals-cold requirements now pass
with ≥ 2.4× headroom. The Performance/resources gate is **not fully closed**: ≤ 8 ms slices at the
1 MiB cap, an enforced temporary-memory bound and hostile-font bounds remain open. Owner review and
independent code review of `converge.rs` are still required before P2.
