# Text programme fix-round integration

This is a working-tree snapshot, not a commit. The user explicitly prohibited
commit/push in this round; the reviewer also observed HEAD 7b48f2c with no lane
commit. Stage paths explicitly during integration, including untracked files.

Treat deletion of `varos/crates/varos-text/src/converge.rs`, its replacement
`src/converge/{mod,analysis,fitting}.rs`, `engine_support.rs`, `text_types.rs`,
and the engine/lib registry changes as one atomic change. Include new composer,
kashida, metrics and paths modules, bundled fonts/licenses and test fixtures.
Do not cherry-pick only tracked diffs: that drops the implementation and fonts.

The sibling core/app/Bridge hotspots, schema, shortcuts and settings are untouched.
The new optional `Engine::compose_with_hyphenation` API is contained in varos-text;
existing `compose` and ordinary/incremental layout callers retain their signatures.

Existing lane shared-file edits remain small and additive: NOTICE attribution,
BASE.md supplemental-patch ledger, three checker lines, six COSMIC shaping lines.
The tatweel patch and checker hash must land together, after the frozen P1b patch.
Do not resolve sibling vendor conflicts by regenerating the frozen P1b patch.
Run the complete vendor reconstruction check after integration.

Future outlines wiring must render/export each GlyphPath independently with
NonZero filling, retaining both contour winding and the per-glyph boundary.
Never concatenate contours across glyphs or silently persist them as EvenOdd.
Headless pixel-parity tests cover this lane's adapter; future core/export wiring
and Arabic owner acceptance remain separate acceptance gates.
