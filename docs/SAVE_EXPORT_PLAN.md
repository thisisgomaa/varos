> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — SAVE (#6) & EXPORT (#10): Staged Plan

> The container-format decision (PDF-native, Illustrator `.ai` dual-data model) is **already made** — this plan grounds it, stages it, and names exact crates/APIs. Synthesized from the 7 research angles.

## 0. The one decision that governs everything

**The editable model is the source of truth. The PDF is a container.** Varos saves ONE file that is simultaneously a **valid, universally-openable PDF** (renders in Acrobat/Chrome/Preview/RIP) AND a carrier for **Varos's own editable model**, embedded privately, so Varos re-opens it *fully editable*. This is the literal Illustrator `.ai` pattern — ~30-year proven, not a hack.

**Non-negotiable hedge:** `varos-core` serializes to a **format-agnostic blob**. PDF object semantics must NEVER leak into `varos-core`. PDF is the container *today*; if it ever proves too costly, swap the container with **zero model rewrite**. Architect the seam now.

## 1. File architecture — valid PDF + embedded model

```
my-design.pdf  (%PDF-, valid PDF 2.0 / ISO 32000-2)
├── Standard PDF (the RENDERED projection every viewer sees)
│   ├── one page per artboard (per-page print boxes)
│   ├── vector streams: paths/fills/strokes/clips/masks/gradients
│   ├── subset-embedded fonts (Arabic shaped via HarfBuzz, NOT outlined)
│   └── ToUnicode CMaps + /ActualText (plain PDF copies correct Arabic)
└── PRIVATE editable model (SOURCE OF TRUTH — only Varos reads)
    ├── EmbeddedFile stream: versioned, compressed CBOR of varos-core doc
    ├── catalog /AF array, /AFRelationship = Source  (PDF 2.0 Associated Files)
    ├── Names tree /EmbeddedFiles (most-preserved location)
    └── catalog /VAROS_Model + /VAROS_SchemaVersion  (Annex E private key)
```

**Two standards-blessed slots, used together:** (1) **Associated File** = the payload home (`/AF` with `/AFRelationship = Source` — "Source" literally means "data this page was generated from"; also list in `/EmbeddedFiles`). (2) **Private catalog key** per **ISO 32000-2 Annex E** = the cheap flag Varos checks first — a **second-class** name `/VAROS_Model` (+ `/VAROS_SchemaVersion`). **Register the `VAROS` prefix** at `github.com/adobe/pdf-names-list`. We do NOT use Adobe's undocumented `/PieceInfo` (reverse-engineered, only for Illustrator interop — out of scope).

**Other apps safely IGNORE it (honest limit):** readers are spec-required to ignore unknown keys/`/AF` — page renders normally. But **"ignore" ≠ "preserve"**: a third-party re-save/optimize/preflight MAY drop our private stream. Acceptable — the rendered PDF always survives; **full re-editability round-trips through Varos only** (exactly Illustrator's trade). Say this in product copy.

**Versioning:** model = self-describing, additive, tolerant **CBOR (`ciborium`)**, not a rigid struct. Version carried twice (`/VAROS_SchemaVersion` int + a field inside the stream). Compress hard (`FlateDecode`/`zstd`). Newer-than-parseable schema → fall back to view-only, keep PDF as fallback, never corrupt.

## 2. Rust stack

**WRITE:** **`krilla`** (~v0.8, Typst's PDF backend) for page rendering — fills/strokes/clips/masks/blend/gradients/patterns/images, CFF+TTF subsetting, **low-level positioned-glyph API**, auto **ToUnicode + `/ActualText`**, CMYK/ICC/separation, PDF/A-1/2/3/4, tagged PDF, `embed` module. Drop to **`pdf-writer`** (~v0.15, krilla's own dependency = built-in escape hatch) for `/AF`+`/VAROS_` plumbing, OCGs, print boxes, overprint — it does NOT validate, so we own correctness. **Rejected base: `printpdf`** (its docs mark Gradients/Patterns/**File attachments as NOT supported** — kills the embedded model); `oxidize-pdf` (RAG-focused).

**READ:** **`lopdf`** (~v0.42, most battle-tested object-level parser) recovers the blob: walk catalog `/AF` / `/Names → /EmbeddedFiles`, `FlateDecode` the stream. No one-call "get my attachment" helper — expect real name-tree parsing (bounded since we control both ends of our own files). **`pdf`** (pdf-rs) secondary for foreign PDFs; **`pdfium-render`** feature-gated for raster preview (pulls C++ pdfium — keep OUT of core path).

**Shaping:** `rustybuzz` default; `harfrust` modern alt; **`harfbuzz_rs` escape hatch** behind a trait seam (the only one with the Arabic fallback shaper — pick it now so a shaping bug isn't mistaken for a format bug). Honest gap: pure-Rust shapers have no fallback shaper, which only bites broken/synthetic fonts, NOT real Arabic fonts. Plus `unicode-bidi`, `ttf-parser`/`read-fonts`.

> Integration seam: krilla doesn't expose `/AF`/private keys/OCGs/named boxes as first-class, and its "layer" = transparency isolation (not Acrobat layers). Hand-write that in `pdf-writer`, keep in sync with krilla's xref/objects, wrap krilla behind a `varos-export-pdf` trait (young, pre-1.0, ~single-maintainer — mitigated by Typst-org backing + pdf-writer fallback), pin versions.

## 3. SAVE system (#6) — staged

**Firewall:** "Save/Open Varos's own PDF" = #6, ships first. "Import arbitrary third-party PDF/AI" = SEPARATE, later, explicitly-lossy. Never let foreign-PDF difficulty block native save.

- **Stage 0 — 1-week de-risking spike (gate before committing):** serialize mixed Arabic+Latin doc → CBOR; render visible page via krilla (HarfBuzz + ToUnicode + ActualText); embed blob via krilla `embed` (or `pdf-writer` for `/AF`+`/VAROS_`); confirm Acrobat+Chrome+Preview render Arabic correct + selectable AND Varos re-opens byte-perfect. Pass → full GO.
- **Stage 1 — MVP save:** single artboard→page (MediaBox); paths/solid fills/strokes/z-order/basic clips/opacity; subset fonts; Arabic moat (minimal: HarfBuzz→krilla glyph API `outlined=false`+ToUnicode+ActualText); CBOR model via `/AF`+`/VAROS_`; `lopdf` recovers on open. **Build the CI validation harness now** (qpdf + pdf.js/PyMuPDF render, Arabic copy-extract, re-open byte-perfect) — guards pdf-writer's no-validation footgun.
- **Stage 2 — full fidelity:** multi-page=artboards (Trim/Bleed boxes), layers=OCGs (`/OC BDC/EMC`, `/Order`/`/ON`/`/OFF`/`/Locked`), gradients/patterns/soft-masks/blend/nested groups, images-once, full color model, "pure PDF / no editing data" export option.
- **Stage 3 — robustness:** schema migration, atomic/crash-safe save + autosave, "dump embedded model" debug tool, detect-and-warn on missing `/VAROS_Model`, optional `.varos` sidecar later.

## 4. COLOR / CMYK

Illustrator **split-brain: CMYK is lossless truth; on-screen RGB is computed, never stored.** Model = tagged union `Process{RGB|CMYK|Gray, ICC?}` / `Spot{name, tint, alternate, tintTransform}` + global Swatch table. **Never auto-convert** (the classic Illustrator silent-shift bug) — conversion is explicit, user-triggered, stores originals, warns on gamut clip. **PDF emission:** process→`ICCBased` (fallback Device*); each spot→**`/Separation`** once (ink name verbatim + tintTransform for preview); multi-ink→**`/DeviceN`**; registration→`/All`. Engine: **`moxcms`** (pure-Rust, default preview/soft-proof) + **`lcms2`** behind a flag for prepress-exact CMYK→CMYK + BPC (final print PDF); avoid `qcms`. Intents: Relative Colorimetric default. **ICC packaging is a real legal task** — FOGRA/GRACoL/SWOP have redistribution restrictions; ship freely-redistributable (sRGB v4) + let users supply press profiles.

## 5. Multi-page + layers

Artboard→page 1:1 (`MediaBox`⊇`BleedBox`⊇`TrimBox`⊇`ArtBox`; CropBox=MediaBox; mixed sizes native). Layer→**document-level OCG** referenced per-page via `/OC BDC…EMC`; **OCGs don't nest** — tree via `/Order` in `/D`; eye→`/ON`/`/OFF`, lock→`/Locked`. Groups/clips/masks/opacity/z-order = content-stream primitives (`q/Q`, `W n`, Form XObjects, ExtGState `/ca`/`/SMask`), **independent of OCGs**; embedded model stores the true group tree. **krilla can't emit OCGs/print boxes → hand-write in pdf-writer + veraPDF/Acrobat-preflight pass** (malformed OCProperties = Acrobat silently drops layers). Tagged-PDF structure tree = separate later phase.

## 6. ARABIC — THE MOAT

**Layer 1 (editability):** logical Unicode + font + run metadata live in the **embedded model**, never outlined; Varos reads THAT, never the glyph soup. **Layer 2 (render+extract everywhere):** per run — (1) `unicode-bidi`→`rustybuzz` (`dir=RTL`, `script=Arab`); **keep logical order + shape RTL, never pre-reverse**; clusters = byte offsets back to logical string; (2) subset-embed font (outlining forbidden); (3) **`/ToUnicode` + `/ActualText`** — ActualText is **non-optional** for Arabic (ligatures/reused glyphs break ToUnicode; without it copy-paste reverses/garbles — PyMuPDF #2199, etc.). **krilla auto-emits all three** from `KrillaGlyph::new(…, range: Range<usize>, …)` where `range` = the glyph's cluster byte-range in the logical string. Varos wins twice: editable model + correct selectable plain-PDF Arabic.

**Recovering OTHERS' Arabic PDFs — honest 3-tier (separate, later, lossy):** T1 ToUnicode→base U+0600–06FF = clean/editable; T2 ToUnicode→Presentation-Forms-B U+FE70–FEFC = normalize→base + re-bidi (heuristic, lossy, "verify before editing"); T3 outlined or no-ToUnicode+custom encoding = OCR only (import as image/paths). **Promise:** *"Varos files stay editable; foreign PDFs recover when they carry real text + ToUnicode."* NOT "edit any Arabic PDF." **Test corpus** (lam-alef, harakat, tatweel, mixed/bidi) verified by **real copy-paste in Acrobat/Chrome/Preview** — headless won't catch clustering bugs.

## 7. Top risks + adversarial verdict

**VERDICT: GO PDF-native — survives review.** The scary "Arabic dies in PDF" objection is about *extracting from the content stream*; Varos sidesteps it on its own files (re-opens from embedded model). It's real only for interop, which *any* format faces. SVG-native doesn't escape: SVG also needs proprietary namespaces for an editable model AND has **no CMYK/PDF-X** — making print PDF (the key MENA deliverable) second-class. PDF-native is structurally better here.

**GO gated on three conditions:** (1) format-agnostic model (§0); (2) firewall Save from foreign-import (**scope-creep into "open any PDF" is the #1 sinker** — lopdf fails ~20% of real corpora); (3) the 1-week spike passes. Fail BOTH dual-stream validity AND visual Arabic correctness → fall back to SVG-native + PDF-export.

**Top risks/mitigations:** preservation≠ignoring (self-sufficient standard layer + `/EmbeddedFiles` + detect-and-warn + later `.varos` sidecar); scope-creep (separate deadlines); pdf-writer no-validation (CI round-trip suite); ToUnicode/ActualText clustering (corpus + multi-viewer); file bloat (compress, store rasters once, "pure PDF" export); krilla young (trait wrapper + pdf-writer fallback); PDF/A vs private-data + **PDF/X strips OCGs/transparency/private data** → print export = a *different profile*, not one file; Arabic fallback-shaper gap (`harfbuzz_rs` escape hatch); binary-PDF debuggability (dump tool).

## 8. EXPORT system (#10) — staged

**Architecture: two render paths, ONE scene — do NOT screenshot the wgpu framebuffer.** `varos-core` → backend-agnostic **`RenderScene` IR** → three sinks: wgpu (canvas), vector serializers (SVG/PDF), CPU rasterizer (PNG/JPG/WEBP). GPU readback is non-deterministic across drivers/machines, can't tile huge @Nx exports, needs a GPU — reserve it only for "copy current viewport". Modules: `varos-export-scene/-raster/-pdf/-svg/-jobs`.

- **Stage 1 — sinks:** raster = **`tiny-skia`** CPU (same engine as resvg, deterministic; tile + stream rows for @Nx) → `png`+`oxipng`, `jpeg-encoder`/`zune` (pure-Rust) or `mozjpeg` (flagged), `image-webp`; resample `fast_image_resize`. PDF = **krilla** (moat path) + embedded model. SVG = **direct scene serialization** (not resvg — it reads), precision slider + embed/link + text/outline toggles via `svag`/`svgtidy`; **Arabic-SVG defaults to OUTLINED paths** with a warning (`<text>` reshapes/breaks in the wild).
- **Stage 2 — asset/scale UX + clipboard:** pure orchestration — `{source × scale × format × suffix × opts}` on a **`rayon`-parallel, cancelable, off-UI-thread** job runner; "Save for Web" = job options; clipboard via `arboard`/`windows-rs`.
- **Stage 3 — Print PDF/X (deferred, EXTRA WORK — no turnkey Rust crate):** thin `pdf-writer` post-pass on krilla (or upstream PR). **X-4 default** (live transparency + ICC RGB + OutputIntent); **X-1a fallback** needs the **transparency flattener** (its own hard subsystem → lower priority). Bleed 3mm, black overprint, registration `/All`. **PDF/X export STRIPS `/VAROS_Model`+`/AF`**; validate with veraPDF/Ghostscript/Preflight ("opens in a viewer" ≠ conformant); never let CMYK/flatten silently outline Arabic. EPS skipped unless demanded.

---

**Full document:** `C:\Users\Gomaa\AppData\Local\Temp\claude\D--My-work-Desgin-tool\ba2e211b-f805-4bf9-b31a-cbeeac15ef91\scratchpad\SAVE_EXPORT_PLAN.md` (note: this is the session scratchpad — move it into the Varos repo/docs when the Save phase starts, since scratchpad is session-temporary).
