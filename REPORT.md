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
