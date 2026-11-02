# Tab reordering and panel drag geometry

2026-11-01. Current owner-requested slice after installed build12.

- Open document tabs drag horizontally using stable document UIDs. Existing documents and view records move together; active UID, document Arc/history/selection/dirty state/tool remain unchanged. Workspace command document.reorder {uid,index} validates arguments before mutation.
- Thin insertion feedback previews until valid release; outside release, focus loss and Escape cancel. Drag owns keyboard so Escape cannot also clear artwork selection or cancel an active tool.
- Overflow uses existing tab-scroll state:18-point inner edge,350ms dwell, one position per interval; leave/release/cancel resets timer. Pointer tests cover18 documents both directions.
- Panel canvas reservation now uses the same preview arrangement as the moving panel layer, avoiding vacated dock strips before release. Emitted-shape tests verify text positions and clipping follow panel chrome during held tab/host dragging. This is evidence for the concrete dock-space mismatch; unobserved detached popovers are not claimed fixed.

Checks:2730 UI tests passed,23 ignored; scoped clippy passed. Release/native verification pending. Cursor Opus5.5 was available by model listing but execution rejected by weekly credits; authorized Grok4.7 read-only review completed, identifying a canceled-release preview-frame mismatch; the fix and its regression now pass. See grok-review-final.json. Logs and receipts in dist/tabs-drag-2026-11-01. No full CI/commit or exhaustive manual acceptance claimed.

## Color drawer evidence

The owner supplied a concrete screenshot where Recent/Document and swatches escaped the Color flyout. picker/drawer.rs replaced the inherited UI clip with a larger drawer rectangle. It now intersects the inherited clip instead. Existing height accounting includes the drawer; the existing outer scroll keeps it accessible. The new regression verifies off-frame painting/clicks are blocked and scrolling reveals the controls. No Color design or artwork behavior changed.

Canceled releases over the appbar/statusbar now immediately restore the committed geometry in the release frame, addressing the independent Grok finding.

## 2026-11-02 owner refinement

Color flyouts now fit current picker mode and expanded drawer content, bounded only by available workspace. Live file-tab dragging paints the actual tab at its grab offset and animates neighbors into insertion slots over90ms; document order remains transactional until valid release. Added emitted-geometry and Color-content visibility regressions.2732 UI tests passed,23 ignored. Build14 packaging/native check in progress.

## Side Color panel, build15

Side Color now independently renders existing RGB/HSB/CMYK/grayscale sliders, hex and spectrum, with a compact recent-color strip and persistent Show Recent Colors menu/command option. Toolbar paint-well wheel popup remains independent. Tests cover side content fitting, popup preference preservation, live popup gestures, and gradient-stop recoloring.2734 UI tests passed,23 ignored.

## Horizontal tab constraint, build16

Dragged document tabs retain their original y position and are clipped to the tab strip. Horizontal live preview and neighbor animation remain. Release outside cancels ordering; independent document windows remain unsupported. Seven focused tab gesture tests pass, including pointer movement with a vertical component.
