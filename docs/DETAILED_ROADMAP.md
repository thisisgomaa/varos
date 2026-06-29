# Varos — Detailed System Roadmap (build order, exhaustive)

> Depth-first: each system reaches at least **Stage 1** before the next, then we circle back to deepen (see Decision 4).
> Stages: **Stage 1** = core/MVP of the system · **Stage 2** = standard · **Stage 3** = advanced. ✅ have · 🟡 partial · 🔴 not started.
> This is the working map (from `ELEMENTS_CATALOG.md`). Ahmed reorders freely; new ideas get slotted into the right system.

## ⓘ Build decisions — 2026-06-29 (review-driven; these OVERRIDE anything below that conflicts)
A grounded code-vs-plan review (5-agent pass over the real repo) produced these amendments. Each is load-bearing:

1. **Serde spine is FOUNDATIONS, not Save §4.** `model.rs` has zero serde today; Color/Stroke/Transform/Layers/Artboard all assume "saved to the `.varos` schema." So `#[derive(Serialize, Deserialize)]` on `Anchor/Path/Group/Document/Rgba/Pt` + one round-trip test ships in Foundations (Stage-0) BEFORE any system. Save §4 then = container (zip/OPC) + dialogs + guards over a schema that already round-trips.
2. **Layers (§5) is a MODEL system, not a panel.** The real model is a flat `Vec<Path>` + a `group_of` side-table; there is no `Layer` type, no z-order on `Group`, no sublayers, and `scene.rs` uses the flat Vec order AS z-order. Layers = build the real scene-graph (a `Layer`/node tree with children + attributes), touching selection + render-order + every tool. Not panel assembly.
3. **One owner for Units + coordinate-space, BEFORE Artboard.** px/pt/mm/cm/in parsing + the world↔screen (px-per-unit + origin) contract is built once (home: Foundations/Artboard) before Artboard hardcodes unitless coords. Transform/Snapping/Rulers/Grid all consume it.
4. **Depth-first stays — with ONE early confirmation slice.** After Colour, build a single end-to-end vertical slice: **draw → colour → save → reopen** ("make a logo and keep it"). It proves the foundation holds before 9 systems stack on it. Then resume depth-first. This is ONE slice, not breadth.
5. **B6 (the property→inspector+save "master component") is NOT built in the foundation.** Build Color + Stroke + Transform by hand (accept wiring each property ~3× for these 3 systems), THEN extract B6 from their real shapes. Designing it blind = abstracting the wrong thing. ⚠️ Explicit contradiction accepted: **the foundation is NOT 100% complete before system 1** — B6 is derived after 3 real systems exist.
6. **Codex parallelism (after the foundation):** FREEZE the shape of `Document` / `Op` / `Snap`; one agent owns those three files, the other sends change-requests. Do NOT parallelize adjacent or renderer-deep systems — **Color‖Stroke is forbidden** (both extend `Path`, both need new renderer pipelines + C2/C4 math). Clean seams = by crate (one in `varos-core`, one in `varos-app`) or non-adjacent systems.
7. **Order tweaks:** Snapping moves up to sit WITH/before Transform (precise transform + drawing need snap to feel right — feel is the thesis). Artboard slot-1 = Stage-1 only (single board), not full multi-artboard. The wgpu 0.19→0.29 bump is its OWN de-risking spike with a fallback (stay on 0.19, defer frosted glass) — it must NOT gate Phase 1. Transform §9.3 (Shift=proportional) is RECONCILED against the locked, Ahmed-verified handle-coupling model (geometry-based couple, **Alt breaks**) before coding — feel wins over the Illustrator default.
8. **v1 scope = Illustrator-class, NOT Figma-class.** Components/Symbols and the Figma layer (Frames/Auto-Layout/Dev-Mode) are post-v1 SYSTEMS (not panel stubs). Extensibility/Plugins/single-schema (the stated moat) is a real system, deferred — given its true weight, not a one-line bullet. (See `ELEMENTS_CATALOG.md` header.)

## Build order — CURRENT (updated 2026-06-29 · review-amended)
Build in THIS sequence. The detailed sections below keep their ORIGINAL §number — find by name.

**0** Foundations *(§0 — now incl. serde spine [D1] + units/coords owner [D3])* → **1** Artboard *(§7 — Stage-1 single board [D7])* → **2** Transform **+ Snapping** *(§3 + §6 [D7])* → **3** Layers *(§5 — model/scene-graph [D2])* → **4** Colour *(§1)* → **▶ CONFIRMATION SLICE: draw → colour → save → reopen [D4]** → **5** Stroke *(§2)* → **6** Save system *(§4 — container + dialogs + recovery [D1])* → **7** Geometric tools *(§9)* → **8** Text *(§10)* → **9** Export *(§8)* → **10** Effects/Appearance *(§11)* → **11** Comments *(§12)*

## Original generation order (reference only)
0. **Foundations (shared base — BEFORE any system)** — The shared three-layer base — design-system UI widgets, the data-model/Op/undo/property-binding architecture, and the GPU/math/units/serialization technical core — that every later Varos system plugs into.
1. **Color system** — The complete color pipeline — picker, color models, fill/stroke targets, swatches, gradients, harmony/recolor, eyedropper, and opacity — that lets users define, store, reuse, and apply every color and gradient in Varos.
2. **Stroke system** — Everything that controls how a path's outline is drawn — weight, caps, joins, alignment, dashes, arrowheads, variable width, and stroke-to-fill order — plus the Stroke panel and Width tool.
3. **Transform system** — Exact numeric and tool-driven transforms — X/Y/W/H, rotate, shear, flip, the 9-point reference, dedicated Move/Rotate/Scale/Reflect/Shear tools with pivots and Alt-copy, Transform Each / Again / Free Transform, plus Offset Path — all wired to one Transform panel and the control-bar fields.
4. **Save / File system** — The native .varos file system — serialize/open/save the document model, with New/templates, autosave + crash recovery, version history, recent files, and Place/Import with linked vs embedded assets.
5. **Layers / Structure System** — The hierarchical document tree — Layers panel with nested layers/sublayers/objects, visibility & lock, targeting, z-order, grouping, masks, compound paths, and structural selection — that organizes every object on the canvas.
6. **Snapping / Guides / Grid / Rulers** — The precision-alignment substrate: rulers, draggable guides, smart alignment guides, document/pixel grids, snap-to-geometry, snap tolerance options, and live dimension readouts that make every move, draw, and resize land exactly where intended.
7. **Artboard / Document System** — Multiple resizable/named artboards on one canvas plus the document model (units, DPI, color format, bleed, presets) that defines the design surface and drives navigation and per-artboard export.
8. **Export system** — Getting finished artwork out of Varos in every format and configuration — single Export As, batch Export for Screens, a persistent Asset Export panel, per-artboard/slice output, format-specific options, and clipboard copy.
9. **Geometric-logic tools** — The geometry-computing tools — Line Segment, Shape Builder, cut tools (Scissors/Knife/Eraser), and Clipping Mask / Compound Path — that build, split, merge, and mask vector geometry through direct canvas interaction.
10. **Text / Type system** — The full typography stack — point/area/path text objects, a GPU text engine (shaping + line layout + glyph rendering), and the Character/Paragraph/OpenType/Glyphs/Styles panels that drive them, Latin-first with Arabic shaping flagged as the later moat.
11. **Effects / Appearance System** — A non-destructive appearance stack per object — multiple fills/strokes, live Effects (shadow, glow, blur, distort, warp, 3D, round corners), Graphic Styles, and Expand Appearance.
12. **Comments / Collaboration system** — Canvas-anchored comment pins with threads, replies, reactions, @mentions, resolve/reopen, a filterable comments panel, presence/cursors, and share/version flows — local-first in the .varos file with an optional backend for real-time multi-user sync.

---

## 0. Foundations (shared base — BEFORE any system)
*The shared three-layer base — design-system UI widgets, the data-model/Op/undo/property-binding architecture, and the GPU/math/units/serialization technical core — that every later Varos system plugs into.*

> Grounded against the real repo: native egui+wgpu UI shell live (`varos-app/src/ui.rs`), `varos-core` (model/editor/tools/geom/scene/boolean), wgpu renderer with stencil-cover fills + frosted plumbing. Snapshot-clone undo, stable u32 IDs, deferred-`Op` panel pattern all exist. Gaps flagged inline (no serde/`.varos`, no unit system, no property-definition abstraction, RGBA-f32-only color, no text glyph / gradient rendering).

> **Stage-0 spine (do FIRST, before any system) [D1/D3]:** (a) serde derives on `Anchor/Path/Group/Document/Rgba/Pt` + one round-trip test; (b) the Units + world↔screen coordinate contract (px/pt/mm + px-per-unit + origin). These two close the gaps every early system silently assumes.

---
## LAYER A — DESIGN-SYSTEM UI PIECES (the panel "puzzle pieces")
---

**A1. Design tokens — the single source of style** _(Stage 1 — core/MVP)_ ✅ (have basic)
- A1.1 Color tokens — exact Figma palette, already hard-coded as consts in `ui.rs`
  - A1.1.a bg: `--bg-app #141313`, `--bg-app-2 #1e1e1e`, `--bg-panel #1f1f22`, `--bg-surface #262627`
  - A1.1.b state bg: `--bg-hover #2c2c2c`, `--bg-active #2e2e2e`
  - A1.1.c borders: `--border #2a2a2d` (hairline), `--border-2 #3a3b3d` (focused field)
  - A1.1.d text: `--text #e6e6e6`, `--text-2 #d2d2d2`, `--muted #8a8a8a`, `--faint #7c7c7c`
  - A1.1.e accent: `--accent #0c8ce9` (azure) — active tool, Share, focus, selection; MUST stay in sync with the wgpu canvas selection color
  - A1.1.f semantic aliases: success-dot green, `--close-red #e81123` (window close hover) — already present
- A1.2 Spacing scale _(Stage 1)_ 🟡 (partial, ad-hoc) — 4/8/12/16px; panel inset ~16px from window edge; inner card padding 12–16px; field height ~28–30px
- A1.3 Radius scale _(Stage 1)_ 🟡 (partial) — panels/pills 12px, buttons 6–8px, inputs 5–6px, round toggles/pills 999px
- A1.4 Typography tokens _(Stage 1)_ 🟡 (partial) — UI = Inter; numbers = JetBrains Mono; base 13px / line-height 1.5; section labels 11–12px letter-spaced
- A1.5 Elevation/shadow tokens _(Stage 1)_ ✅ (have basic) — floating shadow `0 8px 30px rgba(0,0,0,.45), inset 0 1px 0 rgba(255,255,255,.03)`; currently one light GPU shadow pass behind panel rects
- A1.6 Promote to a real token module _(Stage 2)_ — extract all consts into one `tokens.rs`/`theme` struct so light/alt themes + a future "solid vs frosted" material flag read from one place (today they are scattered consts)
- A1.7 Motion tokens _(Stage 3)_ — hover/press transition durations, easing curves for panel show/hide + dropdown open

**A2. The floating panel container** _(Stage 1 — core/MVP)_ ✅ (have basic) — tool rail + inspector dock exist
- A2.1 Body — rounded 12px, `--bg-panel`, hairline `--border`, GPU drop-shadow; rects tracked in `Ui.rects` for the shadow pass
- A2.2 Material toggle _(Stage 2)_ 🟡 (plumbing exists) — frosted-glass (blur scene texture behind panel) vs solid; `Ui.frosted` flag + `frost_pipe` in renderer already wired but currently forced solid
- A2.3 Header — title text, optional left icon, optional right action icons (collapse, more "…"), drag-to-move affordance _(Stage 2)_
- A2.4 Show/hide + collapse — whole-panel toggle (Window menu, A11) + collapse-to-header _(Stage 2)_
- A2.5 Dock vs float _(Stage 3)_ — panels can be docked to an edge or float over the board; remember last position
- A2.6 Resize + min/max size + scroll-on-overflow _(Stage 2)_ — inspector grows; internal scroll when content exceeds height
- A2.7 Tabbed panel host _(Stage 2)_ 🟡 (spec'd) — e.g. `Design | Layers`, `Transform·Align·Pathfinder` / `Properties·Layers·Libraries` per PANELS_PRO_SPEC
- A2.8 Tear-off / re-dock + multi-monitor _(Stage 3)_
- A2.9 Panel-arrange/coordinate system _(Stage 2)_ — shared layout that positions every panel + reports its rect to the GPU shadow/frost pass (phase-1 of the GPU-UI plan)

**A3. Buttons** _(Stage 1 — core/MVP)_ ✅ (have basic) — tool chips + window controls drawn hand-painted
- A3.1 Variants — primary (azure filled `Share`), secondary (surface), ghost/icon-only (tool rail), destructive (red)
- A3.2 Sizes — toolbar 28px square, standard 28–30px, small inline
- A3.3 Content — icon-only, text-only, icon+text, with optional dropdown caret
- A3.4 Segmented/grouped buttons _(Stage 1)_ 🟡 — align row, alignment-resizing toggles, text-alignment row (6 icons)
- A3.5 Toggle/sticky button _(Stage 1)_ ✅ — active tool = azure filled chip; pressed-state persists
- A3.6 Split button (action + menu) _(Stage 3)_

**A4. Toggles & segmented controls** _(Stage 1 — core/MVP)_ 🟡 (partial)
- A4.1 Checkbox + tri-state — eye (visible/hidden), lock, link/constrain (`Ui.lock` exists)
- A4.2 On/off switch (round 999px) — for boolean prefs
- A4.3 Segmented control — mutually-exclusive set (resizing mode, alignment, fill/stroke target) ✅ (paint target exists)
- A4.4 Radio group _(Stage 2)_
- A4.5 Icon-toggle with active tint (azure) + tooltip _(Stage 1)_ ✅

**A5. Number fields (the pro scrub input)** _(Stage 1 — core/MVP)_ ✅ (have basic, X/Y/W/H/rot/opacity/stroke-w in `ui.rs` via `Op`)
- A5.1 Type-to-edit — direct numeric entry, Enter commits, Esc reverts
- A5.2 Click-drag scrub — drag horizontally on the field/label to change value (Illustrator/Affinity); Alt = fine, Shift = coarse step _(Stage 1)_ 🟡 (verify present)
- A5.3 Mouse-wheel over field — ±1 step, Shift = ±10 _(Stage 2)_
- A5.4 Stepper arrows — up/down micro-arrows _(Stage 2)_
- A5.5 Keyboard arrows — ↑/↓ ±step, Shift ×10, Alt ÷10 _(Stage 1)_
- A5.6 Units suffix + parsing — px/pt/mm/cm/in/% display + accept typed units, convert on commit _(Stage 2)_ 🟡 (no unit system yet — see C7)
- A5.7 Inline math expressions — `100+20`, `50*2`, `100/3` evaluated on commit _(Stage 2)_
- A5.8 Min/max clamp + precision + dynamic placeholder ("Mixed" for multi-select) _(Stage 1)_ 🟡
- A5.9 Linked/proportional fields — W↔H link toggle that scales both _(Stage 1)_ ✅ (`Ui.lock` + ref-point `Ui.refpt`)
- A5.10 Reference-point picker (9-dot anchor for X/Y/W/H origin) _(Stage 2)_ 🟡 (refpt state exists, picker UI todo)

**A6. Text inputs** _(Stage 2 — standard)_ 🟡 (egui text edit available, not styled)
- A6.1 Single-line — layer rename, hex entry, search; placeholder, select-all-on-focus, Enter/Esc
- A6.2 Multi-line / textarea _(Stage 3)_ — for the future Type tool's content
- A6.3 Validation states — error ring (red border), valid; inline message
- A6.4 Search field variant — magnifier icon + clear-X (icons already loaded: `IC_SEARCH`, `IC_X`)
- A6.5 Hex color field — `#` prefix, 3/6/8-digit accept, live swatch preview _(Stage 1, with Fill)_

**A7. Dropdowns / selects / menus** _(Stage 2 — standard)_ 🟡 (egui popups; clipping was the web-era blocker, native fixes it)
- A7.1 Simple select — font weight, units, blend mode; current value + caret, popup list, checkmark on selected
- A7.2 Combo/searchable select — font-family picker (filter as you type)
- A7.3 Context menu (right-click) — canvas + layer + object menus _(Stage 2)_
- A7.4 Menu bar / app menu — top-bar `☰` menu (`IC_MENU` loaded); File/Edit/Object/Type/Select/Effect/View/Window equivalents _(Stage 2)_
- A7.5 Submenus, separators, disabled items, shortcut hint text, danger items _(Stage 2)_
- A7.6 Popovers that escape the panel rect — solved natively (was impossible with WebView2) _(Stage 1 enabler)_ ✅

**A8. Sliders** _(Stage 2 — standard)_ 🟡
- A8.1 Single-value slider — opacity, stroke weight, font size; track + thumb + optional fill
- A8.2 Slider + number-field pair (drag coarse, type exact)
- A8.3 Range/dual-thumb slider _(Stage 3)_
- A8.4 Gradient/hue/alpha slider tracks (for the color picker) _(Stage 2)_ — special render: hue spectrum, alpha checkerboard
- A8.5 2D area picker (SV square in color picker) _(Stage 2)_

**A9. Color swatch chips + picker entry** _(Stage 1 — core/MVP)_ 🟡 (fill/stroke swatches conceptually present)
- A9.1 Swatch chip — rounded square, checkerboard behind alpha, "no color" = red diagonal slash, "none/mixed" states
- A9.2 Fill/Stroke pair with swap + default (X/D in Illustrator) ✅ (have basic, in core)
- A9.3 Click → opens color picker popover _(Stage 1)_
- A9.4 Recent + document swatches strip _(Stage 2)_
- A9.5 Eyedropper button inline _(Stage 1)_ ✅ (eyedropper tool exists)
- A9.6 Gradient swatch preview (linear/radial bar) _(Stage 3)_

**A10. Tabs, collapsible sections, tooltips, misc** _(Stage 1–2)_ 🟡
- A10.1 Tabs — panel-level (Design|Layers) + within-panel pill tabs _(Stage 2)_
- A10.2 Collapsible section — header (label + chevron + optional inline action icons like eye/droplet/`+`) that expands a body; remembers open/closed _(Stage 1)_ 🟡 (sections drawn flat today)
- A10.3 Tooltip — hover delay, label + optional shortcut hint, theme-styled _(Stage 1)_ ✅ (tool tips in rail)
- A10.4 Divider / separator (`—` between tool groups) _(Stage 1)_ ✅ (`group_end`)
- A10.5 Label + section title (muted, letter-spaced 11–12px) _(Stage 1)_ ✅
- A10.6 Badge/pill (status, count), green status dot + chevron _(Stage 2)_
- A10.7 List row (Layers row template: thumbnail, name, eye, lock, drag handle) _(Stage 2, used by Layers)_
- A10.8 Empty-state + "No selection" placeholder _(Stage 1)_ ✅
- A10.9 Toast / inline notification _(Stage 3)_
- A10.10 Modal dialog + confirm + the splash/loading screen _(Stage 1 splash)_ ✅ (splash exists)
- A10.11 Scrollbar styling, progress/spinner, skeleton _(Stage 2/3)_

**A11. Window (Show/Hide) menu system** _(Stage 2 — standard)_ 🟡
- A11.1 Each panel registers a name + visibility flag readable/toggleable from a Window menu
- A11.2 Checkmark next to visible panels; keyboard shortcuts to toggle (e.g. F-keys)
- A11.3 Workspace presets (save/restore panel layout) _(Stage 3)_
- A11.4 Reset-to-default layout _(Stage 2)_

**A12. Icon set** _(Stage 1 — core/MVP)_ ✅ (have basic — Lucide subset embedded as SVG path-data, white-stroked, rasterized to egui textures)
- A12.1 Source = Lucide, thin outline ~1.7 stroke, viewBox 24; tool + field-label + window + top-bar icons already inlined as consts
- A12.2 Rasterize-to-texture pipeline (SVG string → `TextureHandle`) — already in place; need a central icon registry instead of per-field `Option<TextureHandle>` _(Stage 2)_
- A12.3 Sizes/tints — resting `--muted`, hover `--text`, active white-on-azure; disabled faint _(Stage 1)_ ✅
- A12.4 Full coverage sweep — every future system's icons added once to the registry _(Stage 2)_

**A13. Interaction states (hover/active/focus/disabled)** _(Stage 1 — core/MVP)_ ✅ (have basic)
- A13.1 Hover — `--bg-hover`, icon → `--text`
- A13.2 Active/pressed — `--bg-active`, azure fill for selected tool
- A13.3 Focus ring — `--border-2` / azure outline on keyboard focus _(Stage 2)_ 🟡
- A13.4 Disabled — faint text, no hover, no pointer
- A13.5 Selected/checked, indeterminate/mixed, loading, error _(Stage 2)_
- A13.6 Keyboard focus traversal (Tab order) across fields _(Stage 2)_

**A14. Cursor system** _(Stage 1 — core/MVP)_ 🟡 (egui cursor read per-frame in `Ui.cursor`; real Illustrator-style SVG cursors spec'd, not all drawn)
- A14.1 Per-tool cursors — Selection (black arrow), Direct (hollow arrow), Pen nib, Pencil, Text I-beam, shape crosshair, Hand grab/grabbing, Zoom magnifier ±
- A14.2 Contextual pen variants — nib + ×/+/−/○/^ that switch live by what's under the pointer (logic proven in pen-spike) _(Stage 1)_ 🟡
- A14.3 Hotspot per cursor (the pixel that points) + 32×32 with @2x hi-dpi
- A14.4 Re-drawn SVGs (not Adobe bitmaps — copyright); native winit cursor when over the wgpu canvas
- A14.5 Drag/resize/rotate cursors (directional resize arrows, rotate curl) _(Stage 2)_

---
## LAYER B — PROGRAMMING / ARCHITECTURE BASE
---

**B1. Document & property data model** _(Stage 1 — core/MVP)_ ✅ (have basic — `varos-core/src/model.rs`)
- B1.1 `Document` = flat `Vec<Path>` (z-order = render + index order) + `groups` + `group_of` map + `ids` counter
- B1.2 Stable u32 IDs (never Vec indices) so selection/active survive deletes/joins — `nid()`, `pidx`, `aidx`, `anchor`/`anchor_mut`
- B1.3 `Path` = anchors + closed + fill/stroke/stroke_width + holes (compound) + opacity + hidden + locked + name
- B1.4 `Anchor` = id + point + hin/hout handles + smooth flag
- B1.5 Groups as a registry layered on the flat list (nest via `parent`, membership in `group_of`, reconciled by `sync_groups`) — keeps `Path` construction sites untouched
- B1.6 Generalize toward an RNA-style node/property schema _(Stage 3)_ — one schema = file + AI + plugins + inspector (the extensibility superpower from the constitution); today the model is concrete structs, not a generic property bag
- B1.7 Node-kind metadata — a path can carry "live shape" params (rect corner radius, polygon sides) so re-editing is non-destructive _(Stage 2)_ 🟡 (`ShapeKind` exists; live params not stored)
- B1.8 Document-level props — page/artboard size, units, color space, ruler origin, grid/guides _(Stage 2)_ 🟡 (none yet)

**B2. Command / Op pattern for all mutations** _(Stage 1 — core/MVP)_ ✅ (have basic, two complementary patterns)
- B2.1 Engine ops on `&mut Editor` wrapped by `begin()` … set `dirty` … `commit()` (the canonical mutation envelope; e.g. `apply_paint`, `apply_current`, z-order, align)
- B2.2 UI-side deferred `Op` enum — panels read a per-frame `Snap`, push `Vec<Op>` during layout, applied to `&mut Editor` after layout (no IPC, no borrow fights) — `Op::Tool/SetBBox/SetRot/SetOpacity/SetStrokeW/Paint/Flip`
- B2.3 Drag gestures as transient state machine — `enum Drag` (PenNew, Anchors, Handle, Segment, Shape, Marquee, Object, Scale, Rotate, …) driven by shared move/up engine; tools only define what a *press* does
- B2.4 Formalize "one Op = one undo step" contract + naming for the Edit-menu history labels _(Stage 2)_
- B2.5 Macro/compound ops (group several mutations into one undo entry) _(Stage 2)_ 🟡 (begin/commit already brackets this implicitly)
- B2.6 Op result + validation (reject illegal mutation, keep model consistent) _(Stage 2)_

**B3. Undo / redo integration** _(Stage 1 — core/MVP)_ ✅ (have basic)
- B3.1 Snapshot model — `begin` clones whole `Document` into `pending`; `commit` pushes to `undo` stack only if `dirty`; `redo.clear()` on new edit; stack capped at 200
- B3.2 `undo()`/`redo()` swap whole `Document` + `clear_transient()` (drops selection/drag)
- B3.3 Memory/perf — whole-doc clone is fine now; move to diff/delta or COW for large docs _(Stage 3)_
- B3.4 History panel + named steps + jump-to-state _(Stage 3)_
- B3.5 Selection/view as part (or not) of undo state — define policy _(Stage 2)_
- B3.6 Coalescing rapid edits (one undo for a continuous scrub) _(Stage 2)_

**B4. Per-frame state-snapshot read pattern** _(Stage 1 — core/MVP)_ ✅ (have basic — `Snap::read(&Editor)` in `ui.rs`)
- B4.1 Panels NEVER mutate the editor directly mid-layout; they read an immutable `Snap` built once per frame (tool, name, sel, x/y/w/h/rot, fill/stroke/sw/opacity)
- B4.2 Multi-select reduction — "Mixed"/N-objects summarization baked into the snapshot
- B4.3 Generalize `Snap` into a typed, per-system snapshot so each new panel declares exactly what it reads _(Stage 2)_
- B4.4 Dirty/repaint signalling — `Ui.repaint` so the GPU only redraws when state changed _(Stage 2)_ 🟡

**B5. "Add-a-system" template (one shape every system follows)** _(Stage 1 — core/MVP)_ 🟡 (pattern exists implicitly in `tools/`, not yet a written template)
- B5.1 Canonical layout per system: model fields (B1) + an engine op set bracketed by begin/commit (B2) + a tool/gesture in `tools/mod.rs` ToolKind + a `Snap` slice (B4) + an `Op` variant + an inspector section (A2/A10) + icons (A12) + cursor (A14) + undo "for free" (B3)
- B5.2 `tools/` module convention — each tool file defines press-behaviour; shared engine handles move/up (already followed by pen/direct/object/shapes/convert/eyedropper)
- B5.3 Write it down as a real checklist/template doc + a code skeleton so new systems are consistent _(Stage 1 deliverable)_
- B5.4 Test scaffold per system (the repo already has `tests/groups.rs`, `tests/math.rs`) _(Stage 2)_

**B6. Property-definition → inspector-field + save binding (the "master component")** _(Stage 2 — standard)_ 🔴 (NOT yet abstracted — biggest foundation gap)
> ⚠️ AMENDED [D5]: do **NOT** build B6 as part of the foundation. Build Color + Stroke + Transform by hand first (accept wiring each property ~3× for these 3 systems), THEN **extract B6 from their real shapes.** Designing it blind abstracts the wrong thing. Explicit, accepted contradiction: **the foundation is therefore NOT 100% complete before system 1.**
- B6.1 Today each property is hand-wired three times (model field + `Snap` field + `Op` variant + bespoke inspector widget) — works but doesn't scale
- B6.2 Target: define a property ONCE (name, type, range, unit, default, widget hint) and auto-derive its inspector field, its read into the snapshot, its Op, AND its serialization — like a Figma master component
- B6.3 Property descriptor registry (RNA-style) keyed by node-kind — enables generic inspector, generic save, plugin/AI access from one schema _(Stage 2/3)_
- B6.4 Binding modes — single value, multi-select reduction ("Mixed"), linked (W↔H), computed/derived
- B6.5 Validation + coercion + units handled centrally per descriptor
- B6.6 This is the extensibility moat: schema = file + AI + plugins + inspector (per the constitution) — design it before the inspector grows further

**B7. Tool/gesture dispatch + input routing** _(Stage 1 — core/MVP)_ ✅ (have basic)
- B7.1 `ToolKind` enum (Object/Direct/Pen/shapes/Convert/Eyedropper) + `gesture` (temporary tool override e.g. space=Hand)
- B7.2 Modifier state (`Mods` shift/alt/ctrl) threaded into every gesture
- B7.3 Keyboard shortcut layer — match by physical key/code (Arabic-layout lesson), tool letters, modifiers _(Stage 2)_ 🟡
- B7.4 Pointer event → world-coord pipeline via `View.s2w` + `ppu` for screen-constant tolerances ✅
- B7.5 Hit-test ordering (top-most first, respect hidden/locked) ✅ (`path_under`, `nearest_anchor`)

**B8. The hard seam (escape hatch to native)** _(Stage 2 — standard)_ ✅ (have basic, architecturally honored)
- B8.1 `varos-core` (model/editor/tools/geom/scene/boolean) is platform-free + UI-free; renderer "knows nothing about winit/tauri"; UI is a thin view layer (the web→native swap already happened cleanly per the GPU-UI pivot, core untouched)
- B8.2 Keep this seam as a hard rule for every new system: logic in core, view in app — so a future native/web/headless host is a re-skin, not a rewrite
- B8.3 Define the core↔host API surface explicitly (events in, scene + snapshot out) _(Stage 2)_

---
## LAYER C — TECHNICAL BASE
---

**C1. GPU rendering primitives** _(Stage 1 — core/MVP)_ ✅ (have basic — `varos-render-wgpu`)
- C1.1 Fills — stencil-then-cover (handles concave + self-intersecting + even-odd holes), MSAA, non-sRGB surface, Mailbox present (`pipe_stencil`/`pipe_cover`)
- C1.2 Strokes — bg/fg vertex buffers (`build_bg`/`build_fg`); flat-tessellated polylines today
- C1.3 Pro stroke geometry — real width/joins (miter/round/bevel)/caps (butt/round/square)/dashes — needs proper stroker _(Stage 2)_ 🔴
- C1.4 Paths — resolution-independent flatten (`ring_px`, adaptive ~4px chords by zoom) feeding the tessellator ✅
- C1.5 Text glyphs — glyph atlas / SDF rendering; not built (future Type tool on own HarfBuzz canvas, NOT egui) _(Stage 3)_ 🔴
- C1.6 Gradients — linear/radial/conic fill rendering; no gradient shader yet (color model is flat RGBA only) _(Stage 2)_ 🔴
- C1.7 Images/raster fills, patterns _(Stage 3)_ 🔴
- C1.8 Compositing & effects — per-object opacity ✅; blend modes, masks, shadow/blur effects _(Stage 3)_
- C1.9 Offscreen scene target + blit — scene rendered to `scene_tex`, blitted to surface; doubles as blur source for frosted panels ✅
- C1.10 Frosted-glass pass — `frost_pipe`/`frost_bg` sample-blur scene behind panel rects (plumbed, off by default) _(Stage 2)_ 🟡
- C1.11 GPU UI shadow pass — light shadow drawn behind each panel rect from `Ui.rects` ✅
- C1.12 wgpu version — currently 0.19/egui 0.27; bump to ~0.29 _(its OWN de-risking spike [D7], NOT a Phase-1 gate)_ 🟡 — fallback: stay on 0.19 and defer frosted glass if the 10-version jump gets hairy. Must not block any system.
- C1.13 Selection/overlay layer — handles, bbox, anchors, marquee, snap guides drawn over the scene (currently part of fg) _(Stage 2)_ 🟡
- C1.14 Dot-grid background render ✅ (have basic)

**C2. Shared math — bezier** _(Stage 1 — core/MVP)_ ✅ (have basic — `geom.rs` + `model.rs`)
- C2.1 Cubic eval (`cubic`), flatten to polyline (`ring`/`ring_px`), nearest-point-on-segment (`nearest_seg`)
- C2.2 Vector math — sub/add/scale/dist/length/norm/mirror/rotate_about, snap45 ✅
- C2.3 Point-in-polygon (even-odd) for hit-testing ✅
- C2.4 Curve subdivision/split at t, curve length, bbox-of-curve (tight, not handle-hull) _(Stage 2)_ 🟡 (current bbox uses handle points)
- C2.5 Curve offset / outline-stroke-to-fill (needed for stroke-to-path + pro strokes) _(Stage 2)_ 🔴
- C2.6 Curve fitting (pencil → beziers), simplify _(Stage 2)_ 🔴
- C2.7 Rent OSS where heavy (kurbo/lyon-style) behind the seam if hand-rolled gets costly _(Stage 2)_

**C3. Shared math — boolean (i_overlay)** _(Stage 1 — core/MVP)_ ✅ (have basic — `boolean.rs`, `run_boolean_curves`)
- C3.1 Ops — union/intersect/subtract/difference (`BoolOp`), curve-aware (`Seg` cubics in/out), result→editable anchors (`make_anchor` reconstructs smooth/handle state)
- C3.2 Compound-path / holes output preserved (`Path.holes`, even-odd) ✅
- C3.3 Robustness — coincident edges, degenerate input, fp tolerance hardening _(Stage 2)_
- C3.4 Confirm i_overlay (or chosen lib) is the long-term engine + keep it behind the seam ✅ (per memory: rent Clipper2/i_overlay-class math)

**C4. Shared math — color conversions** _(Stage 2 — standard)_ 🔴 (color is `Rgba [f32;4]` only — NO conversions yet)
- C4.1 RGB ↔ HSB/HSL — for the color picker SV-square + hue slider
- C4.2 RGB ↔ Hex (3/6/8-digit) parse + format — for the hex field
- C4.3 RGB ↔ CMYK — for print workflows
- C4.4 Alpha handling + premultiply for compositing
- C4.5 sRGB ↔ linear (renderer is non-sRGB surface — define the gamma policy once) _(Stage 1 correctness)_ 🟡
- C4.6 Color type — promote flat `Rgba` to a Color enum (solid/gradient/none/swatch-ref) _(Stage 2)_ 🔴
- C4.7 Wide-gamut / color-management / ICC _(Stage 3)_

**C5. Shared math — transform & geometry** _(Stage 1 — core/MVP)_ 🟡 (partial — done per-gesture, not a unified matrix)
- C5.1 Translate/scale/rotate applied to anchors+handles inside Drag handlers (Object/Scale/Rotate work in the frame's LOCAL un-rotated space) ✅
- C5.2 A real 2D affine matrix type (compose/invert/apply) used everywhere _(Stage 2)_ 🔴 — today transforms are inline math, no shared `Mat` (skew/shear impossible cleanly without it)
- C5.3 Bounding boxes — control-point bbox (`bbox`) + visual outline bbox (`outline_bbox`); tight-curve bbox todo (C2.4)
- C5.4 Per-object transform vs baked geometry policy — decide whether objects store a transform or bake into points (affects rotate/scale fidelity + SVG export) _(Stage 2)_
- C5.5 Snapping math — grid/guide/object-edge/anchor snap, angle snap (snap45 exists) _(Stage 2)_ 🟡

**C6. Coordinate + zoom + camera system** _(Stage 1 — core/MVP)_ ✅ (have basic — `View` + `Editor.ppu`)
- C6.1 `View { pan, zoom }` with `s2w`/`w2s`; `screen = world*zoom + pan`
- C6.2 `ppu` (pixels-per-unit) threaded so all grab tolerances stay constant on screen ✅
- C6.3 Zoom-to-point, fit, 100%, zoom levels, min/max clamp _(Stage 2)_ 🟡 (basic zoom only)
- C6.4 Pan (Hand/space) + scroll-wheel + pinch _(Stage 2)_ 🟡
- C6.5 HiDPI / device-pixel-ratio scaling + per-monitor DPI _(Stage 1 correctness)_ 🟡
- C6.6 Multiple artboards / infinite canvas coordinate strategy _(Stage 3)_

**C7. Units system** _(Stage 2 — standard)_ 🔴 (NONE — everything is bare f32 "world units")
- C7.1 Unit registry — px, pt, pc, mm, cm, in, % with document DPI for px↔physical conversion
- C7.2 Document-unit setting + per-field unit display/parse (feeds A5.6)
- C7.3 Ruler/origin + measurement readouts _(Stage 2)_
- C7.4 Precision/rounding policy per unit
- C7.5 Scale-aware (e.g. 1:50 architectural) _(Stage 3)_

**C8. .varos serialization skeleton** _(Stage 1 — core/MVP)_ 🔴 (NOT started — `Document` has no Serialize; no file I/O found)
- C8.1 Schema-versioned container — version tag + migration path from day one
- C8.2 Serialize the whole model — paths/anchors/handles/holes/groups/group_of/ids + doc props + fills/strokes; use serde + a stable format (JSON for readability or a compact binary)
- C8.3 Save/load/new/recent + autosave + crash-recovery _(Stage 2)_
- C8.4 IDs preserved across save/load (stable u32) so references survive ✅ (model designed for it)
- C8.5 One schema drives file + inspector + AI + plugins (tie to B6 property registry) — design serialization and the property descriptor together _(Stage 2)_
- C8.6 Embedded assets (images/fonts), compression, thumbnail _(Stage 3)_
- C8.7 Import/export adapters (SVG/PDF/PNG) as separate systems but plan the model→SVG mapping now _(Stage 2)_

**C9. Panel ↔ engine binding** _(Stage 1 — core/MVP)_ ✅ (have basic — the in-process deferred-Op model)
- C9.1 No IPC: egui shares OUR wgpu Device/Queue; panels read `Snap`, emit `Vec<Op>`, host applies to `&mut Editor` after layout — clean borrow story
- C9.2 Renderer signature already carries a UI channel: `render(world, &ui, view)` so canvas pass + egui pass compose on one frame ✅
- C9.3 Window actions from the custom title bar routed to the host via `WinAction` (Minimize/ToggleMaximize/Close) ✅
- C9.4 Generalize the `Op` bus so every new system adds a variant + an apply arm in one place (ties to B2/B6) _(Stage 2)_
- C9.5 Repaint-on-demand + input event forwarding (egui_winit `State`) ✅
- C9.6 Keep this binding as the ONE way panels talk to the engine (no new system bypasses it) — architectural rule _(Stage 1)_

---
## BUILD ORDER (what "Foundations complete" means)
---

**Stage 1 (must exist before ANY system):** token module (A1), panel container + tool rail + inspector (A2), core widgets — button/toggle/number-field/swatch/section/tooltip (A3–A5, A9–A10), icon pipeline (A12), states (A13), basic cursors (A14); the data model (B1), Op + begin/commit + undo (B2/B3), per-frame Snap (B4), the written add-a-system template (B5), tool dispatch (B7); GPU fills/strokes/paths (C1.1–C1.4, C1.9–C1.12), bezier + boolean + transform/view math (C2/C3/C5/C6), the panel↔engine bus (C9), and a first-cut `.varos` save (C8.1–C8.2). 🔴 Real Stage-1 gaps to close: `.varos` serialization (C8), and the wgpu bump (C1.12).

**Stage 2 (makes it pro):** property-definition→field/save binding (B6, the master-component moat), units (C7) + unit-aware number fields (A5.6), pro strokes (C1.3) + gradients (C1.6) + color conversions/picker (C4, A8.4), dropdowns/menus/context-menus (A7), frosted material (A2.2/C1.10), tabs/collapsibles/Window-menu (A10/A11), snapping (C5.5/C6.3), affine-matrix refactor (C5.2).

**Stage 3 (advanced):** RNA-style generic schema (B1.6/B6.3), text glyph rendering (C1.5), images/blend modes/effects/masks (C1.7–C1.8), history panel (B3.4), workspaces/tear-off panels (A2.5/A11.3), color management (C4.7), import/export adapters (C8.7).

---

## 1. Color system
*The complete color pipeline — picker, color models, fill/stroke targets, swatches, gradients, harmony/recolor, eyedropper, and opacity — that lets users define, store, reuse, and apply every color and gradient in Varos.*

**1. Color targets & active-color control (Fill / Stroke / None)** _(Stage 1 — core/MVP)_ ✅ (have basic)
- 1.1 Dual color chips (Fill + Stroke) — overlapping swatch pair, Fill in front, Stroke behind; click to set active target
  - 1.1.a Active target ring/highlight — visually indicates whether Fill or Stroke receives the next color change
  - 1.1.b Click an already-active chip → opens full Color Picker dialog (double-click in Illustrator)
  - 1.1.c Stroke chip shows a hollow/ring rendering to distinguish from solid Fill chip
- 1.2 Swap Fill/Stroke control — small bent double-arrow; shortcut `Shift+X` ✅ (have)
- 1.3 Default Fill/Stroke control — mini black-stroke/white-fill icon; shortcut `D` ✅ (have)
- 1.4 Color/Gradient/None mode trio — three buttons under the chips (solid color, gradient, none); shortcut `<` solid, `>` gradient, `/` none
  - 1.4.a None — removes paint from the active target; chip shows red diagonal slash
  - 1.4.b Last-used color/gradient remembered per target when toggling None on/off
- 1.5 Target applicability rules — disable/grey Stroke for objects that can't stroke; type objects color fill of glyphs
- 1.6 Multi-select behavior — mixed fills show a "?" / mixed indicator; applying overrides all selected ✅ (have basic apply)
- 1.7 X to toggle active target (Fill ⇄ Stroke) without changing colors _(Stage 2 — standard)_
- 1.8 Drag-and-drop color — drag chip/swatch onto a canvas object to apply without selecting; onto Fill vs Stroke chip _(Stage 2 — standard)_

**2. Color models / color space representations** _(Stage 1 — core/MVP)_ 🟡 (partial — basic apply exists, no model UI)
- 2.1 RGB — 0–255 per channel (R, G, B); the on-screen working model _(Stage 1)_
- 2.2 Hex — `#RRGGBB` and `#RGB` shorthand; paste/typing with or without `#`; copy hex _(Stage 1)_
- 2.3 HSB / HSV — Hue 0–360°, Saturation 0–100%, Brightness 0–100%; primary model behind the wheel _(Stage 1)_
- 2.4 Grayscale — single 0–100% (or 0–255) channel (K-style) _(Stage 2 — standard)_
- 2.5 CMYK — Cyan/Magenta/Yellow/Black 0–100%; print model _(Stage 2 — standard)_
  - 2.5.a Out-of-gamut indicator (CMYK can't reproduce some RGB) — warning triangle + "nearest CMYK" swatch to click
  - 2.5.b Document color mode (RGB vs CMYK) drives which model is primary; conversion on mode switch
- 2.6 HSL — Hue/Saturation/Lightness (Affinity offers it alongside HSB) _(Stage 2 — standard)_
- 2.7 Lab — L (0–100), a/b (−128…+127); device-independent, widest gamut _(Stage 3 — advanced)_
  - 2.7.a Used internally for accurate gradient interpolation & color distance (harmony/recolor)
- 2.8 Web-safe / named CSS colors lookup _(Stage 3 — advanced)_
- 2.9 Model switcher — dropdown in Color panel menu to pick which model's sliders/fields show; per-panel persistence
- 2.10 Color management plumbing _(Stage 3 — advanced)_
  - 2.10.a ICC profile assignment (document RGB profile e.g. sRGB; CMYK profile e.g. US Web Coated)
  - 2.10.b Soft-proof / proof colors toggle; gamut-warning overlay
  - 2.10.c 8-bit vs 16-bit per-channel precision

**3. Color Picker dialog (full)** _(Stage 1 — core/MVP)_ 🟡 (partial)
- 3.1 Spectrum field + vertical hue slider (default Illustrator picker) — large SB square + hue bar _(Stage 1)_
- 3.2 Color wheel + triangle alternative (Affinity-style HSB wheel) — ring = hue, inner triangle = sat/brightness _(Stage 2 — standard)_
  - 3.2.a Toggle between wheel / spectrum-box / sliders-only layouts
- 3.3 Live numeric fields for ALL models simultaneously (HSB, RGB, Lab, CMYK, Hex) updating in lockstep _(Stage 1 core: HSB+RGB+Hex; Stage 2: CMYK; Stage 3: Lab)_
- 3.4 New vs Current color comparison swatches (top half new, bottom half current) _(Stage 1)_
- 3.5 "Only Web Colors" filter checkbox; out-of-gamut + out-of-web cube/triangle alert swatches _(Stage 3)_
- 3.6 "Color Swatches" toggle inside picker — browse/select existing swatches without leaving dialog _(Stage 2)_
- 3.7 Eyedropper button inside the picker to sample from anywhere on screen _(Stage 2)_
- 3.8 Add-to-swatches button from within picker _(Stage 2)_
- 3.9 Recently used strip at bottom of picker _(Stage 2)_
- 3.10 Keyboard: arrow-nudge values, Tab between fields, Enter to commit, Esc to cancel _(Stage 1)_

**4. Color panel (docked inspector)** _(Stage 1 — core/MVP)_ 🟡 (partial — chips exist in inspector)
- 4.1 Fill/Stroke chips mirrored at panel top-left _(Stage 1)_
- 4.2 Active model's slider stack with numeric entry per channel + live gradient-fill on each slider track _(Stage 1: HSB/RGB/Hex)_
- 4.3 Hex field with copy/paste _(Stage 1)_
- 4.4 Spectrum/ramp bar at bottom — click-drag to pick; CMYK ramp shows tint bar _(Stage 1)_
  - 4.4.a None / White / Black quick chips at ends of the ramp
- 4.5 Panel flyout menu — Grayscale / RGB / HSB / CMYK / Lab / Web-safe RGB / Invert / Complement / Add to Swatches / Show Options _(Stage 2)_
- 4.6 Show/Hide Options (collapse to ramp-only vs full sliders) _(Stage 2)_
- 4.7 Out-of-gamut + out-of-web warning indicators with one-click "snap to nearest" _(Stage 3)_
- 4.8 Tint slider for global/spot swatches (single 0–100% tint when a global color is selected) _(Stage 2)_
- 4.9 Invert / Complement quick actions _(Stage 2)_

**5. Swatches panel** _(Stage 2 — standard)_
- 5.1 Swatch grid/list views — toggle thumbnail size (small/large) and list-with-names _(Stage 2)_
- 5.2 Swatch types & their badges _(Stage 2)_
  - 5.2.a Process color — plain CMYK/RGB swatch, non-global (editing swatch does NOT update placed art)
  - 5.2.b Global process color — corner-dot/triangle badge; edit once → updates every object using it (tints supported)
  - 5.2.c Spot color — dot-in-corner + spot badge; named ink (Pantone-like), prints on own plate, supports tint %
  - 5.2.d Registration swatch — special "prints on all plates" swatch (default present)
  - 5.2.e None / White / Black default swatches (locked, always present)
  - 5.2.f Gradient swatches — store gradients as named swatches
  - 5.2.g Pattern swatches — tile fills (place when pattern system exists) _(Stage 3)_
- 5.3 Swatch groups (color groups) — named folders; create from selection; drag to reorder/regroup _(Stage 2)_
  - 5.3.a A color group folder icon; expand/collapse; group can be sent to Color Guide / Recolor
- 5.4 New Swatch / New Group / Duplicate / Delete / Merge swatches toolbar (bottom of panel) _(Stage 2)_
  - 5.4.a New Swatch dialog — name, color type (process/global/spot), color mode, channel values, "Global" checkbox
  - 5.4.b Merge — combine selected; first selected's definition wins; art reassigned
  - 5.4.c Select All Unused → delete to clean up
- 5.5 Sort — by name, by hue/kind, or manual drag order _(Stage 2)_
- 5.6 Show-Find field / filter; "Show" dropdown (All / Color / Gradient / Pattern / Color Groups) _(Stage 2)_
- 5.7 Double-click swatch → Swatch Options dialog (edit name/type/values; live "Preview") _(Stage 2)_
- 5.8 Apply on click to active Fill/Stroke target; drag swatch to canvas object _(Stage 2)_
- 5.9 Add Used Colors / Add Selected Colors — harvest colors from artwork into swatches _(Stage 2)_
- 5.10 Swatch Libraries (Stage 3 — advanced)
  - 5.10.a Open Library dropdown — built-in libraries (color books, gradients, patterns, harmonies)
  - 5.10.b Brand/ink books (Pantone-style spot libraries) — licensing note; ship neutral/open sets first
  - 5.10.c User libraries — Save Swatch Library as `.varos`-swatches / export as ASE (Adobe Swatch Exchange) / GPL (GIMP) / ACO _(import + export ASE = key interop)
  - 5.10.d Persistent library panel that stays open across documents; "Add to Swatches" from a library
  - 5.10.e Library as separate floating panel vs dropdown inside Swatches
- 5.11 Document swatches saved IN the file; new-document default swatch set / templates _(Stage 2)_

**6. Gradients** _(Stage 2 — standard)_
- 6.1 Gradient types _(Stage 2)_
  - 6.1.a Linear — angle-defined axis
  - 6.1.b Radial — center, with optional aspect-ratio (elliptical) and focal-point offset (highlight)
  - 6.1.c Freeform / mesh-like gradient (points + lines mode) — drop color points anywhere _(Stage 3 — advanced)_
  - 6.1.d Conic / angular gradient (Affinity has it) _(Stage 3 — advanced)_
- 6.2 Gradient panel contents _(Stage 2)_
  - 6.2.a Type selector (Linear/Radial/Freeform), big gradient preview thumbnail
  - 6.2.b The gradient slider/ramp — add stops by clicking, remove by dragging off
  - 6.2.c Per-stop: color (opens picker / assigns swatch), location % field, opacity % (alpha stop)
  - 6.2.d Midpoint diamonds between stops — drag or type 0–100% to bias the blend
  - 6.2.e Angle field + dial (linear); aspect-ratio % field (radial ellipse)
  - 6.2.f Reverse-gradient button; Stops count
  - 6.2.g Apply gradient to Fill vs Stroke selector
  - 6.2.h Stroke-gradient mode (only when Stroke active): "within stroke / along stroke / across stroke"
- 6.3 Gradient stops detail _(Stage 2)_
  - 6.3.a Stop color via picker, double-click stop, or drag swatch onto stop
  - 6.3.b Opacity (alpha) per stop for fades to transparent
  - 6.3.c Add/delete/duplicate stop; Alt-drag to copy a stop; even-distribute stops
  - 6.3.d Assign a global/spot swatch to a stop (stays linked)
- 6.4 On-canvas Gradient tool (`G`) _(Stage 2)_
  - 6.4.a Drag to set direction, length, and angle; Shift constrains to 45°
  - 6.4.b Interactive gradient annotator — the bar with editable stops/midpoints directly on art
  - 6.4.c Radial: drag the dotted ring to resize, the small dot to move center, square to set ellipse/aspect, focal dot for highlight
  - 6.4.d Apply one gradient across multiple selected objects (drag spanning the whole selection)
  - 6.4.e Toggle annotator visibility (View > Hide Gradient Annotator)
- 6.5 Gradient on stroke + gradient mesh interplay; expand-gradient-to-mesh _(Stage 3 — advanced)_
- 6.6 Save current gradient as a gradient swatch; gradient libraries _(Stage 2)_
- 6.7 Color interpolation space option (RGB vs Lab vs perceptual) for smoother blends _(Stage 3 — advanced)_

**7. Opacity / alpha (color-level)** _(Stage 1 — core/MVP, basic)_ 🟡 (partial)
- 7.1 Global object opacity 0–100% (Transparency panel / inspector field) _(Stage 1)_
- 7.2 Independent Fill opacity vs Stroke opacity (appearance-level) _(Stage 2 — standard)_
- 7.3 Per-gradient-stop alpha (see 6.3.b) _(Stage 2)_
- 7.4 Alpha embedded in color value (RGBA / hex8 `#RRGGBBAA`) input _(Stage 2)_
- 7.5 Blend modes dropdown (Normal, Multiply, Screen, Overlay, etc.) — lives with opacity _(Stage 3 — advanced)_
- 7.6 Opacity masks / knockout group _(Stage 3 — advanced)_

**8. Eyedropper / color sampling** _(Stage 1 — core/MVP)_ ✅ (have)
- 8.1 Sample a color from any object → apply to selected/active target _(Stage 1)_
- 8.2 Sample from anywhere on screen (outside the app window) — hold to pick desktop pixels _(Stage 2 — standard)_
- 8.3 Alt/Option-click — apply sampled color FROM the eyedropper TO the target (reverse direction) _(Stage 2)_
- 8.4 Shift-click — sample only the fill color (not full appearance) _(Stage 2)_
- 8.5 Eyedropper Options (double-click tool) — choose what is sampled/applied: fill, stroke, color, opacity, stroke weight, type attributes, transparency, etc. _(Stage 3 — advanced)_
- 8.6 Sample-area size (point vs 3×3 / 5×5 average) for raster sampling _(Stage 3 — advanced)_
- 8.7 Sample a gradient or pattern, not just flat color _(Stage 3 — advanced)_

**9. Color Guide / harmony rules** _(Stage 3 — advanced)_
- 9.1 Color Guide panel — base color → live harmony swatch sets _(Stage 3)_
- 9.2 Harmony rules dropdown — Complementary, Analogous, Triadic, Tetradic/Compound, Split-Complementary, Monochromatic, Shades, High-Contrast, Pentagram, etc. _(Stage 3)_
- 9.3 Tints/Shades, Warm/Cool, Vivid/Muted variation columns (rows of lighter/darker) _(Stage 3)_
- 9.4 Set base color from active fill; "Save group to Swatches" _(Stage 3)_
- 9.5 Limit harmony output to a chosen swatch library/book _(Stage 3)_
- 9.6 Edit-Colors handoff button (open the generated group in Recolor) _(Stage 3)_

**10. Recolor Artwork** _(Stage 3 — advanced)_
- 10.1 Recolor dialog/panel launched from selection _(Stage 3)_
- 10.2 Color wheel (smooth/segmented) showing all current colors as handles; drag to remap _(Stage 3)_
  - 10.2.a Link/unlink harmony handles (move all together vs individually)
  - 10.2.b Brightness slider for the whole set
- 10.3 Edit vs Assign tabs _(Stage 3)_
  - 10.3.a Assign — current colors → new colors mapping table; reduce/merge to N colors; randomize
  - 10.3.b Color reduction (limit to 1..N colors) + presets
- 10.4 Recolor with a swatch library / harmony rule preset _(Stage 3)_
- 10.5 Preserve/exclude specific colors; preserve black/white/spots; sort by hue _(Stage 3)_
- 10.6 Live preview on canvas; "Recolor Art" checkbox; randomize hue/brightness buttons _(Stage 3)_
- 10.7 Generative / quick "apply palette" one-click presets _(Stage 3)_

**11. Persistence, interop & document integration** _(Stage 2 — standard)_
- 11.1 Colors/swatches/gradients serialized into the `.varos` document schema (single-schema RNA) _(Stage 2)_
- 11.2 Import/Export: ASE (Adobe Swatch Exchange), GPL (GIMP), ACO, plain hex list, CSS variables _(Stage 3 — advanced; ASE first)_
- 11.3 Recently-used colors history (per session + persisted) _(Stage 2)_
- 11.4 Global-color relink — editing a global/spot swatch re-renders all dependent art & gradient stops _(Stage 2)_
- 11.5 Copy/paste color between objects (Cmd/Ctrl-C of attributes) & "paste appearance" _(Stage 2)_
- 11.6 Undo/redo coverage for every color/swatch/gradient edit ✅ (have undo/redo) _(Stage 1)_
- 11.7 Color variables / tokens (named, document-wide, library-shareable) for design-system use _(Stage 3 — advanced)_

**12. Accessibility, theming & ergonomics** _(Stage 3 — advanced)_
- 12.1 Contrast checker / WCAG ratio readout between two chosen colors _(Stage 3)_
- 12.2 Colorblind-simulation preview of the artwork palette _(Stage 3)_
- 12.3 Numeric scrubbing (drag on field labels), arrow-key nudge, copy-as (hex/rgb/css) on any swatch _(Stage 2)_
- 12.4 Frosted-glass floating Color/Swatches/Gradient panels consistent with Varos GPU UI shell; dockable/tear-off _(Stage 2)_
- 12.5 Keyboard-first: shortcuts for `<`/`>`/`/`, `X`, `Shift+X`, `D`, `G`, `I` (eyedropper) wired to the modeless engine _(Stage 1 for the core ones)_

---

## 2. Stroke system
*Everything that controls how a path's outline is drawn — weight, caps, joins, alignment, dashes, arrowheads, variable width, and stroke-to-fill order — plus the Stroke panel and Width tool.*

**1. Stroke data model & foundations** _(Stage 1 — core/MVP)_
- 1.1 Stroke as a per-object attribute — every path/shape/text outline carries 0..n strokes; ✅ (have basic single-color apply)
  - 1.1.a Stroke must store: enabled flag, paint (color/gradient/pattern ref), weight, cap, join, miter limit, alignment, dash array, dash offset, arrowheads, width-profile, scale-with-object flag, blend mode, opacity
  - 1.1.b Stroke geometry derives from the path centerline + these attributes — never baked into the path until Outline Stroke
  - 1.1.c Zero-weight or disabled stroke = no render but attribute retained (so toggling back restores all settings)
- 1.2 Single vs multiple strokes _(Stage 3 — advanced; Affinity allows 1, Illustrator stacks many via Appearance)_
  - 1.2.a Stage 1 ships ONE stroke per object; multi-stroke stacking deferred to an Appearance system
  - 1.2.b Each stacked stroke = independent full attribute set, painted bottom-to-top
- 1.3 Units & precision — weight in document unit (px/pt/mm/in), respects ruler unit; sub-pixel weights allowed (e.g. 0.25)
  - 1.3.a Min weight 0 (hairline behavior = smallest renderable), practical max generous (e.g. 1000)
  - 1.3.b Hairline option (Affinity) — render at a fixed device-pixel width regardless of zoom _(Stage 3)_
- 1.4 Default stroke — new shapes inherit current default (e.g. 1px black, butt cap, miter join, center align); ✅ (have default fill/stroke)
- 1.5 Rendering correctness — stroke must tessellate via the geometry core (offsetting/stroking the centerline); caps/joins/dashes all GPU-drawn, never DOM

**2. Weight (thickness)** _(Stage 1 — core/MVP)_ ✅ (have basic)
- 2.1 Weight field in Stroke panel — numeric entry + unit suffix
  - 2.1.a Stepper arrows (up/down by 1, or 0.25 with modifier)
  - 2.1.b Scrub/drag-on-label to change value; ↑/↓ arrow keys nudge (Shift = ×10 step)
  - 2.1.c Dropdown of common presets (0.25, 0.5, 1, 2, 3, 4, 5, 8, 10, 12, 16, 20…)
- 2.2 Live preview while dragging — weight updates canvas in real time
- 2.3 Multi-select behavior — shows blank/"mixed" when values differ; setting applies to all
- 2.4 Keyboard shortcuts — increase/decrease stroke weight (Illustrator default ⌘/Ctrl no, but assign e.g. `]`/`[` or panel-focused arrows)
- 2.5 Negative/zero handling — clamp at 0; 0 = effectively no stroke drawn

**3. Caps (line ends)** _(Stage 1 — core/MVP)_
- 3.1 Butt cap — stroke ends exactly at endpoint, flat, no extension (default)
- 3.2 Round cap — semicircle of radius = ½ weight added beyond each endpoint
- 3.3 Projecting (square) cap — square extends ½ weight beyond endpoint
- 3.4 UI — 3 toggle buttons (icon set) in Stroke panel, single-choice
- 3.5 Applies to open-path ends AND to each dash segment end (interacts with dash rendering)
- 3.6 Cap affects bounding box / visual length — projecting/round lengthen the visible line by one weight total

**4. Joins (corners)** _(Stage 1 — core/MVP)_
- 4.1 Miter join — sharp extended corner; default
- 4.2 Round join — corner rounded by arc of radius ½ weight
- 4.3 Bevel join — corner flattened/chamfered
- 4.4 UI — 3 toggle buttons in Stroke panel, single-choice
- 4.5 Miter limit _(Stage 1)_ — numeric ratio (default 10, range ~1–500); when miter length ÷ stroke weight exceeds limit, join auto-falls back to bevel to avoid spikes on sharp angles
  - 4.5.a Field greyed/disabled unless miter join selected
  - 4.5.b "×" suffix indicating ratio
- 4.6 Join applies at every corner anchor AND at closing point of closed paths
- 4.7 Smooth (curve) anchors don't trigger joins — only true corner anchors do

**5. Alignment / position of stroke** _(Stage 1 — core/MVP)_
- 5.1 Align Stroke to Center — stroke straddles the path (½ in / ½ out); default, only option that works cleanly on open paths
- 5.2 Align Stroke to Inside — full weight grows inward from the path
- 5.3 Align Stroke to Outside — full weight grows outward from the path
- 5.4 UI — 3 toggle buttons in Stroke panel
- 5.5 Constraints
  - 5.5.a Inside/Outside only valid on CLOSED paths (defines an inside) — for open paths the buttons disable or fall back to center
  - 5.5.b Text objects: alignment may force-disable in IL until outlined
- 5.6 Implementation — Inside/Outside = offset the stroke centerline by ±½ weight then stroke at center; must use the geometry/offset core (Clipper2 / bezier offset)
- 5.7 Affects bounding box, snapping bounds, and align/distribute "use bounds" option

**6. Dashed lines (dash pattern)** _(Stage 2 — standard)_
- 6.1 Dashed toggle — checkbox enabling the dash array
- 6.2 Dash/Gap fields — up to 3 dash + 3 gap pairs (Illustrator: dash1/gap1…dash3/gap3 = 6 fields)
  - 6.2.a Any subset usable (e.g. just dash1+gap1 = even dashes)
  - 6.2.b Values in document units; 0 dash + round cap = dotted line
- 6.3 Dash offset / phase — start the pattern partway in (field; enables marching-ants-free positioning)
- 6.4 Dash alignment to corners & ends — two modes (Illustrator buttons):
  - 6.4.a "Preserves exact dash & gap lengths" — pattern as typed, may break at corners
  - 6.4.b "Aligns dashes to corners & path ends, adjusting lengths" — auto-stretches gaps so a dash lands on every corner/endpoint (cleaner)
- 6.5 Caps interact — round/projecting caps round/extend each dash; butt = crisp rectangles
- 6.6 Dashes follow width profile & arrowheads correctly
- 6.7 Live preview; dash array stored as part of stroke attribute
- 6.8 Edge cases — total pattern length 0 → solid; negative clamp; very small dashes vs weight

**7. Arrowheads** _(Stage 2 — standard)_
- 7.1 Start & End pickers — independent dropdowns, each a gallery of arrowhead shapes
  - 7.1.a Library: none + arrows (filled/open/curved), bars, circles, squares, diamonds, feathers, crowfoot/CAD ends (~20–40 presets)
  - 7.1.b "None" default both ends
- 7.2 Swap start↔end button — flips which marker is on which end
- 7.3 Scale — independent start/end scale % (default 100); link toggle to scale both together
- 7.4 Flip start / Flip end — reverse arrowhead direction along the path (two buttons)
- 7.5 Align/tip placement — two buttons: "extend tip beyond end of path" vs "place tip at end of path" (Illustrator) controlling whether the path retracts to the arrow base
- 7.6 Arrowheads inherit stroke color and (proportionally) weight unless scaled independently
- 7.7 Rotation — arrowhead auto-orients to the path tangent at the endpoint
- 7.8 Custom arrowheads _(Stage 3)_ — define a symbol/path as a reusable arrowhead marker
- 7.9 Only meaningful on open path ends (and per-end); closed paths show none

**8. Variable width / width profiles + Width tool** _(Stage 3 — advanced)_
- 8.1 Width profiles dropdown in Stroke panel — gallery of preset profiles (Uniform, Width Profile 1–6: tapers, bulges, calligraphic-like)
  - 8.1.a Uniform = constant weight (default)
  - 8.1.b Apply profile = modulates weight along the path (0–200% of base) 
  - 8.1.c Flip profile along (horizontal) and across (vertical) buttons
- 8.2 Width tool (W) — dedicated tool to edit width points directly on the path
  - 8.2.a Hover shows width handles; drag a point outward = widen, inward = narrow
  - 8.2.b Add width point by dragging anywhere on the stroke
  - 8.2.c Drag symmetrically (both sides) or Option/Alt-drag = adjust one side only (asymmetric/offset width)
  - 8.2.d Move a width point along the path; delete (select + Delete)
  - 8.2.e Shift-drag to move width points discontinuously; double-click point = numeric Width Point Edit dialog (side1, side2, total width)
  - 8.2.f Continuous vs discontinuous width (two adjacent points = abrupt step)
- 8.3 Save custom profile — "Add to Profiles" stores current variable-width as a reusable named profile; delete/reset profile
- 8.4 Width data stored as a list of {t-position, leftWidth, rightWidth} along the path
- 8.5 Variable width composes with caps, joins, dashes, arrowheads, and Outline Stroke
- 8.6 Renderer must build a swept-outline (variable offset) ribbon, not a constant offset

**9. Pressure / pen input** _(Stage 3 — advanced)_
- 9.1 Pressure-sensitive width — tablet/stylus pressure maps to stroke width while drawing (pencil/brush/width contexts)
- 9.2 Pressure stored as a width profile on the resulting path (same data structure as §8)
- 9.3 Velocity/tilt mapping options _(Stage 3, optional)_ — speed→width, tilt→width
- 9.4 Min/max width range + pressure-curve sensitivity settings
- 9.5 Fallback for non-pressure devices — uniform width

**10. Stroke order relative to fill (paint stacking)** _(Stage 3 — advanced; needs Appearance)_
- 10.1 Default — stroke painted ABOVE fill (straddling center looks half-covered by inside half)
- 10.2 Option to paint stroke BELOW fill (Illustrator Appearance reorder) — makes inside half of a centered stroke hidden behind fill, only outer half shows
- 10.3 Multiple strokes/fills reorderable in an Appearance panel (drag to restack) _(Stage 3)_
- 10.4 Per-stroke blend mode & opacity _(Stage 3)_

**11. Scale stroke & effects with object** _(Stage 2 — standard)_
- 11.1 "Scale strokes & effects" preference/toggle — when ON, scaling an object by N% multiplies stroke weight (and dash, arrowhead) by N%
  - 11.1.a When OFF, geometry scales but stroke weight stays constant
  - 11.1.b Global pref (Transform/Preferences) AND a per-transform/Transform-panel flyout option
- 11.2 Non-uniform scale — stroke weight handling (average or axis-aware) when scaled unevenly
- 11.3 Interacts with width profiles (profile scales proportionally) and dashes

**12. Outline Stroke (expand stroke to filled shape)** _(Stage 2 — standard)_
- 12.1 Command (Object ▸ Path ▸ Outline Stroke) — converts the stroke ribbon into a standalone filled closed path
  - 12.1.a Result fill = former stroke paint; original fill becomes a separate object/compound
  - 12.1.b Honors weight, caps, joins, miter, alignment, dashes (each dash → separate subpath), arrowheads, AND variable width
- 12.2 Output grouped (stroke-outline + fill) or compound path as appropriate
- 12.3 Needs the offset/stroke-to-outline core (bezier offsetting + Clipper2 boolean to merge self-overlaps)
- 12.4 Irreversible (beyond undo) — original live stroke attributes lost
- 12.5 Use cases — booleans on strokes, gradient-on-stroke workarounds, export fidelity

**13. Stroke paint type** _(Stage 1 solid; Stage 2/3 gradient & pattern)_
- 13.1 Solid color stroke — via Fill/Stroke system; ✅ (have basic apply)
- 13.2 Stroke/Fill swap & X-default already exist; ✅ (have swap/default)
- 13.3 Gradient on stroke _(Stage 3)_ — apply gradient along stroke / across stroke / within stroke (3 modes, Illustrator)
- 13.4 Pattern on stroke _(Stage 3)_
- 13.5 None — toggling stroke paint to none disables render but keeps attributes

**14. The Stroke panel (contents & layout)** _(Stage 1 core, fields added per stage)_
- 14.1 Weight field + stepper + presets _(Stage 1)_ ✅
- 14.2 Cap: 3 buttons (butt / round / projecting) _(Stage 1)_
- 14.3 Join: 3 buttons (miter / round / bevel) _(Stage 1)_
- 14.4 Miter limit field (× ratio) _(Stage 1)_
- 14.5 Align stroke: 3 buttons (center / inside / outside) _(Stage 1)_
- 14.6 Dashed line checkbox + 6 dash/gap fields + offset + 2 corner-align buttons _(Stage 2)_
- 14.7 Arrowheads: start picker, end picker, swap, 2 scale fields + link, 2 flip buttons, 2 tip-align buttons _(Stage 2)_
- 14.8 Width-profile dropdown + flip-along / flip-across buttons _(Stage 3)_
- 14.9 Panel header overflow menu — Show Options (collapse to just weight), panel options
- 14.10 "Mixed" state display for multi-selection
- 14.11 Inline access from the Inspector (right panel) per Varos UI — stroke section with the above, expand for full options
- 14.12 Live numeric scrubbing on all labels; tab between fields; enter commits

**15. Selection, multi-edit & state behavior** _(Stage 1 — core/MVP)_
- 15.1 Reflect current selection's stroke attributes when one/many selected
- 15.2 Apply edits to all selected objects; partial values show mixed/blank
- 15.3 No selection — edits set the tool default for next drawn object
- 15.4 Eyedropper picks up full stroke attributes (weight/cap/join/dash/etc.); ✅ (have basic eyedropper) — extend to copy all stroke props
- 15.5 Undo/redo covers every stroke attribute change; ✅ (have undo/redo)

**16. Edge cases & interactions** _(Stage 2–3)_
- 16.1 Open vs closed path — disables inside/outside align & arrowheads-on-closed correctly
- 16.2 Very thick stroke vs tiny path — self-intersection handled by offset core; Outline Stroke must clean self-overlap
- 16.3 Stroke + boolean ops — strokes ignored by Pathfinder (operate on fills) unless outlined first; surface a hint/auto-outline option
- 16.4 Stroke + transform (rotate/scale already shipped) — respect scale-with-object flag; rotation keeps weight
- 16.5 Stroke on groups — applies to members (or as group appearance in Stage 3)
- 16.6 Zoom-independent rendering — weight in document units scales with zoom; hairline mode is the exception
- 16.7 Export fidelity — SVG (stroke-width, stroke-linecap, stroke-linejoin, stroke-miterlimit, stroke-dasharray, stroke-dashoffset, vector-effect for non-scaling); markers for arrowheads; variable width & inside/outside often require outlining on export — provide auto-outline-on-export option
- 16.8 Snapping/measurement — stroke alignment changes visual bounds used by smart guides/align

---

## 3. Transform system
*Exact numeric and tool-driven transforms — X/Y/W/H, rotate, shear, flip, the 9-point reference, dedicated Move/Rotate/Scale/Reflect/Shear tools with pivots and Alt-copy, Transform Each / Again / Free Transform, plus Offset Path — all wired to one Transform panel and the control-bar fields.*

> ⓘ AMENDED [D7]: build **together with Snapping (§6)** — precise transform + drawing need snap to feel right. And §9.3 handle-coupling is RECONCILED to Varos's verified model (see below).

**0. Foundations & shared model** _(Stage 1 — core/MVP)_
- 0.1 One transform pipeline — every numeric field, dialog, and tool feeds ONE affine transform applied to the selection (translate · rotate · scale · shear). bbox drag handles ✅ (have basic) already write into this; numeric entry 🟡 (missing) is the gap this system closes.
- 0.2 Selection bbox source — transforms act on the selection's bounding box; define which box (see 5) so W/H/X/Y are unambiguous.
- 0.3 Reference point (origin) — all transforms pivot around the active reference point; default = bbox center for tools, top-left for X/Y readout (Illustrator convention). One shared origin concept across panel + tools.
- 0.4 Single transaction = one undo step — a full gesture (drag, dialog OK, Enter on a field) commits exactly one undo entry; live preview updates are not separate undo steps. undo/redo ✅ exists, wire transforms to it.
- 0.5 Units & coordinate space — fields accept the document unit (px/pt/mm/cm/in) with on-the-fly unit suffix parsing ("10mm"); internal math in document points; angle in degrees, CCW-positive (match Illustrator's visual CCW); document origin top-left, Y-down (note IL shows Y-down ruler).
- 0.6 Multi-object selection — transform the combined bbox as a rigid group by default (objects move/scale together), distinct from Transform Each (5 below) which transforms each object about its own origin.
- 0.7 Precision & rounding — store full float, display rounded to unit precision (configurable decimals); never accumulate rounding error across repeated transforms (compose from original, not from displayed value).

**1. Transform panel** _(Stage 1 — core/MVP)_ — the driving panel for this system; ship the tool WITH this panel.
- 1.1 9-point reference-point proxy — 3×3 grid of clickable points (corners, edge-midpoints, center); the lit point is the origin for W/H scaling and rotate/shear from the panel; X/Y readout reports the coordinate OF that point.
  - 1.1.a Clicking a different proxy point re-reads X/Y for that point WITHOUT moving the object (readout-only switch).
  - 1.1.b The chosen reference persists as the session default for new selections (Illustrator remembers it).
- 1.2 X field — horizontal position of the reference point; type to move; arrow-up/down to step; Tab to next field.
- 1.3 Y field — vertical position of the reference point; same entry rules.
- 1.4 W field — bounding-box width; typing scales horizontally about the reference point.
- 1.5 H field — bounding-box height; scales vertically about the reference point.
- 1.6 Constrain-proportions link (chain icon) — when on, editing W scales H proportionally and vice-versa; also governs corner-handle drags. 🟡 (missing as a toggle).
- 1.7 Rotate field — absolute/relative angle entry; rotates about the reference point; accepts negative + values >360.
- 1.8 Shear field — slant angle entry; shears about the reference point (default horizontal axis; advanced axis control in dialog, see 4.5).
- 1.9 Field input mechanics _(Stage 1)_ — Enter commits & keeps focus, Tab commits & advances, Esc reverts; up/down arrow steps by 1 (Shift = 10); supports inline math ("100/2", "50+10mm"); scrubby-slider drag on the field label to nudge (Stage 2).
- 1.10 Panel flyout menu options:
  - 1.10.a Flip Horizontal / Flip Vertical _(Stage 1)_ — mirror about the reference point.
  - 1.10.b Scale Strokes & Effects (toggle) _(Stage 2)_ — when scaling, multiply stroke width / effect sizes by the scale factor; off = geometry only. Mirrors the global pref (see 7).
  - 1.10.c Align to (Pixel) Grid / Transform Object Only / Transform Pattern Only / Transform Both _(Stage 3)_ — controls whether a fill pattern transforms with the object.
- 1.11 Live readout while dragging — bbox handle drags ✅ update these fields in real time; numeric fields update during tool drags too (two-way binding).
- 1.12 Empty / mixed states — no selection = fields blank/disabled; multi-selection shows combined bbox values; mixed rotation shows blank rotate field (typing applies absolutely to all).
- 1.13 Panel chrome — collapsible, dockable in the inspector, matches the floating frosted panel shell; reachable via Window ▸ Transform (Shift+F8).

**2. Top control-bar numeric fields** _(Stage 1 — core/MVP)_ — the always-visible quick-transform strip; mirror of the panel.
- 2.1 Mini reference-point proxy + X / Y / W / H inline — same semantics as panel, condensed into the control bar when a selection exists.
- 2.2 Rotate + Flip H / Flip V buttons inline — one-click common ops without opening the panel.
- 2.3 Constrain-link toggle inline — duplicates 1.6 in the bar.
- 2.4 Context-sensitivity — fields appear only when an object is selected; swap to type/shape-specific controls when those tools are active (shapes ✅ exist — rectangle shows corner-radius etc.; transform fields remain present).
- 2.5 Bind to same model as panel — editing in the bar updates the panel and vice-versa (one source of truth).

**3. Reference point / pivot system** _(Stage 1 — core/MVP)_
- 3.1 Panel proxy (9-point) — discrete origin selection for panel/control-bar math (see 1.1).
- 3.2 Tool pivot crosshair — for R/S/O/Shear tools, a draggable pivot glyph rendered on canvas; default at bbox center.
- 3.3 Click-to-set-pivot — with a transform tool active, a single click relocates the pivot to that point (then the next drag transforms about it). Alt+click sets pivot AND opens the exact-value dialog (Illustrator behavior).
- 3.4 Snap pivot — pivot snaps to anchors, bbox handles, path intersections, smart-guide points (align & distribute ✅ has the geometry; reuse snap targets).
- 3.5 Pivot persistence — pivot stays put across successive transforms until moved or selection changes; resets to center on new selection.
- 3.6 Keyboard origin override — hold a modifier (e.g. Alt) during a panel transform to flip origin to opposite corner momentarily (Stage 3 nicety).

**4. Dedicated transform tools** _(modeless behaviors + tool variants; pivot + Alt-copy + double-click dialog)_
- 4.1 Move (drag + exact dialog) _(Stage 1)_ — bbox/object drag ✅ exists; ADD: Enter (or double-click Selection tool / Object▸Transform▸Move) opens Move dialog.
  - 4.1.a Move dialog fields: Horizontal, Vertical, Distance, Angle (the four are interlocked — distance/angle derive from H/V); Preview checkbox; Copy button (Alt-Enter) to move a duplicate.
  - 4.1.b Arrow-key nudge — move selection by keyboard increment; Shift = 10×; increment set in prefs (reuse anchor nudge wiring already speced).
  - 4.1.c Alt-drag = move a copy; Shift = constrain to 45° axes.
- 4.2 Rotate tool (R) _(Stage 1)_ — rotate selection about pivot.
  - 4.2.a Click sets pivot, drag rotates; Shift constrains to 45° increments.
  - 4.2.b Alt+click pivot → Rotate dialog (Angle, Preview, Copy; Transform Objects / Transform Patterns checkboxes).
  - 4.2.c Alt-drag = rotate a copy (foundation for radial step-and-repeat via Transform Again).
  - 4.2.d Double-click the tool icon = rotate about bbox center via dialog.
  - 4.2.e bbox outside-corner rotate ✅ exists — keep as the V-tool shortcut; R tool adds explicit pivot.
- 4.3 Scale tool (S) _(Stage 1)_ — scale about pivot.
  - 4.3.a Click pivot, drag scales; Shift = constrain proportional (or constrain to one axis depending on drag direction); Alt-drag = scale a copy.
  - 4.3.b Scale dialog: Uniform (%) OR Non-Uniform (Horizontal % / Vertical %); Scale Strokes & Effects checkbox; Transform Objects / Patterns; Preview; Copy.
  - 4.3.c Double-click tool = dialog about center. bbox corner/edge scale ✅ exists as V-tool shortcut.
- 4.4 Reflect tool (O) _(Stage 1)_ — mirror across an axis line.
  - 4.4.a Click sets a point on the mirror axis, second click/drag defines axis angle → reflects; Shift constrains axis to 45°.
  - 4.4.b Alt-drag = reflect a copy (mirror-duplicate, very common).
  - 4.4.c Reflect dialog: Axis (Horizontal / Vertical / Angle°); Transform Objects/Patterns; Preview; Copy.
  - 4.4.d Flip H / Flip V (panel + control bar, 1.10.a) = the no-dialog fast path of Reflect about the reference point.
- 4.5 Shear tool _(Stage 2)_ — slant about pivot.
  - 4.5.a Click pivot, drag shears; Shift constrains shear axis.
  - 4.5.b Shear dialog: Shear Angle, Axis (Horizontal / Vertical / Angle°); Transform Objects/Patterns; Preview; Copy.
- 4.6 Shared tool conventions _(Stage 1 where the tool is Stage 1)_:
  - 4.6.a Alt held at gesture START = operate on a copy; the original stays.
  - 4.6.b Enter while tool active (no pivot click) = open that tool's dialog about current/default origin.
  - 4.6.c Esc cancels an in-progress drag (no commit); release commits one undo step.
  - 4.6.d Live ghost/outline preview during drag; Preview checkbox in dialogs shows result before OK.
  - 4.6.e Modeless: these tools transform the persistent selection; switching among V/R/S/O never clears selection (matches the V↔A modeless rule already in the spec).

**5. Transform Each** _(Stage 2 — standard)_ — Object ▸ Transform ▸ Transform Each (Alt+Shift+Ctrl+D).
- 5.1 Per-object origin — transforms EACH selected object about ITS OWN reference point simultaneously (vs. the combined-bbox behavior of normal transforms).
- 5.2 Dialog sections — Scale (Horizontal %, Vertical %), Move (Horizontal, Vertical), Rotate (Angle), plus Reflect X / Reflect Y checkboxes.
- 5.3 Random checkbox — randomizes each value within ±the entered amount per object (scatter/organic layouts).
- 5.4 9-point reference proxy inside the dialog — sets each object's local origin.
- 5.5 Options — Scale Strokes & Effects, Transform Objects / Transform Patterns, Preview, Copy.

**6. Transform Again** _(Stage 2 — standard)_ — Object ▸ Transform ▸ Transform Again (Ctrl+D).
- 6.1 Repeat last transform — re-applies the most recent move/rotate/scale/reflect/shear (including the Copy variant) with identical parameters.
- 6.2 Step-and-repeat arrays — Alt-drag-copy once, then Ctrl+D repeatedly = linear array; rotate-copy once then Ctrl+D = radial array. Cornerstone workflow.
- 6.3 Remembers the transform delta AND whether it was a copy — Transform Again after a duplicate keeps duplicating.
- 6.4 Scope — last transform is tracked per document/session; cleared by non-transform edits.

**7. Scale Strokes & Effects (global + per-op)** _(Stage 2 — standard)_
- 7.1 Global preference — Edit/Preferences ▸ General ▸ "Scale Strokes & Effects": when on, any scale (handle drag, dialog, W/H field) multiplies stroke width and effect sizes by the scale factor. fill/stroke ✅ exists — strokes must read this flag.
- 7.2 Per-dialog override — Scale dialog and Transform Each expose the checkbox so it can be set per operation regardless of the global default.
- 7.3 Control-bar / panel toggle — surface it where scaling happens (1.10.b) so it's discoverable, not buried.
- 7.4 Effects — once an effects system exists, drop-shadow/blur radii scale too; for now scope to stroke width.

**8. Flip Horizontal / Vertical** _(Stage 1 — core/MVP)_
- 8.1 Flip H — mirror across the vertical axis through the reference point.
- 8.2 Flip V — mirror across the horizontal axis through the reference point.
- 8.3 Surfaces — panel flyout (1.10.a), control-bar buttons (2.2), Object ▸ Transform menu, and keyboard shortcuts.
- 8.4 Reference-point aware — flipping about center vs. an edge gives different positions; honors the active 9-point proxy.
- 8.5 In-place vs. copy — plain flip mirrors in place; Reflect tool Alt-drag (4.4.b) is the mirror-copy path.

**9. Constrain proportions (link)** _(Stage 1 — core/MVP)_ 🟡 (missing)
- 9.1 Link toggle state — chain icon in panel (1.6) and control bar (2.3); persists per session.
- 9.2 Field coupling — with link on, typing W recomputes H by the original aspect ratio (and vice-versa); locks the W:H ratio.
- 9.3 Handle coupling — ⚠️ **RECONCILED [D7] to Varos's locked, Ahmed-verified model: corner-handle drags couple PROPORTIONALLY by geometry, and `Alt` BREAKS the couple (frees the ratio). Do NOT implement Illustrator's "Shift = proportional" default — match the verified feel.** (Old Illustrator note, superseded: ~~Shift forces proportional regardless~~.)
- 9.4 Edge handles — mid-edge handles scale one axis only and ignore link (single-axis is intentional).

**10. Free Transform tool (E)** _(Stage 2 — standard; distort = Stage 3)_
- 10.1 Unified widget — one bbox handle set that does move (drag inside), scale (corner/edge), rotate (just outside corner) without switching tools.
- 10.2 Modifier-gated extras (Free Transform "touch widget" or modifier press):
  - 10.2.a Distort — Ctrl-drag a single corner moves it freely (skew the box into a quad). _(Stage 3)_
  - 10.2.b Perspective — Shift+Alt+Ctrl-drag a corner = symmetric perspective foreshortening. _(Stage 3)_
  - 10.2.c Shear — Ctrl-drag a mid-edge handle. _(Stage 3)_
  - 10.2.d Constrain — Shift = proportional scale / 45° rotate.
- 10.3 Free Distort relationship — Effect ▸ Distort & Transform ▸ Free Distort offers the same quad-corner distortion as a (non-destructive) effect; Free Transform is the direct/destructive version.

**11. Offset Path** _(Stage 2 — standard)_ — Object ▸ Path ▸ Offset Path (a path-generation transform).
- 11.1 Dialog — Offset (distance; +outward / −inward), Joins (Miter / Round / Bevel), Miter limit.
- 11.2 Geometry engine — build concentric inner/outer contours; reuse the boolean/Clipper2 offset capability (boolean engine ✅ exists — Clipper2 provides polygon offsetting).
- 11.3 Self-intersection handling — clean up overlaps on inward offset (collapsed regions removed); preserve winding.
- 11.4 Multiple / open paths — offset each subpath; open paths offset to a capped outline or to two parallel sides depending on convention; group result.
- 11.5 Live vs. destructive — Stage 2: destructive command producing a new path; Stage 3: as a live Effect (re-editable) once the effects/appearance system lands.
- 11.6 Result handling — new path(s) placed above original, selected, inheriting fill/stroke; original retained.

**12. Menu integration & shortcuts** _(Stage 1 for core ops, Stage 2 for the rest)_
- 12.1 Object ▸ Transform submenu — Move (Shift+Ctrl+M), Rotate, Scale, Reflect, Shear, Transform Each (Alt+Shift+Ctrl+D), Transform Again (Ctrl+D), Reset Bounding Box.
- 12.2 Tool shortcuts — R / S / O for Rotate / Scale / Reflect; E for Free Transform; Shear nested under Scale in the tool rail.
- 12.3 Reset Bounding Box _(Stage 2)_ — after rotating an object, its bbox stays rotated; this command re-aligns the bbox to the page axes (without un-rotating geometry).
- 12.4 Right-click context menu — Transform submenu on selected objects (mirror of 12.1).
- 12.5 Tool rail nesting — Rotate/Reflect grouped; Scale/Shear/Reshape grouped; Free Transform standalone; long-press to reveal flyout (matches existing tool-rail pattern).

**13. Canvas feedback & affordances** _(Stage 1 — core/MVP)_
- 13.1 Live HUD — show delta during drag (ΔX/ΔY for move, angle for rotate, % for scale) near the cursor.
- 13.2 Smart guides during transform — alignment/measurement guides snap and show distances (reuse align & distribute ✅ snapping infra).
- 13.3 Cursor glyphs — distinct cursors for move / rotate / scale / reflect / shear / pivot-set (consistent with the cursor set in UI_FIGMA_SPEC).
- 13.4 Pivot glyph rendering — crosshair/target at the active pivot; ghost outline of the result during drag and dialog Preview.
- 13.5 Rotated-bbox display — after rotation, handles render rotated with the object; W/H report the object's local (un-rotated) box, X/Y report in page space (Illustrator behavior — clarify and pick one consistently).

**14. Edge cases & correctness** _(Stage 2 — standard)_
- 14.1 Zero / negative dimensions — typing W=0 or negative: clamp or flip (negative W = horizontal flip, like Illustrator); prevent degenerate collapse.
- 14.2 Locked / hidden objects — excluded from transform; warn or skip (layers ✅ provides lock/hide state).
- 14.3 Groups & nested — transform applies to the whole group's combined box; nested children transform rigidly (groups/ungroup ✅ exists).
- 14.4 Strokes/effects with Scale-S&E off — geometry scales, stroke width constant (visible "thin where scaled up" look) — verify this is the toggle's actual behavior.
- 14.5 Compound paths / boolean results — transform as a unit; winding preserved (boolean engine ✅).
- 14.6 Rounding stability — repeated Ctrl+D must not drift; compose each repeat from the stored delta, not re-measured bbox (see 0.7).
- 14.7 Multi-select rotate origin — combined-bbox center by default; Transform Each for per-object (don't confuse the two).
- 14.8 Reference-point + flip interaction — confirm flip about a corner moves the object as expected; cover in tests.
- 14.9 Unit mismatch — entering "10mm" in a px document converts correctly; reject unparseable input and revert field.

**15. Build order summary** _(meta)_
- 15.1 Stage 1 (core/MVP) — Transform panel (9-point proxy, X/Y/W/H, rotate, shear fields) + control-bar fields wired two-way to the existing bbox; constrain-proportions link; Flip H/V; Move dialog + arrow-nudge + Alt-copy; Rotate/Scale/Reflect tools with click-pivot, Alt-copy, dialogs; canvas HUD + cursors. This closes the 🟡 numeric-entry gap and makes the existing bbox transforms precise.
- 15.2 Stage 2 (standard) — Shear tool; Transform Each; Transform Again (Ctrl+D arrays); Scale Strokes & Effects (global pref + per-op); Free Transform (move/scale/rotate); Offset Path (destructive); Reset Bounding Box.
- 15.3 Stage 3 (advanced) — Free Transform distort/perspective; live/effect Offset Path; pattern-transform options; scrubby sliders; random in Transform Each refinements; momentary origin-override modifiers.

---

## 4. Save / File system
*The native .varos file system — serialize/open/save the document model, with New/templates, autosave + crash recovery, version history, recent files, and Place/Import with linked vs embedded assets.*

> ⓘ AMENDED [D1]: the **serde spine** (derives on the model + round-trip test) moves to **Foundations §0**, built before any system. §4 here = the **container (zip/OPC) + native dialogs + dirty/recovery guards** over a schema that already round-trips. Split Stage 1 → **1a** (round-trip one `.varos` + Save/Open + dirty flag) and **1b** (dialogs, atomic write, .bak, new-doc preset); drop advisory file-locks + per-artboard thumbnails out of MVP.

**1. The `.varos` document format — on-disk container** _(Stage 1 — core/MVP)_
- 1.1 Container shape — a ZIP/OPC-style package (zip64-capable) renamed `.varos`, NOT a single flat blob; lets us stream-read large docs & swap parts without full rewrite
  - 1.1.a `mimetype` first entry, stored (uncompressed) — magic-bytes sniff so the OS/loader IDs a `.varos` even with wrong extension
  - 1.1.b `document.json` (or binary) — the serialized `Document` (paths, groups, group_of, ids) — 🟡 (partial): model struct exists in `model.rs`, but has ZERO serde — adding `#[derive(Serialize,Deserialize)]` across Anchor/Path/Group/Document/Rgba/Pt is the first concrete task
  - 1.1.c `manifest.json` — format version, app version, creator, created/modified timestamps, doc UUID, schema-version, feature-flags used (so an old build refuses gracefully)
  - 1.1.d `/assets/` folder — embedded raster/vector payloads (PNG/JPEG/SVG bytes), each keyed by content-hash for dedupe
  - 1.1.e `/thumbnails/` — `thumb.png` (document preview for OS file dialogs, Recent, "Open" grid) + per-artboard thumbs
  - 1.1.f `/previews/` — optional rasterized page previews for fast quick-look without loading the engine
- 1.2 Serialization foundation — the SINGLE schema is the spine (rides the property/RNA-style serialization layer the whole app shares)
  - 1.2.a Choose wire format: human-diffable JSON (Stage 1, debuggable) with a path to a compact binary (bincode/CBOR/MessagePack) for big files (Stage 3) — keep them isomorphic via the same serde types
  - 1.2.b Stable IDs persisted as-is — `Document.ids` counter + every `u32` id must round-trip so selection/links/history survive reload ✅ (have basic): ids already stable u32, not Vec indices — serialization-friendly by design
  - 1.2.c Float precision policy — store f32 verbatim (or f64 for coordinates to avoid drift); fix a rounding/locale rule (always `.` decimal, never comma)
  - 1.2.d Enum/bitflag stability — `ShapeKind`, smooth/closed/hidden/locked serialize by stable name/tag, never by ordinal, so reordering variants never corrupts old files
  - 1.2.e Compound paths & holes — `Path.holes` rings must round-trip; nested group hierarchy (`groups` + `parent` + `group_of`) must reload byte-identical
- 1.3 Versioning & forward/backward compat _(Stage 2 — standard)_
  - 1.3.a `schema_version` integer bumped on breaking change; loader runs an ordered chain of migrations old→current
  - 1.3.b Unknown-field tolerance — newer files opened in older app: preserve-and-passthrough unknown keys where safe, else refuse with a clear "made in a newer Varos" message
  - 1.3.c "Minimum reader version" in manifest so a too-old build won't silently drop data
  - 1.3.d Migration test corpus — golden `.varos` fixtures per version, round-trip + open-old tests in CI
- 1.4 Integrity & compression _(Stage 2 — standard)_
  - 1.4.a Per-entry CRC (zip native) + optional whole-doc checksum in manifest to detect truncation/bit-rot
  - 1.4.b Deflate level policy — fast for autosave, max for explicit Save; assets already-compressed (PNG/JPEG) stored, not re-deflated
  - 1.4.c Atomic write (see 3.5) so a crash mid-save never yields a half-zip
  - 1.4.d Optional file-level compression toggle in Save dialog (smaller vs faster) _(Stage 3 — advanced)_

**2. New document & New-from-template** _(Stage 1 — core/MVP)_
- 2.1 New (Ctrl/Cmd+N) — 🟡 (partial): "New" menu row exists in `ui.rs` but is a no-op stub; tab bar already supports multiple docs
  - 2.1.a New-document dialog — preset categories (Print, Web, Mobile, Social, Film/Video, Art & Illustration, custom)
  - 2.1.b Per-preset fields — width/height, units (px/pt/pc/in/mm/cm), orientation (portrait/landscape), number of artboards, artboard spacing/columns
  - 2.1.c Color mode (RGB/CMYK), raster effects resolution (72/150/300 ppi), bleed (top/right/bottom/left, linked toggle)
  - 2.1.d Default fill/stroke, background (transparent/white/custom), DPI metadata
  - 2.1.e Recent presets + "save current as preset"; remember last-used
- 2.2 Quick New — instant blank doc with last-used preset, no dialog (modifier/secondary action)
- 2.3 New from Template (Ctrl/Cmd+Shift+N) _(Stage 2 — standard)_
  - 2.3.a Template = a `.varost` (locked template variant) — opening it spawns an UNTITLED copy, never overwrites the template
  - 2.3.b Template browser — thumbnail grid, categories, search, "my templates" vs bundled
  - 2.3.c Bundled starter templates (business card, A4, social sizes, logo grid, icon sheet)
- 2.4 New from current selection / New from clipboard _(Stage 3 — advanced)_ — make a doc sized to the pasted/selected art

**3. Save / Save As / Save a Copy / Save as Template** _(Stage 1 — core/MVP)_
- 3.1 Save (Ctrl/Cmd+S) — 🟡 (partial): menu row + `Ctrl+S` label present in `ui.rs`, NO handler/serializer yet
  - 3.1.a First-ever save on an untitled doc routes to Save As (needs a path)
  - 3.1.b Subsequent saves overwrite the bound path silently, clear the dirty flag, refresh thumbnail/manifest timestamps
  - 3.1.c No-op fast-path — if not dirty, Save does nothing (and says so)
- 3.2 Save As (Ctrl/Cmd+Shift+S)
  - 3.2.a Native OS file dialog (Windows IFileSaveDialog) — filename, folder, type filter (`.varos`)
  - 3.2.b Rebinds the document to the NEW path & name; old file untouched; tab title + window title update
  - 3.2.c Filename collision → OS overwrite prompt; sanitize illegal chars; default name = artboard/last name or "Untitled-1"
- 3.3 Save a Copy (Ctrl/Cmd+Alt+S) — writes a snapshot to a new path but KEEPS editing the original (document stays bound to original path, dirty state unchanged)
- 3.4 Save as Template _(Stage 2 — standard)_ — writes `.varost`, optionally to the user templates dir so it appears in New-from-Template
- 3.5 Atomic / safe save mechanics _(Stage 1 — core/MVP)_
  - 3.5.a Write to temp sibling file → fsync → atomic rename over target (never truncate-in-place)
  - 3.5.b Keep one `.bak` of the previous version (toggle in prefs)
  - 3.5.c Handle read-only/locked file, disk-full, permission-denied, removable-drive-removed, path-too-long, network-path-offline — each a specific recoverable error, never silent data loss
  - 3.5.d File-lock / "already open elsewhere" advisory lock to avoid two instances clobbering
- 3.6 Dirty-state tracking & guards _(Stage 1 — core/MVP)_
  - 3.6.a Modified flag flips on any undoable mutation; title shows unsaved marker (•/asterisk), tab shows a dot
  - 3.6.b On close-tab / close-window / quit with unsaved → "Save / Don't Save / Cancel" prompt (batched "save all" on quit with multiple dirty docs)
  - 3.6.c Tie dirty flag to the undo system: undoing back to the last-saved state clears dirty
- 3.7 Save UX feedback — progress for large files, background save so UI never freezes, last-saved time indicator _(Stage 2 — standard)_

**4. Autosave & crash recovery** _(Stage 2 — standard)_
- 4.1 Autosave engine — timer-based (every N minutes, configurable) + idle-triggered + on significant-edit-count
  - 4.1.a Writes to a recovery cache dir (e.g. `%LOCALAPPDATA%/Varos/recovery/<docUUID>/`), NOT the user's file, so it's non-destructive
  - 4.1.b Incremental/delta autosave for big docs to keep it cheap; throttle to avoid thrashing during continuous drag
  - 4.1.c Works for UNTITLED docs too (keyed by UUID, so unsaved-never-named work survives a crash)
  - 4.1.d Pause autosave during active interactive operations (mid-drag) to avoid inconsistent snapshots
- 4.2 Crash detection — a "clean shutdown" sentinel; if missing on next launch → recovery was needed
- 4.3 Recovery flow on relaunch — "Varos closed unexpectedly. Recover N documents?" with a list, per-doc recover/discard, preview thumbnails
  - 4.3.a Recovered doc opens as dirty + untitled-bound-to-original so user consciously re-saves
  - 4.3.b Purge recovery cache after a successful manual save / explicit discard
  - 4.3.c Corrupt-recovery-file handling — skip & report, never block startup
- 4.4 Settings — autosave interval, on/off, keep-N-versions, recovery location, max cache size _(Stage 2 — standard)_

**5. Version history / snapshots** _(Stage 3 — advanced)_
- 5.1 Local version history — periodic + on-save named snapshots stored inside/aside the doc package
  - 5.1.a History panel — timeline list (timestamp, label, size, auto vs manual), thumbnail per version
  - 5.1.b Actions — preview, restore (as current / as new copy), rename a version, delete, pin/keep-forever
  - 5.1.c Storage strategy — delta chain or full snapshots with pruning policy (keep hourly→daily→weekly)
- 5.2 Named milestones / "mark a version" with a note
- 5.3 Compare versions (visual diff overlay) — far-future _(Stage 3 — advanced)_
- 5.4 Relationship to undo — version history is coarse/persistent & survives restart; undo is fine/in-session (System #? undo already ✅ have basic)

**6. Open / Recent files** _(Stage 1 — core/MVP)_
- 6.1 Open (Ctrl/Cmd+O) — 🟡 (partial): "Open…" row present in `ui.rs`, no handler
  - 6.1.a Native open dialog with `.varos` filter (+ "All supported" incl. importable SVG/PDF/raster, see §7)
  - 6.1.b Open into a new TAB (tab system already ✅ have basic) or new window; focus existing tab if file already open
  - 6.1.c Multi-select open; drag-a-file-onto-window opens it; OS "Open With Varos" / file association + double-click launch
- 6.2 Recent Files _(Stage 2 — standard)_
  - 6.2.a File ▸ Open Recent submenu (last N), + a Home/Start screen grid with thumbnails
  - 6.2.b Pin/favorite, clear-recents, "remove from list", missing-file greyed with "locate" option
  - 6.2.c Per-entry metadata — path, last-opened, size, thumbnail; integrate with Windows Jump List / taskbar recent
- 6.3 Start / Home screen on launch _(Stage 2 — standard)_ — New, New-from-template, Open, Recent grid, (optional) "what's new"
- 6.4 Reopen / Revert to Saved (File ▸ Revert) — discard all changes back to last saved on disk, with confirm _(Stage 2 — standard)_
- 6.5 Error handling on open — corrupt/truncated zip, unknown schema, missing parts → partial-recover-what-we-can + clear report, never a blank crash

**7. Place / Import (bring external art in)** _(Stage 2 — standard)_
- 7.1 Place command (File ▸ Place) — load-cursor place, click-to-drop or drag-to-size, place-multiple queue
  - 7.1.a Place options — Link vs Embed checkbox, Template (dimmed/locked) checkbox, Replace-current-selection
  - 7.1.b Drag-drop from OS / paste-from-clipboard as an alternate place path
- 7.2 Vector import
  - 7.2.a SVG — parse paths/shapes/groups/transforms/fills/strokes into native `Path`/`Group`; map gradients, basic text, clip/mask where feasible; report unsupported features
  - 7.2.b PDF / EPS / AI(pdf-compatible) — page picker, vector extraction; rasterize fallback for unsupported constructs _(Stage 3 — advanced)_
  - 7.2.c Import fidelity report — what converted, what was rasterized, what was dropped
- 7.3 Raster import — PNG/JPEG/WebP/TIFF/GIF/BMP/HEIC; store bytes in `/assets/`, create an image object referencing it (needs a raster-image node type added to `model.rs`)
  - 7.3.a Color profile / DPI honored; large-image downscale-on-place option
- 7.4 Open vs Place distinction — Open SVG/PDF starts a NEW doc; Place drops into the CURRENT doc
- 7.5 Clipboard interchange — copy-as-SVG / paste-SVG, paste-as-PNG, system-clipboard formats _(Stage 2 — standard)_

**8. Linked vs Embedded assets + Links panel** _(Stage 3 — advanced)_
- 8.1 Asset model — every placed asset is either EMBEDDED (bytes live in `/assets/`) or LINKED (a stored relative+absolute path to an external file) — needs an `Asset`/`AssetRef` registry in the document model (new)
  - 8.1.a Relative-path-first so moving the project folder doesn't break links; absolute as fallback
  - 8.1.b Content-hash + size + mtime cached so we can detect "modified since linked"
- 8.2 Links panel — list of every linked/embedded asset with thumbnail, name, status icons
  - 8.2.a Status — OK / Modified (source changed) / Missing (not found) / Embedded
  - 8.2.b Columns/metadata — name, kind, size, dimensions, ppi/scale, location, # of placements
- 8.3 Links actions
  - 8.3.a Relink (point a missing/changed link to a new file), Relink-all-in-folder, smart-relink by name
  - 8.3.b Update Link (pull latest from modified source), Update-all
  - 8.3.c Edit Original (open in external editor, watch for save, prompt to update)
  - 8.3.d Embed (linked → embedded) and Unembed (embedded → write out to a file + relink)
  - 8.3.e Go-to-link (select & zoom to the placement on canvas), reveal-in-OS-explorer
- 8.4 Missing-link resolution on open — dialog listing all missing links with locate/replace/ignore, optional auto-search sibling folders
- 8.5 Package / Collect-for-output — gather doc + all linked assets (+ fonts) into one folder/zip for handoff _(Stage 3 — advanced)_

**9. Document metadata & properties** _(Stage 2 — standard)_
- 9.1 File Info / Document Properties dialog — title, author, description, keywords, copyright, color mode, units, ruler origin
- 9.2 Embedded prefs — grid/guide settings, view state (last zoom/scroll/active artboard) optionally persisted so reopening restores the workspace
- 9.3 Fonts-used record + (later) font embedding/subsetting policy _(Stage 3 — advanced)_
- 9.4 Statistics — object count, artboard count, file size, asset count

**10. OS & shell integration** _(Stage 2 — standard)_
- 10.1 File association — register `.varos`/`.varost` MIME + icon, double-click opens, "Open With"
- 10.2 Thumbnail provider / shell extension — Explorer shows the embedded `thumb.png` _(Stage 3 — advanced)_
- 10.3 Jump list / recent in taskbar; "pin to Start"; drag-out save
- 10.4 Single-instance handling — second launch with a file routes to the running instance as a new tab
- 10.5 OS-level "recently used" + Windows cloud-folder (OneDrive) awareness / offline file handling _(Stage 3 — advanced)_

**11. Panels & UI surfaces this system needs**
- 11.1 File menu — New, New from Template, Open, Open Recent ▸, Close, Save, Save As, Save a Copy, Save as Template, Revert, Place, Export ▸, File Info, Document Setup _(Stage 1 — core/MVP)_ — 🟡 (partial): a stub File dropdown with New/Open/Save/Export rows already drawn in `ui.rs`, all non-functional
- 11.2 New-document dialog (§2.1) _(Stage 1)_
- 11.3 Native Open/Save OS dialogs (§3.2, §6.1) _(Stage 1)_
- 11.4 Start / Home screen with Recent grid (§6.3) _(Stage 2)_
- 11.5 Save-changes / Revert / Recovery confirm dialogs (§3.6, §4.3, §6.4) _(Stage 1–2)_
- 11.6 Version History panel (§5) _(Stage 3)_
- 11.7 Links panel (§8.2) _(Stage 3)_
- 11.8 Place load-cursor + Place-options dialog (§7.1) _(Stage 2)_
- 11.9 Document Properties / File Info dialog (§9) _(Stage 2)_
- 11.10 Preferences ▸ File Handling section — autosave interval, recovery location, keep-backups, recent-count, default save format/compression _(Stage 2)_
- 11.11 Title bar / tab bar status — dirty dot, file name, save indicator ✅ (have basic): multi-doc tab bar exists; add dirty marker + bind real file paths

**12. Build-order summary (what to ship first within this system)**
- 12.1 Stage 1 spine — add serde to the model (§1.2) → write/read the `.varos` zip (§1.1) → Save/Save As/Open with native dialogs (§3.1–3.2, §6.1) → New + new-doc dialog (§2.1) → dirty-flag + save-prompt + atomic write (§3.5–3.6). This makes the tool genuinely usable (work survives closing the app).
- 12.2 Stage 2 standard — autosave + crash recovery (§4), Recent + Home screen (§6.2–6.3), Revert (§6.4), Place/Import SVG+raster (§7), templates (§2.3, §3.4), metadata (§9), OS association (§10).
- 12.3 Stage 3 advanced — version history (§5), linked assets + Links panel (§8), PDF/EPS/AI import (§7.2.b), binary/compressed format option (§1.4.d), package-for-output (§8.5), thumbnail shell extension (§10.2).

---

## 5. Layers / Structure System
*The hierarchical document tree — Layers panel with nested layers/sublayers/objects, visibility & lock, targeting, z-order, grouping, masks, compound paths, and structural selection — that organizes every object on the canvas.*

> ⚠️ AMENDED [D2]: this is a **MODEL system, not panel assembly.** Today the model is a flat `Vec<Path>` + a `group_of` side-table (no `Layer` type, no z-order on `Group`, no sublayers; `scene.rs` uses flat-Vec order AS z-order). Stage 1 here = **build the real scene-graph** (a `Layer`/node tree owning children + attributes) and migrate selection + render-order + every tool onto it. The panel is the easy part on top.

**1. Layers panel — shell & structure** _(Stage 1 — core/MVP)_ ✅ (have basic)
- 1.1 Panel container — floating rounded panel in the native GPU UI; dockable/collapsible; resizable height; scrollable list
  - 1.1.a Panel header — "Layers" title; optional tab grouping with future panels (e.g. Layers / Assets)
  - 1.1.b Panel footer toolbar — row of action buttons (see 1.6)
  - 1.1.c Empty state — show single default layer ("Layer 1") when document is new
- 1.2 The tree model — single source of truth = document scene graph rendered as an indented list
  - 1.2.a Top-level rows = Layers (containers); can hold objects, groups, sublayers
  - 1.2.b Sublayers — a Layer nested inside another Layer (recursive, unlimited depth)
  - 1.2.c Object rows = each path/shape/text/image as a leaf row (every object appears, not just layers) — this is the Illustrator model (vs Figma where only frames/groups nest)
  - 1.2.d Group rows = `<Group>` container rows holding child objects
  - 1.2.e Row ordering top→bottom = z-order front→back (top of list = front-most)
- 1.3 Row anatomy (left→right) — disclosure triangle ▸ / thumbnail / name / target dot / selection-color swatch / lock col / visibility col
  - 1.3.a Disclosure triangle — expand/collapse any container (layer/sublayer/group/compound); alt/option-click = expand all descendants
  - 1.3.b Indentation — each nesting level adds left indent so hierarchy is readable
  - 1.3.c Row height + hover highlight + selected-row highlight (accent #0c8ce9 tint)
- 1.4 Auto-naming defaults — "Layer 1, Layer 2…", "`<Path>`", "`<Group>`", "`<Rectangle>`", "`<Ellipse>`", "`<Text>` first-glyphs", "`<Image>`", "`<Compound Path>`", "`<Clip Group>`" _(Stage 1)_
- 1.5 Two-way selection sync ✅ (have basic) — selecting on canvas highlights rows; selecting rows selects on canvas; must stay bidirectional for all new features
- 1.6 Panel footer buttons _(Stage 1 core, some Stage 2)_
  - 1.6.a New Layer (+) — adds layer above active layer _(Stage 1)_
  - 1.6.b New Sublayer — adds sublayer inside active layer _(Stage 2)_
  - 1.6.c Delete (trash) — removes selected layer/object with confirm if non-empty _(Stage 1)_
  - 1.6.d Duplicate layer/object _(Stage 2)_
  - 1.6.e Create/Release Clipping Mask button _(Stage 2)_
  - 1.6.f "Locate Object" button — scroll tree to selected object's row _(Stage 2)_
  - 1.6.g Collect/Flatten shortcut buttons (optional, can live only in menu) _(Stage 3)_

**2. Layer & object naming** _(Stage 1 — core/MVP)_ 🟡 (partial — auto-names exist)
- 2.1 Rename via double-click on name label → inline text field; Enter commits, Esc cancels
- 2.2 Auto-name objects by type when created (see 1.4); text objects auto-name from their content string (live-update until renamed)
- 2.3 Custom names persist in file schema; renamed objects keep angle-bracket-free display
- 2.4 Distinguish auto-name vs user-name in schema (so type changes can refresh auto-names but never overwrite a user name) _(Stage 2)_
- 2.5 Tab/Shift-Tab to move rename focus to next/prev row _(Stage 3)_
- 2.6 Duplicate-name handling — allowed (names are display labels, not unique IDs); IDs are internal _(Stage 1)_

**3. Visibility (show/hide)** _(Stage 1 — core/MVP)_
- 3.1 Eye toggle per row — click toggles visibility; hidden objects not rendered, not selectable on canvas
  - 3.1.a Container visibility cascades to all children (hiding a layer hides its contents)
  - 3.1.b Child can be individually hidden while parent visible
  - 3.1.c Visual states — eye open / eye closed / dimmed eye when parent forces hidden
- 3.2 Drag down the visibility column — toggle multiple consecutive rows in one drag _(Stage 2)_
- 3.3 Alt/Option-click eye — hide all OTHER layers (isolate visibility); alt-click again restores _(Stage 2)_
- 3.4 Ctrl/Cmd-click eye — toggle this layer between Preview and Outline view mode (Illustrator behavior) _(Stage 3)_
- 3.5 "Show All Layers" / "Hide Others" menu commands _(Stage 2)_
- 3.6 Hidden objects excluded from export, snapping, and selection _(Stage 1)_

**4. Lock / unlock** _(Stage 1 — core/MVP)_
- 4.1 Lock toggle column (padlock) — click to lock; second column slot next to eye
  - 4.1.a Locked objects — not selectable on canvas, not movable, can't be edited; still rendered
  - 4.1.b Container lock cascades to children; child can be locked while parent unlocked
  - 4.1.c Locked-state visual — solid padlock; empty slot when unlocked (reveals on hover)
- 4.2 Drag down the lock column to lock/unlock multiple rows _(Stage 2)_
- 4.3 Alt/Option-click lock — lock all OTHER layers _(Stage 2)_
- 4.4 Menu commands — Lock Selection (Ctrl/Cmd+2), Unlock All (Ctrl/Cmd+Alt+2), Lock Others _(Stage 1 for lock/unlock-all)_
- 4.5 Locked objects skip during Select-All and marquee selection _(Stage 1)_

**5. Targeting & appearance dot** _(Stage 2 — standard)_
- 5.1 Target dot (○ / ◉) per row — indicates whether the row is targeted for appearance attributes (effects/fills applied to the container itself)
  - 5.1.a Hollow dot = not targeted / no appearance; filled/ringed dot = targeted; shaded dot = has appearance beyond a single fill+stroke
  - 5.1.b Clicking the target dot targets that layer/group/object for the Appearance panel + Effects
- 5.2 Targeting a Layer/Group applies effects to the whole container (group-level appearance) — depends on Appearance system (later) _(Stage 2/3)_
- 5.3 Double-click target dot opens appearance options _(Stage 3)_
- 5.4 NOTE: full appearance attributes are a later system; Stage-1 layers can ship the dot as selection-state only and wire appearance later

**6. Selection-color indicator** _(Stage 2 — standard)_
- 6.1 Per-layer selection color swatch — each layer assigned a distinct color used for its objects' selection bounds, anchor points, bounding box, smart guides
- 6.2 Auto-assign rotating palette on layer creation; user-editable
- 6.3 Selected-object highlight strip — colored bar/box appears at right edge of object rows whose object is currently selected on canvas (the "current selection" marker)
- 6.4 Used to visually trace which layer an on-canvas object belongs to
- 6.5 Override note — Varos brand uses azure #0c8ce9 for primary selection; per-layer colors apply to layer-distinguished selections (decide product rule)

**7. Thumbnails** _(Stage 2 — standard)_
- 7.1 Per-row mini raster preview of the object/layer content
- 7.2 Thumbnail size options — None / Small / Medium / Large (panel options menu)
- 7.3 Toggle which row types show thumbnails (layers only vs objects too) for performance
- 7.4 Live re-render thumbnails on edit (throttled/cached) _(Stage 2)_
- 7.5 Transparency checkerboard behind thumbnails _(Stage 3)_

**8. Reordering & restructuring (drag-drop)** _(Stage 1 — core/MVP)_
- 8.1 Drag a row up/down to change z-order within same parent
  - 8.1.a Insertion indicator line shows drop position
  - 8.1.b Drop ONTO a container row (highlighted) = move INTO that layer/group as child
  - 8.1.c Drop BETWEEN rows = reorder as sibling
- 8.2 Drag selects-then-moves multiple selected rows together (preserve relative order) _(Stage 2)_
- 8.3 Cross-layer move — dragging object from Layer A onto Layer B re-parents it _(Stage 1)_
- 8.4 Auto-scroll the list while dragging near top/bottom edges _(Stage 2)_
- 8.5 Drag respects locked/hidden constraints (can't drop into locked layer) _(Stage 2)_
- 8.6 Reorder updates render order immediately (two-way sync) _(Stage 1)_

**9. Z-order commands (arrange)** _(Stage 1 — core/MVP)_ ✅ (have)
- 9.1 Bring to Front (Ctrl/Cmd+Shift+]) ✅
- 9.2 Bring Forward (Ctrl/Cmd+]) ✅
- 9.3 Send Backward (Ctrl/Cmd+[) ✅
- 9.4 Send to Back (Ctrl/Cmd+Shift+[) ✅
- 9.5 Scope rule — reorder within the object's own container/parent (front/back relative to siblings)
- 9.6 "Send to Current Layer" / move-selection-to-active-layer command _(Stage 2)_
- 9.7 Right-click context-menu Arrange submenu mirrors all four + cross-layer move _(Stage 1)_

**10. Groups & ungroup** _(Stage 1 — core/MVP)_ ✅ (have, nested)
- 10.1 Group (Ctrl/Cmd+G) — wrap selection in `<Group>` container row ✅
- 10.2 Ungroup (Ctrl/Cmd+Shift+G) — dissolve one nesting level, children move to parent ✅
- 10.3 Nested groups — group of groups, recursive ✅
- 10.4 Group preserves z-order and re-parents children under the group row _(Stage 1)_
- 10.5 Grouping objects from different layers — Illustrator pulls them onto the topmost object's layer; define Varos rule (collect to active or topmost layer) _(Stage 2)_
- 10.6 Enter-group editing — double-click to enter group context (isolation), edit children, click out to exit (ties to isolation mode §13) _(Stage 2)_
- 10.7 "Add to Group" / "Remove from Group" via drag in panel _(Stage 2)_

**11. Compound paths** _(Stage 2 — standard)_
- 11.1 Make Compound Path (Ctrl/Cmd+8) — merge multiple paths into one path object with holes (even-odd / non-zero fill rule)
  - 11.1.a Result is a single `<Compound Path>` row; child subpaths collapse under it (or become anonymous subpaths)
  - 11.1.b Inherits front-most (or bottom-most, define rule) object's appearance
- 11.2 Release Compound Path (Ctrl/Cmd+Alt+8) — split back into separate paths
- 11.3 Fill-rule control — Non-Zero Winding vs Even-Odd toggle (in inspector/attributes) governs which overlaps become holes
- 11.4 Direct-select to edit individual subpaths; reverse subpath direction to flip hole/solid _(Stage 3)_
- 11.5 Distinction from Boolean Unite — compound path is non-destructive/releasable; relates to existing Pathfinder engine (reuse fill-rule + winding math) _(Stage 2)_
- 11.6 Auto-compound on text-to-outline of letters with counters (o, a, e) _(Stage 3)_

**12. Clipping masks** _(Stage 2 — standard)_
- 12.1 Make Clipping Mask (Ctrl/Cmd+7) — top-most selected object becomes the clip path; objects below are clipped to its shape
  - 12.1.a Creates a `<Clip Group>` container; the clip path row shown as the mask (often underlined/italic name)
  - 12.1.b Clipping path itself becomes mask — its own fill/stroke removed by default (clip is shape-only); optionally restore stroke
- 12.2 Release Clipping Mask (Ctrl/Cmd+Alt+7) — un-clips, restores objects, clip path returns as normal object
- 12.3 Edit clip path independently (enter clip group, direct-select the mask path)
- 12.4 Edit clipped contents independently (enter group)
- 12.5 Layer-level clipping mask — top object in a layer clips the entire layer (the "clip" button on layer) _(Stage 3)_
- 12.6 Add/remove objects to existing clip group via panel drag _(Stage 3)_
- 12.7 Clip indicator in panel — dotted underline on the clipping-path row name _(Stage 2)_
- 12.8 Affinity parity — non-destructive "mask" by drag-nesting one object under another as a child mask _(Stage 3)_

**13. Isolation mode** _(Stage 2 — standard)_
- 13.1 Enter — double-click a group/object, or "Isolate Selected Group" button; dims & locks everything outside the isolated container
- 13.2 Isolation breadcrumb bar — shows path (e.g. Layer 1 ▸ Group ▸ Subgroup) at top of canvas; click a crumb to step up
- 13.3 Gray overlay on non-isolated content; only isolated content editable/selectable
- 13.4 Exit — Esc, double-click empty canvas, click breadcrumb root, or click the isolation-exit arrow
- 13.5 New objects drawn while isolated are added to the isolated container
- 13.6 Layers panel reflects isolation (shows only isolated branch or marks it) _(Stage 3)_

**14. Opacity masks** _(Stage 3 — advanced)_
- 14.1 Make Opacity Mask — luminance of a top object controls transparency of objects below (white=opaque, black=transparent)
  - 14.1.a Lives in Transparency panel (separate system) but appears as a mask thumbnail pair in the layers/transparency UI
- 14.2 Mask thumbnail + link toggle (link/unlink mask position from artwork)
- 14.3 Edit-mask mode toggle (click mask thumbnail to paint/edit the mask shapes)
- 14.4 Invert Mask, Clip option, Release Mask
- 14.5 NOTE: depends on the transparency/blend system (later) — list here for structural completeness, build after fill/stroke+transparency lands

**15. Locate / find object** _(Stage 2 — standard)_
- 15.1 "Locate Object" command — auto-expands tree and scrolls to the selected object's row, flashes/highlights it
- 15.2 Panel-options toggle "show layers only" vs full object tree (filter noise) _(Stage 3)_
- 15.3 Search/filter field — type to filter rows by name _(Stage 3)_
- 15.4 Reverse-locate — click row to flash the object on canvas _(Stage 3)_

**16. Paste-remembers-layers** _(Stage 3 — advanced)_
- 16.1 Panel-options toggle "Paste Remembers Layers"
  - 16.1.a OFF (default) — paste lands on the currently active layer
  - 16.1.b ON — paste returns objects to the layer(s) they were copied from (create layer if missing)
- 16.2 Applies to Paste, Paste in Place, Paste in All Artboards
- 16.3 Requires storing source-layer identity in clipboard payload

**17. Flatten / merge / collect** _(Stage 3 — advanced)_
- 17.1 Merge Selected — combine selected layers into one (top-selected becomes target); contents preserved, z-order merged
- 17.2 Flatten Artwork — collapse ALL layers into a single layer (hidden layers discarded or kept — prompt)
- 17.3 Collect in New Layer — move all selected rows into a freshly created layer
- 17.4 "Release to Layers (Sequence)" — distribute a group's children each onto its own new layer (for animation export) _(Stage 3)_
- 17.5 "Release to Layers (Build)" — cumulative stacking variant _(Stage 3)_
- 17.6 Reverse Order — reverse the stacking order of selected rows _(Stage 3)_

**18. Select same / similar** _(Stage 2 — standard)_
- 18.1 Select > Same submenu — Fill Color, Stroke Color, Stroke Weight, Opacity, Blending Mode, Fill & Stroke, Shape, Symbol Instance, Link, Font Family/Size
- 18.2 Select > Object submenu — All on Same Layers, Direction Handles, Brush Strokes, Clipping Masks, Stray Points, Text Objects, All
- 18.3 Select Similar from context menu / control bar "Select Similar Objects" button
- 18.4 Select All (Ctrl/Cmd+A) / Select All on Active Artboard ✅ likely / Deselect (Ctrl/Cmd+Shift+A) / Reselect / Inverse _(Stage 1 for All/Deselect)_
- 18.5 Save Selection / Edit Selection (named selections) _(Stage 3)_
- 18.6 Grow/expand selection by matching attribute tolerance _(Stage 3)_

**19. Context menu (right-click in panel & on canvas)** _(Stage 1 — core/MVP)_
- 19.1 On row — Rename, Duplicate, Delete, New Layer/Sublayer, Lock, Hide, Options for [name]
- 19.2 Arrange submenu (z-order) ✅ commands
- 19.3 Group/Ungroup ✅, Make/Release Clipping Mask, Make/Release Compound Path
- 19.4 Select submenu (Same/Similar) _(Stage 2)_
- 19.5 Collect/Merge/Flatten _(Stage 3)_
- 19.6 Isolate / Enter Group _(Stage 2)_

**20. Panel options & global commands menu** _(Stage 2 — standard)_
- 20.1 Panel flyout menu — New Layer, New Sublayer, Duplicate, Delete, Options for Selection, Merge Selected, Flatten Artwork, Collect in New Layer, Release to Layers, Reverse Order, Paste Remembers Layers, Locate Object, Thumbnail size, Show/Hide options
- 20.2 Layer Options dialog — Name, Selection Color, Show/Hide, Lock, Print (export), Preview/Outline, Dim Images %, Template flag _(Stage 2/3)_
- 20.3 Template layer — dimmed, locked, non-printing reference layer (for tracing) _(Stage 3)_
- 20.4 Keyboard map — all shortcuts above registered (Group, Ungroup, Arrange, Lock/Unlock, Hide/Show, Clip, Compound, Select-All/Deselect) _(Stage 1 for the existing/core ones)_

**21. Schema, performance & edge cases** _(cross-cutting — Stage 1 foundation, refine through stages)_
- 21.1 Single schema source of truth (Blender-RNA-style) — layers panel is a VIEW over the scene graph; no parallel state
- 21.2 Stable internal IDs distinct from display names; reorder/regroup mutate parent + index, not identity
- 21.3 Virtualized list rendering for documents with thousands of rows (only render visible rows) _(Stage 2)_
- 21.4 Undo/redo coverage ✅ for every structural op — create/delete/rename/reorder/group/clip/compound/lock/hide
- 21.5 Edge cases — deleting active layer, empty layer cleanup, last-layer-can't-be-deleted rule, dropping a parent into its own descendant (forbid cycle), grouping across locked/hidden, masks inside masks (nesting)
- 21.6 Two-way selection integrity under multi-select, isolation, and cross-layer ops _(Stage 1)_
- 21.7 Thumbnail + appearance re-render throttling to keep panel responsive _(Stage 2)_

---

## 6. Snapping / Guides / Grid / Rulers
*The precision-alignment substrate: rulers, draggable guides, smart alignment guides, document/pixel grids, snap-to-geometry, snap tolerance options, and live dimension readouts that make every move, draw, and resize land exactly where intended.*

**0. System foundations & shared snap engine** _(Stage 1 — core/MVP)_
- 0.1 Single snap solver — one engine that all draw/move/scale/rotate/pen ops query; returns best snapped point + which target(s) it locked to (never two competing snappers fighting). 🟡 (partial: only `snap45` angle-constraint exists in `geom.rs`/`editor.rs`, no object/grid/guide snapping)
  - 0.1.a Candidate model — every potential snap is a candidate (type, world point, axis, source object id, priority weight); solver collects all in-tolerance candidates and picks the highest-priority/closest.
  - 0.1.b Tolerance in SCREEN pixels, not world units — distance threshold measured post-zoom so snapping feels identical at 50% and 1600% (convert px → world by dividing by zoom). Critical: world-space tolerance would make snapping useless when zoomed out.
  - 0.1.c X / Y solved independently — a point can snap its X to a guide and its Y to a grid line simultaneously (two different targets per drag).
  - 0.1.d Snap suppression key — holding a modifier (Illustrator: Ctrl while dragging temporarily toggles Smart Guides; Affinity: hold to disable snapping) lets user place freely; document the exact key.
  - 0.1.e Snap priority order — point/anchor > path/segment > intersection > guide > grid > smart-alignment > nothing; ties broken by nearest. Make order a constant table so it's tunable.
  - 0.1.f Hot-point of the drag — what actually snaps: the grabbed handle, the bounding-box edges/center, the cursor, or all anchors of the moving selection (Illustrator snaps the point under the cursor at grab time). Define per-tool.
- 0.2 Snap evaluation timing — recompute candidates each pointer-move frame against a spatial index (grid bucket / R-tree) of static geometry so it stays 60fps with thousands of objects.
- 0.3 World ↔ screen transform plumbing — rulers/guides/grid all need the same camera (pan/zoom) matrix the canvas already uses; expose px-per-unit + origin to every subsystem. ✅ (have basic: zoom + GPU board camera exist)
- 0.4 Global enable master switch — one "Snapping on/off" toggle (Affinity-style) that gates the whole engine, independent of which snap *types* are enabled. _(Stage 1)_
- 0.5 Persistence — all toggles, units, guide positions, grid config, custom snap options saved per-document (in the `.varos` file) and sensible app-level defaults for new docs. _(Stage 2)_

**1. Rulers** _(Stage 1 — core/MVP)_
- 1.1 Show / hide rulers — top horizontal + left vertical bars along canvas edges; menu item + shortcut (Illustrator Ctrl/Cmd+R); remembered per document.
  - 1.1.a Layout — rulers occupy fixed-px gutter; corner square at top-left intersection (click = reset/menu in Illustrator).
  - 1.1.b GPU-drawn, not DOM — must be rendered in the wgpu UI layer like the rest of Varos chrome (per native-GPU-UI decision), floating/aligned to the board viewport.
- 1.2 Tick marks & labels — major ticks with numeric labels + minor subdivision ticks; density adapts to zoom (more subdivisions as you zoom in, fewer labels when crowded — "nice number" tick algorithm).
- 1.3 Units _(Stage 1 for px; Stage 2 for the rest)_
  - 1.3.a Supported units — px, pt, pc (picas), in, mm, cm; (Stage 3) ft, meters, custom; per-document default unit.
  - 1.3.b Unit switcher — right-click ruler → unit context menu (Illustrator behavior) + a document-units setting in preferences/document setup.
  - 1.3.c Mixed-unit input — numeric fields accept "10mm", "2in", "5pc" and convert regardless of doc unit (ties into inspector field parsing).
  - 1.3.d Per-axis is identical (no separate X/Y units) but support non-uniform if ever needed (parked).
- 1.4 Zero origin (ruler origin) _(Stage 2)_
  - 1.4.a Default origin — top-left of the active artboard (Illustrator: artboard rulers) vs top-left of the global canvas (global rulers); offer both modes.
  - 1.4.b Set custom origin — drag from the corner square onto the canvas to drop a new 0,0 (Illustrator classic gesture); live readout while dragging.
  - 1.4.c Reset origin — double-click the corner square restores default.
  - 1.4.d Y-axis direction — down-positive (screen/Illustrator default) with an option for up-positive (math/CAD); document which is canonical for the file format.
- 1.5 Cursor tracking indicators — thin marker line on each ruler follows the pointer showing its current X/Y; updates live. _(Stage 1)_
- 1.6 Selection extent shading — highlight the span of the selected object's bbox on both rulers (Illustrator-style shaded region) so you see its position/size at a glance. _(Stage 2)_
- 1.7 Video/artboard rulers variant — Illustrator's "Change to Video Rulers" (origin/pixel-aspect for video) — _(Stage 3, parked)_.
- 1.8 Click-and-drag from ruler = create guide (see §2). _(Stage 1)_

**2. Guides (manual ruler guides)** _(Stage 1 — core/MVP)_
- 2.1 Create by dragging from ruler — drag from horizontal ruler → horizontal guide; from vertical ruler → vertical guide; live position readout while dragging; snaps to grid/objects as it's placed.
  - 2.1.a Double-click ruler to drop a guide at a precise point (some tools) / dialog to enter exact coordinate _(Stage 2)_.
  - 2.1.b Drag with modifier to swap axis (Illustrator: hold Alt/Opt while dragging converts H↔V guide). _(Stage 2)_
- 2.2 Guide types
  - 2.2.a Straight horizontal / vertical guides — infinite (canvas-spanning) lines. _(Stage 1)_
  - 2.2.b Angled / rotated guides — guide at an arbitrary angle (Affinity supports; Illustrator via make-guides-from-rotated-line). _(Stage 3)_
  - 2.2.c Path/vector guides — convert any vector object into guides ("Make Guides", see 2.3). _(Stage 2)_
- 2.3 Make Guides from objects — select object(s) → Make Guides (Illustrator Ctrl/Cmd+5) converts geometry to non-printing guide lines; Release Guides (Ctrl/Cmd+Alt+5) converts back to paths. _(Stage 2)_
- 2.4 Move / reposition a guide — select & drag; arrow-key nudge; exact-position field in a Guides/Transform readout. _(Stage 1 drag; Stage 2 numeric)_
- 2.5 Lock guides — Lock/Unlock Guides toggle prevents accidental selection/move (Illustrator Ctrl/Cmd+Alt+;). _(Stage 1)_
- 2.6 Hide / show guides — Show/Hide Guides toggle (Ctrl/Cmd+;) without deleting them. _(Stage 1)_
- 2.7 Delete a guide — select + Delete; drag back onto its ruler to remove (some tools); Clear All Guides command. _(Stage 1)_
- 2.8 Clear all guides — single command to wipe every guide in the doc/artboard. _(Stage 1)_
- 2.9 Snap to guides — moving/drawing objects snap their edges/center/anchors to guide lines (gated by snapping master + a "snap to guides" sub-toggle). _(Stage 1)_
- 2.10 Guide appearance / preferences _(Stage 2)_
  - 2.10.a Color — default cyan; user-settable per-document guide color (Illustrator Guides & Grid prefs).
  - 2.10.b Style — solid line vs dotted/dashed (Illustrator offers Lines/Dots).
  - 2.10.c Per-guide overrides (parked) vs global guide style.
- 2.11 Guides are non-printing / non-exporting — never appear in raster/SVG/PDF export; flagged as editor-only objects. _(Stage 1)_
- 2.12 Guide ownership & scope _(Stage 2)_
  - 2.12.a Document-level vs artboard-level vs layer-level guides — Illustrator stores guides on layers and per-artboard; decide Varos model (recommend artboard-scoped + a "global guides" layer).
  - 2.12.b Guides move with their artboard when the artboard is repositioned (Illustrator behavior).
- 2.13 Smart guide-to-guide spacing — show equal-spacing badges between multiple guides (advanced). _(Stage 3)_
- 2.14 Guide list / management panel — optional panel listing all guides with X/Y, lock, visibility, delete (not in Illustrator; a Varos nicety). _(Stage 3)_

**3. Smart Guides (dynamic alignment guides)** _(Stage 2 — standard; the signature "feels-pro" feature)_
- 3.1 Master toggle — Smart Guides on/off (Illustrator Ctrl/Cmd+U); independent of static guides. Appears only transiently while interacting.
- 3.2 Object-to-object alignment guides — while moving/drawing, draw temporary lines when the dragged object's edge/center aligns with another object's edge/center.
  - 3.2.a Alignment references — left / center-x / right edges; top / center-y / bottom edges; object center; (Affinity also bounding-box midpoints).
  - 3.2.b Multi-object alignment — extend the alignment line through every object sharing that edge/axis (Figma/Affinity-style red lines spanning many objects).
  - 3.2.c Snap onto the alignment — pointer locks to the aligned position within tolerance (not just a visual hint — actually snaps).
- 3.3 Anchor & path snapping — snap to other objects' anchor points, path segments, and curve points while drawing with pen/shape tools or dragging. _(Stage 2)_
  - 3.3.a Point labels — "anchor", "path", "center", "intersect" text label at the snap (Illustrator Smart Guides text hints).
  - 3.3.b Intersection snapping — snap to where two paths/guides cross. _(Stage 3)_
- 3.4 Measurement & spacing labels — show pixel/unit distance between the moving object and neighbors; equal-spacing indicators (matching gaps highlighted, Figma/Affinity style). _(Stage 2 distance labels; Stage 3 equal-gap distribution hints)_
  - 3.4.a Dimension labels — live W×H / Δx / Δy badges near the cursor while transforming.
  - 3.4.b Gap measurement — distance between selection and nearest object on each side, with arrowed dimension lines.
  - 3.4.c Equal-spacing badges — when the gap matches an adjacent gap, show "=" tick marks (distribute-by-eye assist).
- 3.5 Construction / angle guides — show guide rays at preference-defined angles (Illustrator "Angles": 45/90/135 or custom set up to 6 angles) while drawing, so lines snap to those directions. _(Stage 2)_
  - 3.5.a Angle snap while drawing lines/pen — extends `snap45` to a configurable angle set. 🟡 (partial: Shift→45° constrain exists)
  - 3.5.b Alignment-to-anchor angle — Illustrator's "construction guides" that extend from a hovered anchor along the angle set.
- 3.6 Transform tool guides — Smart Guides during scale/rotate show reference values (angle readout, scale %, "snapping to 90°"). _(Stage 2)_
- 3.7 Smart Guide preferences panel section _(Stage 2)_
  - 3.7.a Color — single configurable Smart Guides color (Illustrator default magenta/green).
  - 3.7.b Which hints to display — checkboxes: Alignment Guides, Anchor/Path Labels, Measurement Labels, Object Highlighting, Construction Guides (mirror Illustrator's Smart Guides prefs).
  - 3.7.c Snapping tolerance for Smart Guides — px value (Illustrator default 4 px). _(Stage 2)_
- 3.8 Object highlighting — outline/highlight the object you're aligning to as the reference (Illustrator "Object Highlighting"). _(Stage 3)_

**4. Grid (document grid)** _(Stage 2 — standard)_
- 4.1 Show / hide grid — View ▸ Show Grid (Illustrator Ctrl/Cmd+"); toggle, remembered per doc; drawn in GPU chrome layer behind artwork.
- 4.2 Snap to grid — Snap to Grid toggle (Ctrl/Cmd+Shift+"); when on, Smart Guides usually auto-suppressed (Illustrator mutual-exclusion behavior — document it).
- 4.3 Grid spacing / gridline-every — distance between major gridlines (e.g., every 1 in / 10 mm / 50 px), unit-aware.
- 4.4 Subdivisions — number of minor divisions between major lines (Illustrator "Subdivisions: N"); minor lines lighter/thinner.
- 4.5 Grid origin — aligns to ruler origin / artboard; updates when origin moves.
- 4.6 Grid style — Lines vs Dots (Illustrator option); major vs minor line weight.
- 4.7 Grid color — configurable (default light gray); separate major/minor shades. _(Stage 2)_
- 4.8 "Grids in back" option — draw grid behind or in front of artwork (Illustrator preference). _(Stage 3)_
- 4.9 Isometric / axonometric grid — angled construction grid for iso drawing (Affinity Designer has a rich isometric grid: plane angles, snapping to iso planes). _(Stage 3)_
- 4.10 Per-artboard grid override (parked) vs single document grid. _(Stage 3)_
- 4.11 Grid snapping respects snap hot-point — bbox edges/center/anchors snap to nearest gridline (X and Y independently). _(Stage 2)_

**5. Pixel grid & snap-to-pixel** _(Stage 2 — standard; essential for UI/icon/web work)_
- 5.1 Pixel grid display — at high zoom (Illustrator: ≥600%) show the 1-unit pixel grid; auto appears/disappears by zoom threshold; toggle in prefs ("Show Pixel Grid").
- 5.2 Snap to Pixel / Pixel-perfect mode — align object edges/anchors to whole-pixel boundaries so vectors render crisp (Illustrator "Align New Objects to Pixel Grid" + per-object pixel-align; Affinity "Move by Whole Pixels" + force pixel alignment).
  - 5.2.a Document-level default — new objects pixel-aligned (web/UI doc preset).
  - 5.2.b Per-object pixel-align toggle — flag an object to stay pixel-snapped on transform.
  - 5.2.c Anchor-level rounding — round each anchor to nearest pixel (can distort curves — warn / make optional).
- 5.3 Half-pixel / sub-pixel snapping — option to snap to 0.5px boundaries for crisp odd-width strokes/hairlines. _(Stage 3)_
- 5.4 Pixel Preview mode — render the vector rasterized to the pixel grid (Illustrator Alt+Ctrl+Y) to preview crispness; not strictly snapping but lives here. _(Stage 3)_
- 5.5 Stroke-on-pixel alignment — option to align stroke to inside/center/outside relative to pixel boundary for crispness. _(Stage 3, ties to stroke system)_

**6. Snap to point / anchor / geometry while dragging** _(Stage 1–2; the everyday workhorse)_
- 6.1 Snap to anchor points — selection's hot-anchor snaps onto other objects' anchors. _(Stage 2)_
- 6.2 Snap to path/segment — snap onto the nearest point along another path (edge snapping), incl. snapping to curve. _(Stage 2)_
- 6.3 Snap to object bounds — snap to bounding-box edges/corners/center of other objects (Affinity "snap to object bounding boxes"). _(Stage 1)_
- 6.4 Snap to object key points — corners, midpoints of bbox edges, centers (Affinity "snap to object geometry" / key points). _(Stage 2)_
- 6.5 Snap to selection / current object's own geometry — useful for pen tool closing a path onto its start anchor (close-path snap). _(Stage 1 for pen close)_
- 6.6 Snap while drawing shapes — rectangle/ellipse/polygon corners snap to grid/guides/objects as you drag out the shape. _(Stage 2; shapes already exist ✅, snapping during draw is new)_
- 6.7 Snap during scale/rotate — resize handles snap to grid/guides/other-object edges; rotate snaps to angle increments + to other objects' angles. _(Stage 2; transform/bbox already exists ✅)_
- 6.8 Snap during pan/duplicate/paste-in-place (parked nuances). _(Stage 3)_
- 6.9 Snap radius indicator — small visual ring/marker at the live snap target so the user sees what they locked onto. _(Stage 2)_
- 6.10 Snap to artboard — edges & center of the artboard act as snap targets (Affinity "snap to spread/artboard"). _(Stage 2)_
- 6.11 Snap to midpoints between objects — snap to the exact center point between two objects (advanced). _(Stage 3)_

**7. Snap tolerance & options panel** _(Stage 2 — standard)_
- 7.1 Snapping options surface — a "Snapping Manager" popover/panel (Affinity has a dedicated snapping flyout) listing every snap toggle in one place.
  - 7.1.a Master snapping on/off. ✅-adjacent (master switch from §0.4)
  - 7.1.b Snap to grid (on/off).
  - 7.1.c Snap to guides (on/off).
  - 7.1.d Snap to objects: bounds / geometry / key points (separate toggles).
  - 7.1.e Snap to pixel (on/off).
  - 7.1.f Smart/alignment guides (on/off) + which sub-hints.
  - 7.1.g Snap to artboard (on/off).
- 7.2 Snap tolerance / radius — px value the user can tune (Illustrator "Snapping Tolerance" default 2 px for point snap; Smart Guides 4 px); slider or numeric.
- 7.3 Snapping presets — Affinity-style presets ("UI design", "Print", "Pixel art") that bulk-set the toggles. _(Stage 3)_
- 7.4 Per-tool snap behavior — some tools (selection vs pen vs shape) may want different defaults; advanced override. _(Stage 3)_
- 7.5 "Move by whole pixels" / arrow-nudge increments — set keyboard nudge distance + shift-nudge multiplier; ties nudging into the snap/units system. _(Stage 2)_
- 7.6 Snap candidate construction options — "snap to extensions" (extend edges as snap lines), "snap to nearest" toggles (Affinity granular options). _(Stage 3)_

**8. Live dimension & position readouts** _(Stage 2 — standard; partly overlaps Smart Guides §3.4)_
- 8.1 Live W×H readout while resizing — floating badge near cursor showing current width/height in doc units, updating each frame.
- 8.2 Live X/Y readout while moving — current position (and/or Δ from start) near cursor.
- 8.3 Live angle readout while rotating — degrees, with snap-to-angle indication (e.g., "45°"). 🟡 (rotate exists ✅; readout is new)
- 8.4 Live length/angle while drawing lines/pen segments — segment length + angle as you draw (Illustrator shows this with Smart Guides on). _(Stage 2)_
- 8.5 Δ distance readout while dragging a guide — show guide's exact coordinate.
- 8.6 Readout unit follows document unit + respects mixed-unit display. _(Stage 2)_
- 8.7 Readout placement logic — offset from cursor, flips to stay on-screen near edges, non-overlapping with snap labels. _(Stage 2)_
- 8.8 Tie into inspector — same values shown live in the Transform/inspector fields (two-way: readout reflects what the inspector will commit). ✅ (inspector exists; live sync is new)
- 8.9 Measure tool (optional adjacent feature) — click-drag to measure distance/angle between two points, results in an Info readout (Illustrator Measure tool / Info panel). _(Stage 3)_

**9. Preferences / settings surface** _(Stage 2 — standard)_
- 9.1 "Guides & Grid" preferences section — guide color/style, grid color/style/spacing/subdivisions, grids-in-back (mirror Illustrator's panel).
- 9.2 "Smart Guides" preferences section — color, display hints checkboxes, angles set, snapping tolerance (mirror Illustrator).
- 9.3 "Units" preferences — general/stroke/type units, point/pica size, units shown in fields.
- 9.4 "Selection & Snapping" prefs — tolerances, snap-to-pixel default, align-new-objects-to-pixel-grid default.
- 9.5 Document Setup integration — per-doc units, artboard ruler origin, pixel-alignment default surfaced in Document Setup (not just app prefs).
- 9.6 Reset to defaults — restore all snap/grid/guide settings.

**10. Menus, shortcuts & discoverability** _(Stage 1 for core toggles; Stage 2 for full set)_
- 10.1 View menu group — Rulers (show/hide, change units), Guides (show/hide, lock, clear, make/release), Smart Guides, Grid (show, snap), Snap to Pixel/Point, Pixel Preview — full View-menu parity with Illustrator.
- 10.2 Keyboard shortcuts — Ctrl/Cmd+R rulers, Ctrl/Cmd+; show guides, Ctrl/Cmd+Alt+; lock guides, Ctrl/Cmd+5 make guides, Ctrl/Cmd+Alt+5 release, Ctrl/Cmd+" show grid, Ctrl/Cmd+Shift+" snap to grid, Ctrl/Cmd+U Smart Guides — make all rebindable. _(Stage 2 for rebinding)_
- 10.3 Status-bar / control-bar quick toggles — snapping master + grid + smart-guides toggles reachable from the top control bar (Affinity-style), not buried in menus. _(Stage 2)_
- 10.4 Context menu — right-click ruler (units, set/reset origin), right-click guide (lock, delete, make/release, edit position). _(Stage 2)_
- 10.5 Tooltips / first-run hints — explain snapping master vs sub-toggles so users aren't confused why snapping "won't turn off." _(Stage 3)_

**11. Edge cases & correctness** _(call out per stage)_
- 11.1 Zoom-independent tolerance verified at extremes (10% and 6400%). _(Stage 1)_
- 11.2 Rotated objects — snapping to rotated bbox vs geometric bounds; which bbox is authoritative (visible vs geometric vs path). _(Stage 2)_
- 11.3 Grouped/nested selections — snap uses the group's combined bbox + each child's anchors as candidates. _(Stage 2; groups already exist ✅)_
- 11.4 Multi-object drag — which point is the hot-point when many objects move together (Illustrator: the one grabbed). _(Stage 2)_
- 11.5 Conflicting snaps — grid vs guide vs object at same spot → priority table (§0.1.e) resolves deterministically; no jitter/oscillation between two near-equal candidates (hysteresis). _(Stage 1)_
- 11.6 Snap suppression must be instant — releasing the suppress key re-enables without a stale frame. _(Stage 1)_
- 11.7 Guides/grid never affect export, hit-testing of artwork, or selection of real objects (unless guide explicitly clicked). _(Stage 1)_
- 11.8 Performance — spatial index rebuild on geometry change is incremental, not full-rebuild per frame. _(Stage 2)_
- 11.9 Undo/redo — creating/moving/deleting guides, changing origin, make/release guides are all undoable steps. _(Stage 2; undo/redo exists ✅)_
- 11.10 Sub-pixel / float precision — store guide & origin positions as f64 world coords; round only for display. _(Stage 1)_
- 11.11 Pen-tool close-path snap radius vs general snap — pen's "close path" snap to start anchor should feel slightly stickier. _(Stage 1; pen exists ✅)_
- 11.12 Smart Guides ⊕ Snap-to-Grid mutual behavior — match Illustrator (grid snap disables smart alignment) or allow both (decide & document). _(Stage 2)_

**Build-order summary (within this system)**
- Stage 1 (MVP, genuinely usable): shared snap solver w/ screen-px tolerance + priority + suppression (§0); rulers show/hide + px units + cursor tracking (§1.1–1.3a,1.5); manual H/V guides drag-create/move/lock/hide/delete/clear + snap-to-guide (§2.1,2.4–2.9,2.11); snap-to-object-bounds + pen close-path snap (§6.3,6.5,6.6 basic); core View toggles + shortcuts (§10.1–10.2 core); zoom-independent tolerance + priority/hysteresis + float precision (§11.1,11.5,11.10).
- Stage 2 (standard): full units + ruler origin/zero (§1.3,1.4); make/release guides + guide color/style + scoping (§2.3,2.10,2.12); Smart Guides alignment + anchor/path snap + measurement labels + construction angles (§3); document grid (§4); pixel grid + snap-to-pixel (§5); anchor/geometry/artboard snapping + snap-during-transform/draw (§6.1–6.10); snapping options panel + tolerance (§7); live readouts (§8); prefs surfaces (§9); rebindable shortcuts + control-bar toggles + context menus (§10.3–10.4); rotated/grouped/multi snap correctness + undo (§11).
- Stage 3 (advanced): angled/path guides + iso grid + half-pixel/pixel-preview + intersection/midpoint snap + equal-gap distribution hints + object highlighting + snapping presets + measure tool + guide-management panel + video rulers (scattered across §1.7,2.2b,2.13–2.14,3.4c,3.8,4.8–4.10,5.3–5.5,6.11,7.3–7.6,8.9).

---

## 7. Artboard / Document System
*Multiple resizable/named artboards on one canvas plus the document model (units, DPI, color format, bleed, presets) that defines the design surface and drives navigation and per-artboard export.*

> ⓘ AMENDED [D3/D7]: slot-1 build = **Stage 1 only (a single fixed board + document props)**, NOT full multi-artboard. This system also **owns the Units + world↔screen coordinate contract** (px/pt/mm/cm/in + px-per-unit + origin), built before it hardcodes unitless coords — Transform/Snapping/Rulers/Grid all consume it.

**1. Document model & data foundation** _(Stage 1 — core/MVP)_
- 1.1 Document = top-level container — holds N artboards + global settings; everything in the schema (Blender-RNA-style single source of truth) ✅ (have basic — single canvas exists)
- 1.2 Core document properties — title/name, canvas/scratch area (infinite board outside artboards), version, created/modified timestamps
  - 1.2.a Global units (see Document Setup §6) stored once, all coords derive from it
  - 1.2.b Color format flag (RGB vs CMYK) at document level — affects all swatches/rendering
  - 1.2.c Raster effects resolution (PPI) used for rasterized effects/export
- 1.3 Artboard object schema — each artboard is a first-class node: id, name, index/order, x/y position (in canvas/global coords), width, height, rotation (0 default; advanced), background fill, bleed values, ruler origin offset
  - 1.3.a Artboards live on the infinite board; coordinates are global, artboard is just a framed rect ✅ (have full-bleed board)
  - 1.3.b Z/stacking is conceptual (artboards don't overlap-paint like shapes) but order matters for numbering & export
- 1.4 Content ↔ artboard association model — decide: artboards are pure "frames" (content belongs to board by spatial overlap, Illustrator-style) vs content parented to an artboard (Figma/Affinity-frame-style)
  - 1.4.a MVP recommendation — Illustrator spatial model: a shape is "on" an artboard if its geometry overlaps; simplest given current flat scene
  - 1.4.b Track which content the artboard "owns" for move-with-content, duplicate-with-content, delete-with-content, export
- 1.5 Coordinate spaces — global/canvas coords vs per-artboard local coords (origin at artboard top-left); inspector shows artboard-relative X/Y when an artboard is active
- 1.6 Limits & defaults — max artboard count (Illustrator caps ~1000/1000-per-doc; pick a sane cap), default new-doc single artboard, min/max artboard size (e.g. 1pt … 16383pt)

**2. New Document creation flow** _(Stage 1 — core/MVP)_
- 2.1 New Document dialog — entry point (File ▸ New / Ctrl+N) before any artboard exists 🟡 (partial — doc opens but no setup dialog)
  - 2.1.a Document name field
  - 2.1.b Number of artboards to create up front (1…N) + initial arrangement (rows/columns, grid, by row/by column) + spacing/gutter between them
  - 2.1.c Size: width × height with unit dropdown
  - 2.1.d Orientation toggle (portrait / landscape) — swaps W/H
  - 2.1.e Bleed (top/right/bottom/left, link toggle)
  - 2.1.f Advanced: color mode (RGB/CMYK), raster effects PPI, preview mode
- 2.2 Preset categories (tabs/sidebar) — Web, Print, Mobile/Devices, Film & Video, Art & Illustration, Social, Custom/Recent _(Stage 2 — standard for full preset library; Stage 1 ships a short list)_
- 2.3 Preset detail — each preset = name, W, H, unit, orientation, color mode, PPI, bleed defaults (see §7 size library)
- 2.4 Recent / saved custom presets — user can save current setup as a named template _(Stage 3 — advanced)_
- 2.5 "Create" produces document + artboard(s) and fits view to them

**3. Artboard tool (canvas-direct manipulation)** _(Stage 1 — core/MVP)_
- 3.1 Tool entry — dedicated Artboard tool in the tool rail (Illustrator: Shift+O) ✅ (have tool rail to add it into)
  - 3.1.a Entering tool shows all artboard frames with name labels, dimension readout, and edit handles; dims the canvas/exits on Esc/Enter
- 3.2 Create by dragging — drag on empty canvas to draw a new artboard rectangle
  - 3.2.a Live W×H readout while dragging; snap to other artboards/guides/grid
  - 3.2.b Modifiers — Shift = constrain square/aspect, Alt/Opt = draw from center
  - 3.2.c Drag a preset/size from the panel onto canvas to place at that size _(Stage 2)_
- 3.3 Select / move artboard — click frame or label to select; drag to reposition on the board
  - 3.3.a Move WITH content vs move WITHOUT content — modifier/toggle ("Move/Copy Artwork with Artboard"); MVP needs at least one explicit mode ✅(have move primitives to reuse)
  - 3.3.b Multi-select artboards (Shift-click / marquee) to move/align several
- 3.4 Resize artboard — drag the 8 handles (corners + edge midpoints); live dimension readout
  - 3.4.a Modifiers — Shift constrain proportions, Alt resize from center
  - 3.4.b Numeric resize via inspector W/H + reference-point (9-point) anchor
  - 3.4.c Option: resize artboard but keep / clip content
- 3.5 Duplicate artboard — Alt-drag, or Duplicate command; copies frame (+optionally its content) and offsets/auto-names
- 3.6 Delete artboard — Delete key / panel; option to delete frame only vs frame + content; last-artboard guard (can't delete the only one)
- 3.7 Rotate artboard — set 0/90/180/270 (portrait↔landscape preserving content) and free rotate _(Stage 3 — advanced)_
- 3.8 Snapping & alignment while editing — snap artboard edges/centers to other artboards, guides, grid, pixel grid; smart guides with gap/spacing hints ✅ (align/distribute exists to extend)
- 3.9 Reorder by drag among siblings (re-indexes artboard numbers) — primarily via panel (§4) but reflectable here

**4. Artboards panel** _(Stage 1 — core/MVP for list; richer options Stage 2)_
- 4.1 Panel purpose — list of all artboards with index number + name; the management hub
  - 4.1.a Row contents — order number, name (dbl-click to rename), thumbnail/dimensions, visibility/lock (advanced), reorder grip
- 4.2 Selection sync — click row selects artboard & scrolls/fits view to it; canvas selection highlights row (two-way, like existing layers panel) ✅ (have two-way pattern in layers)
- 4.3 Reorder — drag rows up/down to change artboard order/numbering; "Reorder All Artboards" / renumber command
- 4.4 Row actions / panel buttons — New (+), Duplicate, Delete (trash); move up / move down
- 4.5 Rename — inline edit; auto-name scheme ("Artboard 1", "Artboard 2") + bulk rename pattern _(Stage 2)_
- 4.6 Panel menu (overflow) — New Artboard, Duplicate, Delete, Delete Empty Artboards, Artboard Options…, Rearrange All Artboards…, Auto-rename/renumber, Convert selection→artboard ✅(have inspector/panel shell)
- 4.7 Per-row export quick action — set export target / open export for this artboard _(Stage 2; ties to §11)_
- 4.8 Filter/search rows when many artboards _(Stage 3)_

**5. Artboard management operations** _(Stage 1 core for the basics; auto-rearrange Stage 2)_
- 5.1 Add artboard — new blank board at default/last size, auto-placed in empty space, auto-named & numbered
- 5.2 Delete artboard(s) — single/multi; frame-only vs with-content; "Delete Empty Artboards" cleanup
- 5.3 Duplicate artboard(s) — with or without content; offset placement; name "copy"/increment
- 5.4 Rename / renumber — manual + auto-sequential; renumber after reorder/delete
- 5.5 Reorder — change index (affects export order, numbering, prev/next navigation)
- 5.6 Auto-rearrange / "Rearrange All Artboards" dialog _(Stage 2 — standard)_
  - 5.6.a Layout type — grid by row, grid by column, single row, single column
  - 5.6.b Columns/rows count
  - 5.6.c Spacing/gutter between artboards
  - 5.6.d "Move Artwork with Artboard" toggle
  - 5.6.e Reflow to remove overlaps / pack neatly
- 5.7 Fit artboard to artwork / selection — resize artboard bounds to tightly contain selected content (Object ▸ Artboards ▸ Fit to Artwork / Fit to Selected Art) _(Stage 2)_
- 5.8 Convert selection → artboard — make an artboard from a selection's bounds / from a shape's bounds _(Stage 2)_
- 5.9 Insert/duplicate maintaining spacing — keep consistent gutters when inserting between boards _(Stage 3)_
- 5.10 Last-artboard / empty-document guards & undo support for all ops ✅ (have undo/redo to hook into)

**6. Document Setup** _(Stage 1 for units/size/color-mode; rest Stage 2)_
- 6.1 Entry — File ▸ Document Setup; a modal/inspector panel; some fields editable later, some at-create only
- 6.2 Units — global ruler unit dropdown: px, pt, pica, in, mm, cm, em/percent; separate unit overrides for type (pt) and stroke if desired
  - 6.2.a Changing units re-displays all measurements; underlying storage stays canonical (e.g. always store in pt or document-unit)
- 6.3 Dimensions & presets — edit document/artboard default size; orientation toggle; link to preset library (§7)
- 6.4 DPI / PPI — resolution for the document; raster-effects resolution (72 screen / 150 medium / 300 high); used by export scaling & rasterized effects
- 6.5 Bleed — top/right/bottom/left, link/unlink chain; shown as red guide outside artboard; included in print export _(Stage 2)_
- 6.6 Color format — RGB vs CMYK document mode; switching converts/warns; affects color picker, swatches, export _(Stage 2; CMYK pipeline is heavy — Stage 3 for accurate conversion)_
  - 6.6.a ICC color profile selection (sRGB, Adobe RGB, Display P3 / CMYK profiles like US Web Coated SWOP) _(Stage 3 — advanced, color-managed)_
  - 6.6.b Bit depth / 8-bit vs 16-bit (mostly raster) _(Stage 3)_
- 6.7 Transparency grid — show/hide checkerboard behind transparent areas; grid size (small/med/large) & two grid colors; simulate-colored-paper/background-color option _(Stage 2)_
- 6.8 Facing pages / spreads — multi-page facing layout (book/magazine spreads), gutter, page binding side _(Stage 3 — advanced; mostly print/layout)_
- 6.9 Type/text setup defaults — highlight substituted fonts/glyphs, default type unit _(Stage 3)_
- 6.10 Edit Artboards button — jump from Document Setup into Artboard tool

**7. Artboard presets & standard size library** _(Stage 1 ships a starter set; full library Stage 2)_
- 7.1 Web/screen — 1920×1080, 1440×900, 1366×768, 1280×800, common breakpoints
- 7.2 Print (mm/in) — A-series A6→A3 (A4 210×297mm), B-series, US Letter 8.5×11, Legal, Tabloid/Ledger, business card, postcard, poster sizes
- 7.3 Mobile/devices — iPhone (e.g. 1170×2532 etc.), Android, iPad/tablet, Apple Watch
- 7.4 Social media — Instagram post/story/square, Facebook cover, Twitter/X header, YouTube thumbnail, LinkedIn _(Stage 2)_
- 7.5 Film & video — 1080p, 4K UHD, DCI, NTSC/PAL/HDV with pixel-aspect & title/action-safe overlays _(Stage 3)_
- 7.6 Art & illustration / custom — square presets, custom W×H, user-saved presets _(Stage 2 for save-custom)_
- 7.7 Each preset carries — unit, orientation, default PPI, color mode, bleed default

**8. Per-artboard properties / Artboard Options dialog** _(Stage 2 — standard; markers Stage 3)_
- 8.1 Open via panel menu or double-click Artboard tool — per-artboard settings
- 8.2 Name field
- 8.3 Preset dropdown + W/H + X/Y position (numeric) + orientation
- 8.4 Constrain proportions lock
- 8.5 Display markers/overlays:
  - 8.5.a Show center mark (crosshair at artboard center)
  - 8.5.b Show cross hairs (lines through edge midpoints)
  - 8.5.c Show video/safe areas (title-safe / action-safe insets) — for video presets _(Stage 3)_
  - 8.5.d Video ruler / pixel aspect ratio _(Stage 3)_
- 8.6 Per-artboard background — transparent vs solid color (paper); "show in export?" toggle
- 8.7 "Fade region outside Artboard" / dim canvas when this tool active
- 8.8 "Update while dragging" performance toggle
- 8.9 Global vs per-artboard ruler origin (rulers reset to active artboard's top-left)

**9. Rulers, guides & grid (artboard-aware)** _(Stage 2 — standard; some pieces overlap other systems)_
- 9.1 Rulers — global ruler (whole canvas) vs artboard ruler (origin per active artboard); unit follows document unit; show/hide toggle
- 9.2 Ruler origin — drag from corner to set custom 0,0; reset; auto-reset to active artboard
- 9.3 Guides — drag-out from rulers; per-document guides; lock/clear; snap artboards & content to guides
- 9.4 Grid & pixel grid — document grid (spacing/subdivisions), pixel grid (snap-to-pixel for web), shown relative to artboards
- 9.5 Smart guides — alignment/measurement hints when moving/resizing artboards (gaps, equal spacing) ✅(align/distribute groundwork)
- 9.6 Note: this is a sibling system; here it only needs the artboard-origin coupling

**10. Navigation: fit-to-artboard & artboard view** _(Stage 1 — core/MVP)_
- 10.1 Fit Artboard in Window — Ctrl+0 zooms/centers active artboard to viewport ✅ (have basic zoom to build on)
- 10.2 Fit All in Window — zoom to show every artboard
- 10.3 Active artboard concept — one artboard is "current"; new content/paste/export defaults target it; click selects active
- 10.4 Next / Previous artboard navigation — arrows / shortcuts / panel; status-bar "Artboard N of M" indicator with dropdown
- 10.5 Artboard navigation control (bottom status bar) — current artboard number field + first/prev/next/last buttons + dropdown menu
- 10.6 Zoom-to-selection-within-artboard, double-click panel row to fit
- 10.7 Scroll/pan respects infinite board; "scroll to active artboard" command ✅ (board pan exists)
- 10.8 Presentation / full-screen artboard preview mode _(Stage 3)_

**11. Per-artboard export targeting** _(Stage 2 — standard; the basics tie to existing export shell)_
- 11.1 Export range — "All artboards" vs "Range" (e.g. 1-3,5) vs current artboard 🟡 (partial — basic export exists)
- 11.2 Each artboard exports as its own file/page — filename pattern includes artboard name/number ({doc}-{artboardName})
- 11.3 Use artboard bounds as export crop (vs artwork bounds vs tight bounds); include/exclude bleed
- 11.4 Per-format scaling/resolution from document PPI (1x/2x/3x, or DPI for print)
- 11.5 Formats — PNG/JPG/SVG/PDF (multi-page PDF = one page per artboard), with per-artboard or per-document settings
- 11.6 "Export for screens" style batch — assign export presets/suffixes per artboard, scale factors, output folder _(Stage 2)_
- 11.7 Asset-level export overrides coexist with artboard export _(Stage 3)_
- 11.8 Background handling on export — transparent vs artboard background color; trim transparent edges option
- 11.9 PDF/print specifics — bleed, crop/registration marks, color bars, page order from artboard order _(Stage 3)_

**12. Interaction edge cases & polish** _(Stage 2/3)_
- 12.1 Overlapping artboards — allowed but warn; export overlap behavior; which board "owns" shared content
- 12.2 Content spanning multiple artboards — appears on each board it overlaps; export duplicates per board (Illustrator behavior)
- 12.3 Off-artboard (scratch) artwork — lives on board, excluded from artboard export, kept in file
- 12.4 Deleting/resizing with content — clear modes (keep content in place vs move/clip with frame) and undo coverage ✅ (undo/redo)
- 12.5 Very large / very small / rotated artboards — readout precision, sub-pixel, zoom limits
- 12.6 Renumber stability after reorder/delete; stable IDs vs display numbers
- 12.7 Copy/paste between artboards — "Paste in Place" / "Paste on All Artboards" honoring relative position _(Stage 3)_
- 12.8 Locking/hiding an artboard frame (not its content) _(Stage 3)_
- 12.9 Status bar dims & cursor feedback for Artboard tool (per UI spec) ✅ (have GPU UI shell/cursors plan)

**13. Panels this system needs (summary)**
- 13.1 New Document dialog _(Stage 1)_ — presets, size, units, orientation, count/arrangement, bleed, color mode, PPI
- 13.2 Artboards panel _(Stage 1)_ — list, reorder, add/dup/delete, rename, panel menu, export quick-action
- 13.3 Document Setup dialog/panel _(Stage 1 core fields)_ — units, dimensions, DPI, bleed, color format/ICC, transparency grid, spreads
- 13.4 Artboard Options dialog _(Stage 2)_ — per-artboard name/size/position/markers/background
- 13.5 Rearrange All Artboards dialog _(Stage 2)_ — grid/row/column, spacing, move-with-art
- 13.6 Artboard navigation control in status bar _(Stage 1)_ — N of M, prev/next, dropdown
- 13.7 Inspector (transform) extension _(Stage 1)_ — when Artboard tool active: name, W/H, X/Y, 9-pt reference, orientation, preset dropdown ✅ (have inspector shell)
- 13.8 Export dialog extension _(Stage 2)_ — artboard range, per-artboard filenames, bounds/bleed, scale

---

## 8. Export system
*Getting finished artwork out of Varos in every format and configuration — single Export As, batch Export for Screens, a persistent Asset Export panel, per-artboard/slice output, format-specific options, and clipboard copy.*

**1. Export architecture & shared pipeline** _(Stage 1 — core/MVP)_
- 1.1 One rasterizer + one vectorizer feed every export path — Export As, Export for Screens, Asset panel, clipboard all call the same render-to-bytes core
  - 1.1.a Vector path: scene graph → flattened geometry → SVG/PDF/EPS writer (no rasterization)
  - 1.1.b Raster path: scene graph → GPU/CPU rasterizer at requested DPI → pixel buffer → PNG/JPG/TIFF/WEBP encoder
  - 1.1.c Single source of truth = same renderer used on-canvas, so export matches what the user sees (WYSIWYG guarantee)
- 1.2 Export scope resolver — what region/objects actually get rendered
  - 1.2.a Whole document (all artboards) ✅ (have basic: artboards exist)
  - 1.2.b Single active artboard
  - 1.2.c Current selection (tight bbox of selected objects, ignoring artboard)
  - 1.2.d Specific named slices
  - 1.2.e Current viewport/visible area (Affinity "Area" mode) _(Stage 3)_
- 1.3 Bounds calculation — exact pixel-edge math
  - 1.3.a Geometry bounds (path outline only) vs visual bounds (includes stroke width, effects, blur, outer glow, drop shadow spill)
  - 1.3.b "Use artboards" → clip to artboard frame exactly; "trim to content" → tight bbox of visible art _(Stage 2)_
  - 1.3.c Bleed margin extension (px/mm) for print exports _(Stage 3)_
  - 1.3.d Clip vs no-clip toggle — whether art overflowing the artboard is cropped or included
- 1.4 Background handling
  - 1.4.a Transparent (default for PNG/SVG/WEBP) vs matte/solid fill (required for JPG)
  - 1.4.b Matte color picker for flattening transparency onto formats with no alpha (JPG, EPS, flattened PDF) _(Stage 2)_
  - 1.4.c Artboard background color honored vs forced-transparent toggle
- 1.5 Color management _(Stage 2, Stage 3 for full ICC)_
  - 1.5.a Document color mode: RGB vs CMYK (CMYK is print/PDF/EPS path) _(Stage 3)_
  - 1.5.b sRGB default; embed ICC profile option; "convert to sRGB" for web
  - 1.5.c Per-format profile embedding (PNG/JPG/TIFF iCCP/APP2 chunks)
- 1.6 Async/non-blocking — exports run off the UI thread with progress; cancelable; batch jobs queued
  - 1.6.a Progress bar/spinner per file + overall batch progress
  - 1.6.b Error surface per asset (e.g. one artboard fails, rest continue)

**2. Export As (single-shot dialog)** _(Stage 1 — core/MVP)_
- 2.1 Entry points — File ▸ Export ▸ Export As… ; Ctrl+Shift+E (Illustrator: Export As; Affinity: Export); right-click ▸ Export Selection
- 2.2 Format dropdown — choose the file type
  - 2.2.a PNG (transparent raster, default for UI/web) _(Stage 1)_
  - 2.2.b JPG/JPEG (flattened photo raster) _(Stage 1)_
  - 2.2.c SVG (vector web) _(Stage 1)_
  - 2.2.d PDF (vector print/share) _(Stage 2)_
  - 2.2.e WEBP (modern web raster, lossy+lossless) _(Stage 2)_
  - 2.2.f TIFF (lossless print raster, layered option) _(Stage 3)_
  - 2.2.g EPS (legacy vector interchange) _(Stage 3)_
  - 2.2.h GIF (indexed, basic; static first, animation later) _(Stage 3)_
  - 2.2.i BMP / ICO / ICNS (icon outputs) _(Stage 3)_
- 2.3 Destination
  - 2.3.a OS save dialog: filename + folder + extension auto-set from format
  - 2.3.b "Use artboards" checkbox → emits one file per artboard with suffix _(Stage 2)_
  - 2.3.c Range field "1, 3-5" when exporting subset of artboards _(Stage 2)_
  - 2.3.d Remember last-used folder per format
- 2.4 Live preview pane inside dialog _(Stage 2)_
  - 2.4.a Thumbnail of result + zoom; before/after for lossy quality
  - 2.4.b Estimated output file size readout (updates as options change)
  - 2.4.c Output pixel dimensions readout (W×H px at chosen scale)
- 2.5 Scale/size controls (raster only)
  - 2.5.a Scale factor (%, or 1x/2x/3x preset) _(Stage 1)_
  - 2.5.b Absolute width/height in px (with lock-aspect) _(Stage 2)_
  - 2.5.c Resolution/DPI field (72/144/150/300/PPI custom) — drives raster pixel count for print _(Stage 2)_
  - 2.5.d Resampling/interpolation method (bilinear/bicubic/nearest for pixel art) _(Stage 3)_
- 2.6 Format-specific options panel switches live with the format dropdown (see §6)
- 2.7 Export button + "Export and keep dialog open" / Cancel
- 2.8 Anti-alias toggle (on/off/art-optimized vs type-optimized) _(Stage 2)_

**3. Export for Screens (batch multi-artboard / multi-asset)** _(Stage 2 — standard)_
- 3.1 Purpose — one dialog, many artboards × many formats × many scales in a single run (Illustrator "Export for Screens")
- 3.2 Two tabs: **Artboards** and **Assets**
  - 3.2.a Artboards tab — grid of all artboard thumbnails with checkboxes; select all/range/by name
  - 3.2.b Assets tab — items dragged into the Asset Export panel (see §4) appear here
- 3.3 Selection controls — Select All / individual checkboxes / range string / filter by artboard-name prefix
- 3.4 Output destination — single root folder picker
  - 3.4.a "Create sub-folders" toggle — group output by format (PNG/, SVG/) or by scale (1x/, 2x/)
  - 3.4.b Folder structure preview tree
- 3.5 Formats & Scales matrix — the core of the dialog
  - 3.5.a Add multiple format rows; each row = one Format + one Scale + one Suffix
  - 3.5.b Per-row: Scale (0.5x…3x or px/DPI), Format (PNG/JPG/SVG/PDF/WEBP/TIFF), Suffix text (e.g. "@2x", "-dark")
  - 3.5.c "+ Add Scale" button to stack rows → cartesian export (e.g. 3 scales × 2 formats = 6 files per artboard)
  - 3.5.d Quick presets: "iOS @1x/@2x/@3x", "Android mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi", "Web 1x/2x" _(Stage 3)_
- 3.6 Naming
  - 3.6.a Prefix + artboard/asset name + suffix + scale-token + extension
  - 3.6.b Filename collision handling (overwrite / append number / skip)
  - 3.6.c Sanitize illegal filename chars; Windows reserved-name guard
- 3.7 Background batch run with progress + per-file success/error list + "open output folder" on completion
- 3.8 "Add Scale presets" management — save a custom format/scale set as a reusable preset _(Stage 3)_

**4. Asset Export panel (persistent dock)** _(Stage 2 — standard)_
- 4.1 The panel — floating/dockable list of "assets" the user has earmarked for export (Illustrator Asset Export / Affinity Export Persona "Slices" panel equivalent)
  - 4.1.a Drag objects/groups/artboards from canvas or Layers panel into the panel → becomes an asset entry
  - 4.1.b Thumbnail + editable asset name (drives output filename) per row
  - 4.1.c Multi-select assets; reorder; delete from panel (without deleting art); "collect for export" from selection
- 4.2 Per-asset format settings (sub-rows)
  - 4.2.a Each asset can carry multiple export targets stacked beneath it
  - 4.2.b Per-target: Scale (@1x/@2x/@3x, px, or %), Format, Suffix
  - 4.2.c "+ Add Scale" per asset; remove target; duplicate target
  - 4.2.d Default suffix auto-fills from scale (@2x ↔ 2x) but editable
- 4.3 Panel-level controls
  - 4.3.a "Export" (selected assets) / "Export All" buttons
  - 4.3.b Format-settings gear → opens advanced per-format options (PNG bit depth, SVG decimals, JPG quality)
  - 4.3.c "Launch Export for Screens" from panel (assets pre-loaded into that dialog)
- 4.4 Live linkage — if the source object changes, re-exporting the asset reflects edits (asset = reference, not snapshot)
- 4.5 Generate-on-save / auto-re-export option (watch + emit on document save) _(Stage 3)_
- 4.6 State persists in the document file so asset list survives close/reopen

**5. Per-artboard & slices** _(Stage 2 for artboard; Stage 3 for slices)_
- 5.1 Per-artboard export ✅ (have basic: artboards as objects)
  - 5.1.a Each artboard treated as an independent export region with its own name → filename
  - 5.1.b Export only-selected vs all artboards; honor artboard background/clip
  - 5.1.c Artboard ordering/numbering tokens in filenames
- 5.2 Slices (sub-regions of art, HTML-era but still used) _(Stage 3)_
  - 5.2.a Slice tool — draw rectangular slice regions; or "Make Slice from Selection"; or guide-based slices
  - 5.2.b Slice types: image slice vs no-image (HTML text) vs background
  - 5.2.c Per-slice settings: name, format, dimensions, URL/alt/target (for HTML maps), background color
  - 5.2.d Slice Select tool: move/resize/align/divide slices; lock; combine; clear all
  - 5.2.e Auto-slices vs user-slices distinction (auto fill gaps)
  - 5.2.f "Save for Web" honoring slices → emits images + optional HTML/CSS
  - 5.2.g Snap slices to guides/objects; subdivide grid

**6. Format-specific option panels** _(Stage tags per format)_
- 6.1 SVG options _(Stage 1 core; advanced sub-options Stage 2)_
  - 6.1.a Styling: presentation attributes vs internal `<style>` vs inline `style=` _(Stage 2)_
  - 6.1.b Decimal precision (1–7 digits) for path coordinates — size/accuracy tradeoff _(Stage 2)_
  - 6.1.c Minify (strip whitespace/comments/metadata) _(Stage 2)_
  - 6.1.d Responsive toggle — drop fixed width/height, keep viewBox so it scales _(Stage 2)_
  - 6.1.e Object IDs: layer names vs minimal vs none (for CSS/JS targeting) _(Stage 2)_
  - 6.1.f Text handling: keep as live `<text>` (with font) vs convert to `<path>` outlines vs embed as image _(Stage 2)_
  - 6.1.g Images: link (external href) vs embed (base64 data-URI) _(Stage 2)_
  - 6.1.h Gradients/effects fidelity: native SVG gradients/filters vs rasterize-unsupported-effects _(Stage 3)_
  - 6.1.i Preserve editing data (Varos-private namespace round-trip) toggle _(Stage 3)_
  - 6.1.j XML prolog/DOCTYPE inclusion; CSS class naming scheme _(Stage 3)_
- 6.2 PNG options _(Stage 1 core)_
  - 6.2.a Bit depth: 8-bit vs 24-bit vs 32-bit (with alpha) _(Stage 2)_
  - 6.2.b Color type: truecolor+alpha vs indexed (palette, with dither + color count) _(Stage 3)_
  - 6.2.c Interlacing (Adam7) on/off _(Stage 3)_
  - 6.2.d Transparency on/off; matte color when off _(Stage 1)_
  - 6.2.e Compression level / lossless optimization (pngquant/oxipng-style) _(Stage 3)_
  - 6.2.f Embed DPI metadata (pHYs chunk) for print _(Stage 2)_
- 6.3 JPG/JPEG options _(Stage 1 core)_
  - 6.3.a Quality slider 0–100 (with live size estimate) _(Stage 1)_
  - 6.3.b Matte/background color (no alpha) _(Stage 1)_
  - 6.3.c Chroma subsampling (4:4:4 / 4:2:2 / 4:2:0) _(Stage 3)_
  - 6.3.d Progressive vs baseline encoding _(Stage 2)_
  - 6.3.e Embed ICC + EXIF/DPI metadata _(Stage 2)_
- 6.4 PDF options _(Stage 2)_
  - 6.4.a Preset: print (high-res/CMYK) vs screen (compressed) vs press/PDF-X _(Stage 3)_
  - 6.4.b Vector-preserving (text live + selectable) vs flatten-to-raster
  - 6.4.c Font embedding: embed full / subset / outline-to-paths
  - 6.4.d Multi-page: each artboard → one PDF page; page order/range
  - 6.4.e Raster image downsampling DPI + compression (JPEG/ZIP)
  - 6.4.f Marks & bleeds: crop marks, registration, color bars, bleed margins _(Stage 3)_
  - 6.4.g PDF/X-1a, PDF/X-4, PDF/A compliance flags _(Stage 3)_
  - 6.4.h Preserve-editing (re-openable in Varos) vs flat output _(Stage 3)_
  - 6.4.i Security: password, print/copy permissions _(Stage 3)_
- 6.5 WEBP options _(Stage 2)_
  - 6.5.a Lossy (quality slider) vs lossless mode
  - 6.5.b Alpha transparency support; alpha quality
  - 6.5.c Effort/compression-speed setting _(Stage 3)_
- 6.6 TIFF options _(Stage 3)_
  - 6.6.a Compression: none / LZW / ZIP / JPEG
  - 6.6.b Bit depth 8/16; RGB vs CMYK; alpha channel
  - 6.6.c Layered (preserve layers) vs flattened; byte order (IBM/Mac)
  - 6.6.d Embed ICC + DPI
- 6.7 EPS options _(Stage 3)_
  - 6.7.a PostScript level (2/3); preview (TIFF/none)
  - 6.7.b Fonts: embed vs outline; transparency flattening (EPS has no native alpha)
  - 6.7.c CMYK/RGB; gradient mesh handling
- 6.8 GIF options _(Stage 3)_
  - 6.8.a Palette (256 max) + dithering + transparency index; animation frames later

**7. Resolution / scale presets & reuse** _(Stage 2 — standard)_
- 7.1 Built-in scale presets — 0.5x, 1x, 1.5x, 2x, 3x, 4x
- 7.2 DPI presets — 72 (screen), 150 (mid), 300 (print), custom field
- 7.3 Named export presets — save a full config (format+scale+options) and re-apply _(Stage 3)_
  - 7.3.a Manage presets (rename/duplicate/delete/reorder); import/export preset files
  - 7.3.b Per-document default export preset
- 7.4 "Export with last settings" — one-click repeat of the previous export (no dialog) _(Stage 2)_
- 7.5 Default-suffix tokens library: `@{scale}x`, `-{format}`, `_{artboard}` _(Stage 2)_

**8. Save for Web (legacy optimized web export)** _(Stage 3 — advanced)_
- 8.1 Dedicated optimization dialog with 2-up / 4-up comparison views (different settings side by side)
- 8.2 Per-view: format + quality, live file-size + estimated download time at a chosen connection speed
- 8.3 Color table editor for indexed PNG/GIF (lock/shift/delete colors, web-snap, dithering)
- 8.4 Image size + percent + clip-to-artboard within the dialog
- 8.5 Output settings: HTML+images vs images-only; slice-driven; background; metadata stripping
- 8.6 Eyedropper to inspect/lock colors; zoom/pan synced across views

**9. Clipboard copy / quick export** _(Stage 2 — standard)_
- 9.1 Copy as SVG — selection → SVG markup on clipboard (paste into code/Figma/browser) _(Stage 2)_
- 9.2 Copy as PNG — selection → raster bitmap on clipboard (paste into chat/docs) at chosen scale _(Stage 2)_
- 9.3 Copy as PDF / vector (platform-native vector clipboard flavor) _(Stage 3)_
- 9.4 Copy as CSS / base64 data-URI (for devs) _(Stage 3)_
- 9.5 Drag-and-drop export — drag an asset/selection out of the window to desktop/Finder → writes a file _(Stage 3)_
- 9.6 Right-click ▸ Quick Export ▸ [PNG/SVG/JPG] using last/default settings, no dialog _(Stage 2)_
- 9.7 Honors copy-scale preference + transparent-vs-matte for clipboard raster

**10. Menus, shortcuts & integration points** _(Stage 1 for core entries)_
- 10.1 File menu: Export As…, Export for Screens…, Save for Web…, Export Selection, Export with last settings
- 10.2 Keyboard: Ctrl+Shift+E (Export As), Ctrl+Alt+E (Export for Screens), Alt+Ctrl+Shift+S (Save for Web), Ctrl+Shift+C-style copy-as variants _(Stage 2)_
- 10.3 Right-click context menu on objects/artboards: Quick Export, Collect for Export, Copy as SVG/PNG
- 10.4 Asset Export panel toggle in Window menu; dockable in the GPU UI shell ✅ (have basic: panel shell exists)
- 10.5 Recent-export indicator / re-export badge on changed assets _(Stage 3)_

**11. Edge cases & correctness guarantees** _(Stage 2–3)_
- 11.1 Empty selection / empty artboard → friendly "nothing to export" guard, not a 0×0 file _(Stage 2)_
- 11.2 Effects/blur/shadow bounds expansion so glows aren't clipped at edges _(Stage 2)_
- 11.3 Strokes: align-to-bounds, so 1px outer stroke isn't half-cut at artboard edge _(Stage 2)_
- 11.4 Text rendering parity — hinting/anti-alias must match canvas; missing-font substitution warning _(Stage 2)_
- 11.5 Very large output guard (e.g. 30000×30000 @4x) → warn + memory-safe tiled rasterization _(Stage 3)_
- 11.6 Sub-pixel/odd dimensions at 2x/3x → rounding rule (round up, snap to integer px) to avoid blurry @2x assets _(Stage 2)_
- 11.7 Overlapping/nested artboards and objects spanning multiple artboards → defined clipping behavior _(Stage 2)_
- 11.8 Hidden/locked layers excluded by default; "include hidden" override _(Stage 2)_
- 11.9 Linked vs embedded images resolved at export; missing-link warning _(Stage 3)_
- 11.10 Deterministic, reproducible bytes where possible (same input → same file) for diffing/CI _(Stage 3)_
- 11.11 Filename collisions across artboards with same name → de-dupe numbering _(Stage 2)_

---

## 9. Geometric-logic tools
*The geometry-computing tools — Line Segment, Shape Builder, cut tools (Scissors/Knife/Eraser), and Clipping Mask / Compound Path — that build, split, merge, and mask vector geometry through direct canvas interaction.*

**1. Shared foundations for all geometric-logic tools** _(Stage 1 — core/MVP)_
- 1.1 Common geometry kernel — all tools route through the same path/boolean engine already used by Pathfinder ✅ (have basic, boolean engine + Pathfinder ops exist)
  - 1.1.a Path representation — each subpath = ordered anchors with in/out handles + closed/open flag; all tools read/write this single model
  - 1.1.b Winding & orientation — store per-subpath winding direction (CW/CCW); needed for even-odd vs nonzero, Shape Builder regions, and hole detection
  - 1.1.c Curve math via rented OSS — bezier flattening + curve/curve intersection (reuse the bezier crate); never re-implement per tool
  - 1.1.d Tolerance constants — single epsilon for "point on path", "anchors coincident", "gap closeable"; expose as engine config, not per-tool magic numbers
- 1.2 Hit-testing primitives — point-on-segment, point-in-region, nearest-point-on-path with parametric t — shared by Scissors, Knife start/end, Shape Builder hover
- 1.3 Cursor + feedback contract — each tool ships its own cursor glyph (Illustrator-style) + live hover highlight; consistent with the native GPU UI cursor set
  - 1.3.a "Snap-to-path" cursor state — cursor changes (e.g. filled vs hollow) when over a hittable path vs empty canvas
  - 1.3.b Modifier echo — holding Shift/Alt shows a transient hint of the constrained/inverse mode
- 1.4 Tool rail placement — Line Segment in the shape/draw cluster; Scissors+Knife+Eraser in a flyout cluster; Shape Builder near Pathfinder; masks/compound live in menus + context menu, not the rail _(Stage 2 — standard)_
- 1.5 Undo atomicity — every committed operation = ONE undo step; mid-drag previews never enter history ✅ (have basic, undo/redo exists)

**2. Line Segment tool** _(Stage 1 — core/MVP)_
- 2.1 Core interaction — click-drag draws a single straight open path of exactly two anchors (start = mousedown, end = mouseup)
  - 2.1.a Geometry computed — store both endpoints; length = Euclidean distance; angle = atan2(dy,dx); no handles (corner anchors)
  - 2.1.b Live readout — show length + angle while dragging (in the inspector/HUD), updating each move
  - 2.1.c Zero-length guard — if mouseup ≈ mousedown (within tolerance), create nothing (or a 1px nub per AI) — define and discard tiny degenerate
- 2.2 Modifiers during drag _(Stage 1 — core/MVP)_
  - 2.2.a Shift — constrain angle to 45° increments (0/45/90/135…); snap the END point to nearest constrained ray
  - 2.2.b Alt/Option — draw from the CENTER outward (anchor the midpoint at mousedown, extend symmetrically both directions)
  - 2.2.c Spacebar — reposition the whole segment live while still dragging (move both endpoints by the delta)
  - 2.2.d Shift+Alt — centered AND angle-constrained simultaneously
- 2.3 Exact dialog (click without drag) — single click on canvas opens a Line Segment Options modal _(Stage 2 — standard)_
  - 2.3.a Fields — Length (numeric + unit), Angle (numeric degrees, with a small dial/compass widget), "Fill Line" checkbox (apply current fill swatch to the open path)
  - 2.3.b Anchor point — the click location is the START; dialog builds the segment from there at the given length/angle
  - 2.3.c Last-used defaults — remember previous length/angle as the dialog's initial values
- 2.4 Result properties — open path, takes current stroke (and fill if checked); appears in Layers; fully editable by pen/anchor tools afterward ✅ (have basic, pen/anchor + stroke exist)
- 2.5 Edge cases — drawing onto a locked/hidden layer (block + notify); snapping the endpoints to existing anchors/guides if snapping is on _(Stage 2 — standard)_
- 2.6 Related primitives (same tool family, defer) — Arc tool, Spiral, Rectangular/Polar Grid as their own mini-dialogs sharing the click-for-dialog pattern _(Stage 3 — advanced)_

**3. Shape Builder tool** 🟡 (partial — boolean/Pathfinder engine exists; the interactive merge/region tool does not) _(Stage 2 — standard)_
- 3.1 Purpose — interactively unite/subtract overlapping selected paths by dragging across the regions they form, instead of clicking Pathfinder buttons
- 3.2 Prerequisite state — operate on the current SELECTION of 2+ overlapping objects; tool subdivides them into atomic "regions" (faces) defined by all intersection points _(Stage 2 — standard)_
  - 3.2.a Region computation — compute the planar arrangement: every curve/curve intersection splits edges; faces = enclosed cells of the arrangement (this is the new geometry work beyond existing boolean ops)
  - 3.2.b Edge atoms — also track open edge segments between intersections (for line-merging / deleting strokes)
- 3.3 Unite mode (default) — drag a continuous stroke across multiple regions; all touched regions merge into one path on mouseup _(Stage 2 — standard)_
  - 3.3.a Region highlight — on hover, the region under the cursor fills with a highlight tint (mesh/dotted shading) showing what will be included
  - 3.3.b Drag trail — the pointer path collects every region it crosses; releasing commits the union of collected regions
  - 3.3.c Single-click merge — click one region to select just it; click an edge to merge two adjacent regions
  - 3.3.d Result styling — merged shape adopts the style of... (option: topmost object's style, or the object the drag STARTED on — Illustrator uses cursor-swatch / first-touched); expose in tool options
- 3.4 Subtract mode (Alt/Option held) — cursor shows minus glyph; click/drag a region to DELETE it (erase that face entirely) _(Stage 2 — standard)_
  - 3.4.a Deleting interior regions creates holes (compound path) when the surrounding face survives
  - 3.4.b Deleting edge atoms removes loose stroke segments between intersections
- 3.5 Gap detection _(Stage 3 — advanced)_
  - 3.5.a Open-path handling — near-closed shapes with small gaps: detect gaps below a threshold and treat regions as closed for building
  - 3.5.b Gap Detection setting — Off / Small / Medium / Large / Custom (numeric) gap-length threshold; controls which open contours count as region boundaries
  - 3.5.c Visual — optionally mark detected/bridged gaps so the user sees why a region was considered closed
- 3.6 Shape Builder Options dialog _(Stage 3 — advanced)_
  - 3.6.a Gap Detection (above) + custom length
  - 3.6.b "Consider open filled path as closed" toggle
  - 3.6.c "In Merge Mode, Clicking Stroke Splits the Path" toggle
  - 3.6.d Pick Color From — Art / Color Swatches (what fills the highlight + result)
  - 3.6.e Selection: "Straight Line"/free-form drag; highlight stroke color + opacity for the region tint; cursor swatch preview on/off
- 3.7 Edge cases — non-overlapping selection (nothing to build → inert); self-intersecting source paths; mixed open+closed; styled strokes vs fills; performance on many regions (cache the arrangement until selection changes)
- 3.8 Leverage existing engine — unite/subtract math = the existing boolean ops ✅; the NEW work is the arrangement/region model + drag-collection UX, not the geometry primitives

**4. Cut tools — Scissors** _(Stage 1 — core/MVP)_
- 4.1 Purpose — split a path at a clicked point, breaking continuity without removing area
- 4.2 Interaction — click directly on a path segment or on an existing anchor
  - 4.2.a Click on a segment — compute nearest point on curve (parametric t), insert TWO coincident anchors at that point; path becomes split there
  - 4.2.b Click on an existing anchor — duplicate that anchor into two coincident anchors
  - 4.2.c Handle continuity — split point inherits the curve tangents so the visual shape is unchanged before the user moves a piece
- 4.3 Result topology — defines the cut's effect on open vs closed paths _(Stage 1 — core/MVP)_
  - 4.3.a Closed path cut once — becomes ONE open path (the loop is broken at the two coincident anchors)
  - 4.3.b Closed path cut twice — becomes TWO separate open paths
  - 4.3.c Open path cut once — becomes TWO separate open paths
  - 4.3.d After cut, the two coincident anchors are selectable by Direct Selection to pull pieces apart ✅ (have basic, direct selection exists)
- 4.4 Constraints & invalids — cannot cut at an exact endpoint of an open path (no-op); cannot cut text/raster (only vector paths); clicking off-path = no-op _(Stage 1 — core/MVP)_
- 4.5 Edge cases — cutting on a very tight curve (snap to true nearest t, not screen-linear); cutting a path inside a group (target the path, keep group); style preserved on both resulting pieces _(Stage 2 — standard)_
- 4.6 Cursor — crosshair/scissor glyph; highlight the exact on-path point that will be cut as you hover _(Stage 2 — standard)_

**5. Cut tools — Knife** _(Stage 2 — standard)_
- 5.1 Purpose — freehand slice that cuts THROUGH objects, producing closed filled pieces (unlike Scissors which only splits a contour)
- 5.2 Interaction — drag a freehand stroke across one or more objects; on release, every object the blade fully crosses is divided along that line
  - 5.2.a Blade path — the cursor trail = a cutting polyline/curve; intersect it with each target's outline
  - 5.2.b Closed-piece generation — for a closed/filled shape, the cut line + the original outline segments form NEW closed subpaths on each side (re-stitch outline arcs with the blade arc)
  - 5.2.c Each resulting piece keeps the original fill/stroke; pieces become independent objects (select-move apart)
- 5.3 Modifiers _(Stage 2 — standard)_
  - 5.3.a Alt/Option — cut in a perfectly STRAIGHT line (click start, click/drag end) instead of freehand
  - 5.3.b Shift (with Alt) — constrain that straight cut to 45° increments
  - 5.3.c Cut applies to all objects under the blade unless a selection limits it to selected objects only (define: no selection = cut everything crossed; selection = cut only selected)
- 5.4 Geometry edge cases _(Stage 3 — advanced)_
  - 5.4.a Blade that enters and exits the shape multiple times — handle each entry/exit pair → multiple pieces
  - 5.4.b Blade that doesn't fully cross (starts or ends inside the shape) — Illustrator does nothing or partial; define: ignore incomplete crossings (no dangling open cut)
  - 5.4.c Cutting an open path — produces split open paths (degrades to Scissors-like behavior, no closed piece)
  - 5.4.d Cutting a group/compound path — operate on member outlines; preserve compound holes correctly
  - 5.4.e Self-intersecting blade stroke — normalize the blade before intersecting
- 5.5 Cursor — knife glyph; show the live blade trail while dragging _(Stage 2 — standard)_

**6. Cut tools — Eraser** _(Stage 2 — standard)_
- 6.1 Purpose — freehand removal of vector AREA (not pixels) by sweeping a round nib; what the nib covers is subtracted from path geometry
- 6.2 Interaction — drag across objects; the swept region (nib outline along the drag path) is booleaned-OUT of every affected path
  - 6.2.a Swept-area geometry — build the eraser stroke outline (offset the drag polyline by nib radius, account for the elliptical nib shape) then subtract from targets via the existing boolean engine ✅ (subtract op exists)
  - 6.2.b Result — closed shapes get bites taken out, splitting into multiple subpaths / creating holes as needed; new closed contours along the erased edge
  - 6.2.c Scope — erases all top objects under the nib, OR only selected objects if a selection exists (define same rule as Knife)
- 6.3 Nib / brush options dialog (double-click tool) _(Stage 2 — standard)_
  - 6.3.a Size — nib diameter (numeric); live bracket keys [ and ] to shrink/grow
  - 6.3.b Angle — rotation of the elliptical nib (degrees)
  - 6.3.c Roundness — 0–100% (1.0 = circle, lower = flatter ellipse)
  - 6.3.d Variation controls — Fixed / Pressure / Random for Size, Angle, Roundness (pressure needs tablet input — defer) _(Stage 3 — advanced)_
- 6.4 Modifiers _(Stage 2 — standard)_
  - 6.4.a Shift — constrain the erase stroke to vertical/horizontal/45°
  - 6.4.b Alt/Option — marquee a rectangular area to erase a clean rectangle of geometry (drag a box, subtract that rect)
  - 6.4.c [ and ] — decrease/increase nib size on the fly
- 6.5 Edge cases _(Stage 3 — advanced)_
  - 6.5.a Erasing a stroked-only (no fill) path — converts to outlined geometry then bites (or removes stroke segments); define behavior
  - 6.5.b Erasing fully through a thin shape — may delete the whole piece; cleanup zero-area fragments
  - 6.5.c Cannot erase text/symbols/raster (vector paths only) — block with notice
  - 6.5.d Erasing inside a group — affects member paths, keeps group
  - 6.5.e Tessellation quality — smoothness of the erased edge depends on nib-sweep flattening tolerance; keep it adaptive to zoom
- 6.6 Cursor — circle/ellipse showing actual nib size + angle at the cursor (scales with zoom) _(Stage 2 — standard)_

**7. Compound Path** _(Stage 1 — core/MVP)_
- 7.1 Purpose — combine multiple paths into ONE object with a single shared fill, where overlaps punch holes (the donut/letter-O case)
- 7.2 Make Compound Path — menu Object ▸ Compound Path ▸ Make (+ shortcut, e.g. Ctrl+8) and context menu _(Stage 1 — core/MVP)_
  - 7.2.a Inputs — 2+ selected paths become subpaths of one compound; combined object adopts the BOTTOM-most object's style (Illustrator convention)
  - 7.2.b Result is a single Layers entry; subpaths editable individually with Direct Selection ✅ (have basic)
- 7.3 Release Compound Path — Object ▸ Compound Path ▸ Release (+ shortcut, e.g. Alt+Ctrl+8) — splits back into independent paths; holes re-fill solid _(Stage 1 — core/MVP)_
- 7.4 Fill rule — defines which overlaps are holes vs fills _(Stage 1 — core/MVP)_
  - 7.4.a Nonzero winding — hole only when an inner subpath winds OPPOSITE the outer; same-direction overlaps stay filled
  - 7.4.b Even-odd — every overlap alternates fill/hole regardless of direction (predictable for designers); expose as the per-object toggle in the inspector
  - 7.4.c Reverse Path Direction control — let the user flip an individual subpath's winding (Attributes-panel style buttons) so nonzero holes behave as intended
  - 7.4.d Default — even-odd is the friendliest default; document the difference visually in UI
- 7.5 Geometry / edge cases _(Stage 2 — standard)_
  - 7.5.a Open subpaths inside a compound — implicitly closed for fill computation but drawn open for stroke; define rendering
  - 7.5.b Nested holes (island-in-a-hole, e.g. letter "e", "B", "8") — winding parity must support multiple nesting levels
  - 7.5.c Non-overlapping members — simply share one fill, no holes
  - 7.5.d Releasing loses which-was-hole info — all become solid; warn if many subpaths
  - 7.5.e Stroke on compound — single stroke follows ALL subpath contours (including hole edges)
- 7.6 Relation to Pathfinder — Pathfinder "Unite/Minus Front" outputs may BE compound paths; ensure boolean results round-trip cleanly into this system ✅ (boolean engine exists)

**8. Clipping Mask** _(Stage 1 — core/MVP)_
- 8.1 Purpose — use the TOPMOST selected object's shape as a window; everything below is shown only inside that shape (clipped, not destroyed)
- 8.2 Make Clipping Mask — Object ▸ Clipping Mask ▸ Make (+ shortcut, e.g. Ctrl+7) + context menu _(Stage 1 — core/MVP)_
  - 8.2.a Mask object = the front-most selected path; its geometry defines the clip region; its own fill/stroke are dropped to none (becomes invisible boundary) per AI default
  - 8.2.b Clipped content = all other selected objects below it, grouped into a "Clip Group"
  - 8.2.c Layers representation — a "Clip Group" node with an underlined/marked mask child; show a clip indicator icon ✅ (have basic, layers panel exists)
- 8.3 Release Clipping Mask — Object ▸ Clipping Mask ▸ Release (+ shortcut) — restores all objects; the former mask returns as a no-fill/no-stroke path (user re-styles if wanted) _(Stage 1 — core/MVP)_
- 8.4 Geometry / rendering _(Stage 1 — core/MVP)_
  - 8.4.a Clip = intersect each underlying object's drawn pixels/vector coverage with the mask region at RENDER time (non-destructive; underlying geometry untouched)
  - 8.4.b GPU implementation — render to a clip buffer / stencil from the mask shape, then draw children masked (fits the self-drawn GPU canvas)
  - 8.4.c Bounding box — clip group's visible bbox = mask bbox; original child geometry still movable inside
- 8.5 Editing a clip group _(Stage 2 — standard)_
  - 8.5.a Enter the group (double-click / isolation) to move/restyle the mask shape or the clipped contents independently
  - 8.5.b Edit Mask vs Edit Contents toggle (Affinity-style) — pick which you're transforming
  - 8.5.c Add objects into an existing clip group; reorder; the mask stays topmost
- 8.6 Mask object rules & edge cases _(Stage 2 — standard)_
  - 8.6.a Valid mask = a single vector path / compound path / text outline; a GROUP can't be the mask directly (use a compound path) — validate and message
  - 8.6.b Multiple shapes as one mask — require Compound Path first (ties this system to #7); enforce or auto-offer
  - 8.6.c Open path as mask — implicitly closed for the clip region
  - 8.6.d Stroke/effects on mask — ignored for the clip boundary (only the fill silhouette clips); optionally preserve a copy if user wants the outline visible
  - 8.6.e Nested clip groups — a clip group inside another clip group (compose the clip buffers)
  - 8.6.f Empty/zero-area mask — nothing shows; warn
- 8.7 Layer Clip vs Object Clip _(Stage 3 — advanced)_
  - 8.7.a Clip an entire layer (top object of the layer masks the whole layer) — Layers-panel "make clipping mask" button
  - 8.7.b Distinguish object-level clip group from layer-level clip in the Layers UI
- 8.8 Interplay — clipping mask + compound path + Pathfinder must compose (a compound path as a mask; a clip group inside a boolean is invalid → message)

**9. Cross-cutting concerns for the whole system** _(spans stages)_
- 9.1 Selection & isolation behavior — all destructive tools (Scissors/Knife/Eraser) respect lock/hide; respect "selected only" vs "all" scope consistently _(Stage 1 — core/MVP)_
- 9.2 Style inheritance rules table — codify, per operation, whose fill/stroke wins (compound=bottom, clip=hidden mask, Shape Builder=configurable, cut pieces=inherit original) so behavior is predictable _(Stage 2 — standard)_
- 9.3 Numeric/exact entry parity — Line Segment dialog now; later expose exact-cut coordinates for Scissors if needed _(Stage 3 — advanced)_
- 9.4 Snapping integration — endpoints, cut points, blade/eraser strokes snap to anchors/guides/grid when snapping is on _(Stage 2 — standard)_
- 9.5 Performance — cache the planar arrangement (Shape Builder) and re-flatten only on selection/zoom change; throttle live previews for Knife/Eraser on big artwork _(Stage 2 — standard)_
- 9.6 Robustness — degenerate-geometry guards (zero-length, coincident anchors, self-intersections, near-tangent intersections) handled uniformly via the shared tolerance constants _(Stage 1 — core/MVP)_
- 9.7 Menus & discoverability — Object menu (Compound Path, Clipping Mask), tool rail flyouts (cut cluster), context-menu entries, and keyboard shortcuts all wired; tool-options dialogs on double-click _(Stage 2 — standard)_

---

## 10. Text / Type system
*The full typography stack — point/area/path text objects, a GPU text engine (shaping + line layout + glyph rendering), and the Character/Paragraph/OpenType/Glyphs/Styles panels that drive them, Latin-first with Arabic shaping flagged as the later moat.*

**1. Text objects & creation tools** _(Stage 1 — core/MVP)_
- 1.1 Type tool (T) — primary entry; click = point text, click-drag = area text box
  - 1.1.a Cursor: I-beam; over closed path → switches to Area Type cursor; over open path → Type-on-Path cursor
  - 1.1.b Click on empty canvas → caret + empty point-text object; placeholder/sample text option (insert "Lorem ipsum" on create, toggleable in prefs)
  - 1.1.c Click-drag → fixed-size area text frame at drag bounds
  - 1.1.d Click an existing text object with Type tool → enters edit mode at clicked glyph (caret placement by hit-test)
  - 1.1.e Esc / Cmd-Enter / click-away with Selection tool → commit & exit edit mode
- 1.2 Point text (auto-sizing) — width grows with content, no wrap until explicit return
  - 1.2.a Single anchor/origin; bounding box hugs text; resizing box scales the type (Illustrator behavior) vs reflow (note both modes, default = scale)
  - 1.2.b Hard returns are the only line breaks
- 1.3 Area / paragraph text — fixed frame, text wraps to width, overflows vertically
  - 1.3.a Resize handles reflow text (do NOT scale glyphs) ✅ (have basic bbox handles to reuse)
  - 1.3.b Overflow indicator: red "+" / overset marker on out-port when text exceeds frame
  - 1.3.c "Auto-size" frame option (Affinity): frame height grows to fit, or grow both directions
- 1.4 Vertical type tool _(Stage 3 — advanced)_ — top-to-bottom, right-to-left columns (CJK); separate tool variant
- 1.5 Convert point ↔ area text _(Stage 2 — standard)_ — toggle via small widget on bbox edge (double-click handle) or menu Type > Convert
- 1.6 Type-on-a-path tool _(Stage 2 — standard)_ — see section 4
- 1.7 Area-type / point-type tool variants in tool rail flyout _(Stage 2)_ — grouped under Type tool ✅ (tool rail exists)
- 1.8 Touch Type tool _(Stage 3 — advanced)_ — per-glyph transform without outlining; select single character → handles to move/scale/rotate/baseline-shift it while text stays editable (see section 13)

**2. Text engine — shaping, layout & rendering** _(Stage 1 core for Latin; Arabic = Stage 3 moat)_
- 2.1 Font loading & management
  - 2.1.a System font enumeration (Windows: DirectWrite/font dir scan) + bundled defaults
  - 2.1.b Embedded/document fonts; user-added fonts (load .ttf/.otf/.ttc/.woff2 from disk)
  - 2.1.c Variable fonts: expose named instances + axes (wght, wdth, opsz, slnt, ital, custom) 🟡 (memory notes wght slider existed in old engine)
  - 2.1.d Font fallback chain when glyph missing (notdef → fallback font stack → tofu box)
  - 2.1.e Missing-font handling: substitute + highlight (pink/yellow highlight), "Resolve missing fonts" dialog
- 2.2 Shaping (glyph selection & positioning) — rent HarfBuzz (per memory: OSS math)
  - 2.2.a Unicode → glyph mapping via cmap; ligatures, contextual forms, mark positioning
  - 2.2.b OpenType GSUB (substitution) + GPOS (positioning/kerning) feature application
  - 2.2.c Bidi algorithm (UAX #9) for mixed LTR/RTL runs _(Stage 3 — Arabic moat)_
  - 2.2.d Arabic joining/shaping (init/medi/fina/isol), kashida/justification elongation, mark stacking _(Stage 3 — deferred moat-deepener; flagged in memory as known-hard)_
  - 2.2.e Script & language itemization (split text into runs by script/font/direction/style)
  - 2.2.f Cluster model: caret/selection must respect grapheme clusters, not raw codepoints
- 2.3 Line layout / line breaking
  - 2.3.a Glyph advance accumulation → line width; wrap at frame edge (area text)
  - 2.3.b Break opportunities: whitespace, UAX #14 line-break classes, soft hyphen (U+00AD), ZWSP
  - 2.3.c Greedy line breaker _(Stage 1)_; optional paragraph-level optimal/Knuth-Plass breaker _(Stage 3)_
  - 2.3.d Leading/line-height resolution (auto = % of size, e.g. 120%; or fixed) → baseline grid positions
  - 2.3.e First-baseline placement options (ascent, cap height, leading, x-height, fixed) — area-text inset
  - 2.3.f Tab stops, tab character measurement, decimal/center/right tab alignment _(Stage 2/3)_
  - 2.3.g Hanging punctuation / optical margin alignment _(Stage 3)_
- 2.4 Glyph rendering on the GPU (self-drawn canvas — per memory, never DOM)
  - 2.4.a Outline extraction from font (glyf/CFF) → bezier contours → tessellate or render via GPU vector path (reuse existing path/fill pipeline) ✅ (have fill/path rendering)
  - 2.4.b Anti-aliasing: grayscale AA; optional subpixel/LCD (note: complicates compositing — likely grayscale-only)
  - 2.4.c Glyph atlas / SDF cache vs direct path fill — choose: cached SDF atlas for small text, vector fill for large/scaled
  - 2.4.d Hinting: typically none for design tools (resolution-independent); rely on AA
  - 2.4.e Anti-alias setting per text object: None / Sharp / Crisp / Strong (Illustrator parity)
  - 2.4.f Sub-pixel positioning so zoom/transform stays crisp
- 2.5 Caret & selection model _(Stage 1)_
  - 2.5.a Caret hit-testing: x/y → text index; index → x/y for caret draw
  - 2.5.b Selection highlight rectangles per line run
  - 2.5.c Caret movement: left/right by cluster, up/down by visual line, word (Ctrl), line start/end (Home/End), doc start/end
  - 2.5.d Bidi caret behavior (logical vs visual movement) _(Stage 3)_
- 2.6 IME / composition input _(Stage 2)_ — composition string underline, candidate window position (CJK/Arabic)

**3. Area / container text features** _(Stage 2 — standard)_
- 3.1 Frame insets (text inset spacing) — uniform or per-side (top/right/bottom/left) padding inside frame
- 3.2 Vertical alignment within frame — top / center / bottom / justify (distribute lines)
- 3.3 Columns & rows _(Stage 2/3)_
  - 3.3.a Number of columns, gutter width, column span
  - 3.3.b Rows (CJK / grids), gutter
  - 3.3.c Auto-flow direction; balance columns
- 3.4 First-baseline offset control (see 2.3.e) + min value
- 3.5 Text threading / linked frames _(Stage 3 — advanced)_
  - 3.5.a In-port / out-port on frame; click out-port → load cursor → click/drag next frame to link
  - 3.5.b Thread visualization: blue link lines between frames (View > Show Text Threads)
  - 3.5.c Reflow across threaded chain; insert/remove frame mid-thread; release/unlink frame
  - 3.5.d Overset text carries to next frame; final out-port shows overset "+"
- 3.6 Text wrap around objects _(Stage 3)_ — wrap text around a shape's bounds with offset; wrap on/off per object
- 3.7 Resize behavior — reflow vs scale; "auto-size frame" toggle (shrink/grow to fit)

**4. Type on a path** _(Stage 2 — standard)_
- 4.1 Apply: Type-on-Path tool / click open or closed path with Type tool → text flows along path
- 4.2 Brackets: start bracket, center (flip) bracket, end bracket — drag to set start/end position & flip text to other side of path
- 4.3 Path-type effects (Illustrator): Rainbow / Skew / 3D Ribbon / Stair Step / Gravity
- 4.4 Align to path: baseline / ascender / descender / center relative to the path
- 4.5 Spacing tightening on sharp curves (adjustable spacing value)
- 4.6 Flip text across the path; reverse direction
- 4.7 Underlying path stays editable (stroke/fill of path independent of type) — path can be hidden (no paint) while carrying text

**5. Character panel** _(Stage 1 core; advanced fields Stage 2)_
- 5.1 Font family — searchable dropdown with live preview, recent fonts, favorites/star, filter by classification (serif/sans/script/mono), variable-font badge ✅ (have basic FontPicker concept from memory)
- 5.2 Font style / weight — subfamily dropdown (Regular/Bold/Italic/etc.) or variable-axis sliders 🟡
- 5.3 Font size — numeric + dropdown presets + up/down stepper + scrub; units (px/pt) ✅ (size exists)
- 5.4 Leading / line-height — numeric (auto vs absolute), "Auto" toggle, % option
- 5.5 Kerning — Auto (font) / Optical / Metrics / Manual value (between two glyphs at caret); units = 1/1000 em
- 5.6 Tracking (letter-spacing) — range applied to selection; 1/1000 em
- 5.7 Horizontal scale (H) — % width distortion
- 5.8 Vertical scale (V) — % height distortion
- 5.9 Baseline shift — raise/lower selected glyphs from baseline (pt), for sub/superscript fine-tune
- 5.10 Character rotation — rotate individual glyphs in place (degrees)
- 5.11 Case controls — All Caps, Small Caps (true SC via OT or synthetic), Superscript, Subscript (buttons)
- 5.12 Underline + Strikethrough — toggle buttons; advanced: underline weight/offset/color, strike style _(Stage 2/3)_
- 5.13 Anti-alias method dropdown — None / Sharp / Crisp / Strong (per text object)
- 5.14 Language assignment — for hyphenation + spell-check + locl OT features
- 5.15 Set as default / load-from-selection (eyedropper for type attributes) _(Stage 2)_
- 5.16 Color/fill & stroke of type — fill color, stroke color + weight (links to fill/stroke system) ✅ (fill/stroke exists; needs per-character application)
- 5.17 No-break toggle (prevent line break in selection) _(Stage 2)_
- 5.18 Panel chrome — collapsible "show options" for advanced fields; flyout menu; scrub-to-change on labels

**6. Paragraph panel** _(Stage 1 for alignment; rest Stage 2)_
- 6.1 Alignment — left / center / right ✅ (concept), justify-last-left / justify-last-center / justify-last-right / justify-all (force)
- 6.2 Indents — left indent, right indent, first-line indent (can be negative for hanging indent)
- 6.3 Space before paragraph / space after paragraph
- 6.4 Hyphenation _(Stage 2/3)_ — on/off; words longer than N, after first N chars, before last N chars, max consecutive hyphens, hyphenation zone, hyphenate capitalized words toggle, hyphenation dictionary per language
- 6.5 Justification settings dialog _(Stage 3)_ — min/desired/max word spacing, letter spacing, glyph scaling; auto-leading %; single-word justification
- 6.6 Composer choice _(Stage 3)_ — single-line vs paragraph (multi-line/optimal) composer
- 6.7 Drop caps _(Stage 3)_ — number of lines, number of characters
- 6.8 Roman hanging punctuation toggle; East-Asian options (kinsoku, burasagari, mojikumi) _(Stage 3)_
- 6.9 Paragraph direction (LTR/RTL) per paragraph _(Stage 3 — Arabic moat)_
- 6.10 Bullets & numbering / lists _(Stage 3)_ — list type, glyph, indent, numbering format
- 6.11 Tab ruler / tabs panel _(Stage 2/3)_ — left/center/right/decimal tab stops, leader characters, drag on ruler

**7. OpenType features panel/controls** _(Stage 2 — standard)_
- 7.1 Standard ligatures (liga) on/off; Discretionary ligatures (dlig); Contextual alternates (calt); Historical (hlig)
- 7.2 Figure styles — Tabular vs Proportional × Lining vs Oldstyle (lnum/onum/tnum/pnum)
- 7.3 Fractions (frac), Ordinals (ordn), Superscript/Subscript (sups/subs), Numerator/Denominator (numr/dnom)
- 7.4 Stylistic sets (ss01–ss20) — toggle list with previews; Stylistic alternates (salt)
- 7.5 Swashes (swsh), Titling alternates (titl), Small caps (smcp) / Caps-to-small-caps (c2sc)
- 7.6 Slashed zero (zero), Localized forms (locl), Case-sensitive forms (case)
- 7.7 Position dropdown (default/superscript/subscript/numerator/denominator) — Illustrator-style
- 7.8 Feature availability greyed-out when font lacks the feature; show only supported features
- 7.9 Arabic/Indic OT features surface later _(Stage 3 — moat)_

**8. Glyphs panel** _(Stage 2 — standard)_
- 8.1 Grid of all glyphs in current font; double-click to insert at caret
- 8.2 Show subset filter — Entire font / Alternates for current selection / Ligatures / Numbers / Punctuation / Symbols / Ornaments / by OT feature
- 8.3 Recently used glyphs row
- 8.4 Glyph alternates popup — small triangle on a glyph reveals contextual alternates inline in the canvas (in-context alternates picker while editing)
- 8.5 Search glyph by name / Unicode codepoint; show Unicode + glyph ID on hover
- 8.6 Font + style switcher inside panel; size zoom slider for the grid
- 8.7 Insert by Unicode value; copy glyph

**9. Character & Paragraph styles** _(Stage 2 — standard; nested overrides Stage 3)_
- 9.1 Character styles panel — named reusable run-level attribute sets (font/size/color/tracking/case/OT…)
- 9.2 Paragraph styles panel — named paragraph-level sets (everything in 6 + character attrs)
- 9.3 Create style from selection; redefine style from current overrides; duplicate; delete
- 9.4 Apply style to selection/paragraph; "Normal/Basic Paragraph" default base style
- 9.5 Override indicator (+) when local formatting differs from style; "clear overrides" button
- 9.6 Style inheritance / "based on" parent style; "next style" for auto-chaining _(Stage 3)_
- 9.7 Edit style dialog: full attribute editor mirroring Character + Paragraph + OT
- 9.8 Load/import styles from another document _(Stage 3)_
- 9.9 Style groups/folders, sort, search _(Stage 3)_

**10. Find / Replace font & text** _(Stage 2 — standard)_
- 10.1 Find/Replace Font dialog — list fonts used in document (with type: system/missing/embedded), replace one font with another across doc, "selection only" scope
- 10.2 Show font usage count + jump-to-next instance
- 10.3 Find & Replace text — match case, whole word, regex/wildcards, search within selection/doc, replace all
- 10.4 Replace special chars (returns, tabs, em-dash) and formatting _(Stage 3)_
- 10.5 Spell check _(Stage 3)_ — per-language dictionary, underline misspellings, suggestions, ignore/add-to-dictionary, auto-correct

**11. Text to outlines (Create Outlines)** _(Stage 1 — core; per memory flagged feasible/needs engine)_
- 11.1 Type > Create Outlines (Shift-Ctrl-O) — convert selected text to editable vector paths (compound paths preserved, counters = holes) ✅ (have boolean/compound-path + path engine to reuse)
- 11.2 Result is a group of compound paths (one per glyph), fully editable with pen/anchor tools ✅
- 11.3 Non-destructive guard: warn it's irreversible (text no longer editable); keep undo
- 11.4 Outline a copy (keep editable text behind) option
- 11.5 Preserves fill/stroke/appearance; maintains kerning/positioning baked at outline time
- 11.6 Optional "object > flatten/expand" path for stroked text → outlined stroke

**12. Editing interactions & selection in text** _(Stage 1 — core)_
- 12.1 Caret placement by click; click-drag to select range; shift-click extend
- 12.2 Double-click = select word; triple-click = select line/paragraph; quad-click = select all paragraph; Ctrl/Cmd-A = select all in object
- 12.3 Standard editing: type/insert, delete/backspace (cluster-aware), cut/copy/paste, paste-without-formatting, paste-with-formatting
- 12.4 Drag-and-drop text move within object _(Stage 2)_
- 12.5 Smart quotes / autocorrect on input (curly quotes, em/en dash) — toggle in prefs _(Stage 2)_
- 12.6 Word/character/line count readout _(Stage 3)_
- 12.7 Insert special characters menu — em/en dash, non-breaking space, ellipsis, em/en space, hair space, ZWJ/ZWNJ, soft hyphen _(Stage 2; ZWJ/ZWNJ matters for Arabic later)_
- 12.8 Show hidden characters (¶, spaces, tabs) toggle _(Stage 2)_
- 12.9 Keyboard nudges for size/leading/tracking/baseline (Alt+arrows etc.) — Illustrator increments

**13. Touch Type tool (per-glyph live editing)** _(Stage 3 — advanced)_
- 13.1 Select one character → on-canvas bounding box with 4 handles
- 13.2 Move (drag), scale (corner = uniform, side = H/V scale → maps to baseline-shift / H-V scale / size), rotate (top handle)
- 13.3 Stays live editable text (attributes stored per-character, no outlining)
- 13.4 Multi-touch / pointer support; works with point & area text

**14. Defaults, preferences & integration** _(Stage 1 for defaults; rest Stage 2)_
- 14.1 Default type attributes (font/size/color/leading) for new text objects; "set as default"
- 14.2 Type preferences pane — size/leading/tracking/baseline increments, greeking threshold, font preview size, recent-fonts count, smart-quotes, missing-glyph protection, enable Asian/Middle-East options
- 14.3 Units & measurement for type (pt/px/mm) consistent with doc units
- 14.4 Schema/RNA integration — every type attribute exposed in single schema (per memory: file + AI + plugins + inspector all read same properties) so inspector fields, .varos file, and plugin API stay in sync
- 14.5 Undo/redo granularity for typing (coalesce keystrokes into sensible undo steps) ✅ (have undo/redo)
- 14.6 Selection-tool interaction — text object behaves as a normal object for move/scale/rotate/align/z-order/group/boolean(after outline) ✅ (reuse existing selection, bbox, layers, align, z-order)
- 14.7 Inspector surfacing — Character/Paragraph live in the floating inspector panel ✅ (inspector shell exists); collapsible sections
- 14.8 Export/render parity — text renders identically in export pipeline (and as outlines for SVG/PDF font-embedding choices) — note export-weight/parity lesson from memory
- 14.9 Copy/paste type attributes via eyedropper ✅ (eyedropper exists — extend to type attributes)

**15. Arabic / complex-script moat (explicit later phase)** _(Stage 3 — advanced / dedicated moat work)_
- 15.1 Full bidi + RTL paragraph direction, RTL caret/selection visual order
- 15.2 Correct Arabic joining/shaping with proper letter connection (memory: current/known weak point vs Figma)
- 15.3 Kashida (tatweel) justification — elongate connections instead of word-spacing
- 15.4 Harakat / diacritic mark positioning & stacking; mark-to-base/mark-to-mark GPOS
- 15.5 Arabic-aware OT features (init/medi/fina/isol, required ligatures, Quranic forms)
- 15.6 Mixed Arabic+Latin runs, Arabic numerals (Eastern/Western) + locl
- 15.7 Language-specific tracking that doesn't break joining (memory: "joining-safe tracking" lesson)
- 15.8 Treated as the differentiator — built on the same engine once Latin pipeline is solid

---

## 11. Effects / Appearance System
*A non-destructive appearance stack per object — multiple fills/strokes, live Effects (shadow, glow, blur, distort, warp, 3D, round corners), Graphic Styles, and Expand Appearance.*

**1. Core concept — the non-destructive appearance stack model** _(Stage 1 — core/MVP)_
- 1.1 Object geometry vs. appearance — the editable vector path (anchors/handles) is the "source of truth"; appearance is a render recipe layered on top, never baked into geometry until Expand. ✅ (have basic — fill/stroke apply exists but as flat single attributes, not a stack)
- 1.2 Appearance record per target — every object/group/layer holds an ordered appearance tree: a list of Fill entries, Stroke entries, object-level Effects, and per-attribute Effects, plus base Opacity & Blend Mode.
- 1.3 Render order is top→bottom in the panel = back-to-front on canvas — the topmost panel row paints LAST (on top); bottom row paints first (behind). Reordering rows visibly changes stacking.
  - 1.3.a Each Fill/Stroke row is its own paint pass over the SAME geometry (so a thin stroke can sit above a fat stroke above a fill).
  - 1.3.b "Contents" row = the object's actual geometry/children, positioned in the stack relative to fills/strokes.
- 1.4 Targets (what an appearance can attach to) — Object, Group, Layer; group/layer appearances cascade to children but are stored on the container (the "targeting" dot in the layers panel).
- 1.5 Live vs. expanded — Effects recompute on any edit (move anchor, scale, change color) until the user runs Expand Appearance, which converts the recipe into real geometry/raster.
- 1.6 Evaluation pipeline (build this as the spine) — for each render: take geometry → apply per-attribute effects to that attribute → composite Fill/Stroke rows in stack order → apply object-level effects to the composited result → apply base opacity/blend → emit to GPU. 🟡 (partial — flat fill/stroke compositing exists; no effect stage, no multi-row)
- 1.7 Caching & dirtying — cache each effect's output; invalidate only the affected sub-tree on edit (don't recompute the whole stack); raster effects cached at current resolution.
- 1.8 Data/schema (Blender-RNA-style single schema so file + AI + inspector + plugins all read it) — appearance = typed node list; each node = {type, enabled, params, blendMode, opacity, children?}; must round-trip to the `.varos` file losslessly.

**2. Appearance panel — structure & rows** _(Stage 1 — core for fills/strokes; effect rows Stage 2)_
- 2.1 Panel header — shows current target name + thumbnail ("Path", "Group", "Layer", "Type"); reflects single vs. multi-selection.
- 2.2 Row list (each row = one appearance attribute) — drag-handle, visibility eye (enable/disable without delete), attribute swatch/label, optional disclosure triangle for nested effects, FX badge.
  - 2.2.a Fill row — color/gradient/pattern swatch + opacity + blend mode; click swatch opens fill editor.
  - 2.2.b Stroke row — stroke swatch + weight + opacity + blend mode; click "Stroke" label opens stroke options (width, caps, joins, dashes, align, arrowheads, profile).
  - 2.2.c Opacity row (object-level) — base opacity + blend mode for the whole object (always present at bottom).
  - 2.2.d "Contents"/Characters row — appears for groups/type; lets you target sub-content.
- 2.3 Selecting a row — clicking a row sets the "active attribute" so newly added effects/colors apply to THAT row (per-attribute) vs. the whole object.
- 2.4 Panel footer toolbar (Illustrator-parity buttons) — Add New Stroke, Add New Fill, Add New Effect (fx), Clear Appearance, Reduce to Basic Appearance, Duplicate Selected Item, Delete Selected Item.
- 2.5 Reordering — drag rows up/down to restack paints/effects; live canvas preview while dragging.
- 2.6 Per-row context menu — Duplicate, Delete, Add Effect to this attribute, Remove effects from this attribute, Move to front/back.
- 2.7 Empty/basic state — "basic appearance" = one fill + one stroke + opacity (matches what Varos has today); panel should make the upgrade path obvious.
- 2.8 Multi-select behavior — show shared rows; mixed values shown as "Mixed"; edits apply to all selected.
- 2.9 Panel options — toggle "New Art Has Basic Appearance" (whether new objects inherit the last-used complex appearance or reset to basic). ✅ (have basic — default fill/stroke logic exists)

**3. Multiple fills & strokes per object** _(Stage 2 — standard)_
- 3.1 Add multiple Fill rows — stacked, each with own color/gradient/pattern, opacity, blend mode; enables "color overlays" and multi-tone effects on one path.
- 3.2 Add multiple Stroke rows — e.g., a wide dark stroke under a thin light stroke for a "lined" look; each with full stroke params + its own profile/dashes.
- 3.3 Offset per paint via effects — a fill/stroke can carry a Transform or Offset Path effect so identical geometry renders shifted (drop-line, double-outline tricks).
- 3.4 Per-paint blend mode & opacity — independent of object opacity; composited within the stack.
- 3.5 Per-paint effects — drop an effect (e.g., blur, transform) on a single Fill or Stroke row only, leaving siblings sharp.
- 3.6 Stroke ordering relative to fill — stroke above fill (default) or fill above stroke (inset look) just by reordering rows.
- 3.7 Use cases to validate — neon (blur fill behind sharp fill), embossed text, sketch/multi-outline, retro layered shadows.

**4. Effects menu — the catalog (live, non-destructive)** _(Stage tag per effect below)_
- 4.1 Apply target — effect applies to active row (per-attribute) OR whole object/group/layer if no row is active.
- 4.2 Effect dialog conventions — every effect dialog has live Preview checkbox, OK/Cancel/Reset, numeric inputs + sliders, and unit awareness (px/mm/pt).
- 4.3 Re-edit — double-click the effect row (or "Effect > [last]") reopens its dialog with current params; never re-applies destructively.
- 4.4 "Apply Last Effect" (re-run with same params) + "Last Effect…" (reopen dialog) menu entries + shortcuts.
- 4.5 Document Raster Effects Settings — global resolution (72/150/300 ppi), background (white/transparent), anti-alias, clipping mask, add-around-object bleed, preserve spot colors — governs ALL raster-based effects (shadows, blurs, glows). _(Stage 2)_

**5. Stylize effects group** _(mixed stages)_
- 5.1 Drop Shadow — Mode/blend, Opacity, X/Y Offset, Blur radius, Color vs. Darkness %, optional "Create Separate Shadows" per object. _(Stage 2 — standard; highest-demand effect)_
- 5.2 Inner Glow — blend, opacity, blur, color, Center vs. Edge source. _(Stage 2)_
- 5.3 Outer Glow — blend, opacity, blur, color. _(Stage 2)_
- 5.4 Feather — softens edges by a radius (vector-to-alpha falloff). _(Stage 2)_
- 5.5 Round Corners — rounds path corner anchors by a radius, live (re-editable, geometry untouched). _(Stage 1 — cheap, pure-vector, high value; pairs with existing shape tools)_ 🟡 (partial — shapes exist but no live corner-round effect)
- 5.6 Scribble — angle, path overlap, variation, stroke width, curviness, spacing; turns fills into hand-drawn scribble strokes. _(Stage 3 — advanced)_
- 5.7 Glow/shadow color picker + "use object color" option across the above.

**6. Distort & Transform effects group (pure-vector, GPU-cheap)** _(Stage 2 standard; some Stage 3)_
- 6.1 Transform effect — Scale (H/V), Move (H/V), Rotate, Reflect (X/Y), plus Copies (n) for repeat patterns, Random checkbox, Reflect-each, transform-objects/patterns toggles, 9-point reference anchor. _(Stage 2 — core enabler for "multiple fills offset", radial repeats)_
- 6.2 Pucker & Bloat — slider −/+ to pull anchors inward (spiky) or push curve outward (bulge). _(Stage 2)_
- 6.3 Roughen — Size (% or absolute), Detail (points/inch), Smooth vs. Corner points; live jitter of the path. _(Stage 2)_
- 6.4 Zig Zag — Size, Ridges per segment, Smooth (wave) vs. Corner (zig-zag) points. _(Stage 2)_
- 6.5 Tweak — random shift of anchors & control handles (H/V amounts, anchor/handle toggles). _(Stage 3)_
- 6.6 Twist — angle-based rotation that increases toward the center. _(Stage 3)_
- 6.7 Free Distort — drag 4 corner handles in a dialog to perspective/shear the bounding box live. _(Stage 3)_

**7. Warp effects group (envelope-style, live)** _(Stage 3 — advanced)_
- 7.1 15 warp presets — Arc, Arc Lower, Arc Upper, Arch, Bulge, Shell Lower, Shell Upper, Flag, Wave, Fish, Rise, Fisheye, Inflate, Squeeze, Twist.
- 7.2 Common params — Horizontal/Vertical orientation, Bend %, Horizontal Distortion %, Vertical Distortion %.
- 7.3 Live re-edit on text without rasterizing (text stays editable underneath). 🟡 (partial — depends on text system existing)
- 7.4 Relationship to true Envelope Distort (mesh/top-object) — note as a separate but adjacent system; warp-as-effect is the non-destructive entry point.

**8. 3D & Materials effects group** _(Stage 3 — advanced, build last)_
- 8.1 Extrude & Bevel — extrude depth, cap on/off, bevel shape + height, rotation (X/Y/Z or presets), perspective, surface shading (no/diffuse/plastic), light sources, blend steps. ✅/❌ (none — new)
- 8.2 Revolve — angle (0–360°), offset from edge (left/right), cap, surface/lighting controls.
- 8.3 Rotate (3D position only) — orient flat art in 3D space with perspective.
- 8.4 Inflate (newer 3D) — puffy depth on a 2D shape.
- 8.5 Materials/lighting — light position gizmo, ambient/highlight intensity, shading steps, shadow toggle.
- 8.6 Map Art — wrap other artwork/symbols onto extrude/revolve surfaces.
- 8.7 Performance note — heavy; render async, cache the raster, only recompute on param change.

**9. Photoshop/raster effects group (Effect Gallery)** _(Stage 3 — advanced, optional)_
- 9.1 Effect Gallery — Artistic, Brush Strokes, Distort (raster), Sketch, Stylize, Texture, blur (Gaussian) filters applied as live raster effects.
- 9.2 Gaussian Blur as a first-class raster effect (commonly used even by vector purists). _(could pull to Stage 2 if blur is wanted early)_
- 9.3 SVG Filters (apply/import .svg filter primitives) — niche; very late.
- 9.4 Resolution dependence — all governed by Document Raster Effects Settings (4.5); warn users about export scaling.

**10. Convert-to-Shape & Path effects** _(Stage 2)_
- 10.1 Convert to Shape — Rectangle / Rounded Rectangle / Ellipse with absolute or relative-to-bbox sizing (turns any object's fill into a live shape behind it — great for button backgrounds behind text). 
- 10.2 Path effects — Outline Object, Outline Stroke (live), Offset Path (offset amount, joins: miter/round/bevel, miter limit). _(Offset Path is high-value, consider Stage 1.5)_
- 10.3 Crop Marks / Trim Marks effect (live). _(Stage 3)_
- 10.4 These mirror destructive Object menu commands but live — same engine, different commit point.

**11. Per-attribute vs. object-level effect targeting** _(Stage 2 — the model that makes the system "pro")_
- 11.1 Object-level effect — sits in the appearance tree BELOW all fills/strokes (or as a top "fx" on the whole object); affects the composited result (e.g., one Drop Shadow for the whole icon).
- 11.2 Per-attribute effect — nested UNDER a specific Fill or Stroke row; only that paint is affected (e.g., blur only the back fill for a glow).
- 11.3 Visual cue — nested effect rows indented under their parent paint, with their own eye/reorder/delete.
- 11.4 Stacking multiple effects on one attribute — they chain in order (output of one feeds the next); reorderable.
- 11.5 Add via panel — select row → "Add New Effect (fx)" applies per-attribute; deselect rows → applies object-level.

**12. Graphic Styles (save / apply an appearance)** _(Stage 2 — standard)_
- 12.1 Concept — a named, reusable snapshot of a full appearance (all fills/strokes/effects/opacity) — like a "master appearance"/CSS class. ✅/❌ (new; analogous to a saved fill/stroke preset)
- 12.2 Graphic Styles panel — grid of swatch thumbnails (each rendered on a sample); list/thumbnail view toggle; default styles library + document styles.
- 12.3 Create style — drag selected object into panel, or "New Graphic Style" captures current appearance; name it.
- 12.4 Apply — select object(s) → click style; replaces (or, with modifier, MERGES/adds) the appearance.
  - 12.4.a Default-click = replace appearance.
  - 12.4.b Alt/Option-click or "Merge" = combine the style's attrs on top of existing.
- 12.5 Live link — editing a graphic style updates ALL objects using it (instance relationship); "break link to graphic style" to detach.
- 12.6 Redefine — update a style from a modified instance ("Redefine Graphic Style").
- 12.7 Manage — duplicate, delete (with "in use" warning), rename, reorder, organize into libraries; import/export style libraries (`.varos`-styles).
- 12.8 Apply to groups/layers — style on a container so children inherit.
- 12.9 Override indicator — show when an instance diverges from its style (like a component override).
- 12.10 Default new-art style — set a graphic style as the default for new objects.

**13. Expand Appearance (commit the recipe to geometry)** _(Stage 2 — needed once effects ship)_
- 13.1 Command — "Object > Expand Appearance" converts the live appearance tree into real, editable artwork (paths + grouped sub-objects), removing all live effects.
- 13.2 Per-attribute expansion — each Fill row → a path; each Stroke row → an outlined path; effects → their computed geometry or embedded raster image.
- 13.3 Raster effects on expand — shadow/blur/glow become a placed raster image at Document Raster Effects resolution (warn user it's no longer resolution-independent).
- 13.4 Result grouping — output wrapped in a group preserving stack order; layers/names preserved where possible.
- 13.5 "Expand" (plain) vs "Expand Appearance" distinction — plain Expand also handles gradients-to-mesh, text-to-path; Expand Appearance specifically flattens the fx stack first.
- 13.6 Partial / non-destructive alternative — keep an original-copy option, or rely on undo; consider "Expand a copy" convenience.
- 13.7 Reversibility — only via Undo; once saved/expanded the recipe is gone (document this clearly in UI).
- 13.8 Engine reuse — Expand reuses the SAME geometry kernels (boolean/offset/outline-stroke/bezier) already built, just committed; this is why the live engine and expand engine must share one code path.

**14. Clear / reduce / utility operations** _(Stage 1–2)_
- 14.1 Clear Appearance — strips to no fill/no stroke (transparent), removes all effects. _(Stage 1)_
- 14.2 Reduce to Basic Appearance — collapses to single fill + single stroke + opacity (drops extra paints & effects). _(Stage 2)_
- 14.3 Duplicate item / Delete item — per-row. _(Stage 1)_
- 14.4 Enable/disable any row via eye without losing its params (toggle effects on/off for comparison). _(Stage 1)_
- 14.5 Copy/paste appearance between objects (eyedropper-appearance: sample full appearance, not just color). 🟡 (partial — basic eyedropper exists for color; extend to full appearance) _(Stage 2)_

**15. Interaction with other systems** _(cross-cutting)_
- 15.1 Layers panel targeting — the "target" circle per row to direct appearance/effects at object vs. group vs. layer; double-ring = has appearance. 🟡 (partial — layers panel exists, needs target indicators)
- 15.2 Selection — black (whole-object) selection edits object/group appearance; direct-select still edits geometry beneath the live effect. ✅ (have basic — selection model exists)
- 15.3 Transform/scale — "Scale Strokes & Effects" preference (do effect sizes scale with the object?); honor during bbox scale. 🟡 (partial — bbox scale exists, needs the toggle)
- 15.4 Undo/redo — every appearance/effect/param change is one undo step; dialog "Preview" must commit cleanly. ✅ (have basic — undo/redo exists)
- 15.5 Pathfinder/booleans — operate on geometry; live effects re-evaluate after a boolean op. ✅ (have basic — boolean engine exists)
- 15.6 Export/raster pipeline — export must rasterize live effects at output scale (not the cached preview res) for fidelity.
- 15.7 Fill/Stroke system — multiple-paint stack supersedes the current single fill+stroke; migrate existing flat model into "basic appearance" cleanly. 🟡 (partial — current flat fill/stroke is the migration source)
- 15.8 Plugins/AI — single schema means an AI command or plugin can read/modify the appearance tree the same way the inspector does.

**16. Performance, caching & edge cases** _(cross-cutting, Stage 2+)_
- 16.1 GPU-first for pure-vector effects (round corners, transform, roughen, zigzag) — recompute geometry on the GPU/CPU kernel, no raster.
- 16.2 Raster effect tiling & resolution — cache at screen res for preview, re-render at export res; handle zoom changes.
- 16.3 Recursion/depth guard — Transform-effect copies + nested effects can explode; cap copies, warn on heavy stacks.
- 16.4 Empty geometry / zero-area paths — effects must no-op gracefully (no NaNs, no crashes).
- 16.5 Effect on open path / on text / on group — define behavior for each (some effects need a fill; warn or auto-handle).
- 16.6 Live-preview throttling — debounce slider drags; show low-res preview during drag, full on release.
- 16.7 Stale-cache invalidation on document raster setting change — re-render all raster effects.
- 16.8 Serialization stability — unknown/future effect types must round-trip (forward-compat) so old builds don't drop data.

**Build-order summary**
- Stage 1 (make it usable): appearance stack data model + evaluation pipeline; panel with fill/stroke/opacity rows, reorder, eye-toggle, add/delete/duplicate; Round Corners + Offset Path (pure-vector); Clear Appearance.
- Stage 2 (pro-standard): multiple fills/strokes; Drop Shadow / Glow / Feather / Blur; Distort & Transform group; per-attribute targeting; Graphic Styles; Expand Appearance; Reduce to Basic; appearance eyedropper; raster settings.
- Stage 3 (advanced): Warp, Scribble, Tweak/Twist/Free Distort, 3D Extrude/Revolve/Inflate + materials/map-art, Photoshop/Effect-Gallery raster filters, SVG filters, crop marks.

---

## 12. Comments / Collaboration system
*Canvas-anchored comment pins with threads, replies, reactions, @mentions, resolve/reopen, a filterable comments panel, presence/cursors, and share/version flows — local-first in the .varos file with an optional backend for real-time multi-user sync.*

**0. Architecture & offline-vs-backend split** _(Stage 1 — core/MVP)_
- 0.1 Local-first foundation — every comment, thread, reply, reaction, mention, resolve-state lives INSIDE the `.varos` file as first-class schema nodes (Blender-RNA-style), so the whole system works 100% offline with zero account/login.
  - 0.1.a Works fully offline: placing pins, threads, replies, reactions, resolve/reopen, the comments panel + filters, anchoring to objects, export/print toggle, local "author identity" (a name/initials/color saved in app prefs, no server).
  - 0.1.b Needs a backend (deferred, post-v1): real-time presence/live cursors, multi-user simultaneous editing, web share links + viewer/commenter roles, push/email notifications, server-side @mention delivery, cross-device sync, hosted version history.
  - 0.1.c Hybrid seam — design the data model so a sync layer can be bolted on later WITHOUT reshaping comment data (same node schema flows to file, AI, plugins, inspector, and eventually the wire).
- 0.2 Comment data model — each comment node carries: stable id, author id, body (rich text), created/edited timestamps, anchor (see 2), thread/parent id, reactions list, resolved flag + resolver/time, mention list, attachment refs, read-by set, soft-delete tombstone.
- 0.3 Identity model _(Stage 1)_ — offline "local author" (display name, initials, avatar color, optional avatar image) stored in prefs; one identity per file edit; upgradeable to a real account when backend lands. ❌ (not built)
- 0.4 Storage & merge _(Stage 2)_ — comments stored append-only with stable ids so file copies/branches merge predictably; CRDT-friendly structure (last-write-wins per field, tombstones for deletes) to prepare for sync. ❌ (not built)
- 0.5 Conflict/merge on reopen of an old file _(Stage 3)_ — when two offline copies diverge, dedupe by id, union reactions, keep both replies, surface "merged" badge. ❌ (not built)

**1. Comment tool & placing pins** _(Stage 1 — core/MVP)_ ❌ (not built)
- 1.1 Comment tool — dedicated tool on the tool rail + shortcut (Illustrator/Figma convention: `C`); activates a comment cursor; does not select/move objects while active.
  - 1.1.a Cursor feedback — speech-bubble/pin cursor; hover highlights the object the pin will anchor to.
  - 1.1.b One-shot vs sticky — modifier or pref to stay in comment mode for multiple pins vs auto-revert to selection after posting.
  - 1.1.c Esc cancels an in-progress (unposted) pin; click-away on empty canvas keeps the composer open or discards per pref.
- 1.2 Place a pin — single click on canvas drops a numbered pin + opens an inline composer popover anchored to the pin.
  - 1.2.a Pin numbering — sequential per file (1,2,3…) for quick verbal reference; numbers persist even after earlier pins resolve (or renumber per pref).
  - 1.2.b Pin appearance — rounded teardrop/bubble marker with author avatar/color; unread = filled/badged, read = outline, resolved = hidden/greyed.
  - 1.2.c Drag-to-place region comment _(Stage 2)_ — click-drag to attach a comment to a rectangular area/marquee instead of a single point.
- 1.3 Composer popover — multiline rich input, Post button, Cancel, character feedback; `Enter` posts, `Shift+Enter` newline (configurable).
  - 1.3.a Rich text in body _(Stage 2)_ — bold/italic, links auto-detected, inline code, lists.
  - 1.3.b Empty-comment guard — Post disabled until non-whitespace content (or a reaction-only pin allowed per pref).
- 1.4 Pin placement modes — free pin on canvas/artboard vs pin pinned to a specific object (see 2 anchoring).
- 1.5 Markup attachments to a comment _(Stage 3)_ — optional freehand scribble / arrow / rectangle drawn near the pin as visual annotation, stored with the comment (Affinity/redline style). ❌ (not built)
- 1.6 Image/file attachment in a comment _(Stage 3)_ — drag an image or file into the composer; stored as embedded blob in the `.varos` (offline) or uploaded ref (backend). ❌ (not built)

**2. Anchoring & spatial behavior** _(Stage 1 — core/MVP)_ ❌ (not built)
- 2.1 Canvas-space anchor — pin stored in document coordinates so it tracks pan/zoom and stays put on the artwork (not screen-fixed).
- 2.2 Object anchoring _(Stage 2)_ — pin bound to an object id + relative offset; when the object moves/scales/rotates, the pin follows.
  - 2.2.a Re-anchor on edit — pin offset re-computed when the bound object's bbox changes.
  - 2.2.b Orphaned pin handling — if the anchored object is deleted, pin detaches to its last canvas position and shows an "orphaned" indicator (not silently lost).
- 2.3 Artboard/page association — each pin records which artboard/page it sits on for filtering and navigation.
- 2.4 Zoom-independent rendering — pin marker drawn at constant screen size regardless of zoom (like selection handles), while its anchor point stays in doc space.
- 2.5 Clustering/declutter _(Stage 3)_ — overlapping pins collapse into a "+N" cluster badge that expands on hover/click.
- 2.6 Off-screen indicator _(Stage 2)_ — when a pin with activity is outside the viewport, show an edge arrow/pip pointing toward it.

**3. Threads, replies & editing** _(Stage 1 — core/MVP)_ ❌ (not built)
- 3.1 Thread = root comment + ordered replies; clicking a pin opens the thread popover.
- 3.2 Reply — composer at the bottom of the thread; replies stamped with author + relative time ("2m ago", hover = absolute).
- 3.3 Edit own comment/reply — inline edit; shows "(edited)" + edited timestamp; only the author can edit (offline: anyone, since single local identity — gated once backend identities exist).
- 3.4 Delete — soft-delete with "comment deleted" tombstone; deleting a root either deletes the whole thread or keeps replies per pref/confirm dialog.
  - 3.4.a Undo — delete/resolve participate in the global undo/redo stack (✅ have undo/redo engine to hook into).
- 3.5 Thread ordering — chronological; optional "newest first" pref; scroll within long threads.
- 3.6 Quote/reply-to-specific-message _(Stage 3)_ — reply referencing a particular earlier message in the thread.
- 3.7 Copy link to a comment/thread _(Stage 2, full value needs backend)_ — copies a deep link that focuses that pin (locally: focuses pin; with backend: shareable URL).

**4. Reactions & emoji** _(Stage 2 — standard)_ ❌ (not built)
- 4.1 Emoji reactions on any comment/reply — quick-react bar (👍 ❤️ 😄 🎉 👀 etc.) + full emoji picker.
- 4.2 Reaction tally — grouped counts with hover tooltip listing who reacted; toggle on/off your own reaction.
- 4.3 Reaction-only acknowledgement — react without replying (useful for "seen/agree").
- 4.4 Custom/recent emoji _(Stage 3)_ — recently-used row, search, skin-tone modifiers.

**5. Resolve / reopen** _(Stage 1 — core/MVP)_ ❌ (not built)
- 5.1 Resolve a thread — checkmark on the thread header; records resolver + timestamp.
- 5.2 Resolved behavior — pin hidden from canvas by default (and from "open" filter), thread archived but recoverable.
- 5.3 Reopen — un-resolve restores the pin and moves it back to open.
- 5.4 Auto-collapse vs auto-hide pref — show resolved as greyed pins vs fully hidden.
- 5.5 Resolve confirmation/undo — toast with Undo; resolve/reopen logged in thread activity.
- 5.6 Bulk resolve _(Stage 2)_ — multi-select threads in the panel and resolve/delete together.

**6. @Mentions** _(Stage 2 — standard, delivery needs backend)_ ❌ (not built)
- 6.1 `@` trigger in composer — typeahead list of collaborators; insert as a styled chip referencing a user id.
  - 6.1.a Offline — mentions resolve against a local "people" list (names you've typed/imported); stored as text+id, no delivery.
  - 6.1.b Backend — mention resolves against real workspace members and triggers a notification (see 10).
- 6.2 Mention rendering — highlighted chip in the comment body; click scrolls to / shows that person.
- 6.3 @here / @everyone-style group mention _(Stage 3)_ — notify all current collaborators (backend only).
- 6.4 Mention autocomplete details — fuzzy match on name/handle, avatar in the dropdown, keyboard nav + Enter to select.

**7. Comments panel / list** _(Stage 1 — core/MVP)_ ❌ (not built)
- 7.1 Dedicated Comments panel — a tab in the right inspector stack OR a floating panel (matches Varos floating-rounded-panel shell); lists every thread as a card.
  - 7.1.a Card content — author avatar, snippet of root text, reply count, last-activity time, reaction summary, resolved badge, pin number, artboard name.
  - 7.1.b Two-way selection — click a card → canvas pans/zooms to its pin and opens the thread; click a pin → highlights its card (mirror the existing Layers-panel two-way pattern).
  - 7.1.c Unread/activity indicator — bold/dot on cards with new activity; "Mark all as read".
- 7.2 Filters — Open / Resolved / All; Mine (authored by me) / Mentions me / Participating; by author; by artboard/page.
  - 7.2.a Search — full-text search across comment bodies and author names.
  - 7.2.b Sort — newest, oldest, most recent activity, by pin number, by artboard order.
- 7.3 Grouping _(Stage 2)_ — group threads by artboard/page or by author.
- 7.4 Density / panel states — empty state ("No comments yet"), loading (backend), collapsed/expanded thread preview inline in the panel.
- 7.5 Jump/navigate controls — next/prev unread thread keyboard cycling; "go to pin" button per card.
- 7.6 Show/hide all comments toggle — global eye toggle to declutter the canvas (Illustrator/Figma convention) without deleting; persists per session.

**8. Presence & live cursors** _(Stage 3 — advanced, backend-required)_ ❌ (not built)
- 8.1 Avatar stack in the top bar — faces of who's currently in the file; overflow "+N"; click to follow/locate.
- 8.2 Live cursors — other users' cursors with name labels moving in real time on the canvas.
- 8.3 Live selection — see what objects others have selected (colored bbox by user color).
- 8.4 Follow mode / spotlight — click an avatar to mirror their viewport; "spotlight" to force-follow you to others.
- 8.5 Typing/commenting indicator — "X is typing…" on a thread; pin pulses while someone is composing on it.
- 8.6 Observers/viewers — see who is merely viewing vs editing; idle/away state.
- 8.7 Local single-user fallback — with no backend, presence simply shows only the local author (graceful no-op).

**9. Sharing & roles** _(Stage 3 — advanced, backend-required)_ ❌ (not built)
- 9.1 Share dialog — invite by email/handle; copy share link; manage access list.
- 9.2 Roles/permissions — Owner / Editor / Commenter (can comment + react, cannot edit art) / Viewer (read-only).
  - 9.2.a Comment-only mode — Commenter role disables editing tools, leaves comment tool + reactions enabled.
- 9.3 Link settings — anyone-with-link vs invite-only; expiry; password; allow-comments toggle on the link.
- 9.4 Offline alternative _(Stage 2, no backend)_ — "share by file": send the `.varos` and all comments travel inside it (true local-first sharing); recipients open and see/add comments offline, merged on round-trip (see 0.4/0.5).
- 9.5 Export with comments _(Stage 2)_ — option to bake pins+thread summary into a PDF/PNG export for review handoff (offline-capable); separate "exclude comments from final export" default so pins never leak into deliverables.

**10. Notifications** _(Stage 3 — advanced, backend-required)_ ❌ (not built)
- 10.1 In-app inbox/activity feed — list of mentions, replies to my threads, resolves, reactions; unread badge on the Comments tab/app icon.
- 10.2 Channels — desktop/system push (Tauri native notification), email, and in-app; per-channel and per-event preferences.
- 10.3 Notification events — @mention, reply to my comment, reply in a thread I'm in, my comment resolved/reopened, reaction to my comment, new comment on my artboard.
- 10.4 Per-file mute / per-thread subscribe-unsubscribe — follow/unfollow a thread; mute a noisy file.
- 10.5 Offline fallback — without backend, "notifications" degrade to an in-file unread/activity badge only (no push/email).
- 10.6 Digest _(Stage 3)_ — batched daily/periodic email summary (backend).

**11. Versions & review context** _(Stage 3 — advanced; local snapshots offline, hosted history backend)_ ❌ (not built)
- 11.1 Local version snapshots _(Stage 2, offline)_ — name/save points in the `.varos`; comments tied to the version they were made on so a thread shows "commented on v3".
- 11.2 Version sharing — share a specific named version for review; comments scoped to that version.
- 11.3 Comment-on-version diff _(Stage 3)_ — view a thread against the artwork state when it was written; "since you commented, this changed" indicator.
- 11.4 Hosted version history _(Stage 3, backend)_ — server timeline of versions with per-version comment counts and restore.

**12. Cross-cutting: shortcuts, accessibility, settings, edge cases** _(Stage 1 unless noted)_ ❌ (not built)
- 12.1 Keyboard — `C` comment tool; Enter post / Shift+Enter newline; Esc cancel; arrow/Tab cycle threads; shortcut to toggle show/hide all comments; `@` mention; `:` emoji (Stage 2).
- 12.2 Accessibility — comment threads navigable by keyboard, screen-reader labels on pins ("Comment 3 by Ahmed, 2 replies, unread"), sufficient contrast on pin states, RTL/Arabic-aware comment text rendering (ties to Varos Arabic-shaping work).
- 12.3 Localization — comment UI strings + relative-time formatting localizable; comment bodies support Arabic/RTL input and mixed bidi.
- 12.4 Settings/prefs — default author identity, show/hide resolved, pin numbering vs renumber, notification prefs (Stage 3), include-comments-in-export default, sticky comment tool.
- 12.5 Performance — virtualized panel list for thousands of comments; pin culling outside viewport; lazy-load resolved threads.
- 12.6 Edge cases — comment on a locked/hidden layer (still placeable, flagged); comment on a deleted/orphaned object (see 2.2.b); very long threads (collapse middle); empty/whitespace guard; emoji-only body; pasting images; duplicate-file id collisions handled by stable ids (0.4); resolving a thread you don't own (allowed offline, role-gated with backend); time-zone display for timestamps.
- 12.7 Privacy/security _(Stage 3, backend)_ — who-can-see-comments respects role; no comment data sent anywhere while offline; export strips comments by default to avoid leaking internal review notes.

**Build-order summary**
- Stage 1 (MVP, fully offline): comment tool + place pin (1), canvas anchoring (2), threads/replies/edit/delete (3), resolve/reopen (5), comments panel + open/resolved/mine filters + two-way select + show/hide-all (7), local author identity (0.3), data model in `.varos` (0.1–0.2).
- Stage 2 (standard, mostly offline + share-by-file): reactions (4), @mentions stored/rendered (6), object anchoring + off-screen indicator (2.2/2.6), bulk resolve, rich-text/attachments, copy-link-to-pin, share-by-file + export-with-comments (9.4/9.5), local version snapshots (11.1).
- Stage 3 (advanced, backend-required): presence/live cursors (8), share links + roles (9.1–9.3), notifications/push/email (10), hosted version history + diff (11.3–11.4), mention delivery, clustering, markup attachments, CRDT merge.

---

