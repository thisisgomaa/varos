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
