# Varos — Snapping & Transform: Verified External Reference

> Compiled from verified research on Adobe Illustrator, Affinity Designer 2, Figma, Sketch, CorelDRAW, and Inkscape.
> Purpose: feed `D:/VAROS/docs/SNAP_TRANSFORM_SPEC.md`.
> **UNCERTAIN** facts are flagged inline — verify them live in-app before hard-coding into the spec. **Never quote a flagged number/shortcut as fact.**

---

## (A) Illustrator — exact Windows shortcuts + transform/snap/nudge behaviors

### A.1 The 11 commands (Windows defaults)

| Command | Windows shortcut | Confidence | Menu path / notes |
|---|---|---|---|
| Smart Guides toggle | `Ctrl+U` | ROCK-SOLID (Adobe + 4 cheat sheets) | View > Smart Guides. Master toggle for alignment guides, anchor/path labels, measurement labels, object highlighting, construction guides. |
| Snap to Grid | `Shift+Ctrl+'` (Shift+Ctrl+quote/apostrophe) | High (Redokun + corroboration) | View > Snap to Grid. Same physical key as Show Grid. |
| Snap to Point | **NO default shortcut** | VERIFIED (menu-only) | View > Snap to Point. Toggle only — do NOT invent a key. Snaps dragged anchor/object to nearby anchors within snapping tolerance. |
| Snap to Pixel | **NO default shortcut** | VERIFIED (no key) | Exposed via Snapping Options / Properties panel; tied to Pixel Preview (`Alt+Ctrl+Y`). "Snap to Pixel Grid" option, not an accelerator. |
| Show/Hide Rulers | `Ctrl+R` | ROCK-SOLID | View > Rulers. (`Ctrl+Alt+R` = Video Rulers — do NOT confuse.) |
| Show/Hide Grid | `Ctrl+'` (Ctrl+quote/apostrophe) | High (Redokun) | View > Show/Hide Grid. Document grid. NOT `Shift+Ctrl+I` (that's unreliable / perspective context). |
| Make Guides | `Ctrl+5` | ROCK-SOLID | Object/View > Guides > Make Guides. Converts selected vector objects into guides (locked by default). |
| Release Guides | `Alt+Ctrl+5` | High (Redokun) | View > Guides > Release Guides. Inverse of Make Guides; works on a selected guide. |
| Lock Guides (toggle) | `Alt+Ctrl+;` (Alt+Ctrl+semicolon) | High (Redokun + Adobe community) | View > Guides > Lock Guides. **NOT** `Alt+Ctrl+5` (that's Release — one source conflated them). |
| Hide/Show Guides | `Ctrl+;` (Ctrl+semicolon) | High (Redokun) | View > Guides > Hide/Show Guides. Clean pairing: `Ctrl+;` = show/hide, `Alt+Ctrl+;` = lock. |
| Transform Again | `Ctrl+D` | ROCK-SOLID | Object > Transform > Transform Again. Repeats last transform (move/rotate/scale/reflect/shear) **including the duplicate** if last action was a transform-copy. |

**Other rock-solid IL keys referenced:** Pixel Preview `Alt+Ctrl+Y`; Show/Hide Bounding Box `Shift+Ctrl+B`.

**No-shortcut toggles (verified — do NOT invent keys):** Snap to Point (View menu), Snap to Pixel (Properties/Snapping options), Show Center (Attributes panel per-object toggle).

> **Key-matching note:** the quote/apostrophe and semicolon are the same physical keys regardless of layout. Per Varos's prior Arabic-layout shortcut lesson, **match by physical key code**, not character.

### A.2 Illustrator transform / snap / nudge behaviors

- **Keyboard Increment (nudge):** Edit > Preferences > General. **Default = 1 pt** (1/72 in = 0.3528 mm). Arrow = 1 increment; **`Shift`+Arrow = 10× the increment** (default 10 pt). The 10× multiplier is fixed behavior, not the increment value. Confidence: high.
- **Constrain Angle:** Edit > Preferences > General. **Default = 0°**, range −360 to 360. Rotates X/Y axes (positive = CCW). **Affects ALL tools/modifiers:** new-object creation, transforms, measurements, Shift-constrain dragging, AND Smart-Guide angles. Confidence: high.
- **Shift-constrain dragging:** Holding `Shift` while moving/drawing/rotating constrains to **45° multiples** (0/45/90/135…), measured relative to the Constrain Angle. Built-in, not a preference.
- **Rotate/Scale around a clicked origin + Alt=copy:** With Rotate (`R`) or Scale (`S`): **click once to set the reference point (origin), then drag**. `Alt+click` sets the origin AND opens the numeric dialog. **Holding `Alt` while dragging a transform leaves the original and transforms a COPY.** Default origin = object center until you click elsewhere. (Adobe doc page timed out on fetch, but behavior corroborated across sources.)
- **Snap to Grid vs Smart Guides:** mutually exclusive — when Snap-to-Grid is on (grid shown), Smart-Guide snapping is suppressed. **Pick one snap authority at a time.**

### A.3 Illustrator — flagged UNCERTAIN (verify live in IL 2024/2025)

- **UNCERTAIN — Smart Guides "Snapping Tolerance" default value.** Edit > Preferences > Smart Guides > Snapping Tolerance (pts), shared by Snap to Point. Commonly cited ~2 pt but **NOT confirmed** from an authoritative Adobe source. Do not treat any number as verified.
- **UNCERTAIN — Construction Guides angle preset list.** Edit > Preferences > Smart Guides > Construction Guides. Presets exist (45/90 mentioned as examples) + up to 6 custom angles, but the **exact menu items are not confirmed verbatim.**

---

## (B) Affinity Designer 2 — the Snapping panel decoded + adopt list

**Where it lives:** a real dockable panel/popover. Master snapping on/off = the **magnet icon on the toolbar**; an adjacent dropdown opens the full **Snapping panel** (also via menu). Top of panel = **Presets** pop-up + a Create-preset button. Master checkbox: "Enable snapping" must be on to change anything else.

> **Top UX idea to steal:** one master magnet toggle on the toolbar + a dedicated Snapping panel for everything else (Illustrator buries this across Preferences panes).

### B.1 Panel options (verbatim-sourced) + Varos verdict

| Option | What it does | Varos verdict |
|---|---|---|
| **Screen tolerance** (slider) | Snap radius — "the distance you have to be to an object before snapping occurs." Measured in **SCREEN px** (zoom-independent feel). | **ADOPT** as a user slider in screen px. **UNCERTAIN — Affinity publishes NO numeric default.** Set & document Varos's own (4–8 screen px is defensible). |
| **Candidates** dropdown | The signature concept — snap only to opted-in "candidate" objects, not all objects. Modes: **Candidate List** (chronological last-N), **Immediate layers** (current layer), **Immediate layers and children**, **All layers**. | **ADOPT (differentiating).** Ship layer-scope modes first (map to Varos's Blender-RNA layer model); chronological Candidate List as phase 2. |
| **Maximum** (candidates) | FIFO ring size for "Candidate List" mode — new candidates replace oldest. Only relevant in Candidate List mode. | ADOPT only with Candidate List mode. Low priority. |
| **Show snapping candidates** | Highlights snappable objects with a **purple outline/halo**. | **ADOPT** — needed to make the candidate model usable. Use Varos azure `#0c8ce9` (or a distinct hue so candidate ≠ selected). |
| **Only snap to visible objects** | Excludes hidden geometry. | ADOPT, **default-ON**. |
| **Snap to grid** | Snaps to grid lines. "Not available when using Force Pixel Alignment." | ADOPT (standard). Replicate mutual exclusion with pixel alignment. |
| **Snap to guides** | Snaps to guides. | ADOPT — table stakes. |
| **Snap to spread** (+ mid points) | Snaps to page/artboard **edges**; sub-option adds page **center** (H/V). | ADOPT, map "spread" → **artboard/page** (rename; "spread" is print jargon). |
| **Snap to margins** (+ mid points) | Snaps to page margins + margin center. | ADOPT **IF** Varos has page margins (relevant to PDF-native plan). Lower priority. |
| **Snap to object bounding boxes** (+ mid points) | Edges + centers of other objects. | **ADOPT — build FIRST.** Single most-used snap. |
| **Snap to gaps and sizes** | Detects **equal spacing** between objects + **equal width/height** to another object; draws measurement arrows (Figma equal-spacing equivalent). Operates between candidates. | **ADOPT — high priority.** Pair with on-canvas spacing labels. |
| **Snap to shape key points** | Shape-semantic anchors (ellipse center/quadrants, rectangle center) beyond bbox. | ADOPT after bbox. Maps to Varos parametric shape primitives. Medium priority. |
| **Snap to object/layer geometry** | Snaps to actual path **vertices/nodes**, not just bbox. | **ADOPT — high priority** for vector/pen work. Makes a vector tool feel precise. |
| **Snap to pixel selection bounds** | Snaps to a raster marquee's marching-ants bounds. | **SKIP/DEFER** — only matters in a mixed raster+vector app. No referent in Varos (vector-only). |

### B.2 Toolbar pixel toggles (NOT panel checkboxes)

- **Force Pixel Alignment** (toolbar): "Snap objects, nodes and handles, and pixel selection areas to full pixels when created, moved or modified." Disables Snap-to-grid. **No documented shortcut (UNCERTAIN — assign Varos's own).** ADOPT as a separate toolbar toggle (keep out of candidate-snapping logic).
- **Move By Whole Pixels** (toolbar, gated behind Force Pixel Alignment): "Constrain the movement of objects, nodes and handles to whole pixels." Difference: **preserves an existing sub-pixel offset** while moving in integer steps (vs Force-Pixel-Align alone which re-snaps to the integer grid). No documented shortcut (UNCERTAIN). ADOPT as a refinement; low priority; gate behind Force Pixel Alignment exactly as Affinity does.

### B.3 Snapping PRESETS (the second idea to steal)

Presets pop-up + "Create preset" (user-saveable). **5 built-ins (verbatim purposes):**
- **Page layouts** — print designs; snap to guides, margins, spreads.
- **Page layouts with objects** — as above + object-to-object alignment.
- **Object creation** — simple object-to-object bbox + midpoint alignment.
- **Curve drawing** — pen/brush (non-geometric) setup.
- **UI design** — pixel accuracy + fixed guides/grid for UI/web.

**ADOPT — strongly.** Ship Varos named presets (e.g. `Pen/Curve`, `Object align`, `UI/Pixel`, `Print/Page`) + user-saveable customs. Fits Varos's modeless/Illustrator philosophy — one click reconfigures the whole snap profile per task.

### B.4 Affinity — flagged

- **UNCERTAIN — numeric Screen Tolerance default** (not published).
- **UNCERTAIN — no shortcuts** for Force Pixel Alignment / Move By Whole Pixels / master snap toggle.
- **NOT PRESENT (do not over-claim):** Designer's Snapping panel has **no "Snap to baseline grid" checkbox.** Only a text-baseline snap ("snap to baseline of other text, first line only"). A true baseline GRID is a Publisher feature. → **DEFER** baseline-grid snapping to Varos's Text/typography system (alongside the Arabic moat), not the core snap MVP.

> **Top 2 to steal:** (1) the **candidate model** (+ candidate highlight); (2) **snapping presets**. Everything else (bbox+midpoints, gaps/sizes, geometry/vertex, grid, guides, spread/margin) is standard — build it, but those two are where Affinity leads Illustrator/Figma.

---

## (C) Figma — numbers & feel

| Aspect | Verified fact | Varos recommendation |
|---|---|---|
| **Object snap threshold** | **No numeric threshold exposed.** Snapping (centers + outermost edges) is automatic and **zoom-relative**: magnet measured in SCREEN px, so canvas tolerance shrinks zoomed-in, grows zoomed-out. **UNCERTAIN — exact internal screen-px constant never published** (~a few screen px). Do NOT quote a number. | Adopt the zoom-relative FEEL (magnet in screen px), but **unlike Figma make it an explicit user-visible setting** (default ~6–8 screen px) + a hard toggle. This is the one place to improve on Figma for an Illustrator-class tool. |
| **Alignment-guide color/style** | Solid **RED** line; distance/spacing labels also red. Appears on move/resize/vector-point edit. **UNCERTAIN — exact hex not published** (~`#F24822` saturated red-orange). Solid 1px. | Use ONE high-saturation accent for both guides + labels. Keep snap/measure **RED distinct from Varos selection azure `#0c8ce9`** so the two systems don't blur. |
| **Distance / spacing labels (while dragging)** | Shows px distance labels when dragged object aligns (H or V) to a near object; **equal-spacing labels** appear when gaps match. Measures between **bounding-box bounds.** | Adopt directly: red px distance labels on alignment + equal-gap labels when two gaps match (the "equal spacing" delight). Measure bbox-to-bbox. |
| **Smart Selection equal-spacing** | Multiple equally-spaced/overlapping objects → **PINK** ring at each center + **PINK** handles in the gaps. Drag a gap handle → adjusts ALL spacing at once (px tooltip). Drag center ring → reorder. Works 1D + 2D grids. | **ADOPT (distinctive).** Pink/magenta for spacing handles (distinct from blue selection + red snap). Implement px tooltip + center-ring drag-to-reorder. Strong differentiator vs Illustrator. |
| **Nudge amounts** | **Small = 1, Big = 10**, resolution-independent **points** (not px). Arrow = small (1); `Shift`+arrow = big (10). Changed in **Preferences > Nudge amount**. | **ADOPT EXACTLY:** arrow = 1, Shift+arrow = 10, both user-configurable. Muscle-memory across Figma/Sketch/XD. |
| **Rotation snap increment** | Holding `Shift` while rotating snaps to **15° increments** (0/15/30/45…). (`Alt/Opt+R` reveals rotation origin — NOT a measure gesture.) | **ADOPT 15° Shift-snap**, make increment configurable for power users. |
| **Alt-measure gesture** | Select object → hold **`Alt` (Win)** → hover a second object → **RED line + H & V px measurements** between the two bbox bounds. Nested layers: **`Ctrl+Alt`**. Hold modifier + arrow to watch distance update live. | **ADOPT verbatim** (select → hold Alt → hover), red line + dual H/V bbox labels, nested modifier `Ctrl+Alt`. Beloved gesture with no clean Illustrator equivalent. |
| **Pixel-grid snapping** | Pixel grid visible only at **≥400% zoom.** "Snap to pixel grid" toggle = **`Ctrl+Shift+'` (Win).** Frames/sections/components **always** snap to pixel grid even when the toggle is off. **UNCERTAIN — default on/off state not documented.** | Adopt: grid visible ≥400%, toggle `Ctrl+Shift+'`. **BUT default Snap-to-pixel OFF** for a vector/illustration tool (needs sub-pixel freedom). Keep "always-snap frames" carve-out only if Varos has artboard-class containers. |

---

## (D) Worth stealing from Sketch / CorelDRAW / Inkscape

### D.1 Power-duplicate & arrays (highest leverage)

- **CorelDRAW — Step and Repeat docker:** Edit > Step and repeat, **`Ctrl+Shift+D`** (confirmed ×2 sources). Set Number of copies + (offset distance OR spacing) independently for H and V → generates a true array. **ADOPT for v1** — the cleanest array model; the feature Varos is most likely to miss vs a real Illustrator-class tool. Bind to `Ctrl+Shift+D` (no IL conflict).
- **CorelDRAW — Duplicate with persistent offset:** `Ctrl+D`. First use prompts a Duplicate-offset dialog (H/V); offset is remembered so repeated `Ctrl+D` steps-and-repeats. **ADOPT** — cheapest "power duplicate." **Shortcut conflict warning** (see D.5).
- **Inkscape — Transform Again:** **`Ctrl+Shift+M`** (confirmed ×2) = duplicate selection AND reapply its last transform (move/scale/rotate/shear). Inkscape's analog of IL Transform Again. **ADOPT — keystone power-duplicate** (radial arrays / clock ticks). Recommend binding to **`Ctrl/Cmd+D`** to match Illustrator (dominant target audience) rather than Inkscape's key.
- **Universal "duplicate then drag, then repeat" idiom (all three):** Sketch = Option-drag to duplicate, then `Cmd+D` repeats the exact offset. Inkscape = `Ctrl+D` / `Ctrl+Shift+M` chain. CorelDRAW = `Ctrl+D` remembered offset, or **drag + tap `Spacebar`** / **right-click while dragging = drop a single copy** / numpad `+` = copy in place. **ADOPT the full idiom** + steal CorelDRAW's "right-click-while-dragging = drop a copy" and "tap modifier mid-drag to leave a copy."
- **CorelDRAW — Transform docker "Copies" field:** type N copies → leaves original, bakes N progressively-transformed duplicates (rotate×N = radial array; scale×N = nested). **ADOPT** into the same array panel. With Transform-Again this fully covers IL's array story.
- **Inkscape — Tiled Clones** (Edit > Clone > Create Tiled Clones): grid/array of **linked** clones, per-step shift/scale/rotate/blur/opacity, randomization, 17 plane symmetries. Clone = `Alt+D`, Paste In Place = `Ctrl+Alt+V`, Duplicate-in-place = `Ctrl+D`. **PARTIAL/LATER** — full symmetry tiling is over-scoped for v1, but the **linked-clone concept** (edit master → all instances update, like Figma components) aligns with Varos's single-schema moat. Revisit once a component/instance model exists.

### D.2 Distribution / equal-spacing (non-negotiable — all three converge)

- **Sketch — Smart Guides + distribution snapping while dragging:** shows measurements to neighbors, snaps to center/edges, and **snaps a third object to extend equal distribution** when 2+ are equally spaced. Hold `Option/Alt` → exact distances to original + artboard. Hold `Cmd` → **temporarily disable** smart guides. **ADOPT as core v1** — the "feels like Figma/Sketch" bar.
- **Sketch — Smart Distribute:** select 3+ equally-spaced layers → **on-canvas gap handles between layers** → drag one changes ALL gaps evenly. **"Tidy"** button force-equalizes uneven layers into columns/grids. Drag-to-swap reflows neighbors (57+). **ADOPT v1 / fast-follow** — best equal-spacing UX of the three. Swap-on-drag = defer.
- **CorelDRAW — Alignment Guides:** toggle **`Shift+Alt+A`** (confirmed). Snap to edges/centers + objects inside a group. **Intelligent Spacing** = distribution snapping (equal/equidistant). **Intelligent Dimensioning** = match a neighbor's size/angle on scale/rotate. Configurable H/V margins (offset / inset / offset+inset, lock-ratio). **ADOPT Intelligent Spacing** (3-tool signal it's mandatory) + the configurable margin offset (snap with a gap, not just flush). Intelligent Dimensioning = defer.

### D.3 Grids (a differentiator opportunity)

- **Inkscape — Axonometric (isometric) grid:** Document Properties (`Shift+Ctrl+D`) > Grids > New > Axonometric. Three line sets: one vertical + two at configurable **Angle X / Angle Z** (default ~30° each → axes 120° apart = standard isometric). Newer versions allow angle-by-ratio (2:1 game-iso = **26.565°**). **ADOPT as a named differentiator** — Sketch and Figma have NO isometric grid. Build rectangular grid first (table stakes), then axonometric with Angle-X/Angle-Z fields + a 2:1 preset. **UNCERTAIN/clarification:** 30° (geometric isometric) and 26.565° (2:1 game-iso) are different variants for different purposes — don't conflate.
- **Inkscape — Rectangular grid:** independent X/Y spacing; snap precedence tunable (Edit > Preferences > Behavior > Snapping) so grids/guides can be the ONLY snap target. **ADOPT** independent X/Y spacing + a **snap-precedence setting** ("grid/guides only" override) to fix "it snapped to the wrong thing."
- **Inkscape — arrow-key nudge:** default **2 px** (SVG user units), `Shift`+Arrow = 10× (20px). Set in Preferences > Behavior > Steps. **ADOPT Shift=10×, but use 1px default** (Sketch/Figma/Illustrator convention) — NOT Inkscape's 2px.
- **GAPS none of the trio ship (Varos-original opportunities):** **Polar/radial** snap grid (concentric circles + spokes), **baseline** grid as a snap target (InDesign has it; these three don't), **modular** grid primitive. If Varos wants a grid edge beyond axonometric, **polar and baseline are genuinely unserved.**

### D.4 Transform origin & pixel fitting

- **Transform origin / pivot:** none of the three exposes a freely-movable pivot as cleanly as Illustrator/After Effects. CorelDRAW = click object twice to enter rotation mode then drag the center crosshair + a **3×3 relative-position anchor grid** in the Transform docker. Inkscape/Sketch rotate about bbox/selection center. **ADOPT a draggable pivot + a 3×3 anchor-point selector** (CorelDRAW model) — an Illustrator-class expectation and a likely Varos miss. **Core transform work, v1.**
- **Sketch — Pixel Fitting:** Arrange > Fit Layers to Pixel Bounds, **`Ctrl+Cmd+X`** → snaps coords to whole px. **ADOPT as optional toggle, OFF by default** (Varos is PDF-native; sub-pixel matters for print/CMYK). Useful for screen/PNG export. Low priority.

### D.5 Inkscape granular snap channels + CorelDRAW dynamic guides

- **Inkscape — Snap Controls bar (5 sections):** Enable Snapping; Snap Bounding Boxes; Snap Nodes/Paths/Handles; Snap Other Points; Snap Page/Grids/Guides. Rule: **nodes snap only to nodes/paths; bbox snaps only to bbox** (channels don't cross). **ADOPT the model but SIMPLIFY** — Inkscape's 30+ toggles are notoriously confusing. Expose ~5 high-level toggles (snap to: edges/centers, nodes, paths, grid, guides), keep bbox-vs-node separation under the hood. Later-stage polish.
- **CorelDRAW — Dynamic Guides:** temporary guides pulled from object snap points (center/node/quadrant/baseline) showing **angle + distance readout** at preset/custom angles, with invisible "ticks" the pointer gravitates to (adjustable spacing, disable-able). **ADOPT the angle+distance readout** (useful for technical/precise work, likely Varos miss); **ticks = defer.** Mid-stage, after basic smart guides + distribution. **UNCERTAIN — no verified default toggle shortcut; leave menu-driven, do NOT invent a key.**

### D.6 Sketch/Corel/Inkscape — flagged UNCERTAIN

- **UNCERTAIN — CorelDRAW Dynamic Guides default toggle shortcut** (feature documented, no verified key).
- **UNCERTAIN — CorelDRAW default alignment-guide margin pixel value** (UI documented, no published number).
- **CLARIFICATION — Inkscape isometric angle:** 30° = geometric isometric; 26.565° = 2:1 game-iso variant. Both real, different uses.
- **Shortcut overload — `Ctrl/Cmd+D`:** Inkscape = Duplicate-in-place; Illustrator/Sketch = Transform Again; CorelDRAW = Duplicate-with-offset. **Pick ONE canonical meaning for Varos and document it loudly** in the spec. Recommendation: `Ctrl+D` = duplicate-with-remembered-offset / Transform-Again (the IL behavior, since IL is the target audience).

---

## (E) Recommended Varos snapping + transform feature set — staged

> Architecture flags to lock from day one (retrofitting is expensive):
> - **One global Constrain Angle** + **one shared Snapping Tolerance**, both feeding Smart Guides, Snap-to-Point, Shift-constrain, and object creation (IL architecture lesson).
> - **Snap tolerance defined in SCREEN px** (zoom-independent feel) — but make it user-visible (improve on Figma's hidden value).
> - **Match shortcuts by physical key code**, not character (Arabic-layout lesson).
> - Store the **last transform delta + "was a copy" flag** to enable Transform-Again from the start.

### Stage 1 — MVP (the "feels like Figma/Illustrator" baseline)

1. **Master magnet toggle on toolbar** + a dedicated **Snapping panel** (Affinity structure).
2. **Snap to object bounding boxes — edges + centers** (build FIRST).
3. **Snap to object geometry / path vertices** (makes the vector/pen work feel precise).
4. **Distribution / equal-spacing snapping while dragging** + red px distance labels + equal-gap labels (Figma/Sketch/Corel all converge — non-negotiable). Measure bbox-to-bbox.
5. **Snap to grid + Snap to guides** (with document grid + guide systems).
6. **Smart guides visual language:** solid 1px **RED** alignment guide + red H/V measurement labels (keep RED distinct from selection azure `#0c8ce9`).
7. **Nudge:** arrow = **1**, `Shift`+arrow = **10**, both configurable in Preferences (Figma numbers).
8. **Rotation `Shift`-snap = 15°** (configurable); **Shift-constrain dragging = 45°** offset by Constrain Angle.
9. **Transform Again** (`Ctrl+D`) — replay last transform incl. duplicate; **Alt-drag = transform a copy.**
10. **Rotate/Scale: click-to-set-origin then drag**; **`Alt`+click = set origin + numeric dialog.**
11. **Draggable transform pivot + 3×3 anchor selector** in the transform panel (CorelDRAW model).
12. **Core IL shortcuts:** Smart Guides `Ctrl+U`, Rulers `Ctrl+R`, Make Guides `Ctrl+5`, Release `Alt+Ctrl+5`, Show/Hide Guides `Ctrl+;`, Lock Guides `Alt+Ctrl+;`, Bounding Box `Shift+Ctrl+B`, Snap to Grid `Shift+Ctrl+'`, Show Grid `Ctrl+'`.
13. **Global Constrain Angle preference** (default 0°) feeding all snap/transform math.

### Stage 2 — Pro depth & differentiators

1. **Snapping presets** (named task profiles: `Pen/Curve`, `Object align`, `UI/Pixel`, `Print/Page`) + user-saveable (Affinity — strong steal).
2. **Snapping candidate model:** layer-scope modes (Immediate layers / +children / All) + **candidate highlight** (azure or distinct hue) (Affinity — strong steal).
3. **Alt-measure gesture:** select → hold `Alt` → hover → red H/V bbox labels; nested = `Ctrl+Alt` (Figma — copy verbatim).
4. **Smart Distribute on-canvas gap handles** (drag = adjust all gaps) + **Tidy/equalize button** (Sketch). Optional **pink** ring/gap handles + px tooltip + center-drag reorder (Figma Smart Selection).
5. **Snap to gaps and sizes** (equal-spacing + equal-size arrows) (Affinity).
6. **Step-and-repeat / array panel:** count + (offset distance | gap) on X/Y + transform-copies (rotate×N radial, scale×N nested) (CorelDRAW). Bind `Ctrl+Shift+D`.
7. **Snap to shape key points** (ellipse center/quadrants, rect center) (Affinity).
8. **Axonometric / isometric grid** (Angle-X/Angle-Z fields + 2:1 preset) — named differentiator (Inkscape).
9. **Force Pixel Alignment** toolbar toggle (snap everything to integer px for UI/web) (Affinity) + **Snap to Pixel** as a Properties toggle (IL). Default **OFF** for vector/illustration. Assign Varos's own shortcut (none published upstream).
10. **Simplified granular snap toggles** (~5: edges/centers, nodes, paths, grid, guides) with bbox-vs-node channel separation under the hood (Inkscape, simplified).
11. **Object center as a snap target by default** (IL — center-to-center snapping); expose center as an Attributes/anchor toggle.
12. **Make Guides from any path** (`Ctrl+5`) + new guides locked by default (IL default).

### Stage 3 — Polish & late-stage power

1. **Move By Whole Pixels** (preserve sub-pixel offset during integer moves) — gate behind Force Pixel Alignment (Affinity).
2. **CorelDRAW Dynamic Guides:** pull angled guides from object snap points with **angle + distance readout** at preset/custom angles. "Ticks" sub-snaps = defer. Menu-driven until a shortcut is verified.
3. **Snapping candidate "Candidate List"** chronological FIFO mode + Maximum count (Affinity).
4. **Snap-precedence setting** ("grid/guides only" overrides object snap) (Inkscape).
5. **Configurable construction-guide angles** (default 0/45/90 + custom) — verify IL's exact preset list first.
6. **Pixel Fitting** ("fit layers to pixel bounds") as optional export-time toggle, OFF by default (Sketch).
7. **Snap to margins** (+ mid points) — once page margins exist in the doc model (Affinity; relevant to PDF-native plan).
8. **Linked-clones / component-instance** array engine (edit master → instances update) — aligns with Varos single-schema moat (Inkscape Tiled Clones concept, scoped down).
9. **Baseline-grid snapping** — deferred to the Text/typography system (alongside Arabic moat), NOT the core snap panel.
10. **Polar/radial snap grid** — Varos-original opportunity (unserved by all six tools).

### Explicitly SKIP / DEFER
- **Snap to pixel selection bounds** (Affinity) — no referent in a vector-only tool.
- **Snap to baseline grid as a core panel checkbox** — it's a typography feature, not core snap.
- **Full 17-symmetry tiled clones** — over-scoped; keep only the linked-clone concept.

---

## Master UNCERTAIN list (verify in-app before hard-coding)

1. **IL Smart Guides Snapping Tolerance** default value (pts) — commonly ~2 pt, NOT confirmed.
2. **IL Construction Guides** exact angle preset menu items.
3. **Affinity Screen Tolerance** numeric default — not published.
4. **Affinity** shortcuts for Force Pixel Alignment / Move By Whole Pixels / master snap toggle — none documented.
5. **Affinity** Snapping panel has **no** baseline-grid checkbox (confirmed absent — don't over-claim it).
6. **Figma** object-snap px threshold — zoom-relative, never published. Do NOT quote a number.
7. **Figma** exact red hex (~`#F24822`, unofficial).
8. **Figma** default on/off state of Snap-to-pixel-grid.
9. **CorelDRAW** Dynamic Guides default toggle shortcut — none verified.
10. **CorelDRAW** default alignment-guide margin px value — none published.
11. **Inkscape** isometric angle: 30° (geometric) vs 26.565° (2:1 game-iso) — different variants, don't conflate.
12. **Shortcut overload `Ctrl/Cmd+D`** — pick & document ONE canonical Varos meaning.

---

### Confirmed source families
- **Illustrator:** Adobe default-shortcuts list + Smart Guides / grids help pages; Redokun, Laura Coyle, Prism, tutorialtactic, Vectips, shapeshed cheat sheets.
- **Affinity:** affinity.studio/help (design-aids-snapping), affinity.help designer2ipad snapping + pixelAlign pages, s3 affinity-docs photo/designer mirrors.
- **Figma:** help.figma.com (nudge, alignment/rotation, measure-distances, zoom/view-options, Smart Selection) + figma.com/blog Smart Selection.
- **Sketch / CorelDRAW / Inkscape:** sketch.com canvas + Smart Distribute blog; product.corel.com help (step-and-repeat, transforming, alignment/dynamic guides); Tavmjong Bah Inkscape manual + inkscape.org keys reference + floss manuals.
