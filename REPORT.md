# Lane F — Arabic UI / ADR-0012 T2–T3
Branch: `feat/w3-arabic-ui`; worktree-only, uncommitted; no push, GUI launch, or install.
Status: implemented (provisional UI, owner design review pending); acceptance limitations below.
Paths below are relative to `varos/crates/`.
- `varos-app/src/shell/kit/text/`: varos-text labels, grapheme elision, fixed role baseline, bounded cache, retained atlas pages, CPU raster.
- K3 text/search/numeric fields share the engine caret, selection, logical clipboard, local undo and IME preedit/commit handling.
- `varos-app/src/i18n/`: 598 en/ar TSV entries; menu/command and preference/panel-title coverage; matching named sentence arguments.
- Preferences adds Arabic through existing ApplyPreferences/API 1.2 preferences flow; system locale default; language changes require restart.
- RTL name/menu/action/dropdown alignment and directional chevrons preserve panel/box order; numeric-only Arabic/Persian digit normalization.
- Native menu titles use OS shaping where the native API permits; authored document strings remain logical UTF-8.
- No new Bridge verb, inline summary expansion, document writer change or format migration; no core UI dependency.
- Shell ratchets 3/3 PASS; limits unchanged; `varos-app/src/ui.rs` remains 805 lines (cap 843).
- All dependencies were cached offline; no missing-crate workaround. Epaint raster attribution is in source and `varos/NOTICE`.
Gates (offline, `-j 3`, from `varos/`):
- `cargo fmt --all --check`: PASS.
- `python3 ../tools/check_dep_directions.py`: PASS; varos-text remains a pure leaf.
- `cargo test --offline --workspace -j 3 --no-fail-fast`: PASS (exit 0; 2,265 reported passes, 0 failures, 15 ignored).
- Native workspace/all-targets clippy `-D warnings`: PASS.
- Windows x86_64-pc-windows-msvc workspace/all-targets clippy `-D warnings`: PASS.
- `cargo check --offline -j 3 -p varos-text --target wasm32-unknown-unknown`: PASS.
- Bridge ratchets: 6/6 PASS; frozen 1.0/1.1 = 23,993 B each; 1.2 = 23,879 / 24,000 B (121 B headroom).
- Focused evidence: 29 K3 tests and 14 shared-text/editor tests PASS in the final run; numeric Arabic Text/Paste/IME, UTF-8 undo and 1×/2× glyph raster parity covered.
Evidence: `arabic-ui-proof.png` is a visually inspected CPU shaping/raster proof, not a window screenshot.
Limits: only bundled Plex Arabic Regular exists; Medium/SemiBold assets are absent, with no synthetic bold substituted.
Limits: catalog coverage does not yet prove every dynamic error/Start message; English fallback remains, and legacy galley measurements remain at some adapted paint sites.
Limits: general chrome alignment beyond the explicit cells above needs owner review; full-component pixel parity and U2-P latency were not established.
Owner checks pending: actual IME/dead keys/emoji palette, mixed-name editing/readback, DPI/display changes, Arabic copy/design approval.
Arabic UI acceptance under ADR-0012 §8 is not claimed complete; no independent review, merge, release build/install or owner hand-test was performed.
