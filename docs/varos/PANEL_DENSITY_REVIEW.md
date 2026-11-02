# Panel density review

Status: shared chrome adjustment implemented; parent build and regression checks in progress; owner visual acceptance pending. Reviewed 2026-11-01.

## Scope and evidence

This review covers source dimensions and shared rendering paths for the 45 built-in panel identities in `crates/ui-egui/src/varos/panel_system/registry.rs`: 43 standard panels, Tools and Control. It does not claim that all 45 bodies were visually tested. No Adobe application was opened or inspected.

A read-only `ui.inspect` request to the existing **Varos Panels Preview 2**, port **17984**, reported `ui.engine_prefs.uiScaling = 1.0`, a logical content window of `1710 × 1021`, and Layers `rowSize = 26`. Its persisted configuration also reports `engine_prefs.uiScaling = 1.0` in `~/Library/Application Support/Varos Panels Preview 2/ui.json`. `prefs_dialog.rs` applies this preference as egui zoom. The disproportion between compact rail icons and Tools therefore comes from their logical dimensions, not an intentionally enlarged UI scale.

## Shared chrome adjustment

The shared dimensions in `panel_system/metrics.rs` now align compact panel controls with hosted Tools. Existing application zoom continues to scale them together.

| Element | Previous | Current logical points |
| --- | ---: | ---: |
| Compact panel identity glyph | 24 | 16, shared with Tools glyph |
| Compact row | 36 | 28, shared with Tools button |
| Group grip band | 16 | 8 |
| Group padding, top plus bottom | 8 | 4 |
| Tab band | 28 | 24 |
| Header control glyph | 18 | 14 |
| Header hit band | 20 | 20, retained |
| Compact rail width | 44 | 44, retained |

A one-panel group previously consumed `16 + 8 + 36 = 60` points vertically. It now consumes `8 + 4 + 28 = 40`. This repeated group overhead was the largest density mismatch in the rail. Keeping the 44-point width avoids changing saved widths, docking placement and compact-mode thresholds. Tools remains 44–80 points wide.

All standard panel identities use the shared chrome renderer (`panel_system/paint.rs` and `geometry.rs`). Full panel dimensions and saved workspace arrangements are retained. Special Control sizing continues to fit its actual content; it does not gain a header band.

## Full body controls retained

- The shared shell body adapter (`varos/shell/boxes.rs`, `panel_contents`) uses 12-point body text, 11-point button text and 14-point headings. These are already compact; no global font reduction is proposed.
- Standard icon buttons (`widgets.rs`, `paint_icon_button`) use 20% rounded padding. A 24-point button paints a 14-point glyph; most common body controls fall around 14–18 points. The hosted Tools glyph is 16 points inside a 28-point button. The Color picker tab glyph is also 16 points (`varos/picker/mod.rs`).
- Standard frameless body margins are 8 points (`dock.rs`). Layers defaults to 26-point rows, and medium Swatches tiles use a 15.5-point tile on a 17-point pitch. Retain these existing sizes and user-selected view modes.
- There are intentional body-specific sizes: Align uses some 32-point buttons; Pathfinder uses 34-point buttons. Preview content such as Symbols thumbnails, Transparency mask previews and gradient swatches is not a chrome icon. These should receive individual review rather than being uniformly scaled down.
- Registry minimum widths, Studio's 320-point full column, the 44-point compact rail and remembered floating sizes remain unchanged. Changing those now would mix density work with layout and clipping behavior.

## Remaining review queue

1. **Properties padding and section density:** source inspection confirms its 16-point body margin is twice the standard frameless margin. Its section header is 32 points, followed by 8 points between sections and 6 points before an expanded body (`panels/properties.rs`, `section`). Review the actual current Properties panel with the owner before changing it. A modest candidate is 12-point outer padding and a 28-point section header, preserving the 11-point labels and fixed context-based order. This is a proposal, not an implemented or visually accepted change.
2. **Color picker clipping:** review the complete Wheel, Sliders, Harmony and Gradient bodies in their real hosts, including the bottom fields and drawer. The picker has dedicated content dimensions and must remain usable while the canvas is interactive. Do not address clipping by globally shrinking all controls or by reducing the saved panel size.

Continue in the same Preview 2 application. The owner reviews one visible slice at a time before the next adjustment.

## Revision history

- 2026-11-01: recorded live scale, shared density mismatch, implemented chrome dimensions and the scoped follow-up queue.

## Follow-up requested during this slice

The owner confirmed Tools dimensions are the reference and remain unchanged. Only its two selection glyphs change: Selection uses an original mouse-pointer outline; Direct Selection uses the same original silhouette with a white fill and neutral contrasting border. The existing 16-point glyph box and 28-point button remain unchanged. Asset provenance is updated in ASSETS.md and NOTICE.
