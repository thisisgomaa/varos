# Lane w2-tools-ui — wave 2 (2026-10-09)

# w2-text — Lane G: Text programme P2–P4

# Lane B — Phase 5 gradients

Baseline: committed `ece20e5`; this fix round is uncommitted; no git writes, merge, push, GUI launch or install.
Scope: 5.0–5.4 plus within-stroke gradients implemented; optional 5.5 along/across/freeform deferred; UI provisional.
Format: provisional NEXT_GRADIENT_VERSION=6; named pure migrate_v5_to_next_gradients; integrator assigns final number.
Keys: doc.swatches {id,name,paint,global,group}; tagged gradient {kind,stops(offset,colour,opacity,midpoint),spread,placement,focal}; swatch_ref {id}.
Frozen v4/v5, gradient/raster goldens and legacy Bridge fixture trees unchanged; no dependencies or ratchets raised.

# w2-images — Phase 3
3.1–3.8 implemented; provisional UI and native owner acceptance remain pending.
Writer remains provisional v6: images/assets/raster_effects_ppi; integrator assigns combined versions.

Lane H — w2-import wave 2; committed pre-review baseline 5f8e19e; fix-round edits uncommitted.
Status: implemented (provisional UI, owner design review pending).
Retains PDF/AI static paths/clips/rotation/dashes, ASCII DXF layers/arcs/polylines/exact clamped splines, clipboard/Open/Place/drop, CLI/Bridge.
Explicit losses/refusals retained: text/images omitted; bitmap prerequisite absent; unsupported PDF profiles/DXF entities/DWG refuse; CLI output never replaces an existing file.
No persisted keys, format bump, migration, new dependencies or attribution changes.

- Status: implemented (provisional UI, owner design review pending); fix-round changes uncommitted.
- Product: editable point/area styled text, Type tool/IME/carets, Type properties, cached outlines, PDF/SVG, Bridge 1.2 and CLI.
- Seam: core owns serializable text; varos-text-layout owns shaping/outlines; core → text remains forbidden.
- Format: provisional next writer 6, named pure `migrate_to_text_boxes`; moderator assigns final number after images/gradients.
- Keys: doc.text_boxes; NodeKind::Text; TextBox id/box_kind/frame/runs/para; run text/style; style font{family,weight,hash}/size/letter_spacing/fill; para align/direction/kashida/line_height.
- Integration: preserve every migration/refusal fixture, combine text/image/gradient export traversal, and retain sibling registrations plus text settlement hooks.

Status: implemented (provisional UI, owner design review pending); baseline committed, fix-round edits uncommitted.
Scope retained: 4B shapes/numeric sheets/rail flyouts; 4C Pencil, Smooth, Path Eraser, Join, Curvature; additive command/Bridge 1.2/CLI routing.
## Fix round

P1 Eraser: bound subdivision storage, precompute every replacement, check resulting path/node/anchor counts before allocating IDs or publishing geometry.
P2 Join: retain brushed endpoint eligibility through merges; untouched and consumed endpoints cannot become join candidates.
P2 Numeric sheets: Rectangle/Ellipse roll back the complete provisional gesture whenever the 3-screen-pixel sheet threshold is met, at every zoom.
P2 Undo/redo: clear drawing start/samples/curvature/preview/dialog; discard active drawing transactions while preserving options and shape preferences.
Disagreements: none. Selection's separately characterized stale-pending behavior remains outside this drawing fix.
Regression coverage: 26 drawing + 2 eraser-limit + 2 history-lifecycle tests PASS; seven new tests cover all four findings and late atomic refusal.
Gates: fmt/dependency directions PASS; workspace PASS: 1916 passed, 0 failed, 15 ignored; three nested child summaries excluded.
Clippy: native and x86_64-pc-windows-msvc --workspace --all-targets -D warnings PASS, zero warnings.
Ratchets: UI 3/3 and Bridge 6/6 PASS; contracts 88 passed/4 ignored; both ratchet sources and all 16 fixtures byte-identical to main; ui.rs 784/843.
Boundaries: no persisted keys, format/writer changes, dependencies or raised thresholds; no new production unwraps; reused join attribution in NOTICE.
Shared changes: one six-line delimited editor history block and one appended delimited NOTICE block; other code edits stay in drawing.rs/tests.
Integration: union images' blob state, gradients' appearance routing, and app/export command routing; preserve drawing options and curvature settlement.
Evidence: /tmp/varos-w2-fix-round/{workspace,clippy-native,clippy-windows,ratchets,bridge-fixtures,fmt,deps}.log plus exit-codes.json/summary.json/compatibility.json.
No git writes, push, merge, GUI launch or installation; native/heat/Windows-runtime acceptance and independent integration review remain pending.

# Lane E — Phase 8 (feat/w2-view)
- Outline, Pixel Preview/snap, Navigator, presentation/Trim and canvas preferences: implemented (provisional UI, owner design review pending).
- API 1.2 view command/discovery + headless CLI retained; normal/export paint behavior preserved.
- No .vrs format/version/migration changes; settings key remains canvas_color [u8;3]; no borrowed code/dependencies.
## Fix round
- Accepted all four P2 findings; no disagreements with the reviewer.
- Pixel Preview samples resolved artwork before screen-resolution overlays; final blit/layer composite use separate normal sampling state.
- Outline now consumes existing cached, viewport-culled/clipped geometry; selection changes do not reflatten paths.
- Navigator invalidates on live document/camera changes and publishes each texture with its matching camera; idle remains Wait.
- Already-active canvas colour returns before scheduling settings persistence; changed colours still save.
- Four added headless regressions cover recorder ordering, cache/culling, live artwork/artboard proxies, and unchanged/changed saves.
- Renderer evidence is WGSL validation and recorder-order regression; native visual acceptance remains unverified.
- Image integration remains REQUIRED: this base has only Layer/Group/Path nodes; moderator must add image-box outlines with the image lane.
- Shared scene/recovery hooks delimited; UI shell/tokens/kit and ratchet thresholds untouched; ui.rs remains 830/843 lines.
- fmt, dependency directions, diff whitespace, native + Windows all-targets Clippy -D warnings: PASS.
- cargo test --offline --workspace -j 3 --no-fail-fast: PASS — 1898 passed, 0 failed, 15 ignored.
- Shell ratchets: 3 passed; Bridge ratchets/frozen 1.0/1.1 fixtures: 6 passed; no fixture edits.
- Evidence: /tmp/w2-fix-*.log; initial test-fixture mistakes corrected before final workspace rerun.
- No git writes/commit/push/merge, GUI launch, install, or new independent review; moderator/owner acceptance pending.

- Review: all five P1/P2 findings addressed; no disagreements. Cross-lane merge risks remain moderator responsibilities per brief.
- P1 Release Build: text-containing subtrees are refused before mutation/history; regression checks atomic refusal and undo.
- P1 Object selection: core text identities survive cleanup; drag/translation, Delete, Copy/Cut/Paste and undo work with mixed selections; rotated groups retain transforms.
- Clipboard: internal payload retains source; public PDF/SVG outlines include selected text only. Unsupported non-translation text transforms are explicitly refused.
- P2 Type fields: kit pending operations settle into the draft before Save/tab lifecycle commits; size/leading/tracking tested headlessly.
- P2 Hits: mixed tree paint order lets foreground paths occlude text; tests cover path above/below and canvas object dragging.
- P2 Cache: separate zoom-independent shaping and bounded LRU outline retention; 300-box pressure/zoom test proves no repeated shaping.
- Gates: workspace 1,919 passed / 0 failed / 15 ignored (113 suites); fmt, dependency directions, native/Windows clippy -D warnings, UI ratchets 3/3 and Bridge contracts/ratchets 94 passed, 4 ignored.
- Evidence: /tmp/w2-text-fix-*.log; 254 fixture/ratchet files byte-identical to HEAD; text hashes 5/5; ui.rs 830/843, ratchets unchanged.
- No git writes, GUI launch, install, push or merge; independent re-review, native GUI/IME, Windows runtime and owner Arabic/design acceptance remain unverified.

P1: restored non-import file-effect dispatch; distinct-handler regression covers Save/SaveAs/exports/Print/Copy/Cut.
P1: PDF CTM computes both coordinates from original X/Y before crop offsets; non-diagonal regression added.
P2: clipboard/Bridge conversion runs on existing IO worker; cancellation, generation, revision, active-board and busy publication checks preserve atomicity.
Bridge imports return accepted tickets; request_status retains completion report, committed revision and undo count.
P2: provisional desktop PDF page chooser and DXF declared-unit/mm/point choices are carried in Job options before conversion.
Keyboard/menu Paste and Paste in Place always defer to host queue; immediate-action bypass regression added.
Six new headless regressions cover dispatch/receipts, geometry, queued Bridge cancellation, clipboard atomicity, page/unit conversion and shortcut deferral.
PASS: fmt; dependency directions; workspace 1909 passed/0 failed/15 existing ignored; native + Windows clippy -D warnings; ratchets 3/3; Bridge fixtures 99 passed/4 ignored.
Compatibility: all 250 protected fixtures/UI/token/ratchet files byte-identical to b3d39ee; ui.rs remains 811/843.
Evidence: /tmp/w2-import-fix-round-*.log; compatibility /tmp/w2-import-fix-round-compat.json.
Shared-file changes limited to dispatch/dialog seams and tests; main.rs worker routing is delimited; kit/tokens/ratchets unchanged.
Disagreements: none. Sibling image/gradient merges still require semantic integration checks.
No git writes, commit, push, merge, GUI or install; new independent review and owner/native interoperability/complex fidelity tests remain pending.

Astra's eight findings and Opus image findings 1–5 addressed; no disagreement with their reproductions.
PDF: removed parallel scene writer; mixed leaves share write.rs exact cubic/stroke/knockout writer.
Images: Flate RGB/SMask, upright three-component JPEG DCT passthrough; CMYK uses decoded RGB.
Save: pinned hash/header/metadata checks + bounded PDF preflight; no second decoded resource store.
Open: original-budget failure retains original bytes and proxy pixels, with a visible load notice.
Boundary: if even proxy residency cannot fit, return TooLarge rather than MalformedPdf; hard cap retained.
Zero opacity: appearance omitted consistently, while native original/proxy streams remain.
Scene: one ordered path lookup; debug 200/400/800 empty paths measured 5.98/14.92/34.78 ms.
Interaction: live image signature, mixed Select All/frame/move/scale/rotate/numeric edits/marquee/copies.
Boards: image membership/visibility/lock and clipping shared; GPU scissors + CPU masks preserve crop.
CPU image exports now use artwork-only scenes, excluding artboard chrome; SVG shares vector dispatch.
Bridge API 1.2 writable capability derives FORMAT_VERSION; account-home lookup and initial Links badge fixed.
Tests: restored envelope-normalized v5 JSON/PDF byte goldens, specific refusal errors, frozen v5→v6 gate.
Original frozen fixtures unchanged; corrected image-writer golden added separately; PDFium 16 pixels PASS.
Gates: fmt/dep directions PASS; cargo test --offline --workspace -j 3 --no-fail-fast: 1,928 passed / 0 failed / 15 ignored.
Native + Windows Clippy -D warnings PASS; shell 3 + Bridge 6 ratchets, panic guards and Bridge fixtures PASS.
Ratchets/fixture sources unchanged; ui.rs 811/843; logs /tmp/w2-fix-{workspace-green,native-green,windows-green,bridge-fixtures}.log.
Integration: stroke::canvas_seam(ed,p,ppu) marks main 9f14e1e cache/cap/backoff/fallback routing; no main merge.
Opus gradients 7–10 absent here; gradients lane must fix them. Mixed v7 golden/v6→v7 gate remain integration work. Low Bridge link hashing/description compaction unchanged.
No git writes, commit/push/GUI/install; fixes uncommitted. Native/GPU behavior and adversarial allocation peaks unverified.

- Astra 1: picker opens/frames/selection changes are read-only; explicit edits create gradients; post-Undo frame regression passes.
- Astra 2–4: resolved None exports no artwork; Pathfinder bakes inherited placement through unit_xform; Wand compares resolved variants.
- Astra 6 / Opus 10: owned current paints survive eyedropper→shape/pen; owned gradients fit recipients; global references stay linked; deletion materializes defaults.
- Astra 5 / Opus 7: thumbnails sample resolved gradient paints and hash live swatch contents/transforms; control-bar chips and colour lists use representative colours.
- Astra 7 / Opus 8: LUTs keyed only by stops/interpolation; shared sampler; pan/zoom writes existing uniforms, preserving textures/bind groups.
- Opus 10: migration no longer validates; literal provisional version pinned; identity test proves no validation/repair.
- Refusal fixtures assert typed errors/details; frozen v5/v6 gates test raw JSON/PDF refusal before malformed models/assets; v5 JSON/PDF/SVG byte goldens pass.
- Opus 9: marked gradient_canvas::coverage seam + headless test; integrator must bind stroke/canvas.rs hotfix cache/world rings/fallback/report.
- Opus images 1–5: image model, Prim::Image and pdf/images.rs are absent here; image writer/compression/budget/signature/tests remain external requirements.
- Disagreement with applicability of Opus 5 here: this lane already retains v5 byte goldens; weakened image-lane tests are not present.
- Integration checklist: [W2_GRADIENT_FIX_INTEGRATION.md](docs/reference/W2_GRADIENT_FIX_INTEGRATION.md); proposed images→Corners→gradients→text; mixed-export/Prim audit pending.
- Gates: fmt, dependency directions, diff check PASS; offline workspace -j 3: 1931 primary + 3 subprocess passes / 0 failed / 15 ignored.
- Native + x86_64-pc-windows-msvc workspace/all-targets clippy --offline -j 3 -D warnings: PASS, zero warnings.
- Shell ratchets 3/3, Bridge ratchets 6/6, Bridge fixture contracts 5/5 PASS; legacy 1.0/1.1 bytes frozen; API 1.2 22705 B; ui.rs 816/843.
- Evidence: varos/target/lane-b-fix-{fmt,deps,workspace-final,clippy-native,clippy-windows,shell-ratchets,bridge-ratchets,bridge-fixtures}.log; re-review/native acceptance/GPU heat unverified.

# w2-export-paths — Lane C
Status: implemented (provisional UI, owner design review pending); committed lane plus uncommitted fix-round changes.
1.8: persisted Advanced state; ranges/bleed/colour/whole-board; card × row exports, presets, names/sub-folders, PDF modes, SVG options.
1.4: kit New Document dialog, categories/units/count/layout/bleed/ppi; physical dimensions preserved across unit changes.
2.3 / 4F: Outline/Offset/Expand, scale-strokes preference, live corner widgets/typed radius, undoable commands.
Hosts: EditCommand/AppCommand, API 1.2 verbs/schema/discovery, CLI new-document/export-screens/apply; frozen 1.0/1.1 retained.
Writer: next version (locally 6), doc.paths[].corners {radius,kind}; migrate_v5_to_live_corners and frozen/refusal fixtures.
Attribution: VectorCraft adaptation headers and NOTICE; no new dependencies.
Merge: integrator assigns version/migration/fixture numbers with sibling writers; union Bridge 1.2 and reconcile shared host changes.
## Fix round
Astra fdd7ec6 review: all five P1/P2 findings accepted and fixed; no disagreements.
P1 corners: shared stroke evaluator resolves live corners; rounded/inverted/chamfer, styled/dashed Outline and Expand match baked geometry.
P1 compositing: Expand refuses mixed fill/stroke with object opacity <1 or stroke alpha <1 before mutation; single-paint expansion remains supported.
P1 Selection PDF: UI disables single mode with a reason, jobs export each snapshot separately; shared planner refuses mismatched combined snapshots.
P2 host parity: app/Bridge/CLI use shared SVG options encoder; headless worker/Bridge and real CLI byte comparisons pass.
P2 precision: 0–8 decimals applied at initial numeric serialization, including stroke/clip geometry; legacy core API preserves frozen goldens.
Cheap coverage fix: corner cache test now checks actual hits, misses, fresh equivalence and Arc reuse after invalidation.
Workspace: cargo test --offline --workspace -j 3 --no-fail-fast PASS: 1,922 passed / 0 failed / 15 existing ignored; 114 suites.
Gates: fmt, dependency directions, native + Windows all-target Clippy -D warnings, git diff --check PASS.
Ratchets 3/3 PASS; Bridge contracts/fixtures 88 passed / 4 existing ignored; no fixture updates.
Audit: all 247 original core/Bridge fixtures byte-identical to b3d39ee; ratchet source unchanged; ui.rs 816 ≤843.
Evidence: /tmp/w2-export-fix-final-*.log, /tmp/w2-export-fix-final-gates.json, /tmp/w2-export-fix-final-audit.json.
No git writes, push, merge, GUI launch, build installation or new independent review performed.
Still pending: independent re-review, integration/version assignment, owner design/native acceptance, Windows runtime, idle heat/undo RAM measurements.

# Lane F — Phase 9 application handoff
Base d403aca; original work committed by coordinator; this fix round remains uncommitted.
9.1–9.6 implemented (provisional UI, owner design review pending): Preferences, registry, shortcuts, History, Actions, Help/preview.
API 1.2 + CLI paths are wired; API 1.0/1.1 fixtures remain byte-frozen; no new dependencies or prior-art lifts.
## Fix round — 2026-10-09
Accepted all seven reviewer findings; no disagreements.
P1 preview: next format provisional 6; matching JSON/catalog stamps, pure migrate_v5_to_next_preview, frozen containers/refusals, VRS_FORMAT updated.
Added keys: /VAROS_Preview and /VAROS_PreviewVersion=1; no authored JSON fields added; original v1–v5 fixtures untouched.
Moderator must renumber preview migration/stamps/fixtures after images, gradients, text and Live Corners in actual merge order.
P2 shortcuts: Reset excludes held Space; old persisted defaults pass to incumbent dispatch; rebound document keys retain repetition.
P2 recording coverage: every changed commit checks semantic coverage; direct gestures/checked creation/Undo/Redo visibly discard unsupported recordings.
P2 target binding: one selection/first supported target set per recording; Bridge checks across requests and refuses partial export on changes.
P2 cache bounds: reads ≤2 MiB plus sentinel, strict 544×246 PNG dimensions and 8 MiB decoder budget; validated bytes reused.
P2 cache failures: generate/embed in memory; optional directory/write/durability failures cannot block native Save.
P2 History: ceiling restored to 200 in core, Preferences and Bridge schema; higher depths await image-memory evidence.
Headless regressions cover every finding, Reset→Apply dispatch, cross-request Bridge targets and direct Undo.
PASS: targeted regressions, frozen v5 hashes/visual goldens, updated old-reader harness 9/9, fmt and dependency directions.
PASS: native + Windows workspace/all-targets clippy -D warnings; app ratchets 3/3 and Bridge ratchets/fixtures 6/6.
PASS: cargo test --offline --workspace -j 3 --no-fail-fast: 1932 passed / 0 failed / 15 ignored.
Evidence: /tmp/w2-app-fix-{workspace-final,clippy-native-final,clippy-windows-final,ratchets-app,ratchets-bridge}.log.
Ratchet files/limits and legacy Bridge fixtures unchanged; ui.rs remains 772 lines (cap 843).
Pending integration: sibling BlobStore preview/save path, combined Bridge size budget, independent re-review and owner native acceptance.
No git writes, push, merge, GUI launch, install or signed Quick Look packaging performed.

---

# feat/w3-render

Lane D renderer capability — feat/w3-render; fix-round hand-back (no git writes).
- Renderer-only primitives, CPU reference and encoder-only RGBA16F GPU passes; producer hookup remains pending.
- New lane modules: raster/{layers,layer_tests,layer_scene}.rs; render-wgpu/{layer_gpu.rs,layer_pass.wgsl}; original shared lib.rs blocks remain unchanged.
- Nested layers, 16 blend modes, masks at LayerEnd, Gaussian blur, shadow/glow; capped subtrees flatten explicitly.
- Default depth 16 / 256 MiB; four surfaces per active layer, quarter-budget effect cache; CPU f32 / GPU f16 accounting.
- Budget covers offscreen/cache reservations; root, geometry, staging/uniforms and encoder-retained resources are outside measured accounting.
- Producer must provide RGBA16F geometry, bucket_zoom and complete-input revisions (including effects/pan); no model/format/API change.
- Attribution headers/NOTICE retained; provisional owner review status unchanged; no GUI launch or installation.

## Fix round
- Accepted all three P2 findings; no disagreements. Cross-lane merge risks are integration obligations, not resolved by this isolated lane.
- Reduced-budget admission trims retained pool against live scratch plus cache before allocating; cache insertion also trims surplus pool.
- CPU/GPU share bounded LRU policy: replace obsolete object revisions, promote hits, evict oldest entries before replacement allocation.
- CPU and WGSL Dodge/Burn use exact backdrop endpoints; independent near-endpoint goldens prevent inverted colours.
- Added deep-pool→85-byte regression, revision 1/2/3/3 blur+shadow hits, bounded LRU recency; independent coloured goldens cover all 16 modes/alpha cases.
- Numeric effect-refusal tests now enclose effects in valid layers and assert specific errors before rendering.
- Targeted headless gate: 39 passed, 0 failed, 2 timing probes ignored; shader parsed/validated without GPU.
- PASS fmt, dependency directions, workspace tests (2283 passed / 0 failed / 17 ignored), native + Windows Clippy -D warnings, core/UI ratchets, Bridge fixtures (6 passed; 1.0/1.1=23993 B frozen; 1.2=23879/24000 B). Evidence: /tmp/w3-render-fix-gates.json and /tmp/w3-render-fix-*.log.
- Integration: reconcile appearance traversal/types/masks/formats/aggregate budgets, effects/live input revisions, CMYK overprint and text outline settlement.
- GPU execution, renewed independent review, producer integration and owner hand-testing remain unverified; no commit/push/merge.

---

# feat/w3-appearance

# Lane A — Appearance / FORMAT v10
State: implemented (provisional UI, owner design review pending); committed baseline `29b46a7`, fix round uncommitted.
Scope: ordered fills/strokes, container looks, Expand, nested clip/alpha masks; kit Properties/row hooks; Bridge/CLI; GPU/CPU/PDF/SVG.
Format: pure v9→v10 migration, legacy byte identity, frozen output/refusal fixtures; moderator owns v10–v14 rechaining.
Guards: GPU depth 6 / 512 MiB with reported flattening; CPU 128 MiB / depth 12 with refusal; existing ratchets unchanged.

## Fix round
Accepted all five findings; no disagreements.
P1 geometry: bake every distinct affected transform unit, including separately rotated Layer children, before reparenting.
P1 topology: stage and structurally/semantically validate masks before publication; Layer sources and targets containing sublayers refuse atomically.
P1 rebuild: eliminate per-entry Editor clones / full export scenes / document-wide scene cache; reuse one evaluator, leaf geometry/stroke caches, culling and aggregate export budget.
P2 drawing: mask-row selection/toggle/activation restores its authoritative Group drawing child.
P2 swatches: preserve authored base/extra paint references separately from resolved display colours during opacity/visibility edits.
Regressions: four added core tests + one UI test; existing pan test now measures real leaf-cache misses; focused core 13/13 passed.
300-leaf debug probe: plain 13.8 ms / appearance 13.0 ms (reviewer's appearance probe: 922 ms); actual thermals remain unmeasured.
Gates PASS: fmt, dependency directions, workspace (2273 logged passed / 0 failed / 15 ignored), native+Windows all-target clippy -D warnings; UI ratchets 3/3, Bridge 113 passed / 4 ignored; frozen 1.0/1.1 match, 1.2 tools/list 23,912/24,000 B; ui.rs 812/843.
Integration: sibling effects/live order+Expand, CMYK/text Forms, GPU pool/CPU parity, Arabic/a11y hooks, format chain and combined UI/Bridge budgets still require moderator checks on the combined tree.
Evidence: `/tmp/varos-appearance-fix.TyEBdP/`; GUI/GPU pixels, PDF viewer pixels, thermals and owner acceptance unverified.
No git writes, commit, push, merge, GUI run or app installation; no dependencies or ratchet increases.

---

# feat/w3-effects

# Lane B — Phase 10 live vector effects (2026-10-10)
State: implemented (provisional UI, owner design review pending); fix-round edits uncommitted.
Typed Offset/Zig Zag/Transform/15 Warp styles; width presets/custom points + ⇧W gestures; one-undo previews.
Shared bounded evaluation feeds canvas/CPU/PDF/SVG/Expand; Bridge 1.2 progressive verbs + typed CLI.
v11 writer + pure v10→v11 migration; temporary v10 reservation must be replaced during integration.
ADR-0016 proposed; prior-art attribution/NOTICE retained; appearance integration deferred to integrator.
No git writes, GUI, install, merge or push; owner visual acceptance remains unverified.
## Fix round
P1 recursion: atomic baking returns errors; command preflight propagates limit refusal before redispatch.
P1 sibling edits: outline targets are replacement leaf IDs only; clear cloned anchor/group selection.
P2 Width Escape: cancel before selection clearing; restores profile, closes transaction, release adds no history.
P2 refusal: frozen v9 header/catalog gates check raw v11 JSON and emitted PDF/model with typed NewerVersion.
Cheap coverage fix: production effects_document cache test proves idle reuse + geometry/paint/effect invalidation.
Regression tests: 24/24 targeted PASS; oversized 41×1,000 copies, selected group leaf (0/2 copies), Escape (2 profiles).
Disagreements: none with the four findings; sibling-lane pipeline/schema/GPU-budget/i18n risks remain integration work.
fmt / dependency directions / diff check: PASS; full logs + exit codes: /tmp/w3-effects-fix-round/.
cargo test --offline --workspace -j 3 --no-fail-fast: PASS 2,278 passed / 0 failed / 15 ignored (152 suites).
Native + Windows x86_64-pc-windows-msvc clippy --all-targets -D warnings: PASS; app ratchets 3/3, Bridge 6/6.
Bridge 1.0/1.1 byte-frozen 23,993 B; 1.2 23,937/24,000 B; ui.rs 821/843; effects SHA256 6/6 PASS.

---

# feat/w3-cmyk

# Lane C — Phase 12 / FORMAT v12

CMYK/Gray/Spot source paints, document mode and bounded ICC metadata; legacy RGB bodies preserved.
Undoable edits, progressive Bridge 1.2 + CLI; provisional existing-kit UI, owner design review pending.
PDF process/ICCBased/Separation/OutputIntent + guarded PDF/X-4; external conformance unverified.
Proof is vector-only; overprint is approximate multiply; gradients stay RGB; images retain existing rendering.

## Fix round

P1: gradient forms reuse one document colour-resource set; no per-path ICC/spot duplication.
P2: picker seeds resolved CMYK/Spot channels, name/tint/alpha; unchanged components remain bit-exact.
P2: panel drafts track selection/target/resolved paints; document-scoped state and field IDs prevent leakage.
P2: Paint/Live validate the prospective document before mutation; ColourManagement shares that check.
P2: bounded editor-owned ICC cache retains screen/proof executors by content; fixed layouts/default intent.
Eight new headless regressions cover all five findings, exact source round-trips, cache reuse/eviction and rollback.
Gates PASS: fmt, dependency directions, workspace (2267 passed / 0 failed / 15 ignored; 150 suites), native + Windows Clippy -D warnings, ratchets.
Bridge PASS: six fixture/ratchet tests; 1.0/1.1 frozen at 23,993 B; progressive 1.2 = 23,904 / 24,000 B (96 B headroom).
ui.rs remains 807/843; tokens/ratchet ceilings unchanged; no new production unwrap; no disagreements.
Integration still requires combined v10→v14 chain and cross-lane appearance/live/render/Arabic/text reconciliation.
Evidence: /tmp/w3-cmyk-fix-round.45sFok/; no git writes, GUI, merge, push or app install; native acceptance pending.

---

# feat/w3-live

# Lane E — Phase 11 — feat/w3-live
Blend, Repeat and Envelope implemented (provisional UI, owner design review pending); FORMAT v13.
Authored sources remain editable; bounded cached evaluation serves canvas, hit bounds, SVG/PDF and Expand.
API 1.2 live verbs use progressive schemas; CLI and frozen native/refusal fixtures are covered.
Sibling appearance/effects, CMYK, render, Arabic/a11y and text/v14 reconciliation remains integration work.

## Fix round
P1 spine replacement: released root-level spine returns to roots; structural precheck precedes publication.
P1 mesh placement: points follow baked/live geometry transforms and offset Paste; duplicate maps only its copy.
P2 swatches: Make preserves authored fill/stroke references; evaluation resolves paints without destroying links.
Four new regressions cover root save/reopen/undo, recolour/Release, Move/copy/Paste, multi-source previews/undo/redo.
Targeted live tests: 18 PASS; fmt, dependency directions, native + Windows all-target clippy -D warnings PASS.
Full offline workspace gate: 2271 PASS / 0 FAIL / 15 ignored (--workspace -j 3 --no-fail-fast).
UI ratchets: 3 PASS; Bridge ratchets/frozen 1.0/1.1 fixtures: 6 PASS; 1.2 tools/list 23887/24000 B.
Ratchets, protected fixtures/tokens/kit and UI source unchanged; ui.rs remains 809/843 lines.
Disagreements: none with the three reproduced defects; merge-risk items require sibling integration review.
Evidence: /tmp/w3-live-fix-{targeted,fmt,deps,workspace,clippy-native,clippy-windows,ratchets,bridge-fixtures}.log.
Fixes are uncommitted worktree edits; no git writes, merge, GUI launch, bundle rebuild, push or installation.
Pending: fresh independent fix review, cross-lane integration, and owner native/design acceptance.
