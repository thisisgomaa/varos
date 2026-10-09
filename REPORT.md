# p1-pdfprint — Phase 1.2 / 1.3 / 1.9

Existing lane: PDF options + kit hook, CLI/API 1.2, Preview print fallback, detached Varos/PDF/SVG/2× PNG clipboard.

## Fix round — 2026-10-09

- P2 selection: core `capture_selection_clipboard(cut)` shares Copy/Cut source rules; direct path selection works; anchors cannot publish stale data. Test: direct Copy/Cut + stale-anchor rejection.
- P2 Cut availability: publish lossless Varos data even without painted bounds; omit only PNG for oversized selections; report actual types/reasons through desktop notices and Bridge. Tests: unpainted/opacity-zero Cut + oversized Cut + undo.
- P2 locked groups: capture lock-filtered Cut sources before publication, then execute core Cut unchanged. Test: all public bytes match filtered payload; OS/internal payloads agree; locked art remains; one Undo restores.
- P2 coincident boards: resolve source bleed in PDF `PageSpec` at planning, never by rectangle lookup. Test: coincident boards with 3/9 pt bleed in all/active exports; parsed boxes checked.
- Merge diagnostics: options writer retains `export_pdf_bytes_with_report` notes and appends ppi note. Test: default/customized writer report preservation.
- Merge seams: sheet/job fields are additive `pdf_options`; dedicated `ui/export/pdf_options.rs` hook; `pdf-*` UI IDs, permitted Cmd+P, no persisted settings keys. Core editor addition is one five-line helper.
- API 1.2 stays opt-in/additive in existing dispatch/discovery; legacy fixtures retained. Integrator must reconcile sibling sheet/job hooks and union of 1.2 tools; no sibling branch merged.
- No finding disputed or skipped; 4 P2 findings fixed, plus local merge risks.
- Focused gates: clipboard 6 passed; PDF options 5 passed (6 new regressions across both suites).
- Full workspace: `cargo test --offline --workspace -j 2 --no-fail-fast` PASS: 1,527 passed / 0 failed / 15 ignored, 88 suite summaries; long Arabic look-ahead regression passed.
- Final desktop after additive field rename/Copy command wiring: 749 passed / 0 failed / 1 ignored; clipboard 6/6 and all 3 ratchets pass.
- fmt PASS; dependency directions PASS, 0 violations; native + Windows x86_64-pc-windows-msvc workspace/all-targets clippy `-D warnings` PASS, 0 warnings each.
- Bridge contracts 71 passed / 0 failed / 4 ignored; 14 tracked legacy fixtures byte-identical. Ratchet source/tokens/historic PDF writer unchanged; `ui.rs` 843/843 lines.
- Evidence logs: `/private/tmp/p1-pdfprint-fix-{tests,app-final,clippy-native,clippy-windows}.log`; whitespace check PASS.
- Snapshot: actual HEAD remains base `f2066f1`; incoming implementation was uncommitted and `REPORT.md` absent. Changes preserved; this report created. No commit/push/merge/GUI/install performed.
- Limits: provisional UI awaits owner review; native print/paste unverified; Preview fallback requires File > Print, not a direct AppKit print panel.
