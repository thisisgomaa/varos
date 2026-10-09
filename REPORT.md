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
