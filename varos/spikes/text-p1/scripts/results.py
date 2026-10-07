#!/usr/bin/env python3
"""Assemble evidence-scoped reports from saved raw outputs; never invent missing runs."""
from pathlib import Path
import csv,json,re,sys
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);repo=root.parents[2]
def records(pattern):
 rows=[]
 for p in sorted(out.glob(pattern)):
  s=p.read_text();line=next(x for x in s.splitlines() if x.startswith('count='))
  r=dict(re.findall(r'([a-z0-9_]+)=([^ ]+)',line));r['rss']=re.search(r'peak_rss_bytes=(\d+)',s)[1];rows.append(r)
 return rows
edits=records('edits-*.log');controls=records('control-*.log');assert len(edits)==len(controls)==12
for r in edits:r['result']='PASS' if float(r['p95_ms'])<=(8 if r['count']=='10000' else 50) and (r['count']=='10000' or float(r['max_ms'])<=100) else 'FAIL'
parity=json.loads((out/'runtime-parity-result.json').read_text());assert parity['result']=='PASS'
contexts=sum(line.startswith('context\t') for line in (out/'native-parity.tsv').open())
patch=json.loads((out/'patch-inventory.json').read_text());added=sum(r['added'] for r in patch);removed=sum(r['removed'] for r in patch)
nonzero=list(csv.DictReader((out/'nonzero.tsv').open(),delimiter='\t'));assert len(nonzero)==600
assert all(float(r['mean_delta'])<=1 and int(r['severe_gt32'])==0 for r in nonzero)
exports=list(csv.DictReader((out/'export-matrix/comparisons.tsv').open(),delimiter='\t'));assert len(exports)==1200
export_summary={}
for backend in ['pdf','svg']:
 r=[x for x in exports if x['backend']==backend];assert {x['case'] for x in r}=={f'{i:04}' for i in range(600)}
 assert all(float(x['mean_alpha'])<=1 and int(x['severe'])==0 for x in r)
 export_summary[backend]=(max(int(x['max_alpha']) for x in r),max(float(x['mean_alpha']) for x in r))
results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',(out/'native-tests.log').read_text())
passed=sum(int(a) for a,_,_ in results);assert passed==32 and all(b==c=='0' for _,b,c in results)
assert all(v['exit']==0 for v in json.loads((out/'gate-exits.json').read_text()).values())
small=max(float(r['p95_ms']) for r in edits if r['count']=='10000')
many=max(float(r['p95_ms']) for r in edits if r['count']=='100000' and r['kind']=='many')
long=[r for r in edits if r['count']=='100000' and r['kind']=='long'];worst_long=max(float(r['p95_ms']) for r in long);worst_max=max(float(r['max_ms']) for r in long)
peak=max(int(r['rss']) for r in edits);cache=max(int(r['paragraph_cache_bytes']) for r in edits)
control_peak=max(int(r['rss']) for r in controls)
summary=f'10k p95 ≤{small:.3f}ms; 100k many ≤{many:.3f}ms; 100k long worst p95 {worst_long:.3f}ms / max {worst_max:.3f}ms'
measurement=f'''# P1b measurements — 2026-10-07

Headless Amendment-1 spike, excluded workspace on `spike/text-p1b`. No commit/push, GUI, application installation, production Rust/model/format/UI changes. All Cargo commands used offline; no missing crate encountered. Recommendation: **another round, not P2**.

## Provenance and fixed contracts

- COSMIC 0.19.0 pristine archive/directory hashes and licences: spike `vendor/BASE.md`, `pristine.sha256`, complete apply-tested `cosmic-text-0.19.0-p1b.patch`. `patch-application.log` verifies patched pristine files equal the vendor byte-for-byte. Current diff: seven files, +{added}/-{removed}; `patch-inventory.json` lists each file.
- Original P1 evidence is intact in `p1-preserved/`, checksummed by `p1-sha256.json`. Original 10 rows / 23 strings and 414 configuration headers match exactly. Revised wrapping gives 600 lines vs original 611; inputs were not removed. Fonts/hashes/licences are preserved; see `fonts-verified.json`, `licenses/`, `language-font-audit.txt`.
- Rust 1.98.1, aarch64-apple-darwin, macOS 27.0.1. `environment.json` records compiler/OS, final binary sizes and source hashes; `artifact-inventory.json` checksums evidence. Hardware model/CPU/RAM query was sandbox-denied (`p1b-hardware.txt`), so no chip model is asserted.
- Unicode tables: bidi 16 / linebreak 15 / segmentation 17. No unified-version conformance claim.
- Alpha tolerance stayed **mean ≤1/255 and no pixel delta >32/255**. Flatten tolerance is 0.005pt at 2×. Initial direct single-sample tiny-skia Winding failed 420/600 (max 64, worst mean .529035, 14,136 severe pixels); preserved in `single-sample-final/`. Tightening geometry to .001/.0001pt still failed (`flatten-sweep.tsv`). Early native logs had an unused zero polygon-comparison field; the current native TSV omits it, and the historical conversion diagnostic alone populates it.
- A 4×4 subpixel sampling policy was declared before its candidate run (`supersample-candidate.log`), then implemented: tiny-skia 0.11.4 Winding per glyph, max-compose subpixel coverage, premultiplied box-filter to requested 2× output, apply opacity once. No threshold, dilation, ignored pixels or enlarged comparison margins. PDF/CoreGraphics and SVG/resvg 0.45.1 render at 8× and use the identical box filter to 2×. This is a changed quality renderer, not relabeling the failed single-sample renderer.
- Native/WASM: exact source/face/glyph/cluster/break/direction/caret/diagnostic identities, positions ≤.001pt, same 2× alpha thresholds. Runtime fixtures use the identical supplied font bytes and fixed fallback policy, with zero host imports.

## Correctness

**Language/script/context PASS (fixture-scoped):** Attrs/AttrsOwned, compatibility, cache keys, fallback and HarfRust buffers carry parsed per-run BCP-47 language and optional script. Buffers clear absent values. Context keys retain HarfRust's five surrounding scalars. ar/fa/ur/und, mixed runs, script overrides/reset, cache alternation, paint joining and real Plex/Noto boundaries match direct shaping. Positioned mark/base oracles caught and fixed an LTR-paragraph cluster-reversal bug. Strong-script paragraph itemization resolves Common/Inherited by neighboring context; this is not full Script_Extensions/paired-punctuation conformance. A bounded BCP-47 syntax subset is documented in README; no blanket all-tags claim.

Plex GSUB arab/URD feature 37, lookup 6 substitutes uni06F6→uni0666: a real Urdu contrast from ar/fa/und, shown in `language-forms@2x.png`. Noto Sans Arabic came from the pristine vendored archive, with its OFL licence; no network font acquisition.

**Legal wrapping PASS over old/added fixtures:** full-paragraph UAX #14 opportunities govern indivisible units across bidi/style/font fragments. Logical fitting backtracks and overflows whole units with diagnostics. Chosen ranges include controls/trailing spaces/hard-break bytes. HarfRust unsafe-to-break flags plus conservative non-space boundaries trigger line-clipped reshaping and forward/backward refit, including opportunities inside prior clusters. ZWSP Arabic fixtures prove changed edge forms, direct-HarfRust identity/advance agreement and maximal fitting; ZWSP is not mislabeled as a missing glyph. Auto/LTR/RTL, NBSP/isolate/neutral/mandatory source coverage, local L1 vs full oracle and cold width round-trips pass. Eight old failures are preserved in `old-illegal-breaks.txt`; independent stock COSMIC logs nine failures under its explicitly requested Plex policy (`stock-cosmic.log`), including the old forbidden positions. The unbounded Word workaround remains; no emergency glyph breaks or automatic hyphenation claim.

**Native NonZero PASS:** `nonzero.tsv` has 600/600 comparisons within unchanged tolerance, max alpha {max(int(r['max_delta']) for r in nonzero)}, worst mean {max(float(r['mean_delta']) for r in nonzero):.6f}, zero severe pixels/interior flips. Original conversion **425/611 failures** remains historical in `p1-preserved/winding.tsv`, never relabeled PASS. Old single-sample pairs are separately labeled in `single-sample-final/` and `single-sample-earlier/`.

Synthetic holes, opposite winding/overlap, alpha-once, EvenOdd/NonZero modes, clip isolation, clear and unsafe ±256 refusal pass the CPU stencil model. It uses independent winding storage and validates pixel-center samples before publishing output; it is not GPU antialiasing evidence or a global geometric overlap proof.

**Export parity PASS over all 600 original lines / 1,200 comparisons:** `export-matrix/cases.tsv` maps every source/configuration/line to native PNG, exact-cubic PDF `f` and SVG nonzero paths. CoreGraphics PDF max alpha {export_summary['pdf'][0]}, worst mean {export_summary['pdf'][1]:.9f}; resvg SVG max {export_summary['svg'][0]}, worst mean {export_summary['svg'][1]:.9f}; zero severe pixels. Raw 8× renders and filtered 2× images accompany `comparisons.tsv`. Alpha/group/clip production export and actual GPU checks remain later integration work. Ordinary source-over vs max-coverage has a separately labeled small-fixture diagnostic, not a substitute reference.

**Runtime parity PASS:** {parity['fixtures']:,} layouts/images plus {contexts} context/fallback records, Node v24.19.0, zero observed geometry/alpha differences (`runtime-parity-result.json`, `wasm-runtime.log`). Includes original sizes/widths/faces/directions, marked text, legal edge cases, mixed language, Latin/Arabic overrides and absent properties. These are shared runtime fixtures; the 32 native unit tests are separately reported. Serialized-fixture WASM memory is not an editor working-set measurement.

## Features, caches and unresolved scope

Patched std engine and throwaway resvg default-feature harness pass native/Windows clippy and WASM lib checks/builds without E0004. `system-discovery` gates locale, filesystem discovery and file sharing. Explicit bytes/locale/fallback only; non-Binary sources are refused even with unified fontdb variants. Isolated graph has no sys-locale/Swash/fontconfig/UI/GPU; combined resvg intentionally enables fontdb fs/memmap/fontconfig. This proves the conflicting feature combination, not a full app build (`gate-exits.json`, feature logs).

Incremental paragraph cache uses stable identity/revision, text, width/direction/font face and immutable compiled font content, style/language/script/features. Unchanged paragraphs reuse results; paint does not reshape. Split/join, eviction, cancellation/refusal and cold equivalence tests pass. A separate resolved-bidi-span cache keys exact text, attributes, level/base direction and bounded context; it does not invent independent Arabic chunks. Character coverage uses fixed Unicode bitsets. Per-line L1 no longer clones the entire paragraph levels vector; L2/carets use contiguous cluster groups and preserve marks.

**Still incomplete:** edited paragraphs rerun line fitting; convergent line reuse and a separate paragraph-analysis cache across width changes are not implemented. `reshaped` counts paragraph calls into the engine, not individual HarfRust executions; lower shape caches can hit. Cancellation checks paragraph boundaries, not ≤8ms slices. Runtime font replacement, hostile-font work and peak temporary-memory bounds remain unproved. These keep the performance/resources exit gate FAIL even if an individual timing workload passes.

Retained payload caps: paragraph 32MiB; COSMIC run cache and adapter span cache each 8MiB / 4,096 entries; outline cache 16,384 entries. Text ≤1MiB; style/language runs ≤16,384; features ≤128. Counts exclude allocator/hash overhead, immutable font bytes and temporary results; RSS is separate. Supersampled coverage refuses >64M intermediate pixels (4M output pixels).

## Sequential character edits

Release defaults; original `Logo شعار 12 ` motif, 48pt / 600pt width; 10k/100k scalars, one paragraph and newline every 100th scalar. Exactly 204 alternating ASCII insert/delete edits at start/middle/end. Timing includes mutation/request/update/result destruction; cold comparison every 50th sample is outside timing. p50 = sorted sample 103, p95 = sample 194, max = sample 204. Fresh child per workload; no concurrent build/raster job. RSS includes the cold-oracle checks. Targets: 10k p95 ≤8ms; 100k p95 ≤50ms, max ≤100ms.

| Scalars | Mode | Edit | Cold ms | p50 ms | p95 ms | Max ms | RSS bytes | Paragraph cache | Engine cache | Gate |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---|
'''
for r in edits:measurement+='| '+' | '.join(r[k] for k in ['count','kind','position','cold_ms','p50_ms','p95_ms','max_ms','rss','paragraph_cache_bytes','engine_cache_bytes','result'])+' |\n'
measurement+=f'''
{summary}. Long mode: 204 paragraph updates, zero untouched-paragraph hits; many-10k: 20,400 hits; many-100k: 204,000 hits. Exact span-cache counters and all samples are in each log. Peak character-edit RSS {peak:,} bytes; retained paragraph cache {cache:,} bytes. Earlier complete matrices and variable smoke/profile runs remain in `before-decoration-range-fix/`, `before-span-cache/`, `before-zwsp-diagnostic/` and named logs; no favorable smoke result replaces the full matrix.

## Legacy comparison and control changes

The original 41-sample benchmark still excludes source mutation/cloning and output destruction; no paragraph-result cache. It must not replace sequential editing.

| Scalars | UTF-8 bytes | Font init ms | Cold layout ms | Warm p50 ms | Warm p95 ms | Lines | Glyphs |
|---:|---:|---:|---:|---:|---:|---:|---:|
'''
for count in [1000,10000,100000]:measurement+=next(x for x in (out/f'legacy-41-{count}.log').read_text().splitlines() if x.startswith('|'))+'\n'
measurement+='''
P1 legacy p50/p95: 1k .699/.721ms; 10k 17.300/17.503ms; 100k 1837.702/2103.614ms. Historical timing is not a controlled same-instant comparison. Profiling and retained intermediate binaries document the overlapping-attribute scan fix, context caches, line-local L1 and contiguous cluster processing.

Control workloads: 204 changes each, width 120/600pt, face Plex/Inter, language ar/fa/ur/und. All sampled outputs match cold recomputation. Global changes invalidate paragraph results; shapes may still hit the context cache. Reported separately from character-edit ceilings.

| Scalars | Mode | Control | p50 ms | p95 ms | Max ms | RSS bytes |
|---:|---|---|---:|---:|---:|---:|
'''
for r in controls:measurement+='| '+' | '.join(r[k] for k in ['count','kind','control','p50_ms','p95_ms','max_ms','rss'])+' |\n'
measurement+=f'''
Peak additional control RSS: {control_peak:,} bytes. No blanket claim that all control updates meet typing ceilings.

## Draw/export and review limits

Small mixed-font/mark fixture, 48pt / 600pt width, 1400×300 output pixels, 41 samples:

```text
{(out/'draw-pdf-bench.log').read_text().strip()}
```

The supersampled CPU quality path is substantially slower than the preserved single-sample diagnostic. It is **not an interactive draw/GPU performance solution**. 100k-document draw/PDF, whole-app binary delta, IME/edit-to-visible latency and production GPU acceptance are unmeasured. Standalone native/WASM sizes include fixture/font/oracle code (`environment.json`).

32 native tests PASS / 0 FAIL / 0 ignored. Final native/Windows clippy and isolated/combined WASM checks pass (`gate-exits.json`); proof/benchmark runners return failure for unmet thresholds. Main `tools/check_dep_directions.py` PASS. Its separate pre-existing unit fixtures are stale on HEAD (spike membership / missing Bridge): 8 tests with 6 assertion/subtest failures across 2 methods, reproduced byte-identically in `dependency-checker-baseline/`; tools were not edited.

CPU language, wrapping/edge and nonzero sheets were visually inspected for content/cropping. Ahmed's Arabic/marks review, independent code review and a production maintenance owner remain UNKNOWN. No claimed GPU or installed-app test. Another bounded round must finish line convergence / analysis reuse, satisfy all long-paragraph/control latency and resource/cancellation requirements, and obtain review before P2.
'''
(out/'measurements.md').write_text(measurement)
shape=next(r for r in patch if r['file']=='src/shape.rs')
report=f'''# Text P1b results — another round required (2026-10-07)

Amendment 1 is owner-accepted; **do not proceed to P2** on this evidence.
Headless work stayed in the excluded spike workspace on `spike/text-p1b`.
No commit/push, GUI, application installation, production Rust/model/format/UI changes.

Evidence: [measurements and raw-log guide]({out}/measurements.md).
[Spike README](../../../varos/spikes/text-p1/README.md) documents reproduction and limits.

| Amendment-1 exit gate | Result | Evidence / remaining boundary |
|---|---|---|
| Regression identity | PASS | Original 10 rows / 23 strings, pinned fonts and 414 configuration headers preserved. P1 raw 611-line evidence copied/checksummed; legal fitting now gives 600 lines. Original source/edit/undo/caret/fallback/control tests retained. |
| Language/script/context | PASS | ar/fa/ur/und, mixed runs, explicit/auto scripts, reset/cache alternation, paint joining, real Plex/Noto context and fallback retry match direct HarfRust. Positioned mark oracles fixed intra-cluster reversal. Real Plex Urdu locl contrast audited/shown. Bounded BCP-47/Common-script policy, not universal Unicode conformance. |
| Legal wrapping | PASS | Original gate and NBSP/isolate/neutral/mandatory/source-coverage/width tests pass. Unsafe/context-sensitive edges reshape and refit; ZWSP form changes match direct HarfRust. Eight old failures retained; stock reproducer logs nine under its Plex policy. UBA L1/L2 and whole-unit overflow verified. |
| Native NonZero contract | PASS, headless | 600/600 cubic/flat comparisons at unchanged tolerance: max delta 16, worst mean .151208, no severe pixels/interior flips. 4×4 subpixel quality renderer; old failed single-sample/conversion evidence preserved. CPU stencil/holes/overlap/alpha/clip/clear/refusal pass. All 600 PDF + 600 SVG renders pass (max {export_summary['pdf'][0]}/{export_summary['svg'][0]}). Actual GPU acceptance remains later. |
| Feature integration | PASS | Patched std + throwaway resvg 0.45.1 graph: native/Windows clippy, isolated/combined WASM builds/checks, no E0004. No sys-locale/Swash/fontconfig/UI/GPU/discovery in isolated engine; combined resvg's fs/memmap/fontconfig features explicitly recorded. |
| Performance/resources | FAIL | {summary}. 204 edits for every workload; cold equality/counters/eviction/cancellation pass. Convergent line reuse, width-analysis reuse, ≤8ms cancellation slices and hostile-font/temporary-memory bounds remain open. |
| Native/WASM runtime parity | PASS | Node v24.19.0, zero imports: {parity['fixtures']:,} layouts/images plus {contexts} context records; exact identities/diagnostics and zero observed geometry/alpha difference. Same byte-fed fonts and fixed .001pt/alpha limits. |
| Reviewable evidence | UNKNOWN | Sources/base/diff/lockfile, raw logs, checksummed 2× sheets, full export matrix and licences saved. Sheets inspected; Ahmed's Arabic review, independent code review and production maintenance owner remain pending. |

## Verification and measured limits

- Native suite: **32 passed / 0 failed / 0 ignored**. No expected-failure masking; benchmark commands still fail unmet ceilings.
- Native/Windows all-target clippy and isolated/combined WASM checks pass; exact exits in `gate-exits.json`.
- Main `tools/check_dep_directions.py` **PASS**. Separate stale HEAD unit fixtures reproduce 6 assertion/subtest failures across 2 methods (8 tests); tools were not edited.
- Original conversion **425/611 failures** and initial native single-sample **420/600 failures** remain historical diagnostics, never relabeled PASS. Tighter flattening alone failed; the new sampling renderer retains original thresholds.
- Character-edit RSS peaks at {peak:,} bytes (includes cold oracles); retained paragraph cache peaks at {cache:,} bytes. Caches expose bounded payload estimates, not total heap guarantees.
- Additional 204-sample width/font/language updates and all p50/p95/max samples are logged; peak control RSS {control_peak:,} bytes. Not all update modes meet typing ceilings.
- Supersampled CPU draw cost is measured separately and is not an interactive-rendering solution. Whole-app delta, 100k draw/export and host edit-to-visible latency remain unmeasured.
- Shared runtime fixtures and 32 native unit tests are distinct evidence. Sizes/source hashes, legacy 41-sample comparison and preserved intermediate runs are in measurements. No crate was missing offline.

## Vendored patch inventory

Base **cosmic-text 0.19.0**: [hash manifest](../../../varos/spikes/text-p1/vendor/BASE.md), [complete patch](../../../varos/spikes/text-p1/vendor/cosmic-text-0.19.0-p1b.patch).
Pristine archive SHA-256: `be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73`.
Directory digest/per-file hashes retained; patch application reproduces vendor byte-for-byte.

| Vendor file | Added / removed | Rationale; upstream-ability |
|---|---:|---|
'''
rationale={
'Cargo.toml':'Separate std/discovery; defaults retained; focused upstream candidate.',
'Cargo.toml.orig':'Mirror release feature manifest; same upstream change.',
'src/attrs.rs':'Shared language/script, owned conversion and compatibility; API review required.',
'src/shape.rs':'Reset/context, script/run keys, bounded attr scans, paragraph units/edge refit and mark-safe ordering; wrapping callback/API need review before upstream promotion.',
'src/shape_run_cache.rs':'Direction/context identity, capped payload/entries; reusable candidate, eviction policy review.',
'src/font/mod.rs':'Unified file variants compile; byte-fed mode refuses file sources; focused candidate.',
'src/font/system.rs':'Explicit discovery gates around locale/scan/file sharing; focused candidate.'}
for r in patch:report+=f"| `{r['file']}` | +{r['added']} / -{r['removed']} | {rationale[r['file']]} |\n"
report+=f'''
Seven vendor files, **+{added} / -{removed}**. Other work stays in spike src/tests/examples/scripts: paragraph/span caches, source validation, legal ranges/scripts, native winding/stencil/export, CPU proofs, offline harnesses and runtime/benchmark runners.
Temporary maintenance owner: unassigned; removal: upstream release or owner-reviewed replacement before production. No indefinite fork approved.

## Recommendation

**Another bounded round, not P2:** finish convergent line/analysis reuse, meet all large-text latency/resource/cancellation requirements, review the cost of supersampled CPU coverage, and obtain owner/independent review. No production renderer, format or UI integration is authorized by these proofs.
'''
assert len(report.splitlines())<=80
(repo/'docs/foundation/work_orders/TEXT_P1B_RESULTS.md').write_text(report)
print(f'report {len(report.splitlines())} lines; character workloads passing {sum(r["result"]=="PASS" for r in edits)}/12; native tests {passed}; runtime {parity["fixtures"]}')
