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
