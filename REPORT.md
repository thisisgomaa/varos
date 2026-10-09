# Lane F — Arabic UI / ADR-0012 T2–T3
Baseline: committed `b157739`; this fix round is worktree-only. No git writes, GUI, install or merge.
Status: implemented (provisional UI, owner design review pending).

## Fix round
Accepted all six reviewer findings; no disagreements.
- Drag-selection anchors at pointer-down; event regression checks copied bytes and replacement boundaries.
- Multiline fields scroll vertically; Arabic paste and return-to-start tests keep caret/IME rectangles inside the field.
- Galley row budgets retain grapheme-safe elision, original row baselines and cell clipping; 1/2/3-row regressions.
- Authored labels bypass translation; artboard/image/property names and isolation paths use the literal path.
- Start Name/Tags/Folder/Modified and both Layers empty states have Arabic production-paint assertions; catalogs now contain 600 entries.
- Complete production label and galley meshes match epaint at 1×/2×, including clipping, atlas updates and composition (mean ≤1/255, max ≤32/255).
- Shared text/editor: 19 PASS; canvas artboard named `Delete` remains literal in Arabic mode.
Gates: fmt, dependency directions, native/Windows workspace all-targets clippy `-D warnings`, varos-text WASM PASS (offline, `-j 3`).
`cargo test --offline --workspace -j 3 --no-fail-fast`: PASS (exit 0; 2,272 passed, 0 failed, 15 ignored).
Ratchets: shell 3/3, Bridge 6/6 PASS; limits unchanged; `ui.rs` 805 lines (cap 843).
Bridge fixtures: 1.0/1.1 byte-identical, 23,993 B each; 1.2 23,879/24,000 B (121 B headroom).
No writer/model/format changes or migration; existing v9 and refusal fixtures unchanged.
Pending: independent re-review, sibling-lane conflict reconciliation (especially a11y), owner native IME/DPI/Arabic acceptance, U2-P and atlas-pressure heat validation; Arabic Medium/SemiBold assets remain absent.
Evidence logs: `/tmp/w3-{workspace-final,clippy-native-final,clippy-windows-final,shell-ratchet,bridge-ratchet}.log`.
