# Notes for the Foundation Charter

**Status:** forward-looking input for the foundation charter being prepared by planning. This file is deliberately separate from the F1 as-is inventory and ownership maps. It is not an accepted ADR or authorization to change code.

## Extraction preconditions proposed during F1

1. A reviewed ADR must define the command boundary before replacing private `ui::Op`.
2. Characterization tests must cover a responsibility before it moves out of `ui.rs` or `editor.rs`.
3. Each later extraction must preserve the crate directions in `DEPENDENCY_MAP.md` and the golden-file round-trip gate.
4. The fate of `codex/p6-header` must be decided before the UI split begins, because it touches the shell/UI boundary.
