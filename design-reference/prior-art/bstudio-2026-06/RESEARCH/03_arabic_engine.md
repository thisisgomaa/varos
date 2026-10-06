# 03 — Arabic Text Engine

**Track:** Arabic engine (the moat)
**Date:** 2026-05-25
**Status:** Deep research, decision-ready. Stand-alone document.

---

## TL;DR — five decisions

1. **Shaping engine: `harfrust`** (the official HarfBuzz Rust port under the `harfbuzz/` GitHub org, formerly forked from rustybuzz in June 2025). Latest **v0.7.0 (2026-05-21)** tracking HarfBuzz **v14.2.0**. Pure Rust, compiles cleanly to `wasm32-unknown-unknown`, ≤25 % slower than C HarfBuzz, shares the `read-fonts` parser with Google's `skrifa`. **Do not adopt `rustybuzz`** — its original maintainer abandoned it in Aug 2023 (`RazrFalcon/rustybuzz#74`).

2. **The kashida moat is already half-built and nobody noticed.** HarfBuzz **v5.1.0 (July 2022)** added `HB_GLYPH_FLAG_SAFE_TO_INSERT_TATWEEL` via Khaled Hosny's PR #3762. Both harfrust and rustybuzz expose it as `BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL`. **No visual design tool — Adobe, Figma, Penpot, Microsoft Word, Apple Pages — currently uses this flag.** Our moat in one sentence: *we will be the first visual editor that reads SAFE_TO_INSERT_TATWEEL and re-runs shaping after every insertion.*

3. **BiDi:** Servo's `unicode-bidi` crate (pure safe Rust, UAX #9) + `icu4x` `BidiClassAdapter` for character properties. ICU4X 2.0 (May 2025) gave a 50–90 % binary-size reduction vs ICU4C.

4. **Layout shell:** Parley (v0.9, pre-1.0) **already uses harfrust internally**. Recommendation: fork Parley's line breaker into our own `pivot-text` crate and graft the kashida pass on top, rather than build line breaking from scratch.

5. **MVP that beats Adobe in 12 months:** interactive drag-to-stretch kashida respecting SAFE_TO_INSERT_TATWEEL, re-shaping every frame, hard-protecting lām-alif and contextual ligatures. JSTF and jalt cycling are explicitly Phase 2+.

---

## 1. Shaping engine — Rust + HarfBuzz on WASM

### 1.1 Landscape (May 2026)

| Crate | Lineage | Latest | HB parity | WASM | Status |
|---|---|---|---|---|---|
| **`harfrust`** | Official HarfBuzz org, fork of rustybuzz | 0.7.0 (2026-05-21) | **v14.2.0** | yes, pure Rust | **active** |
| `rustybuzz` | RazrFalcon's port, transferred to harfbuzz org | 0.20.1 (2024-11-12) | v10.1.0 | yes | **abandoned by original maintainer** ("syncing only") |
| `harfbuzz_rs` | C bindings to upstream HarfBuzz | — | matches linked C lib | technically yes via emscripten | low-traffic maintenance |
| `swash` (dfrg) | Independent pure-Rust shaper using USE | 0.2.x | n/a | yes | single maintainer |

### 1.2 Why harfrust

- HarfBuzz org built it specifically to consolidate on **fontations**' `read-fonts` (same parser `skrifa` uses) — saves ~1.5 MB in WASM bundle vs running `ttf-parser` (rustybuzz) and `read-fonts` (skrifa).
- Tracks upstream HarfBuzz roughly monthly (v13.0.0 → v14.1.0 in April → v14.2.0 in May).
- Ships `hr-shape`, CLI mirroring `hb-shape`, for shaper-diff debugging.
- **Critically**: exposes `BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL` and `BufferFlags::PRODUCE_UNSAFE_TO_CONCAT`.
- Performance gap to C HarfBuzz (<25%) is irrelevant; shaping is not the hot path.

### 1.3 Why not the alternatives

**`rustybuzz`:** Maintainer wrote *"I'm giving up on this project. I have no further plans on working on it"* (Aug 2023). HarfBuzz org's response was to fork it as harfrust.

**`harfbuzz_rs`:** Three FFI boundaries to reach WASM (CMake → emscripten → wasm-bindgen). Loses Rust's safety at the layer we'll re-shape hardest. No perf justification.

**`swash`:** Uses Microsoft's **Universal Shaping Engine** (generic Indic/SE-Asia path), not HarfBuzz's special-case Arabic shaper. USE historically lags on Arabic edge cases. Does not expose SAFE_TO_INSERT_TATWEEL semantics. Single maintainer.

### 1.4 WASM specifics

- Target `wasm32-unknown-unknown` (no WASI needed for browser).
- `wasm-pack build --target web --release` + `wasm-opt -O3`.
- Expected stripped wasm: 600 kB–1.2 MB (reference: `loveencounterflow/rustybuzz-wasm` ships ~800 kB compressed).
- HarfBuzz also has separate **WASM shapers inside fonts** concept (2024); harfrust supports it. Irrelevant Phase 1 but worth knowing.

### 1.5 Rust API sketch

```rust
use harfrust::{Face, ShaperData, ShaperInstance, UnicodeBuffer,
               Direction, Script, Language, BufferFlags, GlyphFlags};

pub struct ArabicShaper<'a> {
    face: Face<'a>,
    shaper: ShaperData,
}

impl<'a> ArabicShaper<'a> {
    pub fn new(font_bytes: &'a [u8]) -> Result<Self, Error> {
        let face = Face::from_slice(font_bytes, 0)?;
        let shaper = ShaperData::new(&face);
        Ok(Self { face, shaper })
    }

    pub fn shape_run(&self, text: &str, lang: Language) -> ShapedRun {
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        buf.set_direction(Direction::RightToLeft);
        buf.set_script(Script::ARABIC);
        buf.set_language(lang);
        buf.set_flags(
            BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL
            | BufferFlags::PRODUCE_UNSAFE_TO_CONCAT
            | BufferFlags::BEGINNING_OF_TEXT
            | BufferFlags::END_OF_TEXT
        );

        let instance = ShaperInstance::from_variations(&self.face, &[]);
        let output = self.shaper.shaper(&self.face)
            .instance(Some(&instance))
            .shape(buf, &[]);

        let infos = output.glyph_infos();
        let positions = output.glyph_positions();

        let glyphs = infos.iter().zip(positions.iter()).map(|(info, pos)| {
            ShapedGlyph {
                glyph_id: info.glyph_id,
                cluster: info.cluster,
                x_advance: pos.x_advance,
                y_advance: pos.y_advance,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
                safe_to_insert_tatweel:
                    info.flags().contains(GlyphFlags::SAFE_TO_INSERT_TATWEEL),
                unsafe_to_concat:
                    info.flags().contains(GlyphFlags::UNSAFE_TO_CONCAT),
            }
        }).collect();

        ShapedRun { glyphs, direction: Direction::RightToLeft }
    }
}
```

Adobe InDesign internally uses HarfBuzz (via `WorldReady`) but its kashida UI calls a separate post-shape tatweel-insertion routine that **ignores SAFE_TO_INSERT_TATWEEL**. That's the breakage TypoArabic and Sibahi document.

---

## 2. Linebender stack — Arabic readiness in 2026

### 2.1 Parley
`linebender/parley` v0.9.0 (April 2026), pre-1.0, 617 stars. Used internally by Linebender's experimental editor / Xilem.

What's there: shaper = harfrust (so we share an engine). BiDi via ICU4X's `BidiClassAdapter` → unicode-bidi. Font fallback handles Arabic + CJK runs. Line breaking, alignment, cursor/selection.

What's missing: **No kashida or SAFE_TO_INSERT_TATWEEL consumption.** **No JSTF.** **No jalt cycling.** Open issue #612 (Apr 2026): RTL newline cursor bug. **Zero verifiable production users.**

### 2.2 Swash
Old answers say Parley uses swash. **Out of date.** Parley uses `skrifa` for outlines and `harfrust` for shaping.

### 2.3 cosmic-text
`pop-os/cosmic-text`. Uses harfrust + unicode-bidi. Ships in System76's COSMIC desktop. Built for terminal/UI text, not design-tool typography. Reference implementation more than dependency.

### 2.4 What we'd build on top of Parley
Fork Parley's line-breaker into a `pivot-text` crate and add:
1. **Kashida-aware justification pass** between line breaking and final positioning, reading SAFE_TO_INSERT_TATWEEL.
2. **jalt feature cycler** re-shapes line with `jalt=on/off`, picks best variant.
3. **Mandatory-ligature guard** (lām-alif plus small allowlist).
4. **Interactive stretch handle** API with frame-stable caching.

Estimated incremental code: 3,000–5,000 LoC in Rust, much of it tests.

---

## 3. Kashida — the moat, in detail

### 3.1 Why every tool today is wrong

W3C **ALReq §4.2** identifies six Arabic justification mechanisms:
1. Inter-word space (Latin-style).
2. Intra-word space between non-joining pairs (e.g., د|ر).
3. **Alternative shapes** (`jalt`) — narrower/wider variants of init/medi/fina/isol.
4. **Ligatures** — selectively on/off.
5. **Kashida** — extending baseline connection between joined letters.
6. **Tatweel** — inserting U+0640, cheap fixed-width.

Adobe InDesign's "Justification Alternates (Naskh)" is closest commercial impl, combining (1)+(3)+(5), and **still wrong** because of where it puts kashidas.

**The structural bug** every shipping tool has:
```
[shape text] → [layout line] → [if short, insert U+0640] → DONE
```

That third step is the bug. After shaping, glyphs are contextual forms chosen by HarfBuzz based on neighbours. Inserting tatweel **changes the neighbours**, breaking the contextual forms. Worse, it can sever a mandatory **ligature**.

Khaled Hosny's example (HarfBuzz issue #3721):
```
"به بـه"  →  [heh.fina | kashida | beh.init | space |
              heh.fina | beh.init]                          // SAFE

"له لـه"  →  [heh.fina | kashida | lam.init | space |
              heh.fina.LamHeh | lam.init.LamHeh]            // UNSAFE
```

In the second, lām-heh got substituted to `.LamHeh` contextual variants. A kashida between them breaks the substitution.

### 3.2 The API fix — already exists, nobody uses it

HarfBuzz PR #3762 (merged 2022-07-30, **HB v5.1.0**, July 2022):
- `HB_GLYPH_FLAG_SAFE_TO_INSERT_TATWEEL = 0x00000004`
- `HB_BUFFER_FLAG_PRODUCE_SAFE_TO_INSERT_TATWEEL` (off by default for perf)

Shaper sets the glyph flag on cluster-leading glyphs that survived shaping without any GSUB substitutions beyond standard init/medi/fina/isol — meaning the cluster is safe to elongate.

Both `rustybuzz` and `harfrust` expose `BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL`. **No major commercial visual editor uses this.** TypoArabic's post-2022 testing of Word, Pages, InDesign, Illustrator shows same broken kashidas as pre-2022.

### 3.3 Sibahi's algorithm and critique

Abdul Rahman Sibahi (ar-ms.me):
> *"If you want to insert Kashidas, the algorithm is simple enough: insert the kashidas before the letters with the `fina` shape, or before the `medi` shape. However, complications would arise from trying to find out which glyphs or HarfBuzz clusters are final or medial."*

Key point about the engine:
> *"This is **not** a failure of the fonts. This is a failure of the justification engine."*

Priority order (matches ALReq + Mushaf Muscat):
1. Variable fonts with justification axis (rare).
2. `jalt` substitutions.
3. Kashida — **only with re-shaping**.

His structural critique: kashida is invisible to Arabic reader (baseline just gets longer); over-using makes pages visually weird without communicating anything. Combine with `jalt` and selective ligatures.

### 3.4 Hallberg's stretchable-kashida LaTeX algorithm

Hallberg's XeLaTeX `kashida-justification` package sidesteps OpenType entirely. Inserts **TeX glue** (stretchable space, not character), so font never sees a kashida and shaping is done exactly once.

Four `XeTeXintercharclass` classes:
- **`confb`** (connect both ways, can follow a connection): 22 letters — ي ئ ه ش س ق ف غ ع ض ص ن م ك ظ ط خ ح ج ث ت ب
- **`conb`** (connects on left but breaks join after): و ؤ ذ د ز ر ة
- **`alif`**: ا أ إ آ
- **`lam`**: ل only

Permitted pairs:
- confb → confb ✓
- confb → conb ✓
- confb → alif ✓
- confb → lam ✓
- lam → lam ✓
- lam → confb ✓
- lam → conb ✓
- **lam → alif ✗** (would break mandatory لا ligature)

What he loses: it's a fake kashida — straight horizontal line, not the font's own kashida glyph. Fine for body text; not for a design tool where designers picked Amiri specifically for calligraphy.

### 3.5 Our synthesis

Combine three sources:
1. **HarfBuzz** SAFE_TO_INSERT_TATWEEL → structural legality (a guard).
2. **Hallberg** class pairs → stylistic legality (avoid bad pairs even when structurally safe).
3. **Sibahi/ALReq** → combine with `jalt` so we have multiple width knobs.
4. **Re-shape after every insertion** — the easy part, that nobody else does.

#### 3.5.1 Two insertion strategies

**A. Insert U+0640 and re-shape.** Simple, standard approach. **Becomes correct** when gated on SAFE_TO_INSERT_TATWEEL.

**B. jalt-aware substitution.** Amiri, Reem Kufi, ~5 other serious fonts ship `jalt` variants. Enable `jalt`, GSUB picks best-width alternative.

Default: **B for fonts that advertise jalt; fall back to A for others; never fall back to broken Adobe behaviour.**

#### 3.5.2 Interactive drag-to-stretch state machine

```
DRAG_START:
    target_width    = current_line.width
    glyph_run       = shape_with_safe_to_insert_tatweel(line.text, font)
    candidates      = collect_safe_positions(glyph_run)
    ligature_guards = collect_ligature_positions(glyph_run, font)
    diacritic_guards= collect_diacritic_positions(line.text)
    safe_positions  = candidates
                    - ligature_guards
                    - diacritic_guards
                    - hallberg_forbidden_pairs(line.text)

DRAG_FRAME(delta_x):
    target_width += delta_x
    n_kashida = solve_kashida_count(target_width, safe_positions, current_line)
    if n_kashida == prev_n && abs(delta_x) < TINY:
        return                           // no reshape, just nudge widths
    new_input = insert_tatweel(line.text, safe_positions, n_kashida)
    reshaped  = shape(new_input, font, with_jalt = true)
    final     = drop_unsafe_insertions(reshaped, prev_reshape)
    line.glyphs = final
    repaint(line)

DRAG_END:
    line.canonical_text = new_input
    history.push(KashidaStretch{from, to, n_kashida})
```

**Perf budget:** 60 fps = 16 ms per frame. Shaping a 50-glyph Arabic line in harfrust is <0.5 ms. Re-shaping every frame is fine.

### 3.6 JSTF — skip in Phase 1

OpenType JSTF is the "official" justification mechanism. Simon Cozens on TypeDrawers:
> *"Each script also may supply a list of extender glyphs… This does not tell you (a) where extender glyphs may be inserted; (b) when they may be inserted."*

The spec defines a list but no policy. HarfBuzz parses JSTF but does nothing with it (open issue #1469, since Dec 2018). Only Windows fonts with JSTF are Arial and Times New Roman; Arial's is mostly empty.

**Position:** skip JSTF in Phase 1. SAFE_TO_INSERT_TATWEEL + jalt cycling + Hallberg pair rules deliver 90% of value.

### 3.7 Adobe's failure mode, precisely

From TypoArabic Part 2:
> *"Straight Tatweels are inserted between contextual alternates, breaking joins that are not aligned on the baseline."*

InDesign has eight kashida modes. Naskh mode is best — does use `jalt` — but still inserts kashidas where glyphs join off the baseline, producing a visible flat bar in the middle of a curve. Illustrator lacks Naskh mode entirely.

**This is the wedge.** Every Arabic typographer who has justified a paragraph in InDesign knows this failure. A tool that simply does not do it is enough to win them.

---

## 4. Reference projects and what to mine

### 4.1 Amiri and the aliftype ecosystem
`aliftype/amiri` — built by Khaled Hosny (also SAFE_TO_INSERT_TATWEEL PR author) explicitly to stress HarfBuzz. Implements full init/medi/fina/isol, hundreds of `calt` contextual ligatures, `jalt` justification alternates, `ss01`–`ssXX` stylistic sets, GPOS mark positioning. **Build the Amiri test corpus into our CI.** If our engine doesn't handle Amiri, it doesn't handle Arabic.

Other aliftype: Aref Ruqaa, Reem Kufi, Qahiri, Qashib, Rana Kufi, Astrolabe, Fontra (browser font editor), Quran-Data (Unicode Quranic corpus).

### 4.2 Cairo University parametric Arabic (2008)
Thesis: *Parameterized Arabic Font Development for Computer Typesetting Systems* (Hossam Fahmy et al.). METAFONT-style parametric letters. Theoretical foundation for **parametric letter stretching**. Phase 3+ direction.

### 4.3 Mushaf Muscat (Oman, 2017)
Only production Arabic typesetting system combining kashida + jalt + swash variants + space variation. Custom font + custom per-spread layout engine. Closed source. Proof ALReq-complete justification is achievable; reference for output quality bar.

### 4.4 Other OSS to mine
- **Godot engine** — kashida-opportunity algorithm in `text_paragraph.cpp` (lines ~332–402). MIT. **Khaled Hosny himself pointed to it as good reference. Mine directly.**
- **Typst** — Rust typesetter, open issue #195 for Arabic justification, uses rustybuzz/harfrust, no impl yet. Potential collaborator.
- **SILE** — Lua/Penlight typesetter; Cozens' experimental JSTF impl lives here.
- **Arabeyes, Saudisoft, RTL Fixer** — mostly old C code, but **mixed Arabic/Latin/digit test corpora** valuable.
- **Figma "Kashida" community plugin** — almost certainly broken post-shape insertion; install and reverse-engineer for limits. Demonstrates demand.
- **Penpot** — RTL detection + basic BiDi, no kashida. Potential downstream consumer.

---

## 5. BiDi handling

### 5.1 Crate choice
- **`unicode-bidi`** (servo/unicode-bidi) — UAX #9 in safe Rust. Used by Parley, cosmic-text, druid. Canonical.
- **`icu4x`** — provides `BidiClassAdapter` for character properties. ICU4X 2.0 (May 2025) 50–90% smaller than ICU4C.

### 5.2 Edge cases that will bite
1. **Mixed digits/Arabic/Latin.** Inkscape bug #1658510 is exactly this failure.
2. **Brackets/quotes.** UAX #9's Bidi_Paired_Bracket. Use icu4x; don't encode yourself.
3. **Paragraph direction.** Never auto-detect only — let users set explicitly.
4. **Cursor logic.** Clicks land at logical position, not visual. Parley's open issue #612 shows this still being worked out.
5. **Ligature selection.** Inkscape bug #1581275 — ligatures treated as single character. Our choice: visually break the ligature during selection, restore on deselect.

### 5.3 Can we use the browser's bidi?
- We are canvas-rendered. DOM text isn't on the menu.
- Must do bidi inside our engine for editing surface.
- For SVG **export** we emit `direction`/`dir` attributes computed by unicode-bidi.

---

## 6. Architecture

### 6.1 Layering

```
+-------------------------------------------------------------+
|  Text Tool UI  (cursor, selection, drag handles)            |
|  - kashida-handle drag                                      |
|  - per-line justify-mode toggle                             |
|  - jalt slider (Phase 2)                                    |
+-------------------------------------------------------------+
|  Layout & Editing Surface  (pivot-text crate)               |
|  - line breaker (forked from parley)                        |
|  - JUSTIFICATION ENGINE  <-- the moat                       |
|      - kashida-opportunity selector                         |
|      - jalt cycler                                          |
|      - ligature guard                                       |
|      - re-shape orchestrator                                |
|  - BiDi resolver (unicode-bidi + icu4x)                     |
|  - cursor/selection model                                   |
+-------------------------------------------------------------+
|  Shaping  (harfrust)                                        |
|  - PRODUCE_SAFE_TO_INSERT_TATWEEL on by default for ar/fa/ur|
|  - shape-plan caching                                       |
+-------------------------------------------------------------+
|  Font I/O  (skrifa + read-fonts, fontations)                |
|  - glyph outlines for text-to-paths                         |
|  - feature tables (jalt, calt, ss01..ssN)                   |
+-------------------------------------------------------------+
|  Renderer  (Skia v1, Vello v2)                              |
+-------------------------------------------------------------+
```

### 6.2 `pivot-justify` as a separate crate

Why:
- Testable in isolation against font corpus (Amiri, Cairo, Reem Kufi, Aref Ruqaa, Noto Sans Arabic).
- Reusable; OSS-extractable for upstream contribution back to Linebender/harfbuzz — marketing value.
- Versioned independently of editor UI.

```rust
pub struct JustifyRequest<'a> {
    pub text: &'a str,
    pub direction: Direction,
    pub script: Script,
    pub language: Language,
    pub font: &'a Face<'a>,
    pub target_width: f32,
    pub mode: JustifyMode,
}

pub enum JustifyMode {
    Space,
    Arabic {
        max_kashida_per_word: u8,
        max_kashida_total_ratio: f32, // 0.30 = 30% line width
        prefer_jalt: bool,
    },
    Calligraphic, // Phase 2+: all six ALReq mechanisms via constraint solver
}

pub struct JustifyResult {
    pub canonical_text: String, // text WITH inserted tatweels
    pub glyphs: Vec<ShapedGlyph>,
    pub width_achieved: f32,
    pub justification_used: JustificationBreakdown,
}

pub fn justify(req: JustifyRequest) -> JustifyResult { /* ... */ }
```

### 6.3 Interactions with other engine parts

**a) Text-to-path (vector engine).** Use `skrifa::OutlineGlyphCollection::draw` for Bezier outlines per glyph. The kashida glyph (U+0640) becomes a regular path. "Expand to paths" is one-way; warn user in Arabic + English.

**b) AI agent generating vector logos.** AI emits SVG with `<text dir="rtl">`. On import we shape and render as editable text by default; convert to paths only if AI flags layer as decorative. Kashida API callable by agent: *"AI, balance this line"* → `justify(target_width=current_width, mode=Arabic{..})`.

**c) Boolean ops.** Operate on paths only. Justification is upstream and unchanged.

---

## 7. Honest assessment

### 7.1 Can we ship in 12 months?
**MVP — yes. Full ALReq — no.**

Phase 1 MVP that beats Adobe:

| Feature | Feasibility | Risk |
|---|---|---|
| harfrust integration in WASM | drop-in dependency | low |
| BiDi via unicode-bidi | mature crate | low |
| SAFE_TO_INSERT_TATWEEL gating | flag exists in harfrust | low |
| Insert U+0640 + reshape | wired once the flag is read | low |
| Hallberg class-pair rules as guard | ~50 lines of table | low |
| Interactive drag-to-stretch | UI engineering + caching | medium |
| Lām-alif protection | hardcoded special + UNSAFE_TO_CONCAT check | low |
| RTL cursor / selection | edge-case bugs guaranteed | medium |
| Mixed ar/Latin/digits | unicode-bidi handles; integration testing | medium |

Excluded Phase 1: JSTF, jalt feature cycling per line (Phase 2), Calligraphic justification, parametric letter outlines, variable fonts with justification axis.

### 7.2 The "minimum demo that beats Adobe"

Smallest deliverable that makes Arabic typographer say *"wait, that's actually right":*

1. Load Amiri.
2. Type "الحمد لله رب العالمين الرحمن الرحيم".
3. Set justify mode to "Arabic — kashida".
4. Kashidas appear **only** where SAFE_TO_INSERT_TATWEEL allows.
5. "الله" (Allāh) never gets kashida inside it.
6. "لا" (lām-alif) never splits.
7. Drag line handle; line re-shapes every frame; no ligature breaks.

That's the demo. ~6 months at one focused FTE.

### 7.3 What can go wrong

1. **harfrust API churn.** Pre-1.0. Pin a version; monthly review.
2. **Parley fork divergence.** Owning a fork means owning merges forever. Alternative: contribute upstream and accept their pace. Spike both for a week before committing.
3. **Font corpus gaps.** Most Google Fonts Arabic lack `jalt`. The jalt-fallback to plain kashida must be graceful.
4. **Persian/Urdu/Sindhi/Uyghur.** Each has script-specific rules. We *will* hit a Sindhi user bug we hadn't considered.
5. **Performance.** Reshaping at 60 fps is theoretically fine; Arabic shaping plans larger than Latin. Measure with real user fonts.
6. **AI hallucinated Arabic.** Agent will sometimes produce almost-but-not-quite-right Arabic. Engine must visually reveal errors, not paper over them.

### 7.4 Open questions for the BLUEPRINT
- **Fork Parley or build from scratch?** Fork saves ~6 person-months on BiDi/cursor/line-break but ties our schedule to Linebender. Spike both 1 week.
- **JSTF in Phase 2?** If HarfBuzz lands JSTF (Hosny is interested), we get it free. Bet: wait and see.
- **Open-source `pivot-justify` standalone?** Strategically yes (positions Pivot as "the Arabic text people"). Lean yes, but not in Phase 1.

---

## 8. References

### Primary technical
- HarfBuzz issue [#3721](https://github.com/harfbuzz/harfbuzz/issues/3721) — Hosny, *Glyph flag for kashida insertion*.
- HarfBuzz PR [#3762](https://github.com/harfbuzz/harfbuzz/pull/3762) — merged implementation (HB v5.1.0, July 2022).
- HarfBuzz issue [#1469](https://github.com/harfbuzz/harfbuzz/issues/1469) — JSTF, open since Dec 2018.
- HarfBuzz issue [#586](https://github.com/harfbuzz/harfbuzz/issues/586) — kashida opportunity primitives; references Godot.
- HarfBuzz issue [#1503](https://github.com/harfbuzz/harfbuzz/issues/1503) — font kashida-support detection.
- `RazrFalcon/rustybuzz#74` — original rustybuzz deprecation.

### Algorithms & theory
- Abdul Rahman Sibahi, *Thoughts on Arabic Justification* — https://ar-ms.me/thoughts/practical-arabic-justification/
- Andreas Hallberg, *Stretchable kashida and Arabic text justification in LaTeX* — http://andreasmhallberg.github.io/stretchable-kashida/
- W3C *Arabic & Persian Layout Requirements* — https://www.w3.org/TR/alreq/ §4.2.
- W3C *Arabic Script Gap Analysis* — https://www.w3.org/TR/alreq-gap/
- TypoArabic, *On Arabic Justification, Part 2 — Software Implementations* — https://research.reading.ac.uk/typoarabic/on-arabic-justification-part-2-software-implementations/
- Titus Nemeth, *On Arabic Justification* — https://doi.org/10.3998/3336451.0023.104
- Simon Cozens, *Making JSTF better*, TypeDrawers — https://typedrawers.com/discussion/3465/making-jstf-better
- Cairo University thesis (2008) — http://eece.cu.edu.eg/~hfahmy/thesis/2008_01_paf.pdf
- OpenType spec, *JSTF* — https://learn.microsoft.com/en-us/typography/opentype/spec/jstf

### Rust crates
- `harfrust` — https://github.com/harfbuzz/harfrust , https://docs.rs/harfrust
- `unicode-bidi` — https://github.com/servo/unicode-bidi
- `icu4x` — https://github.com/unicode-org/icu4x
- `parley` — https://github.com/linebender/parley
- `cosmic-text` — https://github.com/pop-os/cosmic-text
- `skrifa` / `read-fonts` — https://github.com/googlefonts/fontations

### Arabic fonts & related OSS
- Amiri — https://github.com/aliftype/amiri
- aliftype — https://github.com/aliftype
- Godot kashida algorithm — see HB issue #586's `text_paragraph.cpp`.
- Mushaf Muscat — https://mara.om/muscat-mushaf/presentation-mushaf/
- Typst Arabic justification — https://github.com/typst/typst/issues/195

---

## 9. One-paragraph synthesis

The Arabic engine is built on three already-existing primitives — **harfrust** for shaping (with `PRODUCE_SAFE_TO_INSERT_TATWEEL` enabled by default for ar/fa/ur runs), **unicode-bidi + icu4x** for BiDi resolution, and **a forked-and-extended Parley line breaker** for the editing surface — wrapped by Pivot's own **`pivot-justify`** crate. `pivot-justify` adds a kashida-opportunity selector that reads HarfBuzz's safe-to-insert-tatweel glyph flag, a `jalt` feature cycler for fonts that ship Justification Alternates, and an interactive drag handle that re-shapes the line at 60 fps. The moat is that no visual design tool today — Adobe, Figma, Penpot, Microsoft — uses the HarfBuzz API that has existed since v5.1.0 (July 2022) for kashida insertion that doesn't break ligatures. We will be the first. A focused Phase 1 in twelve months delivers an MVP that already beats Adobe on the single demo every Arabic typographer recognizes: stretching a Quranic line in Amiri without breaking the lām-heh or the lām-alif ligature.

---

## TWO FACTS THAT FLIPPED PRIOR BELIEFS

1. **rustybuzz is effectively dead** (`RazrFalcon/rustybuzz#74`, Aug 2023) — harfrust is the supported successor under the harfbuzz org.

2. **The kashida API hook (`SAFE_TO_INSERT_TATWEEL`) has existed in HarfBuzz since v5.1.0 (July 2022)**, exposed in both rustybuzz and harfrust. **The moat is not "build something HarfBuzz can't do" — it's "be the first design tool that actually uses what HarfBuzz already does."**
