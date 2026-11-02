# Align density and Control snapping access

2026-11-01. Align panel uses the same licensed alignment glyphs and 26-point buttons as Properties, with 8-point spacing and no extra padding around center alignment. Distribution buttons match the compact size.

Control reserves four fixed 28-point controls on the right: Grid, Pixel, Snapping options (original code-drawn magnet) and Control options. Existing commands drive all toggles; the popups include Point, Smart Guides, Glyph, alignment, tangent/perpendicular and Preferences. Grid priority over Pixel is indicated on hover. Original code-drawn glyphs introduce no asset files or Adobe artwork. Contextual controls scroll independently; one-row layout retained.

Checks: 24 Align tests, 1 quick-access pointer test, 102 panel workspace tests (4 optional captures ignored), scoped clippy passed. Release/native acceptance pending. No commit/full CI claimed.

## Home and modeless Preferences in the same owner-requested update

Home includes Back to document above Recents only while documents are open; it resumes the active document without modifying artwork or layout. Its identity uses the original cached Varos mark. Preferences is movable and modeless, with Apply/OK/Cancel; Apply validates atomically, applies only edited draft fields and preserves concurrent live editor changes. Apply retains the window; Cancel discards only edits after the last Apply. Document-unit edits reject switching the active document rather than applying to another document. Preferences fields retain text focus without swallowing canvas shortcuts when unfocused. Native title dragging stays on screen and remembers position.

Full UI run: 2720 passed, 0 failed, 23 ignored. Scoped clippy and release build records are in dist/ui-finalization-2026-11-01. Owner acceptance remains pending.

Other settings dialogs must be audited individually for draft/preview ownership and document binding before modeless conversion; confirmation/file-choice workflows retain their existing behavior. Ordinary panel workspace remains modeless. This update does not claim conversion of every dialog.
