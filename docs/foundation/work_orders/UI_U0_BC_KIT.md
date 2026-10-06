> **Status:** reference — minimum U0-B/C implementation, merged to `main` in `2cb072a` (2026-10-04, after the batch review, fixes `e772f16`). Its Start consumers were since replaced by Start v2 (`6405dc9`); owner acceptance of these controls on their own is not recorded.
# U0-B/C — minimum Start controls

2026-09-27, baseline `04dd7f2`. Implements only the controls required before E2/F2 under [UI System K1/K5](../../specs/UI_SYSTEM.md). No host wiring, document tab migration, command registry, persistence, generic inputs, or editor access.

## Implemented contract

- `shell::kit` is a library module. `Control` takes a stable caller-owned egui ID, accessible label, optional vector icon, resolved help, selected/focus state and `Availability::{Enabled, Disabled(reason), Busy(reason)}`. No application command types or I/O cross this boundary.
- `action` covers neutral action buttons and icon-only Home/removal; `list_row` covers a title and caller-formatted path/date/missing detail. `section_heading` and `notice` supply the minimum text roles. All product copy comes from the caller; no new localization/label registry is invented.
- The returned `ControlResponse` exposes the egui response/rectangle and one `activated` boolean. Consume that boolean once, not both it and `response.clicked()`. Pointer release, a nonrepeating unmodified Enter/Space press, or egui accessibility activation can trigger it. Disabled/busy/disabled-parent controls never activate.
- E2 owns the existing Start model's keyboard routing. Set `pointer_only = true` when the caller dispatches Enter/Space itself, and use `focused` for that model's keyboard focus. Standalone controls use egui Tab navigation. Caller IDs remain stable when rows reorder; they must be unique within the active surface.
- Disabled takes precedence over selected and hover; selected Home is neutral SURFACE. Azure is only the keyboard-focus outline, visible above selected fill. A pointer press switches back to pointer modality; pointer movement does not erase keyboard focus. Busy is static and prevents repeat actions; no cancellation is implied.
- Controls are at least 24 logical points; normal buttons 32, rows 56. Long row text ellipsizes within the clip; callers provide the full path as help. New spacing/size/stroke tokens share `shell/tokens.rs`. Vector icons avoid font-dependent symbols. Notice text is intended for PANEL/SURFACE.
- Normal text uses TEXT. Secondary MUTED is restricted to PANEL/SURFACE; hovered rows use TEXT because MUTED fails 4.5:1 on HOVER. Disabled text stays readable. Focus meets 3:1 across the three backgrounds.

## Evidence and limits

Four CPU-only scripted tests cover 1×/2× pointer release, first key press/repeat suppression, host-owned keyboard mode, no paint activation, disabled/busy/disabled-parent input, pointer-vs-keyboard focus, selected focus outline, Tab skipping disabled rows, stable IDs after reorder, long-path ellipsis/bounds, minimum hit size and text/focus contrast. Tests construct neither a Renderer nor an EventLoop.

`cargo run -p varos-app --example kit_gallery` is a separate manual Winit/Egui/WGPU window. It previews Home, New/Open, recent/long/missing rows, busy/removal and empty/error notices; accepted actions only increment an in-memory counter. It is not the Start page and cannot open/change documents. Native Mac Retina inspection on 2026-09-27 showed legible controls and neutral Home; Tab then Enter drew the focus outline and changed the counter from 0 to 1. A temporary `/tmp/Varos-U0BC.app` ran the gallery; `/Applications/Varos.app` was not replaced.

The native accessibility tree still exposes only the window, not egui controls. Widget metadata is supplied, but screen-reader/AccessKit integration is **not accepted**. Arabic shaping/caret limitations from [U0-A](UI_U0_A_FONTS.md) remain open. Windows validation is compile-only. No owner acceptance, end-to-end Start test, or independent review is implied by this gallery.

Fresh gates and their exact results are recorded in [GATE_LOG](../GATE_LOG.md). Next: E2 draws the Start model with these controls, routes each action once through host lifecycle, and proves Home/recents/shortcut isolation. Broader U0 controls remain outside this slice.
