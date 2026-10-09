# p2-geom — core geometry lane

- Scope: kurbo 0.13.1 adapter, shape constructors, fitter and pure path edits; attribution in source + NOTICE.
- Files: core `src/geom/{kurbo,shapes,fit,edit}.rs`, `src/geom.rs`, `tests/geometry.rs`, Cargo.toml/lock, NOTICE, PLAN.
- Snapshot discrepancy confirmed: HEAD=f2066f1; pre-review geometry is uncommitted; REPORT.md was absent. No commit created, per instruction.

## Fix round

- P2 fit.rs: certify sample distance against converted f32 curves; exact sample polyline fallback on failed certification.
- Test: reviewer’s (100000000,0)/(100000008,8)/(100000024,0) example and transposed case both satisfy 0.01.
- P2 kurbo.rs: retain the current point after ClosePath; subsequent segments create a separate contour instead of disappearing.
- Test: post-close LineTo/QuadTo/CurveTo, repeated closes, following LineTo and explicit MoveTo; compound open extras reject.
- P2 edit.rs: cache nearest pair candidates in a heap with stable slots/generation invalidation; recompute only affected pairs.
- Test: 1,000 paths retain deterministic ordering/2,000 anchors; exactly 3,992,004 distances vs 666,666,000 before.
- Complexity: O(n²) distance evaluations/storage, O(n² log n) heap work; existing reversal/handle/endpoint tests pass.
- Merge risks: document common coordinate space, zero-ID allocation and one-undo caller contract on Join; command reconciliation remains deferred by core-only brief.
- No shared editor/app/Bridge/storage changes or new shortcuts/settings/commands; stroke consumers can use the public kurbo adapter.
- Targeted: geometry 14 passed/0 failed; join complexity regression 1 passed/0 failed.
- Gates: fmt PASS; dependency directions PASS; workspace 1,527 passed / 0 failed / 15 ignored.
- Native + Windows-target Clippy (`--workspace --all-targets -D warnings`) PASS, 0 warnings; final test-only lint correction rerun 1/1 PASS.
- Logs: `/tmp/p2-geom-fix-{tests,clippy-native,clippy-windows,join-final}.log`.
- Ratchets: 3/3 PASS, source unchanged; Bridge contracts 70 passed / 0 failed / 4 ignored; all 14 fixtures byte-identical to HEAD.
- No findings disputed; no commit/push/GUI/install; independent re-review and owner acceptance remain pending.

# Lane p2-stroke — 2.1 StrokeStyle / format v5
Checkout HEAD/base 7b48f2c; original implementation and fix round remain uncommitted here.
Scope: authored style + core coverage, GPU/CPU/PDF/SVG, v5 migration/corpus, opt-in Bridge 1.2, provisional Stroke UI.
## Fix round
P1 PDF: original rings now emitted only for native stroke; baked knockout paints coverage alone. Stream regression + independent PDFium test (3 PDFs / 12 pixels) pass.
P1 raster: Scene.errors yield diagnostics and no pixels; encoding/sampling refuse failed rasters; checked page API and Bridge preserve errors. Rotated locally-valid dashed-path raster + board/page Bridge refusal tests pass.
P2 scrub: numeric gesture accumulator publishes one checked field batch on release; typed arrow steps retain kit behaviour. Real multi-frame kit drag, single undo, and arrow-step tests pass.
P2 no-op: identical SetStrokeStyle exits before begin; changed-target batches omit unchanged styles. Revision/history/redo preservation regression passes.
P2 dash: explicit Dash(index) operation and selective same-length differences preserve each target's other entries. Mixed [6,3]/[10,8] scrub and core difference tests pass.
P2 idle: stroke inspection cached by revision + object/anchor/direct selection, bypassed during transactions/dirty edits; has_length uses active controls without arc integration. Cache invalidation + handle-degeneracy tests pass.
All six findings accepted and fixed; no findings skipped or disputed.
Corpus: refreshed only 33 incorrect baked v5 PDF files and hashes; 43 JSON/PDF/SVG triples and 131 hash entries; historical fixtures untouched.
Merge risk: new numeric field IDs use p2-stroke-number namespace; no new shortcuts/settings/window keys; Bridge remains additive and gated to 1.2.
Shared-file fixes use local blocks; raster refusal reaches thumbnail worker, picker, and Bridge; Reverse Path head/scale swap regression added.
Integrator still owns sibling kurbo adapter unification, crash-rollback reconciliation, worker/Prim consumers, and first-merged-writer version ownership.
Gates: fmt / dependency directions / whitespace PASS.
Gates: cargo test --offline --workspace -j 2 --no-fail-fast: 1556 passed / 0 failed / 15 ignored (89 suites), exit 0.
Gates: native + x86_64-pc-windows-msvc clippy --all-targets -D warnings PASS (exit 0 each).
Gates: ratchets 3/3; Bridge contracts 73 passed / 4 ignored; all 14 Bridge + 39 v1-v4 fixture files byte-identical to HEAD; ratchet source/ui.rs unchanged.
Logs: /tmp/p2-stroke-fix-{workspace-final2,clippy-native-final,clippy-windows-final,ratchets,bridge-contracts}.log.
Rendered regression: python3 tools/check_stroke_pdf_render.py (local pypdfium2 + Pillow); no writer-dependent pixel goldens.
No commit/push/merge, GUI launch, installation, Windows runtime test, native acceptance, or .bad quarantine test performed.
