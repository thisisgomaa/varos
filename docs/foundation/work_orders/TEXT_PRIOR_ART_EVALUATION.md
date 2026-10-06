# BStudio text prior-art evaluation — 2026-10-07

**Verdict: valuable Arabic shaping/kashida prior art, not an ADR-complete replacement. Recommend B: retain the bounded COSMIC patch path and port selected rules/fixtures. P2 remains blocked.**
Evaluated on `docs/text-prior-art-eval`, baseline `68176a6`; headless/offline only. Amendment 1's accepted body controls despite the ADR's stale “proposed” header. No production edits, commit, push or GUI.
[Archive + reproduction instructions](../../../design-reference/prior-art/bstudio-2026-06/README.md) · [raw evidence](../../../design-reference/prior-art/bstudio-2026-06/evaluation/) · [P1 baseline](TEXT_P1_RESULTS.md).

## Executed evidence and limits

- Original v0 `cargo test --offline -p bstudio-text`: **70 passed (56 unit + 14 integration), 0 failed/ignored**, using original Amiri/Cairo fixtures; these are not the 15 Varos acceptance tests.
- COSMIC P1 rerun: **13 passed / 2 failed / 0 ignored** (`legal_line_breaks_gate`, `winding_corpus_gate`). Language, integration and performance remain additional blockers outside that fraction.
- New harness: exact **10 rows / 23 strings × 2 faces × 3 sizes × 3 widths = 414 configurations**, identical bundled Inter/Plex hashes in `environment.json`; no substituted/downloaded fonts. Face requests are direct: BStudio has no automatic fallback.
- Deterministic repeated output in 414/414; finite placement plus valid byte-boundary clusters in **396/414**. LF inputs expose paragraph-local offsets; CRLF additionally produces invalid global boundaries. Finite numbers do not prove correct source coverage.
- Full-paragraph UBA oracle over observed per-line clusters: **45 mismatches / 521 checked lines**; **2 illegal breaks / 161 checked boundaries**, both `A B` at byte 3, 200 pt/120 pt (one per face). LF/CRLF order excluded because offsets are already broken. Trimmed whitespace/control coverage is unproved.
- `layout_text.caret_x` misses **836 repeated grapheme-boundary queries**, including ends/ligatures. Separate `shape_visual_line` passes ordering on 25 unwrapped paragraph slices; 253 equal-X caret inverse checks pass but **4 return inside graphemes**. Neither path implements two affinities or font ligature-caret data.
- **36,468 curve segments** extracted/checked across the matrix; quadratic→cubic formula sampled at five parameters within 0.001 pt after Y flip. No raster, NonZero, PDF or positioned-mark visual parity claim follows from this arithmetic check.
- Kashida: **26 Arabic-containing corpus tokens**, 14 accept insertions; all returned positions pass original HarfRust safe-flag + Hallberg checks, source stays exact and reshaping reconstructed working strings matches output. All four lam-alef variants reject insertion. These are structural checks, not Arabic quality approval.
- Original and preserved v0 library `wasm32-unknown-unknown` **checks pass**. No WASM runtime/native pixel equivalence run, Windows/combined-app feature check, CPU proof-sheet review or full-app timing was performed.

## Corpus findings (Plex unless a font difference is named)

| ADR row | Configs (both faces) | Observed result; remaining acceptance gap |
|---|---:|---|
| Joins | 18 | Contextual `بب` differs from isolated `ب`; Plex glyphs present. Inter emits .notdef instead of fallback; image quality unreviewed. |
| Lam-alef / Allah | 18 | Real ligatures and no lam-alef kashida slots; layout caret coverage incomplete, internal caret geometry unproved. |
| Stacked marks | 36 | Shaper returns GPOS offsets (8 nonzero-offset glyphs in first phrase); `PositionedGlyph` drops both offsets, so layout cannot place these marks correctly. |
| Mixed bidi / price | 36 | **27/76 line-order mismatches** across faces; token reversal mishandles punctuation/numbers. Separate visual-line path does not repair area layout. |
| Appointment / wrapping | 18 | 35 observed breaks legal on this phrase; whitespace-only breaker is not UAX #14, explicit direction/line context still absent. |
| Joining controls | 72 | Source borrowed unchanged; ZWJ/ZWNJ/literal tatweel shaped, lam-alef guard retained; scalar carets still insufficient. |
| Paint / font boundary | 18 | Single-face word shapes; no style-run/font-switch API to exercise the requested boundary tests. Unproved, not a paint-context PASS. |
| Explicit directions / empty | 54 | **Unsupported in layout**: paragraph direction always auto, empty line LTR; explicit direction exists only on single-run shaping. |
| Isolates / NBSP / newlines | 90 | **18/62 checked line-order mismatches**, both illegal NBSP breaks; paragraph offsets reset, trailing spaces lose layout/caret coverage. |
| Latin / fallback / emoji | 54 | Inter Latin shapes; missing Arabic face yields four .notdef for `شعار`; emoji .notdef retained without structured unsupported reporting. No fallback policy. |

No “BStudio passed X/15” is assigned: its API cannot express several P1 tests. Matrix diagnostics, upstream unit tests and Varos acceptance tests have different denominators.

## What exists, and what does not

v0 is real direct **HarfRust 0.7.0**, unicode-bidi 0.3.18, Skrifa 0.42.1/read-fonts 0.39.2; ICU properties 2.2.0 is declared but not referenced by its source. No Parley implementation is present despite aspirational comments. COSMIC 0.19.0 resolves HarfRust **0.5.2**: moving 0.7→0.5.2 is a **downgrade**, not an upgrade; both expose tatweel safety flags. Keep one production shaper; port the small API seam rather than require a second HarfRust version.
`shape_run_weighted` hardcodes RTL→Arab/ar, LTR→Latn/en and BOT/EOT, with empty feature list; language/script itemization, contextual style boundaries and OpenType controls need work. `ShaperData` is reconstructed on every run/token; no incremental cache exists.
`layout_text` greedily splits Unicode whitespace, shapes tokens independently, reverses tokens for RTL, strips trailing spaces and loses GPOS offsets. `visual.rs` is a separate, stronger single-line L2 path, not integrated with wrapping; wrapped lines need original paragraph levels/L1, not fresh paragraph detection.
`insert_kashida` accepts `&str`, creates a temporary string, inserts tatweels, then reshapes. **It does not mutate author text.** Returned clusters live in expanded working-string space; adding original run starts can overlap subsequent runs. Caret “snap/clamp” is not an inverse map. Require explicit source↔working/virtual-glyph mapping before reuse in editing.
Safe flags + pair filters are useful; “width increased” is only a heuristic, not proof that marks/ligatures remain correct. `dropped_unsafe` measures width rejection, not a final unsafe-flag audit; stacked combined insertions are not separately revalidated. Persian joining classes, `jalt` cycling and typographic balance are incomplete. MAX_KASHIDA caps inserted count, not candidate reshaping cost.
The later **v1 is not a proven upgrade**: inspected `text-core` still hardcodes direction-derived language/script; its kashida function has no font argument and applies the pair table without HarfBuzz flags. Its document/display-list coupling is not a Varos adapter. v1 was inspected, not tested.
Web `textLayout.ts` has source spans, wrapping/justify caches and size reflow; `TextOverlay.tsx` hosts editing in a textarea with UTF-16↔UTF-8 conversion. These supply UX/regression ideas, not native Winit IME/undo integration. Old QA/memories describe different builds (including a Penpot Arabic defect); they do not establish current v0 acceptance.

## Requirement mapping

“Reuse” means unchanged isolated logic/fixtures; “port” means adapt and re-prove. Neither engine supplies Varos's persisted document or host integration.

| ADR requirement | BStudio reuse / port / missing | COSMIC P1 comparison |
|---|---|---|
| TextBox / v4 / commands / undo | Missing Varos model; port web intent only | Missing production model; toy source/paint edit-undo tests exist |
| Explicit paragraph direction | Port bidi resolver to accept base level; missing layout control | Explicit UBA-level adapter already tested |
| Language / script / features / context | Port shaper controls; missing itemization, context and feature API | Language/script patch still required; features and paint adapter exist |
| Fallback / immutable face snapshot | Missing; one byte-fed face; `probe_font` falsely badges Inter Arabic via nonzero space glyph | Pinned whole-cluster fallback/issues tested; host snapshot integration missing |
| Incremental layout / bounds / limits | Missing; repeated font parsing/shaping, no bounded caches/cancellation | Missing incrementality; basic refusal/ink bounds exist |
| Bidi / carets / affinity / selection | Port visual-run logic and fixtures; replace scalar, single-valued caret map | P1 grapheme/set-valued affinity adapter is more complete; host policy unproved |
| IME / native shortcuts | Missing; textarea behavior is not reusable Winit code | Missing in both engines; belongs in varos-app |
| Area text / legal breaks / overflow | Replace approximate breaker/ordering; fix offsets and source coverage | Legal-break patch required; production frame/overflow missing in both |
| Outline / NonZero / metrics / axes | Reuse outline extraction concept; port exact cubics/Y flip and axis location (v0 helper uses default axes) | Cubics/placement exist; failed conversion, native NonZero extension still needed |
| PDF / SVG / CPU export parity | Missing Varos export; web parity intent can become fixtures | Missing production export; same Amendment §3 gate applies |
| Kashida / tracking / harakat | Reuse pair classifier/mark predicates + fixtures; port safe flags and source mapping; do not copy caret code wholesale | Missing safe-flag propagation; port through one COSMIC shaping path |

## Performance and portability

Release defaults (opt=3, no LTO/strip), Rust 1.98.1, macOS arm64; CPU model query denied. Same P1 seed `Logo شعار 12 `, 48 pt/600 pt. BStudio uses Plex only and omits fallback/full caret construction, so timings are not equivalent-output speed comparisons.

| Workload | Cold ms | Warm p50 / p95 / max ms | Assessment |
|---|---:|---:|---|
| 10k, legacy 41 full relayouts | 34.490 | 41.643 / 44.329 / 45.161 | Historical P1 p95 17.503; unequal output, no speed claim |
| 100k, legacy 41 full relayouts | 425.934 | 420.784 / 488.808 / 499.517 | Historical P1 p95 2103.614; cheaper incorrect layout is not gate success |
| 10k, 200 sequential insert/delete edits | 41.621 | 41.291 / **44.371** / 48.754 | FAIL ≤8 ms p95 |
| 100k single paragraph, 200 sequential edits | 428.016 | 337.650 / **464.409** / **702.109** | FAIL ≤50 ms p95 and ≤100 ms max |

Sequential cost includes clone/edit/layout/drop, cycles start/middle/end, and has no incremental reuse. Many-paragraph/language/font/width invalidation, cache bounds and cancellation remain unrun. Cumulative child peak RSS reached 246,153,216 bytes; not an isolated cache measure. Harness executable measured 2,951,040 bytes including fonts; this is not a production size delta. WASM runtime/size, outline draw/PDF size and whole-app delta remain unmeasured.

## Recommendation, effort and P1b disposition

**Choose B.** Preserve and port the Arabic-specific knowledge; retain COSMIC's richer run/fallback/layout machinery and P1 adapters. v0's 3,254 Rust source lines (about 2,052 before inline test modules) save real research, but do not eliminate ownership of UAX #14, itemization, fallback, affinity or incremental layout. The P1 adapter/report source is 1,071 lines, on top of upstream COSMIC. Own-engine maintenance would encompass Unicode updates and all those algorithms, not just 3k lines.
A would require roughly **15–25 engineering days** for replacement layout/fallback/carets/cache plus the same NonZero/integration/runtime proofs; discovery removes blank-sheet kashida work, not the missing contracts. This is an estimate, not a measured schedule. C (direct shaping injected into COSMIC layout) retains the unsupported shaper seam and dual ownership; it buys no demonstrated simplification. B retains an upstream patch burden, bounded by explicit diffs, a named maintenance owner and a removal/update policy before promotion.
**B P1b plan: 9–14 engineering days total** (accepted 8–12 plus 1–2 for prior-art fixtures/safe-flag plumbing, overlapping where possible). Keep Amendment §§1–7 decisions and all exit gates; no A-style rewrite or engine switch is warranted. No implementation is authorized by this report.
1. Keep §1 language/script/cache/context patch and §2 full-paragraph legal-break patch. Add the measured NBSP, mark-placement, newline offsets and grapheme-caret counterexamples as non-regression cases; never port BStudio token layout or scalar carets verbatim.
2. Extend the same COSMIC shaping patch to request/preserve SAFE_TO_INSERT_TATWEEL through shape/layout glyphs and expose a **slot-query only** seam. Port Hallberg/marks fixtures, compare 0.5.2 vs v0's pinned 0.7 outputs; do not introduce a second production shaper or automatic insertion. Any working-string composer remains later gated research with exact source mapping and final-shape validation.
3. Keep §3 native NonZero CPU/stencil/PDF/SVG proofs unchanged; neither library solves render winding. Keep §4 single-paragraph incremental ceilings, §5 isolated+combined native/Windows/WASM graph checks, §6 runtime parity/proofs and §7 owner/independent review before P2.
4. Carry typography intent: Arabic tracking default 0, manual/display-heading kashida scope, ≥130% leading, right/centre defaults, Arabic punctuation and mirrored layout. Do not enforce old brand fonts or auto-rewrite authored punctuation; BStudio tracking controls are experiments, not a reason to override those defaults.

## Preservation and licence

21 original files / **272,433 bytes** copied verbatim under `design-reference/prior-art/bstudio-2026-06/`: crate src/tests/benches/manifest, specs/001, research, two web text files, KashidaSidebar and Figma rules, plus original MPL licence/workspace metadata. Hash manifest verified; no font binaries, WASM, node_modules or production workspace membership. New harness/logs are separate in `evaluation/`.
MPL-2.0 can be combined with this GPL-3.0 work via §3.3 **if secondary-licence eligible**; retain notices and additionally distribute covered portions under MPL and GPL. No applied incompatibility notice found in copied crate; generic Exhibit B in the licence is not one. Do not silently relabel the archive GPL-only. [Mozilla FAQ Q14](https://www.mozilla.org/en-US/MPL/2.0/FAQ/#q14-may-i-combine-mpl-licensed-code-and-lgpl-licensed-code-in-the-same-executable-program).
