---
name: Varos Workspace Study
description: Neutral charcoal desktop editor with independently configurable areas.
colors:
  base: "#131314"
  panel: "#222223"
  raised: "#2b2b2d"
  field: "#303032"
  line: "#363638"
  text: "#e6e6e7"
  muted: "#a4a4aa"
  quiet: "#7c7c83"
  accent: "#8fb9f7"
  active: "#29394e"
typography:
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
    lineHeight: 1.45
  title:
    fontSize: "14px"
    fontWeight: 700
    letterSpacing: "-0.3px"
  label:
    fontSize: "11px"
    fontWeight: 500
rounded:
  field: "5px"
  button: "6px"
  tile: "7px"
  popover: "10px"
  panel: "12px"
spacing:
  compact: "4px"
  field-gap: "6px"
  row-gap: "8px"
  gutter: "12px"
components:
  icon-button:
    rounded: "{rounded.button}"
    width: "28px"
    height: "28px"
    padding: "5px"
  icon-button-active:
    backgroundColor: "{colors.active}"
    textColor: "{colors.accent}"
  field:
    backgroundColor: "{colors.field}"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "6px 8px"
  panel:
    backgroundColor: "{colors.panel}"
    rounded: "{rounded.panel}"
---

# Design System: Varos Workspace Study

## Overview

The confirmed direction is a modern, neutral charcoal desktop editor with macOS-like restraint, dense controls, and independent configurable areas. English is the interface language. The source is the standalone [HTML prototype](../01-approved-html/index.html); [minimal-v1.html](../04-earlier-studies/minimal-v1.html) preserves the earlier minimal study.

This record describes implemented HTML/CSS/JavaScript, not the Varos engine. The evidence file [`original-check-results.json`](../05-verification/original-check-results.json) records 27 passing LinkeDOM tests using modeled geometry and select/event polyfills. Those checks support DOM/state behavior only. The user subsequently approved this HTML direction. No automated browser screenshots, native pointer acceptance, or installed Varos integration were verified; user design approval and automated validation are separate evidence. A prior file-navigation security denial prevented browser validation; no workaround or render was attempted for this documentation.

## Colors

The shell uses layered charcoal neutrals. Pale blue marks active icons, focus, selection, and docking feedback. The `base` token separates areas; `panel` holds editors; `field` distinguishes inputs; `line` outlines containers. Use `text`, `muted`, and `quiet` for the existing hierarchy without treating muted text as a disabled state. Warm artwork colors belong to the sample document, not the application palette.

## Typography

System sans-serif is the interface family. Body text is compact; editor labels use the label role. Most controls and secondary text use observed sizes of 10–11px, with 9px metadata. Numeric inputs and measurements use tabular figures. The brand uses the title role. No separate display-face system is established.

## Layout

The desktop shell has a 42px titlebar, 40px workspace bar, and 25px statusbar. The workspace starts 82px below the viewport top with 12px side gutters and 26px bottom clearance. Nested horizontal and vertical splits define independent areas. Properties is an ordinary editor area, independent of Layers.

The default Design workspace contains a main viewport and a right column split into Layers and Properties. Divider rails carry other editors as icons. Populated rails are 34px thick; empty rails are 10px. Both orientations accept icons. Panel bodies scroll independently.

Design, Typography, Board, and Focus retain separate panel layouts, including panel state and floating areas, in local browser storage. Artwork/pages are shared across workspace switches. Resetting the current layout preserves artwork. This is local persistence, with JSON snapshot export available; no cloud sync is implemented.

At a panel width of 310px, optional header controls and contextual actions hide and spacing tightens. Viewport breakpoints at 1000px and 700px reduce shell chrome; the narrower rule also hides the Properties category rail. Workspace tabs scroll and support middle-button drag. These are source-defined adaptations, not verified mobile acceptance.

## Elevation & Depth

Docked areas use tonal separation and thin borders. Floating panels, menus, and drag ghosts use shadows to communicate temporary elevation. Docking previews use a pale blue outline and translucent fill; divider targets add a dashed border. The sidecar records exact shadow, motion, and breakpoint values without duplicating the frontmatter tokens.

## Shapes

Rounded panel containers frame compact rectangular controls. Inputs are slightly tighter than buttons; icon tiles and popovers have their own radii. Icons are inline SVG, normally 17px with a 1.65 stroke width. Focus-visible controls receive a 2px accent outline with a 2px offset. This describes source styling, not a completed accessibility audit.

## Components

- **Editor selector:** Every panel exposes its editor selector at the top left. Switching the selector changes that panel alone. The panel options menu provides duplicate, float, collapse, and, for docked areas, split and maximize/restore actions.
- **Top-edge grip:** There is no permanent panel drag titlebar. A textless grip sits in an 84×12px hit region at the top center. Its 30×3px bar appears within 15px of the top edge, on hover, or on keyboard focus. Enter/Space opens panel actions. Reduced-motion preferences disable transitions.
- **Docking:** Drag the grip or a divider icon. Drop on any divider to iconify, on an area edge to split, or inside an area to float. Feedback labels identify the destination. Alt held at drag start duplicates panel view state; it does not duplicate the underlying artwork. Escape cancels an active move/resize. Pointer cancellation and window blur also clear active gestures.
- **Divider icons and floats:** Clicking an icon opens a temporary flyout. Moving/resizing can promote it to persistent ownership. Floating panels resize from the lower-right corner and can collapse back to a divider. Dividers resize the adjacent areas.
- **Layers and Properties:** Selection is shared with the SVG artwork. Layer search, hide/lock, geometry, solid fill, text controls, and alignment operate locally. Properties sections fold and reorder; pinned Properties can retain their own object target. The sample supports object duplication, deletion, nudging, assets, history, undo/redo, page switching/duplication, and SVG download.
- **Preview controls:** Gradient editing previews its colors only; it does not apply gradients to artwork. Kashida is a control-layout preview without Arabic shaping. Board nodes move and share fill controls, while their connection is a visual preview. These features must remain explicitly labeled.

## Do's and Don'ts

- **Do** preserve independent areas, selectors at the top left, icons on either divider orientation, and the small approach-revealed grip.
- **Do** keep the shell neutral and compact, using accent color for functional state.
- **Do** preserve English interface text and clear preview labels.
- **Don't** add a permanent panel drag titlebar or couple Properties permanently to Layers.
- **Don't** represent modeled DOM tests as browser/native-pointer or visual acceptance.
- **Don't** imply that this HTML sample implements the Varos rendering or text engine.
