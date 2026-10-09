> Status: implemented with provisional UI; owner design review and native acceptance pending.
# Phase 8 — Lane E

The owner authorized an extension of the existing box shell and kit without a design round. Outline, Pixel Preview, Trim, Presentation, per-row outline and Navigator camera requests are transient editor state; `.vrs` gains no keys or format version. Existing snap flags and transparency-grid document setting are reused. App settings gain additive `canvas_color: [u8; 3]`, defaulting from shell tokens; the existing settings writer preserves sibling lane keys.

`EditCommand::View(ViewAction::Depth(…))` is the common entry point. API 1.2 uses `edit` → `view` with `ids: []` and `action: {"depth": ACTION}`. `list_verbs` lists `view`; `schema {api:"1.2", tool:"edit", verb:"view"}` discloses all actions. Legacy API schemas and fixtures remain unchanged. Native and burger View menus mirror commands. The Layers eye accepts primary-modifier click for outline without hiding the row.

Actions: `outline`, `pixel_preview`, `snap_pixel`, `move_whole_pixel`, `trim`, `presentation`, `exit_presentation`, `transparency_grid`; objects `{"outline_node":{"id":N}}`, `{"navigator_pan":{"center":[x,y]}}`, `{"navigator_zoom":{"percent":600}}`, `{"canvas_color":{"rgb":[20,19,19]}}`. IDs are core node IDs, not path IDs. Navigator coordinates are document points.

Pixel spacing is `72 / ppi` points, independent of camera zoom. Force alignment quantizes final positions; whole-pixel motion quantizes deltas. Pixel Preview quantizes GPU canvas sampling, with a pixel grid at zoom ≥ 6 when the pixel lattice is at least two physical screen pixels apart. Normal sampling uses the original texture-sampling path. Editor overlays share the canvas preview sampling; native UI is drawn afterward.

Outline emits transformed path/hole centerlines at one physical pixel, in MUTED token colour, including mask-source paths in global Outline. Unstyled export scenes retain normal paints. The base model has no Image variant: image-box outline integration must follow the image lane.

Navigator is a real dockable PanelId, available in Window and the box panel chooser. Its whole-board 224×126 proxy uses the thumbnail cache and is rebuilt on session/revision changes. Rasterization is synchronous and bounded in output size; large-document responsiveness needs the owner's native test. Viewport movement and zoom only change overlay/commands, not cached pixels. No periodic repaint request is added; pacing tests exercise idle Wait.

Trim/Presentation clip to the union of visible artboards with disjoint strips, avoiding duplicate opacity where boards overlap. Presentation hides editor chrome without mutating saved shell layout; Shift+F enters and Escape exits. Canvas drawing presses are suppressed in Presentation. Smart Guide HUD gives angle/distance for shape drawing and the next Pen segment.

Headless inspection: `varos-cli view-depth FILE.vrs --batch ACTIONS.json`, where ACTIONS is an array of the DepthAction payloads above. It reports state, pixel spacing, Navigator bounds and requested camera/preferences without rewriting the input. For actual desktop view changes use attached `bridge edit` with the API 1.2 view verb.

Owner checks after moderator merge/install: Cmd+Y, Option+Cmd+Y at 600%; 144 ppi snapping; primary-click Layers eye; Window → Navigator, drag and zoom; Trim with overlapping boards; Shift+F then Escape; canvas presets persist after relaunch; transparency grid; Pen/shape readouts; leave Navigator open and verify idle pacing/heat. No GUI or install was run in this lane.
