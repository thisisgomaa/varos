# Agreed Varos interface architecture

## Product direction

Combine Blender's configurable area architecture with the modern, calm charcoal visual language selected by the user. This is a Varos editor with Varos tools. The latest approved HTML is the appearance baseline; the Blender screenshots explain structural behavior. The final decision supersedes earlier literal Blender-look studies.

## Non-negotiable rules

1. Every main region is an independent area/box. The main Viewport is an area, just like Properties, Layers, Assets or another editor. Do not hard-code the entire application around a single immutable central canvas and one sidebar.
2. Properties remains a distinct, important editor. It has category icons, current or pinned selection context, and collapsible/reorderable sections. It is not replaced by a collection of disconnected mini-panels.
3. Any divider between adjacent areas may host panel icons, vertically or horizontally, at any depth of a split layout. A single privileged global icon rail does not satisfy the design.
4. An editor instance may be docked in an area, iconified on a divider, or floating within an owning area. A click on an icon opens a temporary flyout. These are presentations of the same editor identity, not separate copies of document data.
5. The editor-type selector has a fixed top-left location. Its icon reflects the editor type. Clicking changes that editor only. Panel-specific controls follow it: searches, display modes, view options or other appropriate tools.
6. There is no extra permanent generic panel drag/titlebar. Retain useful contextual editor controls. The drag affordance is a small top-edge bar, hidden at rest and revealed on approach/hover/focus. The selector is not the drag handle.
7. A panel can be moved or duplicated. A duplicate has independent view state while observing shared document content. Properties can pin its target separately. Duplication must not silently duplicate artwork.
8. Small viewport tool and navigation palettes can be repositioned inside their owning viewport. Keep them reachable when it shrinks; their placement does not alter the artwork.
9. Property sections have collapse/expand and local reordering. Reordering a section is distinct from moving the whole Properties editor. An extracted property/tool panel may remain a separate editor without destroying the parent Properties view.
10. Workspace tabs remain one row. Support middle-button hold-and-drag horizontal panning and usable trackpad/scroll or overflow access. Workspace changes affect layout; file-level Pages change content. Never conflate these two concepts.
11. Every drag has a visible target preview, a clear result and cancellation. A committed editor instance has exactly one persistent owner. Hover previews must not steal ownership or leave duplicates.
12. Preserve the approved neutral surfaces, subtle borders, coherent SVG icons, functional accent color and English UI. No new Arabic product explanations/tooltips are requested.

## Vocabulary and ownership

| Term | Responsibility |
| --- | --- |
| Document / Page | Artwork data, object identities, edits, selection context. |
| Workspace | User's area arrangement, editor instances, per-editor view state and divider rails. |
| Area | Rectangular host for an editor; can participate in nested horizontal or vertical splits. |
| Editor instance | Type, view state, optional pinned object, collapsed sections, local settings. |
| Divider / rail | Resize boundary and ordered icon locations; not tied to a fixed screen side. |
| Floating panel | Persistent editor presentation bounded to an owning area; resizable and dockable. |
| Flyout | Temporary view of a rail-owned editor. Closing keeps the icon; dragging/resizing can promote it. |
| Viewport widget | Small tool/navigation palette scoped to its viewport. |
| Property section | Reorderable subsection inside Properties; does not become a top-level layout node simply because it folds. |
| Popover / menu | Transient action UI, dismissed without becoming a persistent editor. |

The prototype expresses the layout as a recursive split tree (`axis`, `ratio`, `rail`, `a`, `b`) whose leaves hold a panel ID. A separate registry holds editor view state; floating placements have an owner and a return rail. This is an explanatory reference model, not a requirement to copy JavaScript data structures into Rust verbatim.

## Interaction contract

| Gesture | Result |
| --- | --- |
| Click top-left editor icon | Open editor chooser; replace only that area's editor type. |
| Approach top edge | Reveal the small grip without a permanent extra bar. |
| Drag grip | Move the editor instance. |
| Drag divider icon | Move the same instance to another presentation. |
| Drop on a divider | Insert an icon at the target position, in either orientation. |
| Drop near an area edge | Dock via an independent split. |
| Drop inside an area | Float within that area's bounds. |
| Click divider icon | Toggle temporary flyout; preserve rail ownership. |
| Alt-drag or Duplicate panel | New editor instance, separate view state, shared artwork. |
| Resize empty divider space | Resize adjacent areas; do not trigger the icon's click action. |
| Drag Properties section grip | Reorder only that panel's sections with an insertion cue. |
| Escape during move/resize | Cancel without losing ownership or leaving a stuck gesture. |
| Maximize / restore | Temporarily focus one area and return to the previous layout. |
| Switch workspace | Restore that workspace's layout while keeping shared content. |

## Main editor families

| Family | Character and purpose |
| --- | --- |
| Viewport | Design canvas, rulers, artwork, selection, bounded tool/navigation palettes and contextual actions. |
| Properties | Persistent inspector, internal category icons, pinned/following selection, sections and numeric controls. |
| Layers | Hierarchy, search, selection, visibility and locks. |
| Color / gradient | Color field, palette, fill context and gradient controls with a clear gradient icon. |
| Typography | Character/paragraph controls, text direction and real Kashida integration when connected to Varos. |
| Transform / alignment | Focused geometric controls usable as a float, rail icon or independent area. |
| Assets / libraries | Reusable document content, search and placement. |
| History | Actual action history and undo/redo. |
| Node board | Optional distinct editor: dotted canvas and nodes. It is not the main design Viewport. |

The sample contains ten editor choices, not the full native Varos panel inventory. `NATIVE_PANEL_INVENTORY.md` records 42 registered panels plus the conceptual main viewport from the inspected local source.

## Implementation status — keep these distinctions

| Capability | Approved HTML status / remaining work |
| --- | --- |
| Three main default areas; arbitrary divider rails; split/float/iconify/duplicate | Implemented in sample; modeled DOM/state checks passed. |
| Hidden grip, editor chooser, Properties sections and pinning | Implemented; four discovered regressions were corrected and rechecked. |
| Solid fills, basic geometry/text, layer visibility/lock, assets, undo/redo, SVG export | Implemented for the sample document. Not a full vector engine. |
| Workspace presets and local persistence; page switching/add/duplicate | Implemented locally. Browser-local state is not bundled by copying the HTML. |
| Gradient | Color-ramp preview only; does not apply gradient fills to artwork. |
| Kashida | Control-layout preview only; no Arabic shaping engine in the HTML. Preserve the real Varos implementation when integrating. |
| Node links | Visual connection preview. Not an executable graph or configurable link editor. |
| Native file open/save/export, rendering, pen/path editing, full panel catalogue | Not implemented or migrated by this UI prototype. |
| Join/swap areas, native separate windows, full file-tab overflow, searchable/rename/delete Pages | Discussed or visible in reference studies; not demonstrated by the final HTML. Do not claim they are shipped. |
| Arbitrary property-section tear-out and insertion of any tool into any panel body | Broader extensibility direction; the sample demonstrates whole-editor moving and local section reordering, not a complete generalized composition system. |

## Acceptance checks for native integration

- Match the approved appearance at actual macOS sizes before treating native code as equivalent.
- Move the same editor area → vertical rail → horizontal rail → floating → new split; verify exactly one persistent owner after every step.
- Duplicate an inspector, pin one instance, change selection and verify both targets, edits and Duplicate actions stay correct.
- Test cancellation, pointer release outside the app, focus loss, narrow areas, resize, zoom and reachable floating controls.
- Preserve layouts across save/reopen; migrate unknown/removed editor IDs safely. Reset layout must preserve artwork.
- Validate keyboard focus, label legibility, overflow, menu placement, performance and multi-display scaling in the actual native application.
- Keep existing Varos data formats, rendering, text shaping and recovery behavior intact while integrating the shell.

These acceptance items describe what to verify next. They are not claims of completed native tests or additional implementation performed by this handoff.
