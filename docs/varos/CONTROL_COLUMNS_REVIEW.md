# Control and independent panel columns — owner review

Status: 2026-11-01, candidate built; installation pending. Native acceptance is pending the owner.
This follows the owner's explicit request to fix Control and then immediately handle
separators, column collapse, and icons beside expanded panels. It updates only the
existing `dist/panel-workspace-2026-11-01/Varos Panels Preview 2.app`.

## Intended behavior

- Control has no generic title row, Control tab, close button, collapse arrow, or
  reserved bottom height. A small inline left grip moves the horizontal bar.
  Both floating and top/bottom-docked forms fit their actual content rows.
- Adjacent columns have separate arrows and presentation states. Collapsing Blend
  beside Artboards/Layers keeps the latter column expanded and preserves its width.
  Expanding restores Blend's retained full width.
- Vertically stacked groups share their physical column's width and presentation;
  their horizontal separator independently controls their heights.
- Compact columns change between icons and icon+name by dragging an edge. Their
  separator changes compact width, not the hidden full-width split ratio.
- An icon opens a full flyout toward the workspace on a right dock. Its arrow
  dismisses the flyout back to the same icon without relocating or hiding its owner.
  Dragging a full flyout keeps the full content visible during the move.
- Full/full separators redistribute their two neighboring regions while honoring
  minimum sizes. Visible horizontal and vertical lines match the drag regions.
- Tools remain visible, bounded to the one/two-column width policy. Ordinary-column
  resizing does not grow Tools, and a Tools edge cannot grow an unrelated sibling.
- Floating X still hides the whole block; Window reopens it. Docked blocks have no X.

## Implementation boundary

The existing layout tree, IDs, panel bodies, original icons, and color tokens are
retained. Optional presentation metadata is stored on groups in each derived column
(a maximal vertical-only subtree). Old layouts omit the metadata and retain their
host presentation. Expanded dimensions remain separate from projected compact widths.
Control uses dedicated frame geometry rather than a special content-size workaround.
Title grip hit regions exclude column controls. The actual front host alone is raised
in egui, preserving pointer ownership when a floating Control overlaps other panels.

## Verification and acceptance

Evidence is recorded under `dist/panel-finalization-2026-11-01/verification.json`.
Focused pointer tests exercise left/right docking, repeated independent collapse,
flyout dismissal, compact edge resizing, separator resizing, saved-layout round trips,
and single ownership. Control tests cover floating/top/bottom geometry, inline dragging,
fields, and fitted height. Software-rendered frame captures are supplementary evidence;
they are not native owner acceptance.

Owner review order:
1. Check Control's height while docked and floating; drag it using the small left grip.
2. In the existing Blend / Artboards+Layers layout, collapse only Blend, then expand it.
3. Open Blend from its icon, dismiss it with its arrow, and reopen it in the same place.
4. Drag the compact edge to reveal names, then narrow it again. Resize full/full and
   stacked separators and check that the neighboring layout remains predictable.

No upstream merge, other app bundle update, new feature set, or final acceptance is
claimed in this slice. No Adobe application was opened and no external assets copied.

## Revision history

- 2026-11-01: scoped Control and independent-column candidate; owner review pending.
