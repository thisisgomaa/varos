> **Status:** current — gap inventory, researcher read of the code (not READMEs), 2026-10-09. Companion to [EXTERNAL_PRIOR_ART.md](EXTERNAL_PRIOR_ART.md) (licence rules, clone paths). Nothing here is decided; every borrow still needs its own work order, our headless tests and an independent review.

# What VectorCraft / PhotoCraft have that Varos does not — and what it costs to bring it in

Paths: `VC/` = `~/Documents/AI workspace/reference/artcraft/vectorcraft` (`a469568`), `PC/` = `…/photocraft` (`4cb7cf3`).
Line counts are `wc -l` and include inline `#[cfg(test)]` modules. Both repos are MIT OR Apache-2.0
(VC `NOTICE`); no GPL/AGPL code or dependency was found in the crates below.

**Three facts that shape every row**
1. VC geometry is `kurbo::BezPath` (Apache-2.0 OR MIT). Varos core has its own `Pt`/`Anchor` model and no kurbo.
   Most "lifts" therefore need either a small Anchor↔BezPath adapter (~60 lines) plus kurbo as a new
   `varos-core` dependency (pure Rust, no UI — fits the seam; needs a one-line ADR note), or a port.
2. VC renders the canvas on the **CPU** (`vello_cpu`, `VC/crates/render`). Their *model and geometry* can come to Varos;
   their *renderer* cannot (ADR-0001: GPU). Every paint feature still needs our own WGSL work in `varos-render-wgpu`.
3. VC is eframe-based. Nothing under `VC/crates/ui-egui` (71,721 lines) or `PC/crates/ui-egui` is portable — skip it wholesale.

Effort = honest working time for one implementer to lift + adapt + write our tests (not counting owner hand-test or review).
"Scratch" = building the same thing from nothing.

## Top 10 — ranked by value to Varos ÷ lift effort

| # | Borrow | Serves | Source (lines) | Lift vs scratch | Verdict |
|---|---|---|---|---|---|
| 1 | Boolean **property-test oracles** (area identities `d+i=a`, `xor=u−i`, self-ops, point-membership sampling, Divide partitions Union) | Hardens our shipped Pathfinder (`varos-core/src/boolean.rs`); Varos has 0 `proptest` today | `VC/crates/pathops/tests/prop_pathops.rs` (259) + `VC/crates/testkit` strategies (1,541 total) | 0.5–1 d vs 3 d | ADAPT (dev-dep `proptest`, MIT/Apache) |
| 2 | **wgpu device-health watch** (records uncaptured errors + device-lost, stops GPU work instead of panicking) | ADR-0001 "GPU failure stays readable"; Varos has 0 hits for device-lost handling | `PC/crates/gpu/src/health.rs` (202, wgpu 30 vs our 29) | 0.5 d vs 2 d | COPY-WITH-ATTRIBUTION |
| 3 | **Panic guard + rollback at the command boundary** (`catch_panic` → restore doc/selection snapshot → typed `Internal` error) | Bridge edit/batch entry (ADR-0009 §3 already stages on a copy, so rollback is free) | `VC/crates/engine/src/guard.rs` (46) + `lib.rs:1073-1095` | 2–4 h vs 1 d | COPY-WITH-ATTRIBUTION |
| 4 | **Dash engine**: arc-length dashes, joined across closed starts, zero-length dashes → round/square dots, *align dashes to corners & ends* (Illustrator's mode), solid fallback for absurd patterns | B2 Stroke (PLAN_MAP §3.6: "0 grep hits for dash/cap/join") | `VC/crates/effects/src/stroke/dash.rs` (332) + tests (306) | 1 d vs 4–5 d | ADAPT (needs kurbo) |
| 5 | **Caps / joins / miter + Outline Stroke + Offset Path** = `kurbo::stroke` + boolean clean-up | B2 Stroke; Offset Path is pending (PLAN_MAP §3.3) | `VC/crates/effects/src/stroke.rs` (141), `VC/crates/pathops/src/offset.rs` (195) | 2–3 d (incl. renderer fill of the outline + PDF native caps/joins) vs 2–3 wk | ADAPT (dep: kurbo) |
| 6 | **Arrowheads** — 29 kinds, tip/extend alignment, trims the line under the head, hollow heads as rings | B2 Stroke | `VC/crates/effects/src/stroke/arrow.rs` (407) + `tests_arrows.rs` (83) | 1 d vs 4 d | COPY-WITH-ATTRIBUTION |
| 7 | **Gradient model**: stops with per-stop opacity + midpoints, `sample`, insert/move/duplicate/swap stop helpers, `GradientGeom` (start/end/aspect/focal) that follows object transforms, fit-to-bounds; plus PC's WGSL trick (stops baked to a LUT texture, `gradient_t`, dither against banding) | Gradient tab is disabled (`Paint` = `None | Solid`, `model.rs:136`) | `VC/crates/color/src/gradient.rs` (794, 15 tests); `PC/crates/gpu/src/compose.wgsl:632-680` | model 1–1.5 d + our shader/PDF 4–6 d vs 3 wk | ADAPT |
| 8 | **Curve fitting + path edit ops** (Schneider least-squares cubic fit with Newton reparam; simplify, smooth, remove redundant points, add anchors, average, join) | Pencil/Smooth/Simplify (absent from `ToolKind`), later Image Trace | `VC/crates/pathops/src/fit.rs` (232), `edit.rs` (453) | 1–1.5 d vs 5 d | ADAPT |
| 9 | **Every-line composer** (Knuth–Plass total-fit, justified 80–150 % word space, ragged with rag zone, hyphen penalties) | B1 paragraph quality; the natural hook for *our* kashida composer | `VC/crates/text/src/composer.rs` (214) | 2–3 d (we must own line breaking, see Text §) vs 1–2 wk | ADAPT-ALGORITHM |
| 10 | **Colour harmony + OKLCH palette math** (harmony rules, Color Guide variation grid, OKLCH→sRGB gamut-fit by chroma reduction) | Picker v3 work order; today `harmony_set` lives in the app (`varos-app/src/ui/picker.rs:68`), not core | `VC/crates/color/src/harmony.rs` (507, 10 tests), `libraries.rs:101-160` | 1 d vs 3 d | ADAPT (move colour math into `varos-core`) |

Runners-up: Live Corners (`VC/crates/geom/src/corners.rs`, 283, 1 d), width profiles (part of B2, below), MCP `list_commands` enabled/`disabled_reason` (idea for `capabilities`).

## 1. TEXT (first, as asked)

**Where Varos is today.** `varos-text` (4,535 lines) is a byte-fed headless engine on vendored cosmic-text 0.19 (HarfRust shaping inside), Skrifa outlines (`outlines.rs`), carets/hit-test/selection rects, styled runs with paint, Arabic boundary-context and mark-position oracle tests against bundled IBM Plex Sans Arabic (`tests/language.rs:143`, `:228`). Line wrapping is **cosmic-text's greedy word wrap** (`converge.rs:977`, `Wrap::Word`). There is **no text object in the document model** (`varos-core/src/model.rs` has none; ADR-0010 plans a TextBox in format v4; PLAN B1: "النص القابل للتحرير لسه مش موجود").

**Where VC is.** `VC/crates/text` (9,532 lines, 165 tests) on harfrust 0.13 + skrifa 0.47 + unicode-bidi, its own font DB, and a full model in `VC/crates/doc/src/text.rs` (1,307).

**Key architectural answer.** We do **not** need to switch to raw harfrust+skrifa like them. Shaping is the same family (HarfRust) and our low-level Arabic verification is already stronger. What we lack is *paragraph machinery*, and almost all of it needs one change: **Varos must break lines itself** from cosmic-text's shaped words (`ShapeLine` glyphs) instead of calling `ShapeLine::layout(… Wrap::Word …)`. Once we own the breaker, composer, area-in-shape, text wrap and threading all become pure geometry on top of cosmic-text.

| Feature | VC source (lines) | Varos today | On cosmic-text? | Lift vs scratch | Verdict |
|---|---|---|---|---|---|
| Text object model: point / area-in-any-shape / on-path (`TextKind`, `doc/text.rs:832`) | `doc/src/text.rs` (1,307) | absent (ADR-0010 TextBox planned) | n/a — our serde model (ADR-0004) | — | IDEA: use their `TextKind` + field list as a checklist; keep our schema |
| Area type in arbitrary frames (per-line span from the frame shape) | `text/src/layout.rs` (1,778) | rectangle width only (`Request.width`) | yes, after we own line breaking | 3–4 d vs 1–2 wk | ADAPT-ALGORITHM |
| Type on a path (glyphs placed by arc length, start/end brackets) | in `layout.rs` (on-path branch) | absent | yes (pure geometry on laid-out glyphs) | 2 d vs 1 wk | ADAPT-ALGORITHM |
| Every-line composer, justify modes (L/C/R/Justify/JustifyAll) | `composer.rs` (214), `layout.rs:1273-1320` | absent (ADR-0010 §Paragraphs: "Justify waits for a policy") | yes | 2–3 d | ADAPT (Top-10 #9) |
| Threaded text across frames | `thread.rs` (185), `engine/cmd/threads.rs` (311) | absent | yes | 1–2 d after area type | ADAPT later |
| Text wrap around objects | `engine/cmd/textwrap.rs` (279) | absent | yes (needs our booleans for wrap outlines) | 1–2 d after area type | ADAPT later |
| Character / paragraph styles (named, inherited) | `doc/text.rs:26` `CharStyle` (tracking, kerning, baseline shift, h/v scale, caps, features…), `:315` `ParaStyle` (indents, space before/after, tabs, direction…); `engine/cmd/textstyles.rs` (584) | absent | n/a (model) | — | IDEA (field checklist) |
| OpenType features panel; ligatures auto-dropped outside tracking −20…+55/1000 em | `features.rs` (210) | `Feature` passthrough exists (`engine.rs:26`) | yes (`FontFeatures`) | 0.5 d | ADAPT (the tracking rule is a nice detail) |
| Variable fonts (Skrifa `Location`) | `fontdb.rs:77-128` | outlines via Skrifa; axis support in our cosmic path **unverified** | partly | check first | IDEA |
| Rich-text range edit / range styling with run splitting | `edit.rs` (271) | `StyledText::replace` (`engine.rs:715`) | yes | — | IDEA (compare, ours exists) |
| Font embedding: licence check (OS/2 `fsType`) + subsetting (`subsetter`, MIT/Apache) | `embed.rs` (261) | absent (PDF has no text yet) | independent of layout | 1 d when text reaches PDF/SVG | ADAPT |
| Create Outlines | `engine/cmd/typecmd.rs:219` | glyph outlines exist (`varos-text/src/outlines.rs`) | — | — | IDEA (we have the primitive) |
| Hyphenation | `hyphen.rs` (71) — heuristic English V-CV/VC-CV, no patterns | absent | — | — | SKIP (toy; use real patterns when needed) |
| Vertical CJK, mojikumi, burasagari | `layout.rs`, `tests_vertical.rs` (18 tests) | absent | — | — | SKIP (not our moat) |
| System font catalog | `fontdb.rs` (1,730) | deliberately byte-fed, no host discovery (`varos-text/src/lib.rs:1`) | — | — | SKIP (conflicts with our boundary) |

**Arabic specifically — what VC does:** HarfRust joining, lam-alef and mark positioning (`tests_bidi.rs:121`, `:111`);
pre/post shaping context across style or font boundaries (`shape.rs:449-456`) so a colour change mid-word keeps joining;
unicode-bidi reordering with per-paragraph direction and auto alignment (`tests_bidi.rs:103`, `:241`, `:270`);
RTL caret, arrows and selection in visual order (`:72`, `:233`). **What it does not do:** no kashida/tatweel anywhere
(0 grep hits in both repos), no Middle-Eastern composer, no digit types, no character direction, no split caret —
their own `VC/ROADMAP.md:215` lists these as missing. Justification is word-space stretch only (`composer.rs:8`).
Their Arabic tests **skip silently** when no system Arabic font is installed (`tests_bidi.rs:11-30`), whereas ours
bundle the font. Net: borrow their paragraph engine ideas; Arabic quality (kashida, the BStudio engine in
`reference-prior-art-bstudio-server` memory) stays ours — VC brings nothing there.

## 2. Other missing elements

| Feature | Varos today (proof) | Source there (lines) | Drags in | Fits our laws? | Lift vs scratch | Verdict |
|---|---|---|---|---|---|---|
| **Stroke model** (cap, join, miter, align in/out, dash, arrows, scale, profile) | weight only: `stroke_width` (`model.rs:200-201`); PDF writes round caps/joins everywhere (PLAN_MAP §3.6) | `VC/crates/doc/src/appearance.rs:346` `StrokeLayer` | serde | yes (model in core) | 0.5 d | IDEA for field set; keep our serde shape |
| Caps/joins/dashes/arrows | see Top-10 #4–6 | `VC/crates/effects/src/stroke/*` (1,250 + 918 tests) | kurbo | yes (pure geometry) | 4–5 d total | ADAPT / COPY |
| **Width profiles + Width tool** | absent | `stroke/width.rs` (367): flattened polygon offsets per sample; `VC/crates/tools/src/distort/width.rs` (702) | kurbo | yes | 1.5 d model+outline; tool UI ours | ADAPT (note: output is polygons, not curves) |
| Gradient on a stroke (within/along/across) | absent | `stroke/gradient.rs` (432) | kurbo | yes | 1–2 d after gradients | ADAPT later |
| **Gradients** linear/radial (+focal, aspect) | absent (`Paint` has no gradient) | Top-10 #7; PDF via `krilla` shadings (`VC/crates/pdf/src/export.rs:766-790`) | — | model yes; render must be our WGSL; PDF via our `pdf-writer` (shading types 2/3) | 5–7 d end-to-end | ADAPT |
| **Freeform gradient** (colour points + Catmull-Rom lines, inverse-distance field) | absent | `VC/crates/color/src/freeform.rs` (575, 7 tests) | kurbo | yes; render = baked texture | 2–3 d after gradients | ADAPT later (their PDF falls back to an image or the *average colour*) |
| **Gradient mesh** (Coons patches, bilinear colour) | absent | `VC/crates/doc/src/live.rs:213-237`, `:487-680` | kurbo | yes | 1 wk | IDEA (they tessellate to solid-colour quads; PDF type-6 export not done) |
| **Live Blend** (anchor pairing, resampling, paint interpolation, spine) | absent; no live/appearance system (PLAN_MAP §3.11) | `VC/crates/doc/src/blend.rs` (1,158, 8 tests) | kurbo | yes (pure) but needs a live-object node first | 3–4 d after an Appearance ADR | ADAPT later |
| **Repeat** radial/grid/mirror | absent | `NodeKind::Repeat` (`doc/node.rs:424`), `doc/pattern.rs` (751) | — | needs live-node design | 2–3 d | IDEA |
| **Envelope distort** (warp styles, mesh, top object) | absent | `doc/live.rs` (1,773), `engine/cmd/distortcmds.rs` (1,166) | kurbo | yes | 1–2 wk | IDEA later |
| **Perspective grid** | absent | `VC/crates/geom/src/projective.rs` (186, 4 tests), `tools/distort/perspective.rs` (1,513), `cmd/perspgrid.rs` (481) | kurbo | homography: yes | homography 2 h; grid tool weeks | COPY homography when needed; rest IDEA |
| Puppet warp / ARAP, Liquify | absent | `tools/distort/arap.rs` (823), `puppet.rs` (749), `liquify.rs` (911) | — | yes | — | SKIP for v1 |
| **Shape Builder / Live Paint** (planar map incl. open paths, faces + edges) | absent; Pathfinder has 4 ops, no Divide (`boolean.rs:20-25`, PLAN_MAP §3.8) | `VC/crates/pathops/src/planar.rs` (1,136), `pathfinder.rs` (505), `tools/src/builder.rs` (1,024) | **linesweeper** (MIT/Apache per crates.io, not in local registry) | yes, but a second boolean engine beside i_overlay/flo_curves | 1–2 wk either way | ADAPT-ALGORITHM onto i_overlay; don't add linesweeper without an ADR |
| **Image Trace** (quantise → despeckle → crack-edge contours → Douglas–Peucker → cubic fit) | absent; images themselves not built (PLAN B4) | `VC/crates/trace` (1,511, 26 tests) | `image` (MIT/Apache) | yes (pure) | 2–3 d after B4 | ADAPT later |
| **Brushes** (calligraphic, scatter, art, pattern, bristle) | absent | `VC/crates/brush` (1,937, 24 tests) | — | stored in `Document::unknown["brushes"]` (`brush/lib.rs:4-6`) | — | IDEA (redo the storage properly) |
| **Symbols** + Symbolism tools | absent | `doc` `Symbol`, `tools/src/symbolism.rs` (323) | — | needs instance-node design | — | IDEA |
| **Curvature tool** | absent (PLAN_MAP §3.8 FR-4) | `tools/src/draw2/curvature.rs` (255, Catmull-Rom through points) | kurbo | yes | 1–1.5 d | ADAPT |
| **Live Corners** | absent | `geom/src/corners.rs` (283): round / inverted / chamfer | kurbo | yes | 1 d | ADAPT |
| **Recolor Artwork** (colour identity keys, Lab k-means reduction, ΔE2000 nearest) | absent | `VC/crates/color/src/recolor.rs` (586, 9 tests), `cms/lab.rs:124` | — | yes (pure) | 1–2 d math; panel ours | ADAPT later |
| **Swatches + libraries** (`.gpl` read/write, OKLCH-designed built-in libraries) | Swatches panel is a dummy (PLAN_MAP §3.4) | `color/src/palette_io.rs` (324), `libraries.rs` (534), `engine/cmd/swatchlib.rs` (505) | — | yes | 0.5–1 d | ADAPT (`.gpl` I/O + OKLCH fit) |
| Eyedropper options (pick-up/apply attribute tree, 1/3/5 px averaging) | eyedropper exists (`varos-core/src/tools/eyedropper.rs`) | `engine/cmd/xform.rs:273-300` | — | yes | 2 h | IDEA |
| **Export for Screens / slices** | absent (PLAN_MAP §3.10) | `engine/cmd/webexport.rs` (1,077), `slices.rs` (717) | `image`, encoders | yes | — | IDEA after PNG export |
| **Place / Links** | absent (B4) | `engine/cmd/links.rs` (1,189), `place.rs` | `image`, `tiff`, `zune-jpeg` | yes | — | IDEA after B4 |
| **Data Recovery** | **exists** (F1/F2, `varos-app/src/recovery_host.rs`, owner-seen 2026-10-06) | `engine/cmd/recovery.rs` (923; multi-instance area locks) | — | — | — | SKIP (Varos is single-instance: `single_instance.rs`) |
| Undo with structural sharing (`Arc<Node>` + `Arc::make_mut`; saved-state = pointer compare) | full deep clones: `undo: Vec<Document>` (`editor.rs:452`) | `engine/src/lib.rs:61-70`, `:1099` | serde `rc` | yes | 1–2 d (`Vec<Arc<Path>>`) | IDEA — do it only if a measurement shows clone cost |
| Replayable command journal with resolved values noted (`note_journal`) | Bridge has receipts; per-AI history is A3-next | `engine/src/lib.rs:987-1022` | — | yes | — | IDEA for per-AI history (record resolved values so replay is deterministic) |
| MCP prompts/resources (poster, icon-set, recolor) | Bridge has tools only | `VC/crates/mcp/src/prompts.rs` (558), `resources.rs` (268) | — | yes | 0.5 d | IDEA, low priority |
| **PC tablet pressure/tilt** (AppKit local event monitor under winit 0.30, pure pen-state mapping tested on all platforms) | absent; nothing consumes pressure yet | `PC/crates/tablet` (1,019: `macos.rs` 181, `appkit.rs` 248) | `objc2`/`block2`/`objc2-app-kit` (same versions we use) | yes — one isolated `unsafe` module in the app crate | 1 d | COPY-WITH-ATTRIBUTION *when* a pressure consumer (brush/width) exists |
| **PC colour management** (own ICC v2/v4 parser, intents, BPC, 3D LUT for GPU) | RGB only; CMYK unresolved (PLAN_MAP §3.4) | `PC/crates/cms` (5,172, 48 tests) | none (moxcms dev-only) | yes | — | SKIP now; if needed depend on `moxcms` (BSD-3 OR Apache) instead of an agent-written ICC stack |
| **PC GPU effects pipeline** (pass planner, paged tiles, effect maps, CPU-vs-GPU parity suite) | render groups/SaveLayer exist (`scene.rs`) | `PC/crates/gpu` (lib 1,929, plan 1,742, fx 715, compose.wgsl 1,178, `tests/parity.rs` 1,526) | wgpu 30 | raster-oriented | — | IDEA: copy the *parity-test pattern* when effects arrive |
| **PC codecs** | PDF + `.vrs`; PNG via tiny-skia for thumbs | `PC/crates/codecs` (13,611) | image, png, tiff, exr, zune, heic-rs (pinned, young) | yes | — | SKIP (raster-editor scope) |

**Command registry and control channel vs our Bridge.** VC's registry (`engine/src/cmd/mod.rs:94-107`) is a `static`
list of `CommandSpec {id, label, menu, shortcut, params: &'static str, enabled, run, journal}` reached by UI, CLI, a
loopback TCP channel and MCP (`run_command`). Worth taking: the `enabled → disabled_reason` pair (good for our
`capabilities`), one id per behaviour shared by menu/shortcut/Bridge, query vs journaled commands. Not worth taking:
`params` is free text, not a schema; no revision check, no idempotency, no atomic multi-op receipt — ADR-0009/0011
(typed verbs, revisions, batch = one undo step, Unix socket 0600 + Ed25519 identity) is already the stronger design.

**No-panic lints.** VC denies `unwrap_used/expect_used/panic/todo/unimplemented/unreachable` workspace-wide with
`allow-*-in-tests` (`VC/Cargo.toml` `[workspace.lints.clippy]`, `VC/clippy.toml`). Varos has ~1,270 `unwrap()/expect(`
occurrences (core 47, bridge 202, app 1,018, tests included). Adopt as a **ratchet** starting with `varos-core` and
`varos-bridge` (≈1–2 d), not a big-bang.

## 3. Worst things to avoid

1. **An unauthenticated control port that drives the UI.** VC binds `127.0.0.1:<port>` with no auth
   (`VC/apps/vectorcraft/src/control_server.rs:28`) and accepts synthetic `ui.click`/`ui.key`/`ui.drag`
   (`VC/crates/ui-egui/src/control.rs:1-24`). Our Bridge must stay socket-file + identity, verbs not clicks.
2. **Silent fidelity shortcuts.** Freeform gradients become their *average colour* in PDF slices
   (`VC/crates/pdf/src/export.rs:801-806`); meshes are solid-colour quads (`doc/live.rs:231-237`); width profiles
   are flattened polygons; brushes hide in `Document::unknown`; hyphenation is an English heuristic. If we borrow,
   we name the loss in the export report (our "no silent loss" rule).
3. **Process-global mutable colour state.** `static ACTIVE: RwLock<Option<Arc<Cms>>>` (`color/src/cms/mod.rs:721`)
   changes `Color::to_rgb` for everyone — kills determinism for headless/Bridge. Pass settings explicitly.
4. **Size and shape of the code.** `ui-egui/src/menus.rs` 198 KB, `canvas.rs` 177 KB; eframe; CPU canvas;
   1:1 Illustrator chrome. Take functions, never files of this kind.
5. **Self-graded tests that skip.** Several Arabic/system-font tests pass by skipping. Anything we lift gets our own
   tests with bundled fonts/fixtures before it lands (rule 6 of EXTERNAL_PRIOR_ART.md).

## 4. Borrowing checklist (per lift)
`// Adapted from VectorCraft <path>@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.` at the top of the item;
add the row to `varos/NOTICE` with their licence texts; kurbo/proptest/subsetter added only through the normal
dependency review; our headless tests first; owner hand-test for anything visible.
