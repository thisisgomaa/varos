# p0-export — 0.4/0.5 fix round

Baseline: pre-review commit `1d27270`; original REPORT.md was absent; prior handoff read.

## Fix round

- P1 durability: retain directory-sync warnings in ExportWrite/ExportResult, Done notes and multi-artboard Bridge receipts (`durable=false`). Tests: injected SyncDir failure, Done aggregation, first/last-page receipt warnings.
- P2 identity: selection cards cache `selection:<object/group ID>`; names are display-only. Test: duplicate names, uncheck one, rename/reorder, reopen preserves the export set.
- P2 legacy DTO: version-aware decoding rejects all five raster fields for default/1.0/1.1, including null, while preserving duplicate/unknown-field rejection. Tests cover save/save_as/export_pdf and explicit 1.2 raster acceptance.
- P2 thumbnails: replace per-card polling threads with a sheet-owned bounded decoder (16 pending), ThumbService completions, terminal failures and cancellation on drop. Tests: unwritable cache completes, bounded admission, drop signals shutdown.
- P2 repaint heat: cache card IDs; persist only after control/checklist changes; skip offscreen preview work. Tests: unchanged-model and actual headless sheet repaints leave preferences untouched.
- Merge risk: partial batch cancellation retains written count and prior durability warnings; regression test passes. Shared changes stay in export-specific blocks; no new shortcut/command/settings key in this round.
- Coverage gap: independent known-rectangle SVG/PNG/WebP/TIFF tests check fill, transparency, scaling and selection crop; crop bounds use floating-point tolerance.
- API 1.2 admission/discovery remains additive; default discovery and legacy fixtures retained. Sibling-lane registry reconciliation remains integrator work.
- Disagreements: none; all ranked findings addressed; fixes pending independent re-review.
- Gates: fmt PASS; dependency directions PASS; offline workspace PASS (1534 passed, 0 failed, 15 ignored).
- Clippy: native and x86_64-pc-windows-msvc all-targets PASS, -D warnings; offline -j 2.
- Explicit gates: ratchets 3 passed; Bridge contracts/fixtures 70 passed, 4 ignored; targeted rendering 1 passed, legacy decoder 2 passed.
- Ratchet ceilings and Bridge fixture files unchanged against f2066f1; git diff --check passes.
- No commit, push, GUI or installation; native visual interactions and Windows runtime remain unverified.
