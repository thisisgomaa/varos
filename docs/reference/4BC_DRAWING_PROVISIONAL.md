# Lane D — 4B/4C drawing tools (2026-10-09)
Owner exception: implemented quickly using the existing kit; owner design review pending.
- Shape and line rail groups open on right-click or 450 ms hold. A group remembers its last tool per document. Rows show registry glyphs and only Illustrator keys.
- Click the canvas without dragging to open a numeric kit sheet. Create is one undo; Cancel changes nothing. Shapes are baked ordinary paths: no format keys or version bump.
- Drag: Shift constrains dimensions/line angle; Option draws from centre. Polygon/Star/Spiral start at the centre. Up/Down edits rounded radius or polygon/star count during drag. Option+Star changes its inner radius with the outer radius held.
- Pencil N: fit samples with core fitter; fidelity/smoothness/continue distance live in the Freehand options popover. Continue a visible unlocked open endpoint, including rotated paths. Each stroke is one undo.
- Smooth and Path Eraser affect selected paths; the options home holds brush radius. Eraser subdivides actual cubics at the brush boundary (0.001 canvas unit/depth 16 resolution) and leaves open stroke remnants, including separate compound rings.
- Join affects selected open paths touched by the gesture, using the core endpoint joiner. Curvature Shift+~ retains clicked through-points with interpolating handles; Enter/tool switch finishes one edit, clicking the first point closes, Escape cancels.
- Bridge 1.2: shape_tool, pencil, smooth_path, path_erase, join_tool, curvature, drawing_options. All register in list_verbs/schema; 1.0/1.1 tools/list fixtures stay frozen.
- Headless CLI: `varos-cli apply INPUT --batch COMMANDS.json --out OUTPUT`; batch API 1.2, commands use `{"Drawing":{"kind":"shape","spec":{...}}}` etc. No new CLI transport or dependencies.
- New glyphs are Varos originals authored from elementary geometry, embedded provenance in each SVG; existing Lucide glyphs keep their ISC registry provenance. No new external source lifts.
- Resume hardening: Shift retains radial drag radius; stationary freehand clicks settle without an edit; continuation respects isolation and aligns the endpoint handle; protected destinations/ID exhaustion/creation limits and mask-source replacement refuse before history; rejected numeric sheets retain values.
- Blob Brush remains deferred by owner scope. Native visual/interaction review and independent integration review remain pending; no GUI/install in this lane.
