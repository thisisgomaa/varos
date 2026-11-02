# F1 — Tools width review

Status: 2026-11-01, F1 implemented and scoped automated checks passed; owner acceptance pending.

Scope: only F1 from [PANEL_FINALIZATION_REVIEW.md](PANEL_FINALIZATION_REVIEW.md).
Tools remains visible as tools, with a 44–80 logical-point width range. Its title arrow
switches between the narrow one-column and wide two-column widths. Edge dragging can
choose widths within those limits. Ordinary panel icon modes are unchanged.

The Tools menu does not offer Collapse to icons. Existing toolbar-column and Tools
collapse/minimize commands use the width policy, while low-level attempts to give a
Tools-containing host an icon/name mode are rejected. Width limits apply to layout
actions, grouping and restored arrangements as well as pointer movement. Existing
compact or oversized saved Tools layouts are normalized without resetting other panels.

Tools stays vertical, docked left/right or floating. The existing square Fill/Stroke
controls and compact paint chips remain. X/Window visibility behavior is unchanged;
this task removes icon collapse, not the previously agreed hide/restore workflow.

Mixed independent icon/full states, general panel resizing, Control's inline grip,
and separator styling remain F2–F6 and are not claimed fixed by this candidate.
Until F2, the arrow of a combined host containing Tools changes Tools width; such a
host cannot collapse all its contents into icons.

Single working app: update the existing `Varos Panels Preview 2` in place, retaining
bundle `com.varos.panels-preview2` and config `~/Library/Application Support/Varos Panels Preview 2`.
Internal product version becomes `0.20.1-tools-width.1`. No additional preview app is
published. The owner requested one working copy until final delivery; other release
variants and platform packaging are deferred. Preserve unsaved work before restarting.

Build, automated checks, signature and startup evidence are recorded in
`dist/tools-width-2026-11-01/verification.json`. Native owner acceptance remains separate.
No commit or full-workspace CI is implied by a preview build.

Verification: full UI suite 2685 passed, 0 failed, 21 ignored, including 80 panel-system
checks. A subsequent clippy-only preferences-load refactor was verified with all six
persistence tests. UI/app all-target clippy, wasm, formatting, layers and assets passed.
Software capture shows the narrow toolbar with actual tool controls. These are automated
and visual checks, not owner acceptance of native pointer behavior.

Owner review: press the Tools arrow repeatedly, resize both floating edges, dock left
and right, and confirm that tool buttons remain visible and the width stops at its limit.
Review this slice before moving to F2.

Revision history: 2026-11-01 — scoped F1 candidate notes added.
