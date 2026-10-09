# w2-text — Lane G: Text programme P2–P4

- Status: implemented (provisional UI, owner design review pending); fix-round changes uncommitted.
- Product: editable point/area styled text, Type tool/IME/carets, Type properties, cached outlines, PDF/SVG, Bridge 1.2 and CLI.
- Seam: core owns serializable text; varos-text-layout owns shaping/outlines; core → text remains forbidden.
- Format: provisional next writer 6, named pure `migrate_to_text_boxes`; moderator assigns final number after images/gradients.
- Keys: doc.text_boxes; NodeKind::Text; TextBox id/box_kind/frame/runs/para; run text/style; style font{family,weight,hash}/size/letter_spacing/fill; para align/direction/kashida/line_height.
- Integration: preserve every migration/refusal fixture, combine text/image/gradient export traversal, and retain sibling registrations plus text settlement hooks.

## Fix round
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
