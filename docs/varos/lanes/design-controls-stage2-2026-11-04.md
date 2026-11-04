# Shared control foundation — stage 2, 2026-11-04

Status: provisional, awaits owner. Scope explicitly authorized after acceptance of side dock fill.
Previous uncommitted changes retained; snapshot under `plan/design-controls-stage2-2026-11-04/`.

Foundation now defines button/control radius, standard/dialog/compact role heights, compact and
legacy control label sizes, global button font, code-field font, button padding and hex field
margin. The theme applies egui defaults from this definition, retaining TextFields' zero minimum
height compatibility adapter. Shared widgets consume scalar hooks: icon/flat/split/primary/secondary
buttons; recessed text/numeric fields, combo, search, hex and optional fields; spinner/chevron
outer corners retain square joined edges. Picker and Middle Eastern controls consume the same
metrics; Pages uses shared button/row heights. Legacy 26pt fields and specialized swatch/wheel
geometry remain intentional roles. Color authority remains Palette/Tokens.

Two broad post-paint radius-rewriting implementations were removed: actual widget paint now uses
shared radius directly. Their closure adapters retain focus/event/return ownership. No event IDs,
command bindings, commit/cancel/undo flow or document geometry was rewritten. The new upstream
change is a marked Foundation import and scalar widget lookups, keeping engine/UI behavior local.

This is the shared-control migration, not a claim that every bespoke widget in the editor has
been migrated. Checkbox geometry, content thumbnails, artwork swatches and special preview
shapes remain separate; runtime themes/import/density and native macOS controls remain later scope.

Two implementation agents owned disjoint consumers (widgets vs picker/Arabic controls); a third
agent added real-widget regressions and reviewed production. A fresh independent final reviewer
found no blocker and verified prior sizing/tab/side-dock sources and tests against the snapshot.
Their four remaining control-radius findings were fixed before final gates.

Claude Fable was invoked but could not review because the account reached its usage quota
(reset announced as 21:00 Africa/Cairo). No Fable approval is claimed. The owner explicitly chose
independent review and installation instead of waiting; the fresh reviewer completed that review.
The quota response is retained in `plan/design-controls-stage2-2026-11-04/fable.json`.

Final local checks passed: `cargo test --locked -j 4 -p vectorcraft-ui-egui -- --test-threads=4`
(2,795 unit + 2 integration, zero failures, 28 ignored); real-control capture; workspace Clippy
with all targets and `-D warnings`; `cargo xtask wasm`; fmt and diff checks. A test-only
vec-initialization lint was corrected before this final pass. Dark and Light software galleries
were generated and visually inspected after adding proper surface backgrounds/image loaders.
Three new normal tests cover actual native/shared painted corners, numeric Enter/Escape editing,
and disabled-button interaction. Existing scrub/picker/accessibility/shortcut suites also pass.
Arabic strings were checked visually; comprehensive RTL editing is not claimed.
Release build passed in 5m 08s. Signed checkpoint **20261104.5** staged and verified at
`dist/design-controls-stage2-2026-11-04/`; it was held after the owner expanded scope to all
remaining UI stages. The installed baseline remains 20261104.4 until the combined build is ready.
Continuation: [complete pass](ui-unification-full-2026-11-04.md).
