# ADR-0010 — Arabic-first Text tool

Status: proposed — awaiting owner

Date: 2026-10-06. Owner: Ahmed. Scope: decision proposal only; no implementation or UI approval.

## Decision in plain language

نقترح **cosmic-text + HarfRust** لترتيب وتشكيل النص، مع **Skrifa** لاستخراج أشكال الحروف، و**fontdb** للخطوط. النص يفضل قابلًا للتحرير داخل `.vrs`، ويتحوّل إلى مسارات وقت الرسم والتصدير فقط. نبدأ باختبار عربي حقيقي بدون نافذة، قبل بناء أداة أو بانل. الكشيدة الاحترافية ودقة المؤشر مش ضمان جاهز من أي مكتبة؛ لازم إثبات. المقترح يحتاج موافقة أحمد، وتصميم مصوّر لكل جزء ظاهر قبل تنفيذه.

Recommend one stack: **COSMIC Text for paragraph layout and hit-test primitives, its HarfRust shaper, Skrifa for outlines/metrics, fontdb for a supplied font database, and its unicode-bidi / unicode-linebreak / unicode-segmentation analysis**. Varos owns the persisted model, editing rules and commands. No second shaper in production. Start from cosmic-text 0.19.0 as a P1 candidate, pin an exact release and resolved features in the spike; this ADR does not install it or freeze an untested dependency set.

Choose **glyph outlines through the existing path pipeline**, including CPU thumbnails and initial PDF output. Choose **format v4** for the first persisted TextBox. Keep the four crates, box system, panel layout, Illustrator shortcuts and single undo queue. All new chrome tokens belong in `shell/tokens.rs`.

This is a recommendation conditional on P1's hard gates, not a claim that Arabic typography works in Varos today. If explicit direction, shaping context or caret control cannot be delivered through a small tested adapter/upstream patch, stop and amend this ADR; do not quietly replace the engine or maintain an open-ended fork.

## Context and inspected baseline

Read first: [CLAUDE.md](../../CLAUDE.md), [AI-native vision](../VISION_AI_NATIVE.md), [PLAN](../PLAN.md), [STATUS](../foundation/STATUS.md). Local baseline: `b0c2d23`. The vision requires the same document and operations on desktop, headless Bridge and a later WASM web mirror. Neither Bridge nor web implementation is authorized here.

| Existing seam | What the source actually does; consequence for text |
|---|---|
| [model.rs](../../varos/crates/varos-core/src/model.rs), `NodeKind` ~260, `Document` ~524, `paint_list` ~726 | Has `Layer`, `Group`, `Path(u32)`; no generic Element enum or text. Paths are flat storage, nodes define hierarchy, children are front-first. `paint_list` yields path indices. Text needs a real leaf kind and mixed traversal, not hidden paths appended after all artwork. |
| [format/mod.rs](../../varos/crates/varos-core/src/format/mod.rs), [migrate.rs](../../varos/crates/varos-core/src/format/migrate.rs), [limits.rs](../../varos/crates/varos-core/src/format/limits.rs) | Current format 3; header version checked before strict decoding; old keys guarded, v1→v2→v3 migrations, structure/semantic/canonical validation on load and save. Model cap 32 MiB; file cap 256 MiB. No text limits yet. |
| [scene.rs](../../varos/crates/varos-core/src/scene.rs), `Prim` ~29 | Render-independent fills are flattened rings with even-odd fill; groups carry clipping/opacity. No glyph primitive. Outline adaptation must address font winding and overlaps. |
| [thumbs/raster.rs](../../varos/crates/varos-app/src/thumbs/raster.rs), `rasterize` ~32 | Creates a CPU Editor snapshot, calls `build_scene` at 1.0, fits content and paints with tiny-skia. It neither shapes text nor reads GPU pixels. It is in varos-app today, not a promised standalone core raster library. |
| [varos-pdf/write.rs](../../varos/crates/varos-pdf/src/write.rs), `drawn_on` ~121, `emit_rings` ~456; [export.rs](../../varos/crates/varos-pdf/src/export.rs) | Writes paths through pdf-writer, including transforms, clips and opacity. No text font resource/subsetting pipeline. Export planning also uses path bounds. Native `.vrs` embeds JSON in PDF; deliverable PDF excludes editable model and private metadata. |
| [shell/fonts.rs](../../varos/crates/varos-app/src/shell/fonts.rs) and [font assets](../../varos/crates/varos-app/assets/fonts/README.md) | Inter 400/500/600, JetBrains Mono 400 and diagnostic IBM Plex Sans Arabic already bundled. Shell font discovery is deliberately disabled. Document font discovery below is a new, separate policy; shell chains stay unchanged. |
| [editor.rs](../../varos/crates/varos-core/src/editor.rs), `ToolKind` ~34; [main.rs](../../varos/crates/varos-app/src/main.rs) | No Text tool. Existing keyboard/egui routing is not a canvas text editor; IME ownership must be integrated deliberately. |

The existing [UI_SYSTEM §8 Arabic gate](../specs/UI_SYSTEM.md#8-arabic--rtl-gate-owner-piece) explicitly holds **canvas Arabic text** off until its named tests and owner checks pass. P1 can prove a headless engine; P3 must not ship around that gate. This ADR proposes a canvas engine, not automatic approval of path A (egui patch) or B (own UI shaping) for shell fields. Ahmed must still choose the UI-field route and see Arabic in Layers, artboard names, Save As, tabs and Recent. A later explicit owner amendment can change that dependency; this paper does not.

## Ecosystem investigation

Sources inspected online on 2026-10-06; upstream branch pages move. Release docs and source manifests are evidence of architecture/APIs, not our own quality benchmark. No shaping probe, binary-size benchmark or WASM build was performed for this docs task.

Two corrections matter: cosmic-text now uses **HarfRust**, not rustybuzz; Parley switched from Swash shaping to HarfRust in 0.6.0. Current Parley describes a Fontique/HarfRust/Skrifa/ICU4X stack. “Parley + Swash” now means Parley layout plus optional Swash rasterization, not two competing Arabic shapers. [COSMIC README](https://github.com/pop-os/cosmic-text), [Parley README](https://github.com/linebender/parley), [Parley changelog](https://raw.githubusercontent.com/linebender/parley/main/CHANGELOG.md).

### Typography comparison

“Supports” here means upstream capability. Joining, ligatures and mark attachment depend on the font's OpenType tables as well as the engine. None of the evidence establishes Illustrator-quality Arabic paragraph justification for our use.

| Candidate | Arabic joining, ligatures, marks; kashida | Bidi and line breaking | Fallback and vertical metrics |
|---|---|---|---|
| **cosmic-text / HarfRust** | HarfBuzz-family contextual shaping; upstream Arabic examples. Use advanced shaping for Latin too. A `Justified` alignment is not proof of script-aware kashida placement, alternate selection or balanced paragraphs. | unicode-bidi + unicode-linebreak, wrapping, cluster-indexed layout and cursor APIs. Explicit paragraph direction and per-run language must be proven: inspected `Attrs` does not expose an obvious language field. | Custom fallback over fontdb; fontdb itself does not choose fallback. Layout exposes font/cluster identity and metrics. Test mixed-font baselines, marks above ascenders, explicit leading and actual ink bounds. |
| **Parley + optional Swash** | Current shaping is HarfRust; credible same engine family, not intrinsically worse Arabic. Swash can rasterize glyphs. No validated kashida composer established by this review. | Rich layout/editing APIs; ICU4X analysis in current stack, line breaking and alignment. Upstream changelog includes spacing and editing fixes. | Fontique handles enumeration/fallback. Skrifa supplies metrics/outlines. Strong rich-style fit; OS-backed font discovery must be disabled/replaced for core and WASM. |
| **rustybuzz + ttf-parser**, assembled layout | Rustybuzz shapes joins, ligatures and positioning; ttf-parser reads font tables/outlines, does not shape. Arabic composer, kashida policy and paragraph layout would be ours. | Add unicode-bidi, unicode-linebreak and segmentation; implement script itemization, line-edge reshaping, visual ordering and caret mapping. | Add fontdb plus our fallback algorithm; read ascender/descender/line-gap in ttf-parser and implement baseline policy. Most integration burden. |
| **HarfBuzz bindings** (`harfbuzz_rs`, `harfbuzz-sys`) | Mature upstream C/C++ shaper is a useful reference oracle. Binding/version choice controls exposed features. Still not a complete kashida paragraph composer. | Shapes directional runs; bidi paragraph analysis, wrapping, editing and fallback remain external. | Font extents exposed; native integrations optional depending on binding. We still own mixed-font line metrics. |

Evidence: [COSMIC layout fields](https://raw.githubusercontent.com/pop-os/cosmic-text/main/src/layout.rs), [Buffer](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Buffer.html), [Attrs](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Attrs.html), [rustybuzz](https://github.com/harfbuzz/rustybuzz), [ttf-parser](https://docs.rs/crate/ttf-parser/0.25.1), [HarfBuzz binding API](https://docs.rs/harfbuzz_rs/2.0.1/harfbuzz_rs/). Arabic justification requirements go beyond inserting spaces: [W3C Arabic & Persian Layout Requirements](https://www.w3.org/TR/alreq/).

### Dependencies, maintenance and portability

| Candidate | Licence / maintenance evidence | Pure core / WASM | Binary-size assessment (not measured) |
|---|---|---|---|
| cosmic-text | MIT OR Apache-2.0; inspected 0.19.0 manifest, Rust 1.89 minimum; active upstream and release API. HarfRust MIT. | Pure Rust layout is suitable **with explicit features**. Default `std` pulls sys-locale and fontdb memmap; defaults also enable Swash/fontconfig. Use byte-fed configuration, not defaults. WASM font sources still need a host adapter. | Includes layout and Unicode tables; optional raster/editor features can be omitted. Existing egui/svg font dependencies may overlap, but versions can duplicate. No honest KiB estimate yet. |
| Parley + Swash | Parley/Fontique/Skrifa and Swash MIT OR Apache-2.0; HarfRust MIT; ICU4X Unicode-3.0 notices also need auditing in resolved graph. Current branch manifest says 0.11.0; ongoing changes, not a stability guarantee. | Rust engine can be platform-independent with `system` off. Default Fontique `system` enables CoreText/DirectWrite/fontconfig bindings. Browser font enumeration is not desktop enumeration. | ICU data, optional dictionaries, Fontique and rasterization affect size. Potentially broader graph, **not evidence it is larger** than COSMIC in Varos. |
| rustybuzz + ttf-parser | Rustybuzz MIT, **archived 2026-07-26**, upstream recommends HarfRust. ttf-parser MIT OR Apache-2.0, 0.25.1 release docs inspected; do not infer it is archived because rustybuzz is. | Pure Rust, no C toolchain; good basic WASM portability. All missing layout pieces must also be portable. | Small configurable parser/shaper building blocks; custom layout, Unicode tables and font bytes erase any unsupported “smallest” claim. |
| HarfBuzz bindings | HarfBuzz Old MIT; `harfbuzz_rs` MIT (verify exact pinned package notices in any spike), published 2.0.1 docs date to 2021. Newer `harfbuzz-sys` docs do not imply that old wrapper uses that version. Upstream engine maintenance and wrapper maintenance are separate. | Native engine fails our pure-Rust/no-system-dependency requirement even when bundled. WASM compilation is possible in principle, but adds a C/C++ build/link path. | System linking externalizes bytes; bundled linking adds engine code. Neither comparison is fair without counting shipped native libraries. |

Manifest evidence: [COSMIC features](https://raw.githubusercontent.com/pop-os/cosmic-text/main/Cargo.toml), [Parley features](https://raw.githubusercontent.com/linebender/parley/main/parley/Cargo.toml), [Fontique system feature](https://raw.githubusercontent.com/linebender/parley/main/fontique/Cargo.toml), [Parley workspace](https://raw.githubusercontent.com/linebender/parley/main/Cargo.toml), [Swash](https://raw.githubusercontent.com/dfrg/swash/master/Cargo.toml), [HarfRust scope/licence](https://github.com/harfbuzz/harfrust), [binding release/dependencies](https://docs.rs/crate/harfbuzz_rs/2.0.1), [sys binding features](https://docs.rs/harfbuzz-sys/0.8.0/harfbuzz_sys/).

Supporting crates are not alternative full engines:

| Crate | Role, licence, maintenance signal and limit |
|---|---|
| [fontdb](https://github.com/RazrFalcon/fontdb) | MIT; inspected manifest 0.24.0. In-memory matching and file/directory scanning, **no OS API and no fallback policy**. Read bytes in core; host may scan system directories. Fontconfig feature is a Rust configuration parser, not by itself libfontconfig. Filesystem/memmap features still do not belong in the core text runtime. |
| [unicode-bidi](https://docs.rs/unicode-bidi/0.3.18/unicode_bidi/) | MIT OR Apache-2.0; inspected 0.3.18. UAX #9 levels/reordering, not shaping or cursor policy. Pure Rust/no_std-capable. Record supported Unicode data version in P1; crate version alone is not Unicode conformance evidence. |
| [unicode-linebreak](https://docs.rs/unicode-linebreak/0.1.5/unicode_linebreak/) | Apache-2.0; published 0.1.5 in 2023, no dependencies. UAX #14 opportunities, not width fitting, hyphenation, dictionary segmentation or Arabic justification. Low release frequency warrants checking Unicode coverage, not an unsupported “abandoned” label. |
| [unicode-segmentation](https://docs.rs/crate/unicode-segmentation/1.13.3/source/Cargo.toml) | MIT OR Apache-2.0; inspected 1.13.3 manifest. Extended grapheme and word boundaries for editing; resolved version/data tables must join the P1 inventory. Do not confuse UTF-8 bytes, Unicode scalars, graphemes and shaped clusters. |

[fontdb feature manifest](https://raw.githubusercontent.com/RazrFalcon/fontdb/master/Cargo.toml), [bidi manifest](https://raw.githubusercontent.com/servo/unicode-bidi/master/Cargo.toml), [linebreak manifest](https://raw.githubusercontent.com/axelf4/unicode-linebreak/master/Cargo.toml). Additional licence evidence: [HarfBuzz Rust wrapper](https://github.com/harfbuzz/harfbuzz_rs), [ICU properties manifest](https://docs.rs/crate/icu_properties/2.3.0/source/Cargo.toml). The [published COSMIC 0.19.0 manifest](https://docs.rs/crate/cosmic-text/0.19.0/source/Cargo.toml) also confirms the proposed feature names; branch manifests are not a lockfile.

**Why this one stack:** COSMIC already combines fontdb, bidi, wrapping and cluster hit testing without requiring a window or a native font engine. That removes more risky glue than assembling a shaper ourselves, and matches the requested fontdb host boundary directly. Parley is a serious alternative, not rejected on Arabic quality: its font-management integration and wider analysis stack offer less immediate benefit for this first bounded tool. Archived rustybuzz is a poor new foundation; native HarfBuzz conflicts with the seam. HarfRust is a maintained successor, not full HarfBuzz feature parity; its documented conformance differences remain relevant.

P1 must resolve the weakness in this recommendation: `Buffer`/`Attrs` convenience APIs may not expose every paragraph-direction, language and variation control needed. Prefer supported lower-level APIs or a small reviewed upstream change. No implementation may fake explicit direction by persisting hidden LRM/RLM characters or reverse the source string. If this fails, bring back a measured ADR amendment, with Parley as the first comparator.

## Proposed engine boundary

- `varos-core::text` owns plain requests/results: source ranges, resolved font keys, glyph IDs, advances, offsets, line baselines, ink bounds and caret stops with bidi affinity. Vendor buffer/cursor types are private, never Serde model fields or Bridge API types.
- Core takes immutable font bytes plus explicit locale/fallback configuration. It never discovers files, queries system locale, calls CoreText, imports winit/egui/wgpu or constructs a renderer. Font handles and caches are runtime values outside `Document`.
- P1 starts with cosmic-text `default-features = false`, `no_std` enabled, Swash/vi/fontconfig/std off; core itself need not become no_std. Skrifa is the one outline provider. Validate the actual resolved graph, including Cargo feature unification across the whole workspace: an app dependency must not silently reactivate cosmic-text `std`/sys-locale. Host fontdb filesystem features may read files outside core; prefer owned bytes to long-lived memory maps.
- `varos-app` loads bundled bytes, scans desktop fonts via fontdb and supplies a versioned font snapshot to the engine. Thumbnails/background PDF jobs get that same immutable snapshot, not a second lookup on the worker. The future headless host explicitly supplies fonts; tests supply fixtures only. The browser host supplies bundled/user-selected bytes, never assumes local system fonts or automatic downloads.
- Cache layout by text/style/paragraph/frame revision + font-content identity + engine policy version. Cache outlines by font hash/face index/variation coordinates/glyph ID; cache tessellation separately by scale tolerance. Font replacement invalidates layout, scene signature, thumbnail and export plan. Missing fonts are a result, not an unnoticed cache miss.

## TextBox model and persistence

Add `TextBox` in **varos-core/model.rs**, backed by `Document.text_boxes`, with `NodeKind::TextBox(u32)` as a new element kind. Keep current path storage and existing IDs; allocate text IDs from the document allocator and validate uniqueness/references across element kinds. Add a runtime typed element reference for mixed traversal and selection. Do not invent a parallel text z-order or serialize generated glyphs as editable path objects.

Proposed authored data (exact wire spellings frozen with v4 fixtures in P2):

| Part | Stored meaning |
|---|---|
| Identity/appearance | Stable ID, optional name, hidden/locked, opacity; node ancestry and unit transform follow existing path rules. Text uses existing `Paint` semantics; first release supports solid fill, with stroke deferred unless separately gated. |
| Geometry | `Point` or `Area` mode; origin in document points. Point origin is first-line baseline at paragraph start (right side for RTL); unbounded width, explicit newlines allowed. Area origin is frame top-left with positive width/height and inset; first baseline comes from line ascent. Area resize changes wrapping, not font size. |
| Content | One logical-order UTF-8 string. LF paragraphs; CRLF paste conversion is an explicit edit, not silent load normalization. Preserve Arabic code points, diacritics, ZWJ/ZWNJ, digits and bidi controls; never store presentation-form substitutions or reorder text for display. |
| Runs | Canonical sorted, non-overlapping half-open **UTF-8 byte ranges** referencing styles, covering all nonempty text; boundaries must be grapheme boundaries. Adjacent identical styles merge. Empty text retains an insertion/default style. Edits adjust ranges atomically with content. |
| Character styles | Font request (family, face/style, weight/stretch), font identity (content hash + collection face index), size in points, solid fill; language tag, OpenType feature overrides, tracking in 1/1000 em, baseline shift, variation coordinates only once P1 proves them. Required Arabic shaping features cannot be disabled by a generic Latin “ligatures off” switch. No fake bold/italic. |
| Paragraphs | Ranges aligned to paragraph boundaries; `Auto/LTR/RTL`, `Start/End/Center` alignment, explicit leading in points or Auto, spacing before/after. `Justify` waits for a separately specified policy; unsupported values fail closed. Empty paragraphs have direction/default style. |

Separate typography values from shell UI tokens: authored font size/colour are document data, not chrome constants. No invisible P5 placeholder controls or persisted fields with undefined behavior. Before P2 freezes v4, omit any unproven optional style feature above; adding it later follows ADR-0008 and may require v5. Likewise P4's area wire fields must be fully specified and round-trip tested in v4 before being accepted there; otherwise P4 bumps the format instead of stretching v4 silently.

Selection, IME preedit, caches, fontdb IDs, absolute font paths and session focus never persist. Default insertion style for an empty TextBox does persist. Preserve arbitrary source text on load; segmentation affects legal edit/style boundaries, not normalization of saved content.

**Version decision: v4, yes.** [ADR-0008](ADR-0008-vrs-format-versioning.md) requires a bump for writer-side schema changes even if a board contains no text. Keep the `.vrs` PDF container and `{varos, doc}` envelope. Set embedded JSON version and `/VAROS_SchemaVersion` together. No downgrade-save.

Migration: frozen v1/v2/v3 inputs follow existing checks then 3→4 adds an empty text collection, leaving all existing geometry, IDs, metadata, masks and transforms intact. Old files remain untouched until Save and show the existing migration notice. Reject text keys or TextBox variants in files claiming versions 1–3, including empty/null/malformed values, before defaults can conceal them. Freeze a v3-reader harness proving v4 is refused **before typed decoding**. Move current “future v4” rejection fixtures to future v5 at implementation time, preserving the test's intent.

Extend structure, authored and canonical validation together: dangling/duplicate text IDs; exact style/paragraph coverage; invalid UTF-8 offsets; grapheme splits; unknown enums/features; nonfinite/invalid size, leading, geometry, axes and opacity; oversized text/run/font requests. Initial ceilings for P1 measurement: 1 MiB UTF-8 per TextBox, 8 MiB document text, 16,384 runs per box, 65,536 runs total; existing 40,000 node cap still applies. Add bounded glyph/outline work and cancellation for hostile fonts or long combining sequences. These are proposed ceilings, not shipped constants; measure and lower before P2, and preserve the 32 MiB total model cap. Save must refuse what reopen would reject, leaving the original file and dirty state intact.

Generalize path-centric operations deliberately: mixed paint traversal, `sync_tree`, Layers leaves, effective hide/lock, groups, transform-unit lookup, artboard moves, bounds/snap, hit testing, copy/duplicate/delete, clipboard, undo, scene signature and PDF bounds planning. Boolean/path-anchor tools reject live text with a reason; a future explicit Convert to Outlines command can make ordinary paths in one undo step. Text may be clipped as content; using live text as a clipping-mask source is deferred and rejected until its silhouette semantics pass tests. Rotation uses the current unit `Xform`; do not smuggle a new general affine transform into this ADR. Object scale must have an explicit tested rule (uniform scaling of text geometry and font metrics initially); unsupported skew/nonuniform text scale is disabled with a reason.

## Layout and rendering

Layout happens in document points independent of zoom/DPI. Analyze the whole paragraph, resolve bidi, select fonts and shape with context, choose legal breaks by measured advances, reshape line edges when needed, then reorder each line for display. Do not shape paint-colour spans independently if that destroys joining; preserve context across style boundaries. Fallback should choose a face for an entire joining segment where possible, keeping bases/marks together; report a missing cluster instead of splitting it into unrelated fonts silently.

Use resolved face metrics consistently: prefer typographic ascender/descender/line-gap, documented fallback to other font metrics when missing. Auto leading derives from the maximum participating run metrics, including baseline shifts; explicit leading remains the author's value. Ink bounds are separate from line boxes: combining marks can extend outside nominal ascent. Point bounds/export culling include that ink; area clipping honors its frame and records overflow without deleting text. Horizontal text with correct vertical metrics is in scope; vertical writing is not.

**Outline route:** obtain unhinted glyph curves from Skrifa using exactly the shaped face/index/axes; scale font units to points, convert the font Y-up axis once to Varos Y-down, place with advances/offsets and apply unit transforms. Convert quadratic curves to cubic form exactly for PDF; flatten only for the screen/CPU scene with a scale-aware tolerance. Reuse existing group/clip/opacity rendering. Glyphs remain transient geometry linked to one TextBox ID; selecting a letter does not select a generated path.

A critical P1 gate: font outlines commonly require **nonzero winding**, while current `Prim::Fill` and PDF use even-odd. Do not concatenate all glyph contours into one even-odd path: overlapping Arabic joins/marks can cut holes. Prove a shared nonzero-to-region conversion, or propose a narrowly scoped render-agnostic fill-rule extension, preserving all existing path semantics. Both CPU and PDF must consume equivalent contours. A glyph atlas does not solve PDF winding.

| Route | Decision and trade-off |
|---|---|
| Paths (chosen) | Matches zoomable vector artwork, transforms, clipping and PDF. Reuses a CPU path. Cost: outline extraction/flattening/tessellation and less small-size hinting; long text requires caching and measurement. |
| Glyph atlas (deferred) | Better repeated small-glyph throughput and potentially hinted text. Adds GPU texture lifetime, scale buckets, eviction, colour glyph handling and another CPU/export route. Reconsider only after measured outline bottlenecks; never put atlas/GPU objects in core. |

CPU thumbnails: supply the resolved text layout/font snapshot before scene construction, then render outlines through tiny-skia like other content. Update the current 1.0 scene-build/final-fit seam so text curves have adequate tolerance at thumbnail scale. No egui galley, GPU readback or Renderer in tests. Shared layout/bounds must make a text-only board nonempty for thumbnail fit and `ArtworkBounds` export. Missing-font thumbnails are explicitly provisional, not cached as final with the same key. A headless snapshot later reuses this seam; extraction of the app raster module is separate work.

Initial scope is monochrome outline fonts (TrueType/CFF as validated in P1). Bitmap-only and colour-only glyphs/emoji must produce an explicit unsupported/missing-glyph result, never vanish. Variable-font coordinates require shape/outline agreement before exposure.

## Editing and host integration

The canvas editor owns a logical anchor/focus selection (byte offset + upstream/downstream affinity) and derives visual rectangles from layout. Reuse engine hit testing where correct, with a Varos adapter and headless tests; do not adopt its default editor keymap or a second undo stack. All committed content/style/frame edits flow through `EditCommand` and the existing FIFO/history. Adjacent typing can coalesce; paste, style change, cursor movement, focus change and IME commit delimit undo groups. Undo restores content, runs and useful caret state together.

Proposed caret contract, to be owner-tested against Illustrator on Mac:

- Left/Right move to the neighboring **visual** grapheme stop; in a plain RTL word, Left commonly increases the logical offset. Up/Down retain desired visual X. Shift extends from a stable logical anchor; a single logical selection can paint several disjoint bidi rectangles.
- Store affinity at direction changes and wrapped-line boundaries so one logical offset can have two visual positions. Hit testing must round-trip position + affinity. Never guess from glyph count or reverse the UTF-8 string.
- Ligatures can span several graphemes: use font ligature caret data when available; otherwise partition the advance by grapheme count as an explicitly tested fallback. Combining marks have no independent normal arrow stop. Backspace removes the previous logical grapheme and forward delete the next, not “the glyph to the left”. Word movement uses segmentation plus visual direction policy; verify Option-arrows and Command-arrows on the Mac reference before enabling them.
- Copy/cut returns logical Unicode order, including selected bidi controls, not visual glyph order. Double-click selects a logical word; drag uses layout hit testing. Undo/redo and replace-selection preserve style and grapheme invariants. Paint-only style splits cannot change Arabic joining.

IME belongs to `varos-app`: enable winit IME only for the focused canvas text session, position the candidate rectangle from the transformed caret in window coordinates, and route `Enabled/Preedit/Commit/Disabled` explicitly. Preedit is transient, visibly marked, excluded from document/save/undo; Commit replaces its captured logical selection once. Empty Preedit clears composition; it must not delete committed text. Normal Unicode input comes from text events, not physical keycodes; suppress duplicate character insertion around IME commits. Winit's local 0.30 event docs specify Preedit cursor ranges in **bytes**; convert/validate explicitly.

Focus, tab switch, save and close must settle composition without guessing its result: ask the host to commit where supported, otherwise cancel only preedit and preserve committed text before proceeding. Escape cancels preedit first, then leaves text editing; it does not undo previously committed typing. During composition, candidate-navigation keys belong to IME and tool shortcuts must not fire. Test event sequences headlessly, then real Mac dead keys and a composing IME; Arabic keyboard typing alone does not prove IME correctness. Browser input/composition bridging is future host work; no DOM-based editor replaces the shared canvas model.

## Tool, cursor and Properties: requirements for later mockups

This section is a **plan**, not a visual design. No controls, cursor artwork or shell layout are built by this ADR. Each visible stage stops at mockup images and owner choice before code; P5 does not retroactively authorize P2's cursor/tool UI.

- **T** activates Type only outside text/field editing. While editing, `t` inserts text. Click empty canvas creates point text at a baseline; drag defines rectangular area text (P4). Click existing text places the caret; clicking a locked/hidden element does not edit it. Drag threshold, empty-object cancellation and point/area conversion need approved interaction mockups.
- Illustrator-like horizontal Type I-beam: creation indicator over empty canvas, insertion I-beam over editable text, area-creation preview during drag. Native cursor implementation stays in the app cursor seam, with owner-approved images and tokens; never reuse a path crosshair as finished type feedback. Type-on-path, vertical type and arbitrary-shape area containers are deferred.
- **⌘T** reveals/focuses the **Character** controls (Ctrl+T on Windows compile path), never creates a tab. Follow ONE-HOME: propose a Character section within existing Properties, with the command revealing that home rather than a duplicate floating panel. Character/paragraph arrangement is Ahmed's choice; changing box mechanics is not an option in this task.
- ⌘A/C/X/V/Z/⇧⌘Z apply to the active text session; object commands resume after exit. Escape returns to object selection after composition is settled. Return inserts a paragraph; it is not a generic “finish text” shortcut. All existing shortcut arbitration remains centralized; verify other Illustrator text shortcuts before adding them.

Verified reference behavior: [Adobe shortcut table: T and Command+T](https://helpx.adobe.com/illustrator/using/default-keyboard-shortcuts.html), [Adobe point/area text workflow](https://www.adobe.com/us/learn/illustrator/web/design-text-based-layout). Exact RTL caret/cursor appearance is **not** hands-on verified here.

Properties needs: font family/search and real face, size, leading, tracking, solid fill, paragraph direction separate from alignment, language, supported OpenType features, point/area mode and area dimensions/inset/overflow status. Show mixed selection values without overwriting them. Whole-object selection formats its text; a text-range selection formats that range; an insertion caret sets the next-typed style. Missing/substituted font identity must be visible. Mark-position controls, kashida amount, advanced justification, stroke, text-on-path, linked frames and variable axes are withheld until their own engine/design gates; no attractive dead controls. Reuse field commit law and the existing box/layout system. Ahmed chooses density, ordering and collapsed sections from later images.

## Fonts and reproducibility

Reuse bundled Inter and JetBrains Mono as document choices as well as UI assets, without duplicating font bytes unnecessarily. **Bundle/use the existing IBM Plex Sans Arabic Regular as the initial Arabic document fallback**, subject to P1's quality review. It is already present under SIL OFL 1.1 with Reserved Font Name “Plex”; preserve the original file and notice. This does not promote it into the blocked egui UI fallback chains. It provides a deterministic starting face, not a promise of Naskh/calligraphic quality. Additional Arabic families/styles need Ahmed's visual choice and an exact source/hash/licence audit.

Desktop system fonts are discovered through host fontdb. Scan asynchronously with bounded work, identify collections by face index, and resolve style/coverage before shaping. Directory scanning may not see every transient/macOS-activated font; do not advertise parity with CoreText enumeration without testing it. Prefer exact font hash+index, then requested family/style with an explicit mismatch, then the configured bundled fallback. Never silently rewrite the author's font request to the substitute on load.

**v4 does not embed arbitrary font binaries.** It stores requests and content identities, not machine paths. Thus portable re-editing is exact only with the same font bytes and engine policy; the native PDF appearance preserves the last saved outlined view for ordinary PDF viewers. If a font is missing or changed, open the editable data with a clear substitution state and provisional layout; require explicit font resolution/replacement before writing a changed appearance to `.vrs` or final PDF. Recovery may retain authored model data without blessing the provisional rendering. Outline export is not a font-distribution or licence loophole. System font usage/embedding rights vary; do not automatically package them or upload local fonts to the web mirror.

Inter/JetBrains/Plex notices and hashes are already in [assets/fonts](../../varos/crates/varos-app/assets/fonts/README.md). Bundling additional or modified OFL fonts must preserve the licence and respect reserved names; this is a packaging constraint, not a legal opinion. Keep full editable-font packaging and font-subset-in-`.vrs` out of v4; they need separate resource/permission/limit decisions.

## PDF decision

**Initial PDF text is outlined**, for both `.vrs` appearance pages and standalone export. The source TextBox/runs remain editable in `.vrs` JSON; Export still carries no editable model. Extend the shared page writer and export bounds to consume the same resolved text geometry as the screen. Maintain hidden-object filtering, page clipping, group opacity and cancellation. Freeze font/layout snapshots when queuing the background job; do not re-resolve fonts halfway through a multi-page save.

| Choice | Consequence |
|---|---|
| Outlines now | Fits the current writer and preserves Arabic glyph positions with no viewer font dependency. Larger files for long text; no searchable/selectable/accessibility text in PDF. State this plainly in later export copy. Does not make the `.vrs` font-independent for future editing. |
| Embed subset later | Smaller repeated text and potential search/copy/accessibility, but needs a subsetter, permitted embedding, CID/Type0 font resources, glyph remapping, positioning, widths, `ToUnicode`, and logical text mapping for ligatures/bidi (possibly marked-content `ActualText`). Basic Unicode-to-glyph encoding is insufficient. |

Do not pretend pdf-writer or HarfRust automatically provides that full subset pipeline. HarfRust's upstream notes point to separate fontations subsetting work; evaluate a pure-Rust subsetter in a later PDF ADR. Tests for that later path must cover Arabic copy/search in logical order, many-to-one clusters, fonts prohibiting subsetting/embedding, variable fonts and multiple PDF viewers. Outlining is the P2/P3 delivery choice, not permanent rejection of accessible PDFs.

## Gated pieces and risks

No stage below starts from this proposed ADR alone. Each gets a bounded work order; every visible stage first delivers mockup images for Ahmed. All automated tests stay headless: no GPU Renderer or EventLoop construction. Real-window checks belong to Ahmed in implementation stages, never this docs task.

| Piece | Deliverable | Exit gate / stop condition |
|---|---|---|
| **P1 — engine spike** | Throwaway/pure headless harness; pin engine, fonts/hashes and Unicode data versions; expose layout, outlines, bidi caret map. No format or UI change. | Corpus below; explicit direction/language/context controls proven; identical byte-fed desktop/WASM results within declared tolerance; no native/UI deps in core feature graph; winding/marks/metrics/fallback tests; CPU proof sheets reviewed by Ahmed. Measure binary/time/memory; fail any hard gate → amend ADR before P2. |
| **P2 — Latin point text** | Approved Type/caret mockups, TextBox/commands, v4 migration, point Latin editing, immutable font snapshot, CPU thumbs and outlined PDF save/export. | Frozen old/new/refusal fixtures; text-only/mixed-path order, transforms/clips/undo/duplicate/delete; save→reopen exact source/runs, background export; Latin IME/dead keys and owner window check. No “Arabic supported” claim. Schema features not ready remain rejected. |
| **P3 — Arabic / RTL** | Joining/marks, explicit paragraph direction, mixed bidi caret/selection, Arabic fallback and outlined PDF parity. | P1 corpus becomes regression suite; no cursor jumps/data loss; native input, copy/paste and rotated-caret candidate placement seen by owner; existing UI_SYSTEM §8 gate passes (or owner explicitly revises it). No automatic kashida/advanced justification claim. |
| **P4 — area text** | Approved frame/overflow mockups, width-based wrapping, leading/alignment, frame clipping, resize reflow; source overflow retained. | Resize/undo/reopen parity; line-edge Arabic joins/marks, narrow frames, mixed fonts, ink bounds and overflow; CPU/PDF match; schema bump if v4 did not already define the wire contract. Full Arabic justification stays a separate research gate. |
| **P5 — Properties** | Owner-selected Character/Paragraph mockups in existing section homes, then working controls and ⌘T reveal/focus. | ONE-HOME, mixed values/ranges, field commit/undo rules, missing-font resolution, Illustrator shortcut routing and Arabic UI gate; tokens only in shell/tokens.rs; box behavior and layout unchanged. |

### P1 headless corpus and measurement contract

These are **planned cases, not tests that have passed**. Use exact pinned Inter/Plex bytes for baseline and at least one separately licensed contrasting Arabic font as a test-only fixture (after provenance audit). Record glyph IDs only per font hash; font changes invalidate goldens deliberately. Differential shaping against a pinned external `hb-shape` is useful as a development oracle, never a core/runtime dependency and never the sole proof of visual quality.

| Text / input | Required evidence |
|---|---|
| `السلام عليكم ورحمة الله` | Contextual joins, spaces, logical source unchanged; glyph IDs/advances/offsets/cluster ranges plus CPU image. |
| `لا لأ لإ لآ — الله` | Lam-alef and font-supported required ligatures; grapheme caret positions inside multi-character clusters; no universal glyph-count assertion. |
| `السَّلَامُ عَلَيْكُمْ` and `قُرْآنٌ كَرِيمٌ` | Stacked marks attached to correct bases, no clipping above/below line; selection/delete round-trip. |
| `Logo شعار v2` and `السعر ١٢٣٫٤٥ ج.م. (USD 12.50)` | UBA levels and per-line visual order, neutrals/brackets/numbers, two caret affinities, logical-order copy. |
| `موعدنا يوم الثلاثاء الساعة 10:30 صباحًا.` | Narrow/wide area wrapping, break opportunities and line-edge reshaping; RTL Start/End alignment. |
| `می‌روم` (contains U+200C), `ب‍ب` (U+200D), `ب‌ب` (U+200C), `سـلام` (literal U+0640) | Joining controls preserved, correct segmentation, literal tatweel respected. Literal tatweel is not evidence of automatic kashida. |
| `شعار` with a paint-only run split inside the word; the same text with a deliberate font change | Paint boundary does not break joins; true font boundary has deterministic context/fallback behavior. |
| ASCII-first text with explicit RTL, Arabic-first text with explicit LTR; empty RTL paragraph | Explicit direction independent of first strong character; never mutate the stored string to force it. |
| `A\u{2067}شعار 12\u{2069}Z` (decode escapes), NBSP, LF, CRLF paste, trailing spaces | Isolates/end-of-line behavior, stable byte maps, intentional newline edit semantics. |
| `office café e\u{0301}` (decode escape), missing Arabic face, emoji ZWJ sequence | Latin ligatures/combining marks, whole-cluster fallback or explicit unsupported result; no silent glyph loss. |

For each: deterministic shape/layout with the same inputs; finite metrics; legal cluster ranges; no splitting base/mark during wrap; visual caret walk terminates and hit-test/affinity round-trips; logical selection copies exactly; delete/insert/undo preserves styles. Include 12/48/200 pt, mixed sizes, baseline shift, narrow frames, rotations, overlapping contours, empty/long text, malformed fonts and resource-limit refusal. Compare native and `wasm32-unknown-unknown` harness output; merely compiling WASM is not rendering/IME acceptance. No browser UI is built in P1.

Measure release builds with identical compiler/profile/LTO/strip settings: baseline and stack delta, standalone headless harness, whole-app executable, font assets separately, WASM raw and compressed bytes. Use the same corpus/feature set for any Parley comparison. Report cold font loading, cold layout, warm single-character edit p50/p95, memory/cache size, CPU outline draw and repeated-text PDF size. Suggested initial workload: 1,000/10,000/100,000 characters and 100 repeated labels; candidate warm-edit target ≤16 ms for the 10,000-character paragraph on the recorded Mac, subject to owner scope choice after measurement. Do not invent size/performance wins from crate download size or README benchmarks. Enforce measured bounds before shipping.

Primary risks: integration controls missing from COSMIC; Unicode-version drift; mixed-font baselines/marks; ambiguous bidi carets; nonzero/even-odd mismatch; large outline cost; font availability/licensing; IME double insertion; path-only assumptions dropping text; PDF non-searchability. Each has a concrete gate above. Expert Arabic composition (kashida placement, justification alternates, word-space balance) remains a separate product research requirement after P4, with Ahmed's visual samples; never implement it as arbitrary U+0640 insertion or tracking between joined letters.

## Evidence and remaining decisions

This document is based on local source inspection and linked upstream documentation, not a working Text tool. No dependencies, Rust source, font assets, format constants, UI or applications were changed. No shaping-quality, final binary-size, text-rendering performance, WASM runtime or real-window result is claimed.

Documentation-task validation on 2026-10-06, run from `varos/` (Rust 1.98.1):

| Required check | Result |
|---|---|
| `cargo test --workspace -j 4` | PASS: 1,087 passed, 0 failed, 8 ignored; existing suite, not text-engine acceptance. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS. |
| `cargo fmt --all --check` | PASS. |
| `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` | PASS; compile/lint only, no Windows runtime claim. |

Repository-relative link targets and whitespace checked. No GUI launched, commit/push performed or application installation touched. This proposal has had a self-review, not an independent agent review.

Awaiting Ahmed: accept/reject the stack direction and outline-first PDF trade-off; confirm initial Plex Arabic document face; choose the existing Arabic UI gate route; approve later visible mockups. P1 still owes measured feasibility before production code. Accepting this ADR does not accept a panel design or authorize changing the box system.
