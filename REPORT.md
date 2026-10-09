# Lane w2-tools-ui — wave 2 (2026-10-09)

# w2-text — Lane G: Text programme P2–P4

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
