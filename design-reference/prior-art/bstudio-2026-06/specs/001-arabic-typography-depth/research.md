# Research: Arabic Typography Depth + Universal Font Loading (001)

**Date**: 2026-06-10 · Grounded in the 5-reader recon (workflow wf_b1d6f82c) of `ui-redesign-port`.

## Ground truth that reshapes the plan (recon corrections)

- **B25 (wght threading) is ALREADY FIXED in the core**: `shape_run_weighted` builds a wght
  `ShaperInstance` (harfrust `from_variations`), threaded through kashida/visual-line/caret, and
  every canvas entry passes `Some(active_weight())`. **Residual gaps**: (1) `svg.rs` export shapes
  kashida UN-weighted (advances drift at weight≠400) and uses the old single-run RTL pipeline;
  (2) `outline.rs glyph_outline` uses `LocationRef::default()` (weight-unaware); (3) the JS
  `justifyKashida` cache key omits weight. US2/US3 = close these three, not rebuild.
- **bstudio-text is already bytes-agnostic** — every public fn takes `font_bytes: &[u8]` with plain
  borrows. The ONLY `'static` coupling is bstudio-web's `FontFamily::bytes()` / `active_font_bytes()`.
- **Shaping = harfrust 0.7.0** (HarfBuzz Rust port) + **skrifa** outlines. NOT rustybuzz.
- **deviceFont is INERT in the model**: the Rust PatchDto silently drops it and to_json never emits
  it — the cache echo erases the value on the same call that sets it. The "d:" path is a dead end
  to be REPLACED, not extended.
- Bundled fonts = 2.08MB of the 3.28MB wasm (5 ttf via include_bytes).

## R1 — Runtime font registry (engine)

**Decision**: `register_font(bytes: &[u8]) -> i32` in bstudio-web. Bytes are `Box::leak`ed
(wasm is single-threaded; session fonts live forever — leak-by-design) so `active_font_bytes()
-> &'static [u8]` keeps its signature and every call site stays untouched. Registry =
`thread_local! { RefCell<Vec<&'static [u8]>> }`; font id space: **0–4 bundled, 5+ runtime** (u8 —
256 fonts/session is plenty; `from_u8`'s silent unknown→Amiri becomes id-resolution with the same
fallback). Returns the new id, or a NEGATIVE error/flag code.
**Registration probe (one pass, returns a report)**: parse via `harfrust::FontRef::new` +
`skrifa::FontRef::new` (both Results → bad font detected here, not per-call); shape a probe string →
`kashida_safe_positions` count + tatweel→gid0 check (**warn-at-load signal — Ahmed's decision:
allow + warn**); read `font.axes()` for a `wght` axis (**enables the weight slider per font**);
Arabic cmap coverage check. Encoded as a small JSON report alongside the id.
**Alternatives rejected**: registry of owned `Vec<u8>` + with-closures (touches every call site);
widening the id beyond u8 (ABI churn for nothing).

## R2 — Font identity & persistence (the hard problem)

Runtime ids are session-ordered — `fontFamily: 7` means nothing in a reopened doc unless the same
font lands at 7. **Decision**: a doc-level **font table** in the 003 settings layer
(`setting:fonts` = JSON array, index == runtime-id − 5):
`[{ source: 'google'|'device'|'upload', family, ref }]` where ref = github-ttf URL (google) /
postscriptName (device) / content-hash (upload, bytes cached in IndexedDB `bstudio-fonts`).
On doc open, JS replays the table in order — re-fetch / re-query / IndexedDB-load — re-registering
so ids line up. Missing source (другой machine, revoked permission, offline): fallback Amiri +
a per-shape warning chip; documented v1 honesty. Embedding bytes in .bs = logged follow-up.

## R3 — Byte sources

- **Upload**: `<input type=file accept=.ttf,.otf>` → ArrayBuffer → register + IndexedDB cache by
  SHA-256 (dedupe).
- **Device** (Chromium): `queryLocalFonts()` → `FontData.blob()` → SFNT bytes (MDN-confirmed) →
  register. Arabic filtering: no API filter — heuristic family-name pass + post-registration cmap
  probe (the R1 report) decides the قائمة placement. Replaces the inert deviceFont/"d:" path.
- **Google Fonts**: keyless. A **curated static catalog JSON committed to the repo**
  (built from google-font-metadata's published artifacts, Arabic subset, ~30 families: name +
  per-weight TTF URL on `raw.githubusercontent.com/google/fonts` — CORS `*`, raw TTF = no woff2
  decompression, harfrust takes it directly). Runtime: fetch → register + IndexedDB cache.
  CSS2-parse and Developer-API rejected (key/scrape fragility).

## R4 — Picker UI (fixes "القايمة مش واضحة خالص")

Replace the native `<select>` with a popover panel: search box + sections (مضمّنة / Google /
جهازك / مرفوعة) + **per-family live preview** (a small canvas per VISIBLE row rendering
"أبجد هوز ـــ" through the engine with that font — IntersectionObserver-lazy + cached bitmap).
Badges: "بدون كشيدة" (no safe-tatweel), "wght" (variable). Selecting = `updateShape({fontFamily: id})`
for all selected texts (the A1-4 multi-apply already ships).

## R5 — Canvas == export at any weight (US2)

`svg.rs export_svg_inner`: switch `insert_kashida` → `insert_kashida_weighted` + the bidi
`shape_visual_line_weighted` pipeline (same as canvas); `outline.rs glyph_outline` gets a weight
param (axes location like lib.rs draw sites). JS: add weight to the `justifyKashida`/wrap cache keys
(textLayout.ts). Acceptance: weight-700 kashida poster exports pixel-identical.

## R6 — Weight slider + tracking (US3)

- Slider 100–1000 (step 1, snap points at 100s) shown when the active font's R1 report has wght;
  static fonts keep the 3-button preset row (nearest static mapping). Persists via existing
  textMeta weight.
- **Tracking**: extra advance must NOT break joining. The shaped stream already carries
  `safe_to_insert_tatweel`/cluster flags — apply tracking only at cluster boundaries where the
  glyphs do NOT join (flag-derived), plus word boundaries always. Engine-side param threaded through
  `shape_visual_line_weighted` (new `tracking_upem` arg, default 0) → canvas + export same path.
  UI: tracking NumberField in Typography.

## R7 — Harakat v1 (US4, Ahmed's scope: toggle + size + color)

Marks are separate glyphs with offsets in the shaped stream (`is_harakat` classifier exists in
kashida.rs). Mirror the `render_arabic_line_2c` tatweel-mask precedent: a mark mask at render —
**hide** (skip mark glyphs), **size** (scale each mark glyph around its own anchor offset),
**color** (separate fill for mark glyphs). New options struct on the render/export entries
(back-compat wrappers keep old signatures). Per-shape fields: `harakatShow?: bool`,
`harakatScale?: number`, `harakatColor?` — through the bus (model additive like opacity).
Export parity in svg.rs same mask.

## R8 — Inline-editor preview parity

The edit textarea uses a CSS font map. Runtime fonts: register a `FontFace` from the SAME bytes at
registration (JS side) so the editing preview matches the engine glyphs closely.

## Build/deploy notes

- ALL waves ride the **light text-wasm pipeline** (`wasm-pack build … --out-name pivot_web` →
  `src/engine/`, ~6s, vite content-hashes — no WASM_VERSION ritual, no emsdk risk).
- bstudio-web/bstudio-text only; render-wasm untouched. Worktree rule + sequential builds hold.
