# UI design system

Status: local implementation accepted by the owner in build20261104.6; aggregate Claude review approved. Medium-font fallback follow-up is under verification. External theme
packages/import/export remain deferred by owner decision (2026-11-04).

## Authority

- `crates/ui-egui/src/varos/theme.rs`: Palette, semantic color mapping and interface font stack.
- `crates/ui-egui/src/varos/theme_components.rs`: Foundation typography, surface radii, control
  density roles and shared tab presentation. These are immutable build-time definitions today.
- `crates/ui-egui/src/theme.rs`: upstream-compatible token API. Its small Varos ownership hook
  applies the resolved Varos style once, retaining the upstream initializer as fallback.
- `widgets.rs`: existing behavior-rich controls; small marked Foundation lookups supply their
  presentation. IDs, parsing, scrubbing, commit/cancel and accessibility remain with the controls.
- `panel_system`, `shell` and `panel_stack`: retain layout and input ownership while consuming
  shared tab fonts, measured dimensions, state ink and surface painting.

A style change belongs at its semantic source. Use the shared control helpers first. Custom
painting reads Tokens and Foundation, never a copied color/font/radius. UI fonts are distinct
from document text and font/glyph previews; replacing an interface font must retain Arabic and
system fallback coverage. Regular, medium, semibold and monospace are explicit family roles.

## Variants

Document, panel, old box/card and subsection tabs share typography and active/hover/inactive ink.
Subsection navigation retains its underline; document tabs retain close/dirty indicators. Caller
IDs, hit rectangles, overflow and drag/close actions remain independent of the shared painter.

Standard controls, dialog actions, compact fields and legacy fields retain their intended
heights (24, 28, 20 and 26 logical points). Foundation supplies role values; it does not stretch
controls when a panel grows. Content scrolls/previews own surplus height. Side-docked internally
scrolling editors fill their column; floating panels retain saved sizes.

Control, checkbox, box, content-card, inset, preview and chip radii are separate semantic roles.
Native egui and custom checkbox painters use the checkbox role. Ordinary native filled buttons
use the same inactive/hover/active fill roles as shared action buttons. Selected, frameless,
ComboBox and DragValue variants retain their native treatment; explicit builder fills/frames
remain authoritative. Native text fields keep the input surface and use the common input-border
and focus-accent roles, with read-only and explicit frame overrides retained. Specialized document glyphs,
miniature artwork and insertion/drag markers keep geometry appropriate to their content.

## Verification and extending the system

`varos/tests_style_contract.rs` lexically scans Rust production sources for direct numeric font,
text-size, corner calls, direct constant painter radii and explicit shadow construction. It ignores comments, strings and test
items. `UI_STYLE_EXCEPTIONS.json` records exact file/expression occurrence counts with reasons;
new, duplicate or stale exceptions fail the gate. Review each exception rather than growing a
blanket allowlist. The scanner deliberately cannot prove complete rendering correctness: computed
sizes, numeric aliases and computed painter-radius arguments require source review and frame tests.

Real-frame tests cover native/shared control paint, numeric Enter/Escape, disabled-button input,
shared tab measurement and state ink, legacy tab rows, panel sizing and side-dock filling.
Optional headless captures cover controls in Dark/Light, Home and the real editor tab/panel frame.
Keep shortcut, drag, accessibility, picker and scrub tests when migrating additional components.
Only the owner can accept the final visible/felt result in the native window.

Future runtime font/metric changes must invalidate font and intrinsic panel measurement caches
and preserve saved workspace semantics. Theme import/export will need versioned data, defaults
and validation; it is not part of this change. Native macOS menus, file dialogs, title bars and
cursors follow system constraints rather than arbitrary app theme drawing.
