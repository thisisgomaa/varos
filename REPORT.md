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
