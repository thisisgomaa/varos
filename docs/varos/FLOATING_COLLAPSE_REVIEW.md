# Floating panel arrow collapse — owner review

2026-11-01. Scoped follow-up to Control/columns; native acceptance is pending.
Same working app and config, product 0.20.2-panels.1, bundle build 20261101.5.

## Agreed behavior

- The floating panel's collapse arrow produces an icon-only column at its current
  visible position, including when a saved floating origin was constrained by the viewport.
- Re-expansion retains the last full size. Clicking an icon opens its contents;
  dismissing that flyout leaves the original compact owner in place.
- Narrowing an expanded panel by dragging its edge only resizes it within its minimum;
  it never switches that full panel into a compact presentation.
- Existing resizing between icon-only and icon+name views after explicit collapse is
  retained. This is separate from the rejected automatic collapse of a full panel.
- X hides the whole floating block; Window reopens its existing owner. Tools and Control
  keep their dedicated policies. No new toolbar, picker or other-panel changes.

## Implementation

Floating arrows request Icons rather than restoring a previously wide Names mode.
Explicit SetColumnMode(Icons) uses the shared compact width. Before the arrow collapses
an undocked block, its actual displayed origin is committed so a stale off-screen saved
position cannot be exposed when the block becomes smaller. Expanded sizes are retained.

## Evidence

`dist/floating-collapse-2026-11-01/verification.json` records tests, package identity and
native startup evidence. This document records candidate scope, not user acceptance.
The app is updated in place; no other app bundle is changed. The approved prototypes
and original artwork are preserved.
