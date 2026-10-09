# Lane C — Phase 12 / FORMAT v12
Worktree feat/w3-cmyk; uncommitted. No push, GUI, installation or agents.
ADR-0017 proposed first; PLAN records provisional UI, owner review pending.
CMYK/Gray/Spot source paints behind Rgba; RGB JSON bodies unchanged apart from v12 stamp.
Document RGB/CMYK mode, bounded hex ICC metadata; moxcms 0.8.1 available offline.
Explicit matching-profile screen conversions; naive unprofiled CMYK approximation documented.
Undoable mode/profile/source edits; progressive Bridge 1.2 colour_management + headless CLI.
Legacy Bridge 1.0/1.1 fixtures unchanged; profile assignment currently headless via Bridge/CLI.
Existing-kit New Document/Setup, picker CMYK/Spot, PDF preset and view toggle controls.
PDF DeviceCMYK/Gray, ICCBased, Separation tint functions, OutputIntent; per-path export notes.
PDF/X-4 vector preset refuses images/page-info fonts; external conformance is unverified.
Overprint is approximate all-vector multiply, not calibrated ink-separation overprint.
Proof is vector-only; images retain their existing rendering. Native GPU/UI acceptance pending.
Existing gradients remain RGB; no DeviceN/multi-ink model, image ICC or press-calibration claim.
v11→v12 pure migration + frozen/refusal fixtures; reserved v10/v11 identity steps MUST be rechained.
Old fixture files unchanged; historical PDF comparison preserves exact dictionaries/stream bytes apart from stamps and offsets.
No prior-art reference code lifted; own existing renderer pipeline adapted; no NOTICE row required.
ui.rs 807/843 lines; tokens and ratchet ceilings unchanged; production code has no new unwrap.
Gates PASS: fmt check; dependency directions; workspace 2260 passed / 0 failed / 15 ignored (151 suites).
Gates PASS: native + x86_64-pc-windows-msvc all-target Clippy -D warnings; workspace ratchets.
Bridge fixtures PASS: 1.0/1.1 23,993 B unchanged; 1.2 23,904 / 24,000 B (96 B headroom).
Tests cover conversions, ICC refusal, swatches, undo, API gating, CLI saves, PDF spaces and CPU overprint.
No commit/push/merge, GUI or app-bundle rebuild/install; integration and owner design/hand testing pending.
