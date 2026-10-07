> **Status:** proposed — awaiting owner (Ahmed). Owner chose path B of [UI_SYSTEM §8](../specs/UI_SYSTEM.md#8-arabic--rtl-gate-owner-piece) on 2026-10-07; this paper says how. No implementation is authorized by this document alone.
# ADR-0012: One text engine for the UI and the canvas — `varos-text` + `kit::text`

**ملخص بالمصري**

العربي في الواجهة بيطلع مربعات فاضية النهارده، وإنت اخترت الحل الكامل (B) مش الترقيعة.\
نطلّع محرك النص بتاع تجربة P1b (cosmic-text المتعدّل) لمكتبة حقيقية اسمها `varos-text`، وهي نفسها اللي أداة النص على الكانفس هتستخدمها بعدين.\
أسماء التابات واللايرز والـRecent والكروت هتترسم بمحركنا جوه egui من غير ما نعدّل egui نفسه؛ والأرقام والقوائم الثابتة تفضل زي ما هي.\
خانات الكتابة (اسم البورد والوصف والتاجز والـrename والبحث) هتبقى خانة بتاعتنا: مؤشر وتحديد ونسخ/لصق وكيبورد عربي، بنفس قانون الحفظ لما تسيب الخانة.\
الخط العربي Plex Arabic، ونضيف وزنين Medium وSemiBold بنفس الرخصة بدل التخين المزيّف؛ الاتجاه بيتحدد من أول حرف، والواجهة نفسها تفضل شمال-ليمين.\
أربع قطع ببوابات: T1 المكتبة، T2 الأسماء المقروءة، T3 الخانات + اختبارات §8 + تجربتك بإيدك، T4 أداة النص؛ وكل حاجة ظاهرة تبدأ بصور قبل الكود.

- **Date / decision owner:** 2026-10-07 / Ahmed. Baseline inspected: `main` @ `8c9c590` (worktree `docs/ui-arabic-path-b`).
- **Amends on acceptance:** [ADR-0005](ADR-0005-crate-dependency-directions.md) edge table (new leaf crate `varos-text`; `varos-app → varos-text` in T1; `varos-core → varos-text` in T4). Closes the path choice left open by [ADR-0010](ADR-0010-text-tool.md) ("Ahmed must still choose the UI-field route") and UI_SYSTEM §8/P8. Leaves ADR-0010's model, format, PDF and P2 entry criteria unchanged.
- **Related:** [vision](../VISION_AI_NATIVE.md), [P1b results](../foundation/work_orders/TEXT_P1B_RESULTS.md), [spike README](../../varos/spikes/text-p1/README.md), [fonts.rs](../../varos/crates/varos-app/src/shell/fonts.rs), [kit/field.rs](../../varos/crates/varos-app/src/shell/kit/field.rs), [ui/fields.rs](../../varos/crates/varos-app/src/ui/fields.rs), [CLAUDE.md](../../CLAUDE.md).

## 1. Context and decision boundary

Facts read from source, not re-measured here:

- egui 0.35 lays out text per `char` into a galley whose cursor model is char-indexed. epaint 0.35 already shapes runs with **HarfRust 0.7** (`epaint/src/text/text_layout.rs` ~224–500), hints outlines with Skrifa (`font.rs` ~203–250, `fonts.rs` `HintingTarget` ~248–303), rasterizes with vello_cpu into 4 subpixel bins (`font.rs` ~106) and maps coverage through `AlphaFromCoverage` (`image.rs` ~423–434). It has **no UBA reordering and no grapheme/cluster caret map**; U0-A recorded `لوحة أولى` as 9 scalars → 14 glyph records with wrong caret and word order. Fixing that is a rewrite of epaint's galley/cursor model, i.e. path A.
- `fonts.rs` registers IBM Plex Sans Arabic Regular only as a named diagnostic family and keeps it out of every egui chain, so Arabic in any label or field shows missing-glyph boxes (the owner's screenshots: Properties ▸ Board name/description/tags and the doc tab chip).
- The P1b spike (`varos/spikes/text-p1`, excluded workspace) passes 7/8 Amendment-1 gates headless: language/script runs, paragraph-level legal breaks, native NonZero outlines, std-without-discovery feature split with no E0004 in a combined resvg graph, native/WASM runtime parity. It **fails** only the large-text latency gate (100k-scalar single paragraph p95 64 ms / max 102 ms); P1c addresses that before canvas P2.

**Decision.** Path B, no egui fork: (1) promote the spike engine into one production crate `varos-text` used by the UI now and by the canvas TextBox later; (2) draw UI text that can carry user/document data through a new `kit::text` painter (our layout → our glyph atlas → `epaint::Shape::Mesh`); (3) replace the egui `TextEdit` inside `kit::field`'s text widgets with our own editor, keeping the K3 transaction exactly; (4) keep egui text for static English chrome and ASCII numerics. Full RTL panel layout (mirrored panels, right-to-left rows) stays out of scope, as §8 says.

UI strings are short and bounded (board name ≤120 chars, description ≤500, tag ≤32: `varos-core/src/board.rs:22–28`; file names ≤255 bytes), so the failed 100k-paragraph gate does **not** block T1–T3. It still blocks T4 through ADR-0010's P2 entry criteria. Accepting this ADR is the owner amendment that lets bounded UI use of the patched engine proceed; P1b's other open items (independent patch review, Ahmed's Arabic proof review, a named maintenance owner) become T1 exit gates.

## 2. One engine: the `varos-text` crate

**Choose one production crate, `varos/crates/varos-text`, a pure leaf.** It owns shaping/bidi/line-break/caret/outline logic and Varos-owned plain result types. COSMIC/HarfRust types stay crate-private (ADR-0010: vendor types never become model or Bridge API types). No Serde model, no egui/epaint/winit/wgpu/windows, no filesystem, no locale or font discovery. Fonts arrive as immutable bytes in a `FontSet`; fallback is a fixed per-script table.

| Spike item (`varos/spikes/text-p1`) | T1 decision |
|---|---|
| `src/engine.rs` adapter: UBA levels, script itemization, language runs, legal-break fitting, line-edge refit, carets/affinity, coverage bitsets, span cache, `paragraphs`, `local_line_levels`, `validate_request` | **Moves.** The hard-coded `Face {Inter, Plex}` enum and `PinnedFallback` become `FaceId` (index into `FontSet`: family, weight 400/500/600, bytes, content hash) and a data-driven `FallbackPolicy` (§7). Add bounded `elide` and `caret_move` (visual grapheme step, word step, home/end) and `selection_rects` (several rects per logical range). |
| `src/outlines.rs` `glyph_outline`, `Command`, `Outline::ink_bounds` | **Moves** (exact cubics, Y-down, native winding). `even_odd_regions`/`rings_path` stay in the spike as diagnostics. |
| `src/incremental.rs` paragraph cache | **Stays experimental** until P1c passes; moves in T4. The UI does not need it. |
| `proof.rs`, `stencil.rs`, `export.rs`, `parity.rs`, `report.rs`, `main.rs`, `examples/`, `scripts/` | **Stay** in the spike as the evidence harness. The WASM parity runner keeps pointing at the spike until T4 re-targets it at `varos-text`. |
| `tests/corpus.rs`, `language.rs`, `wrapping.rs` | **Move** as `varos-text` headless tests (fonts by `include_bytes!` from `varos-app/assets/fonts`, as the spike does). `nonzero.rs`, `incremental.rs` stay. |
| `vendor/` (gitignored, rebuilt by `reproduce.sh`): cosmic-text 0.19.0 + P1b patch (7 files, +344/−31) | **Checked in** as `varos/vendor/cosmic-text` with `[patch.crates-io]`, exactly like `egui_tiles`: pristine hash, patch file and removal condition in `docs/VENDOR_PATCHES.md`, hash check in `tools/check_vendor_patches.*`. A fresh clone must build offline. |

**Dependency rules** (amends ADR-0005; `tools/check_dep_directions.py` `EDGES` changes in the same commits):

| Crate | May depend on | When / reason |
|---|---|---|
| `varos-text` | no Varos crate | T1. Added to the forbidden-UI/GPU regex set; a new graph check (`cargo tree -e features -p varos-text`) fails on `sys-locale`, `swash`, cosmic `fontconfig`/`system-discovery`, or any egui/winit/wgpu crate. |
| `varos-app` | + `varos-text` | T1/T2. `kit::text` and the field editor. |
| `varos-core` | + `varos-text` | **T4 only** (ADR-0010 P2). |
| raster / pdf / bridge / cli / render-wgpu | unchanged | They receive text geometry through core's scene (`Prim::Fill` with NonZero), never call the engine directly. |

**Why core → text and not text above core.** ADR-0010 already put layout under the model: `varos-core::text` owns requests/results, takes immutable font bytes, and `build_scene` must emit glyph outlines for every consumer (wgpu, CPU thumbnails, PDF, SVG, Bridge `describe`/bounds/hit-test, the later WASM mirror). With core → text, every host gets identical geometry through one call and no new edges. The rejected alternative, a `TextLayout` trait in core implemented above it, makes each of five hosts inject the engine and invites thumbnail/PDF/screen drift. Keeping the engine a **separate crate** (not a core module) still pays: the compiler hides COSMIC types, the UI can use it in T1 before the model changes, its feature graph is auditable alone, and it is the unit the WASM parity runs against. The font snapshot type is re-exported by core in T4, so other hosts still need no direct edge.

**Feature graph.** cosmic-text `default-features = false, features = ["std", "shape-run-cache"]`; the patch's `system-discovery` feature stays off. T1 proves in the **real** main workspace (not a throwaway graph): no E0004; native test + clippy; `--target x86_64-pc-windows-msvc` clippy; `cargo check -p varos-text --target wasm32-unknown-unknown`; `check_dep_directions.py` PASS. Honest note: the full app graph still contains fontdb `fs/memmap/fontconfig` through resvg; the invariant is that `varos-text` never invokes discovery, which the byte-fed mode enforces (it refuses file sources). Second shaper: epaint keeps its internal HarfRust 0.7 for static chrome while COSMIC uses HarfRust 0.5.2; T1 measures the binary delta and records a dedupe trigger (a COSMIC release on the same HarfRust). That internal egui shaper is not a Varos text path and shrinks as chrome moves to `kit::text`.

**Maintenance owner (P1b left it unassigned).** Proposed: the moderator lane owns the vendored patch; Codex reviews every rebase; T1 submits the two focused patches (feature split; run language/script) upstream without waiting on a merge. Removal condition: an upstream release containing them, or an owner-reviewed replacement. No indefinite fork.

## 3. Drawing UI text inside egui without forking it

**Choose a glyph atlas owned by `kit::text`, painted as `Shape::Mesh`.** Flow per label: `varos-text` layout (positioned glyphs with `FaceId`, glyph id, cluster bytes, level) → for each glyph, an atlas entry keyed by `(face, glyph id, physical px size, subpixel bin)` rasterized on the CPU → one textured mesh per label (quads, vertex colour = role colour) → `painter.add(Shape::Mesh)`; clipping by the painter's clip rect as today. Selection highlight and caret are plain rect shapes under/over the mesh.

To look identical to the Latin egui still draws next to it, the rasterizer **copies epaint 0.35's recipe**: Skrifa hinted outlines with the same `HintingTarget`, vello_cpu fill (already in the lock through epaint), the same 4 subpixel x-bins, whole-pixel y snap, and `AlphaFromCoverage` from the current visuals; white premultiplied coverage tinted by vertex colour, like egui's font texture. One `TextureHandle` (`ctx.load_texture` + `set_partial` uploads), 1024² growing to 2048², cleared on a `pixels_per_point` change (moving between displays). Rasterization lives in `varos-app` (`shell/kit/text/`), not in `varos-text`: it is a UI concern tied to egui textures; the canvas path stays outlines (ADR-0010).

| Alternative | Decision / reason |
|---|---|
| Outlines → tessellated meshes per label | Reject for UI: no hinting, egui meshes get no AA at 11–13 pt, triangle count per glyph; fine for the zoomable canvas (ADR-0010), wrong for chrome. |
| Inject our glyphs into an egui `Galley` | Reject: galley UVs point into epaint's private char-keyed atlas; the cursor model is still char-indexed. A fork by another name. |
| Path A, vendor/patch epaint | Rejected by the owner. Would carry a fork of epaint's galley/cursor model through every egui upgrade. |
| One texture per label | Reject: texture churn and upload cost with hundreds of rows; atlas reuse across labels is the whole point. |

**Cost model and cache.** Key = `(text hash + len, role id, max width or none, ppp)`; value = `Arc<UiLayout>` (lines, glyph quads, carets for fields). Retained across frames, touched per frame, LRU-capped at 4,096 entries / 4 MiB payload; entries untouched for 600 frames drop. A label lays out only when first drawn; later frames are cache hits. Provisional budgets, measured by a T1 headless bench and the U2-P protocol in T2: cold layout of a 120-char mixed name p95 ≤ 0.25 ms; a warm label (lookup + mesh emit) ≤ 5 µs; 300 warm labels ≤ 1.5 ms per frame; Start's first open with 200 cold Recent rows ≤ 8 ms or spread over frames; zero layout and zero atlas uploads on an idle frame. K4's `ui_build` p95 ≤ 8 ms stays the frame gate. Display input over 4 KiB is laid out as a bounded prefix + ellipsis (display only; stored text untouched).

Accessibility parity: every `kit::text` widget sets `Response::widget_info` with the logical string, so AccessKit sees what egui labels expose today. Kit law K1 holds: `kit::text` takes strings, role, width, colour; it knows no binary types. One `UiText` system per process, created by `fonts::install` beside egui's definitions and stored in context data.

## 4. Editable fields: our editor inside the K3 law

The K3 session in `kit/field.rs` (focus-in snapshot, commit on Enter/Tab/blur/settle, Esc reverts, Tab ring, invalid keeps focus, `pending`/`settle`/`end_frame`) **does not change**. Only the inner widget changes: `text_field` (`field.rs:338`), `text_area` (`:348`) and `search_field` (`:593`) stop calling `egui::TextEdit` (`:388`, `:390`, `:607`) and call `kit::text::Editor`; `select_all` (`:144`) moves from `TextEditState` to the editor state. `number_field` (`:450`, TextEdit `:495`) **stays egui**: its text is ASCII numerals in mono, "numbers stay Latin". T3 adds one fix there: Arabic-Indic digits (U+0660–0669, U+06F0–06F9) and the Arabic decimal separator U+066B typed into a number field are mapped to ASCII before the TextEdit sees them, so an Arabic keyboard never produces tofu or a false "Type a number".

Editor contract (state lives in egui temp data keyed by the field id, like the session):

- **Model:** logical UTF-8 buffer; anchor and focus as byte offset + upstream/downstream affinity; every boundary a grapheme boundary. Never a reversed string, presentation forms, or inserted LRM/RLM.
- **Keys** (consumed only while focused; `set_focus_lock_filter` keeps arrows/Tab from moving egui focus; Tab/Esc/Enter still reach the K3 session): ←/→ move one grapheme **visually**; ⌥←/⌥→ by word (segmentation + visual direction); ⌘←/⌘→ line start/end visually; ↑/↓ in `text_area` keep desired x; Shift extends from the stable anchor; Backspace deletes the previous **logical** grapheme, Delete the next; ⌘A selects all. Mac behaviour is checked against TextEdit.app by the owner, not assumed.
- **Mouse:** click = `hit(x, y)` → byte + affinity; drag extends; double-click selects a logical word; the selection paints as several rects across bidi runs.
- **Text input / IME:** consume `egui::Event::Text`, `Event::Ime(Preedit{..} | Commit)`, `Copy`, `Cut`, `Paste` from the frame input; egui-winit already translates winit 0.30 `Ime` events. While focused, publish `PlatformOutput::ime = IMEOutput { rect, cursor_rect }` from our caret so egui-winit enables IME and places the candidate window. Preedit is shown underlined, never in the buffer, never committed by blur; Commit replaces the selection once; empty Preedit clears composition only; no duplicate insertion around Commit. Settle/Save/tab switch with an active preedit drops the preedit, then runs K3 on committed text.
- **Clipboard:** copy/cut put the **logical** substring via `ctx.copy_text`; paste inserts as typed (CR LF → LF only in `text_area`; single-line fields replace newlines with spaces, matching today's TextEdit).
- **Field-local undo:** ⌘Z/⇧⌘Z inside a focused field undo typing within the session (K2 row 4 already routes them to the field); the commit is still exactly one document undo step.
- **Direction and alignment:** §6.

## 5. Every label and field site, classified

User/document/file-system strings move; static English copy and numerics stay egui until a later localization decision. A ratchet test (T2) fails if a known user-string source (`tab.label`, `row.name`, `ab.name`, `card.name`, `r.name`, tags, folders) reaches `painter.text`, `RichText::new`, `on_hover_text` or `start_page::text*`; its ceiling reaches 0 at the end of T2.

| Site (file:line @ `8c9c590`) | Shows | Class |
|---|---|---|
| Doc tab chip `ui/bar.rs:202`; its tooltip `:218`; hidden-tabs overflow rows `:494` | document name / path | **T2** |
| Layers row name `ui/panels/layers.rs:734` (+ char-count `elide` `:890` → shaped elision) | layer/group names | **T2** |
| On-canvas artboard name `ui/panels/artboard.rs:253` | artboard name | **T2** |
| Start/Home: Recent row `start_page.rs:1327`, card name `:1471`, card description `:1479`, list name `:1580`, list description `:1586`, folder paths `:1496`, `:1596` (`path_galley` `:482`), tag filter chips `:1385`, tag pills `:1767` | board names, descriptions, tags, folders | **T2** |
| Recovery card name `recovery_card.rs:544`, folder `:550`, headline `:434` where it embeds a name; status-bar recovery line `ui/bar.rs:747` | document names | **T2** |
| Properties ▸ Board tag chips (display part of `ui/fields.rs:236`) | tags | **T2** |
| Properties ▸ Board name / description / tags input `ui/fields.rs:192`, `:210`, `:236` (called from `ui/panels/properties.rs:240–242`) | editable | **T3** |
| Artboard panel name `ui/panels/artboard.rs:35` (`fields::name` `ui/fields.rs:133`) | editable | **T3** |
| Layers rename `ui/panels/layers.rs:713`; on-canvas artboard rename `ui/panels/artboard.rs:247` (`fields::rename` `ui/fields.rs:146`) | editable | **T3** |
| Layers search `ui/panels/layers.rs:459` (`search_field`) | editable, commits nothing | **T3** |
| Picker hex `ui/picker.rs:906` (`fields::hex` → `text_field`) | ASCII hex | T3 by inheritance (same widget); no visible change intended |
| All number fields (`fields::num/num_value/num_disabled` in `control_bar.rs`, `properties.rs`, `artboard.rs`, `picker.rs`, `controls.rs:161`) | ASCII numbers, mono | **Keep egui** + digit mapping (§4) |
| Menus (`ui/menus.rs`, `kit::menu_row`), section headings, panel tab titles `shell/boxtree.rs:630`, static tooltips, status "Fit"/"Artboard"/zoom % `ui/bar.rs:756–787`, rulers `ui/canvas_overlay.rs:82,170`, Start titles/buttons/presets, `export_ui.rs:131` | static English, numerics | **Keep egui** |
| Window title, macOS menus, `rfd` dialogs (Open, **Save As** name box, notices in `file_ports.rs`/`lifecycle.rs`) | native AppKit | **Native, untouched** — already renders Arabic; the owner's Save As check is about the name then shown in tabs/Recent |

## 6. Bidi and RTL rules for UI strings

- **Direction per string:** UBA P2/P3 (first strong character); no stored override. An empty field starts LTR and flips on its first strong RTL character, like macOS. Each paragraph in `text_area` resolves separately.
- **Mixed names:** full UBA with neutrals/brackets/numbers. `لوحة — Café logo` is an RTL paragraph; visually `Café logo — لوحة`, Arabic at the right. Display never adds controls; bidi controls the user typed are honoured and preserved.
- **Alignment (owner rule: Arabic right/centre):** single-line labels **and** single-line fields stay on the cell's leading (left) edge of the LTR shell, as Finder/Figma do in an English UI, so a Layers name does not jump when rename opens. Multi-line text (board description field and Start descriptions) aligns by paragraph direction: RTL paragraphs right. The single-line choice is **for the owner to confirm on the T2 mockup** (the alternative, right-aligning RTL names inside left cells, is one token).
- **Elision:** cut at the logical end on a grapheme boundary using shaped widths (replaces `layers.rs:890`'s `0.55 × size` estimate), reshape the kept text so the last letter takes its correct form, append `…` at the paragraph's logical end (visual left for RTL). Paths elide in the middle by segment; each folder segment is laid out alone and segments are placed LTR with `/` between, so an Arabic folder cannot reorder the path.
- **Numbers stay Latin:** digits display as stored; no Arabic-Indic substitution anywhere in chrome; number fields map typed Arabic-Indic digits to ASCII (§4).
- **Letter-spacing:** role tracking (`start_page.rs:54–68`) applies to non-Arabic runs only; Arabic-script runs always get 0.
- **Line height (owner rule ≥130 % headings, 150–160 % body):** a role's vertical metrics are fixed from the union of its Latin face and Plex Arabic, so baselines do not shift between Arabic and Latin rows. Every single-line role must be ≥130 %: `BODY_MEDIUM` (13 pt on 16 = 123 %) becomes 17. Multi-line description text uses ≥150 % (`DESC` 12 on 17 → 18; the 3-row board description field likewise). These are visible token changes, shown on the T2 mockup first.
- **Out of scope:** mirrored panels, RTL row order, Arabic UI copy/localization, Arabic-Indic numerals as a preference, vertical text, kashida in UI.

## 7. Fonts

- **Faces:** Latin UI stays Inter 400/500/600; mono stays JetBrains Mono 400; Arabic UI face = **IBM Plex Sans Arabic**, used only through `kit::text`. egui chains stay exactly as `fonts.rs` defines them; Plex never enters an egui chain (existing `tests/fonts.rs` assertion stays). `fonts.rs` remains the one home: the same `include_bytes!` statics feed both egui definitions and the `varos-text` `FontSet` (no copied bytes).
- **Weights — decision: bundle Plex Sans Arabic Medium (500) and SemiBold (600)** beside Regular, unmodified, from the same IBM Plex release, OFL 1.1 with Reserved Font Name "Plex" (shipped unmodified, so the name is kept), source/length/SHA-256 recorded in `assets/fonts/manifest.json` and README; ~+0.47 MB estimated from Regular's 236,708 bytes, measured in T1. Role mapping: `ui-400` → Plex Regular, `ui-500` → Medium, `ui-600` → SemiBold, so `لوحة — Café logo` in the active tab (12/500) has matching weight on both sides. **No synthetic bold** (ADR-0010; emboldening blurs Arabic dots and counters). Fallback if the owner declines the bundle: 500/600 Arabic use Regular, with a visible weight mismatch recorded as a known gap.
- **Fallback order (fixed table, whole joining segment from one face, missing clusters reported, no system lookup):**

| Script of the segment | Order |
|---|---|
| Latin / Common / digits / punctuation | role's Inter weight → Noto Sans Symbols 2 → Noto Sans Symbols → Plex Arabic (same weight) → egui's bundled default fallbacks |
| Arabic | Plex Arabic (role weight) → missing-glyph box + `Issue::UnsupportedCluster` |
| Mono role (paths) | JetBrains Mono → Inter 400 → Plex Arabic 400 → symbols → egui defaults |
| Anything else (emoji, Hebrew, CJK …) | egui's own default faces our chains already extend (bytes from `FontDefinitions::default().font_data`: NotoEmoji-Regular, emoji-icon-font, …) → box |

T1 test `coverage_no_regression_vs_egui_chain`: every scalar the current egui chain renders, `kit::text` renders too. System-font fallback for other scripts is a later, separate decision.

## 8. Acceptance

Automated, headless (no Renderer, no EventLoop; egui `Context::run` with synthetic `RawInput` as `ui/fields/tests.rs` already does):

| Test | Proves | Lives in |
|---|---|---|
| `arabic_name_shapes_joined_clusters` | `لوحة أولى`: clusters cover all 9 scalars, joining forms differ from isolated, matches direct HarfRust oracle | varos-text + kit CPU golden |
| `mixed_bidi_visual_order_matches_uba` | `Logo شعار v2` and `لوحة — Café logo` run order and x positions | varos-text + kit |
| `rtl_caret_moves_by_grapheme` | arrows over `السَّلَامُ` and mixed text step one grapheme visually, never inside a cluster; affinity at direction changes | kit editor |
| `arabic_selection_copy_paste_roundtrip` | shift-select → `Copy` yields the logical substring; `Paste` back gives identical bytes | kit editor |
| `stored_text_unchanged` | typed/IME/pasted Arabic reaches the commit `Op` and the saved JSON byte-identical (no presentation forms, no reorder, no added controls) | app fields tests |
| plus | `rtl_name_elides_at_logical_end_on_grapheme`, `baseline_is_content_independent`, `arabic_tracking_is_zero`, `ime_preedit_never_commits_on_blur`, `ime_commit_inserts_once`, `arabic_indic_digits_type_as_latin`, `coverage_no_regression_vs_egui_chain`, `latin_label_matches_epaint_pixels` (Inter `Untitled-1` at 1×/2×, mean ≤1/255, none >32/255 vs egui's own rendering), all existing K3 tests unchanged | kit / app |

**Fixtures.** The owner's tofu screenshots (Properties ▸ Board name/description/tags; the tab chip) are committed in T2 under `docs/foundation/evidence/ui-arabic/before/` as the "before"; matching headless CPU renders of the same strings through `kit::text` are the regression goldens; the owner's own window screenshots after T2/T3 are the "after". No synthetic UI driving of the real window by agents.

**Owner hand check (after T3, short):** type an Arabic name and a mixed name (`لوحة — Café logo`) in Layers rename, the artboard name and Save As; read them back in the tab, Layers, Recent and the Properties board fields; move the caret with arrows and select/copy/paste once. Only after this passes: §8's "off until it passes" lifts for UI fields/labels, and Varos may claim Arabic **UI names** (not Arabic canvas text, which stays ADR-0010 P3).

## 9. Pieces, estimates and risks

Estimates are engineering days of implementation + review rounds, not elapsed time.

| Piece | Deliverable | Gate before the next | Estimate |
|---|---|---|---|
| **T1 — engine crate** | `varos-text` from the spike (§2), `FontSet`/`FaceId`/fallback table, checked-in vendored COSMIC + vendor-patch check, ADR-0005/`EDGES` update, moved corpus/language/wrapping tests, short-string bench, Plex Medium/SemiBold bundled with manifest | Workspace tests + clippy (Mac, Windows target), wasm check of `varos-text`, dep/vendor checks, feature-graph audit, binary-size delta logged; independent review of the patch; Ahmed has looked at the P1b Arabic proof sheets. No visible change. | 4–6 |
| **T2 — read-only labels** | Before/after mockups first; `kit::text` atlas painter + cache; all §5 T2 sites; shaped elision; role line-height tokens; ratchet; fixtures | §8 shaping/bidi/elision/baseline/pixel-parity tests; U2-P numbers; owner looks at tabs, Layers, Recent, Start, Properties chips in the window | 6–9 |
| **T3 — editable fields** | `kit::text::Editor` (caret, selection, mouse, words, IME, clipboard, field undo) inside unchanged K3; §5 T3 sites; digit mapping | All five §8 names + extras + every existing K3 test; owner hand check (§8) | 8–12 |
| **T4 — canvas TextBox** | ADR-0010 P2 consumes `varos-text` via core (`core → varos-text` edge), incremental cache moves after P1c | ADR-0010 P2 entry and exit criteria, unchanged | inside P2; reuse ≈ 1–2 extra |

| Risk | Mitigation / stop condition |
|---|---|
| Per-frame cost (cold Start page, DPI change, atlas growth) | Layout only visible rows; LRU + idle-frame zero-work test; budgets in §3 measured before T2 lands; stop and report if `ui_build` p95 regresses beyond U2-P spread. |
| egui upgrade coupling | Public surface only: `Shape::Mesh`, `TextureHandle::set_partial`, input `Event::{Text, Ime, Copy, Cut, Paste}`, `IMEOutput`, focus-lock filter, `AlphaFromCoverage`/hinting recipe. Headless event-sequence tests + the pixel-parity test fail loudly on an upgrade. Far smaller than an epaint fork. |
| IME on macOS (winit 0.30 `Ime` via egui-winit) | Arabic keyboards send plain text, so they do not prove IME; tests cover Preedit/Commit/empty-Preedit/blur ordering; owner checks a dead key and the emoji palette in the real window. Stop if egui-winit cannot place the candidate window from `IMEOutput`. |
| Two HarfRust versions in the binary; vendored COSMIC upkeep | Measure in T1; named owner, upstream PRs, removal condition (§2). |
| Visible weight mismatch between egui Latin and `kit::text` Latin | Same Skrifa hinting/vello_cpu/subpixel/coverage recipe; `latin_label_matches_epaint_pixels`; a component never mixes the two painters in one line. |
| WASM mirror later | Engine already has native/WASM runtime parity (P1b); the atlas painter is plain egui and runs on WebGPU; browser IME is future host work, not promised here. |

## 10. What this paper is

Docs only: no Rust, manifests, fonts or lockfiles changed; nothing built, run or committed for this ADR. Line references were read at `8c9c590` and must be re-checked when a piece starts. Awaiting Ahmed: accept path-B architecture (one crate, core edge in T4); accept bundling Plex Arabic Medium/SemiBold; confirm the single-line alignment rule on the T2 mockup; commission T1. Each piece gets its own work order, repository gates, independent review before merge and the owner's window check; visible pieces start with mockup images.
