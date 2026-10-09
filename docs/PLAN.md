> **Status:** current — the single execution map, approved by the owner 2026-10-09 (plan mode). Governed by `docs/foundation/FOUNDATION_CHARTER.md` §3. Previous map: `docs/reference/PLAN_HISTORY_2026-10-09.md`. Progress per slice is recorded in the **Progress** section at the end and in `docs/foundation/GATE_LOG.md`.

# Varos — the execution plan (ordered, no dates)

## Context
- Owner (Ahmed), 2026-10-09: colour picker v3 is complete, installed and approved. He asked for ONE complete, ordered plan built from a full product inventory and his answers to the planning questions, then autonomous execution by the moderator (Fable) with Codex Sol 6.1 / Opus workers, Figma options for every visible piece (he picks, tweaks and finalises), gates + independent review per slice, Mac install after each merge, owner hand test after install.
- His answers (memory `feedback-planning-answers-2026-10-09` + this session): borrow from VectorCraft/PhotoCraft case by case (code as-is when excellent and it fits, with attribution; never eframe / CPU renderer / their chrome; no silent export fallbacks); appearance = Figma-like smart multi fill/stroke, designed with him first, NOT at the start; masks = a container on the layer/group row, draw any vector inside, alpha or clip, several per row, plus Illustrator ⌘7 (confirmed «أيوه ده المعنى بالظبط»); raster effects at the very end; PDF export with ppi options like the famous apps; **export and print before the stroke engine**; **stroke engine before gradients**; **Place = embed by default + Link option**; **CMYK/ICC/PDF-X committed as a late phase**; **Autosave writes into the file itself** (not only recovery copies); text/typography LAST as its own programme; v5 ordering, rail flyouts, tools order = tech lead's call; plan = order only, no dates.
- Inputs verified against code at `main` `b1de865`: gap inventories (files/import/export/canvas; tools/paint/layers/app) in `docs/reference/gap/GAP_1_FILES_IO_CANVAS.md` and `GAP_2_TOOLS_PAINT_LAYERS_APP.md`; `docs/reference/APPEARANCE_MASKS_STUDY.md` (+ study A/B/C); `EXTERNAL_PRIOR_ART_SURVEY.md`; `EXTERNAL_PRIOR_ART.md` (borrowing rules).
- Path keys: `c/`=`varos/crates/varos-core/src/`, `a/`=`varos-app/src/`, `r/`=`varos-render-wgpu/src/`, `p/`=`varos-pdf/src/`, `b/`=`varos-bridge/src/`, `ras/`=`varos-raster/src/`; `VC/`=`~/Documents/AI workspace/reference/artcraft/vectorcraft/crates/`@a469568, `VCe/`=`VC/engine/src/cmd/`, `PC/`=`…/photocraft/crates/`@4cb7cf3.

## Principles of ordering
1. Basics before power: export, Place, stroke options, print PDF, the Select menu land before appearance stacks, live objects, colour management, raster effects, text.
2. Unlock order = dependency order: S0 fixes → export/print → stroke engine → image object → tools → gradients → appearance/masks → import → view → app → live objects → CMYK → raster fx → text.
3. At most one format bump per phase; the bump slice carries every new key of that phase with its render + export + Bridge behaviour (no dormant fields, ADR-0010:103). Numbers are taken by the first merged writer; docs say "next bump" until merge.
4. Owner-visible value every phase; every slice ends with a hand test the owner can do after install.
5. Lift first where the survey says COPY/ADAPT (attribution header + `varos/NOTICE` on the first borrow; our headless tests before landing); BUILD the renderer (WGSL), PDF, SVG, UI.
6. Design-first: rows marked Figma=yes cannot start code before the owner picks from Figma options.
7. Agent-native: every capability = `EditCommand` (`c/command.rs`) + Bridge verb (API 1.2 opt-in; 1.0/1.1 fixtures byte-frozen) + CLI where headless.
8. Laws every slice: box system/tokens/kit untouched; Illustrator keys only; heat (idle = Wait); ratchets never raised; gates + author≠reviewer + owner hand test; stage by name; push main after each merge; install only when `pgrep -x varos` is empty.

## Phases and slices
Columns: Lift · Dep · Bump · ADR/spec · Figma · Size · Owner exit test.

### Phase 0 — Fix first, crash safety, export wiring (no bump)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 0.1 | S0 render fixes | clip opacity inside masks (`r/tess.rs` group_draws), clip bit for StrokeCov/Knockout (`r/lib.rs` draw_steps), full-paint `scene_signature`/`thumb_key`, one `painted_extent()` for cull+hit, `is_mask_group()` instead of `== GroupRole::Clip`, no-raster-in-Document guard test, empty `ExportReport` type | BUILD | — | no | study §4 | no | M | 50 % object inside a mask looks 50 %; translucent stroke cut by the mask |
| 0.2 | Crash safety | panic guard + doc/selection rollback at `Editor::execute`/Bridge batch; wgpu device-lost/uncaptured-error watch → readable "GPU stopped" state; no-panic clippy lints as a ratchet on core+bridge | COPY `VC/engine/src/guard.rs`, `PC/gpu/src/health.rs` | — | no | note | no | S–M | forced-panic test leaves the document intact |
| 0.3 | Boolean oracles | proptest identities for the 4 Pathfinder modes | ADAPT `VC/pathops/tests/prop_pathops.rs` | — | no | dep review | no | S | n/a |
| 0.4 | SVG export wired | File ▸ Export ▸ SVG (artboard/active/all), CLI `export-svg`, Bridge `export_svg`; export sheet gets a format chooser | ours `c/svg.rs` | 0.1 | no | — | **yes** (export sheet v2) | S+sheet | export SVG, open in browser + Figma |
| 0.5 | Raster export | PNG via `ras/rasterize_artboard` (scale/ppi, transparent bg); JPEG/WebP/TIFF/BMP/GIF via `image`; fidelity warnings in the report | ADAPT `VCe/fileio/encode.rs:434-449`, `PC/codecs/src/fidelity.rs:160` | 0.4 | no | dep review | yes (same sheet) | M | @2x transparent PNG; JPEG quality |
| 0.6 | File menu basics | Save a Copy ⌥⌘S (reuse Bridge path), Revert, Close All ⌥⌘W, Export Selection, Export Cancel + Show in Finder; split `a/chrome.rs` menu tables into `menus/` | ADAPT `VCe/fileio/save.rs:55,511`, `docmenu.rs:203` | — | no | — | no | S | each row works |
| 0.7 | View quick wins | Fit All ⌥⌘0, typed zoom %, Zoom (Z) + Hand (H), Make/Release/Clear Guides + typed position, Show Grid ⌘' + spacing (reads `grid_spacing`), artboard reorder in panel + Fit to Art + Convert to Artboards | ADAPT `VCe/docmenu.rs:123,150`, `layer.rs:142`, `menucmds.rs:155` | — | no | — | yes (grid prefs, zoom field) | M | ⌘' real grid; Z/H tools |
| 0.8 | Command wiring wave | Select menu (Same ×10, Inverse, Next Above/Below, Reselect, All on Artboard); Lock/Hide ⌘2/⌘3 + Unlock/Show All; Distribute 6 + Spacing + key object; Join ⌘J/Average/Reverse/Add Anchor Points/Clean Up; Compound Path ⌘8/⌥⇧⌘8; New Layer/Sublayer, Paste Remembers Layers, Send to Current Layer; Expand Transform wired; Group Selection + Lasso (Q); Anchor tool ⇧C + Add/Delete anchor exposed; shortcut parity table as data + test | ADAPT `VCe/select.rs:22-145`, `object.rs:85-176`, `VC/pathops/src/edit.rs:299-346`, `VC/tools/src/xform/wand.rs:49,67`; COPY `VC/tools/src/catalog.rs:20-140` as data | — | no | — | yes (rail: 3 new tool slots) | M (2 PRs) | Select ▸ Same Fill; ⌘2; ⌘8; ⌘J |
| 0.9 | Clipping mask ⌘7 | Make/Release ⌘7/⌥⌘7 + Bridge verbs on existing `clip_group` (`c/model.rs:1360,1375`) | ours | 0.1 | no | LAYERS_VISION §3.2 note | no | S–M | shape over art → ⌘7 → clipped; PDF matches |

### Phase 1 — File basics, print-ready PDF, autosave (no bump)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 1.1 | Document Setup | sheet: units, ppi (first UI for `DocUnits.ppi`), bleed (reads `c/model.rs:376`), transparency-grid toggle; red bleed guide on canvas | ADAPT `VCe/docsetup.rs` | — | no | short spec | **yes** | M | 3 mm bleed → red line |
| 1.2 | PDF options | presets (Print/Press/Smallest), MediaBox⊇BleedBox⊇TrimBox, crop/registration marks, image ppi options (72/150/300/custom), compression; report lists every degradation | BUILD on `p/write.rs:274`; ideas `VC/pdf/src/presets.rs:56-83`, `marks.rs` | 1.1 | no | SAVE_EXPORT_PLAN §8 lite | **yes** | M | TrimBox in Preview; marks |
| 1.3 | Print ⌘P | export PDF → macOS print panel | IDEA `VCe/print.rs:38` | 1.2 | no | — | no | S–M | ⌘P prints |
| 1.4 | New Document dialog | category presets, size/units, artboard count, bleed, raster-effects ppi (RGB only until Phase 12) | ADAPT `VCe/newdoc.rs:19,125` | 1.1 | no | — | **yes** | M | ⌘N → 3 A4 artboards with bleed |
| 1.5 | Document Info | objects/colours/artboards (links/fonts later) | ADAPT `VCe/docinfo.rs:16` | — | no | — | yes | S | counts match |
| 1.6 | Templates | Save as Template / New from Template by folder convention (opens Untitled) | ADAPT `VCe/fileio/save.rs:56`, `load.rs:281` | — | no | — | yes (Start row) | S | opens Untitled |
| 1.7 | Window memory | window size/position persisted | BUILD | — | no | — | no | S | relaunch restores |
| 1.8 | Export for Screens | `{artboard × scale × format × suffix}` job on the IO worker; SVG options | ADAPT `VCe/fileio/screens.rs:1-5`, `VC/svg/src/lib.rs:191-216` | 0.4, 0.5 | no | — | **yes** | M | 1x/2x PNG + SVG per artboard |
| 1.9 | OS clipboard out | Copy publishes PDF + SVG + PNG flavours (NSPasteboard via `objc2`) | ADAPT `VCe/clipboard/flavours.rs:1-7` | 0.4, 0.5 | no | — | no | S–M | ⌘C → ⌘V in Figma/Keynote |
| 1.10 | **Autosave to file** (owner) | preference (on/off, interval, default on), writes the open file atomically on the IO worker after N s of inactivity, never while a gesture/transaction is open, dirty dot clears, Revert = escape hatch; recovery copies stay | BUILD on `core/file.rs` atomic write + `a/storage/scheduler.rs` | 0.6 (Revert), 9.1 pref store (minimal pref first) | no | short spec (when it must NOT write) | yes (pref row + status hint) | M | edit, wait, relaunch → changes are in the file |

### Phase 2 — Stroke engine (**v5**)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 2.0 | kurbo + adapter | `kurbo` into varos-core (pure), `Anchor`↔`BezPath` adapter, `NOTICE` created | ADAPT `VC/geom/src/path.rs:33` | — | no | ADR note "kurbo" + dep review | no | S | n/a |
| 2.1 | **v5 StrokeStyle** | `StrokeStyle{cap, join, miter, dash[], phase, align_corners, align Centre/Inside/Outside, arrows{start,end,scale}}` on Path; GPU outline-to-fill via `kurbo::stroke` (`r/tess.rs:131-149`); PDF native caps/joins/dash (`p/write.rs:211,395`), arrows as fills; SVG attrs; CPU raster; hit/cull via `painted_extent`; Bridge 1.2 fields; migration v4→v5 + fixtures + refusal first | ADAPT `VC/effects/src/stroke.rs`, `stroke/dash.rs`; COPY `stroke/arrow.rs`; `PC/vector/src/stroke.rs:101` | 2.0, 0.1 | **v5** | StrokeStyle spec + ADR-0008 amendment + VRS_FORMAT §5 | no | L | v4 files unchanged; Bridge sets a dashed stroke; PDF shows it |
| 2.2 | Stroke section UI | Properties ▸ Stroke: weight, cap/join/miter, align, dash editor, arrowheads + scale; control-bar mirror | BUILD (kit) | 2.1 | no | — | **yes** | M | dashed arrowed inside-aligned stroke; ⌘Z per field |
| 2.3 | Outline Stroke / Offset Path / Expand | Object ▸ Path ▸ Outline Stroke, Offset Path sheet, Object ▸ Expand; Transform "scale strokes" | ADAPT `VC/pathops/src/offset.rs:45,79`, `VC/effects/src/stroke/outline.rs`, `VCe/expand.rs:36` | 2.1 | no | — | yes (Offset sheet) | M | outline a dashed arrow → editable fills |

### Phase 3 — Image object, Place, links, Package (**v6**)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 3.0 | ADR-0014 raster object + links | `NodeKind::Image{blob_key,w,h,xform,link,placement}`, blob table (mime/bytes/proxy), link info (abs+rel path, mtime, hash, proxy), undo rule (bytes refcounted outside clone stacks), texture budget, PDF/SVG image rules; **Place default = embed, Link toggle** (owner) | ADAPT `VC/doc/src/node.rs:326-340`, `lib.rs:512-520`, `VC/doc/src/links.rs:22` | — | — | **ADR-0014** | no | S (doc) | owner reads summary |
| 3.1 | **v6 image object** | model + limits + migration + fixtures; wgpu textures (mips, budget, no idle upload); CPU raster; PDF image XObject (+ppi downsample from 1.2); SVG `<image>`; Bridge 1.2 `add_image`/describe; undo RAM measured | ADAPT model; BUILD render/PDF | 3.0, 2.1 | **v6** | ADR-0014 | no | L | Bridge places a PNG; reopen; PDF/SVG carry it |
| 3.2 | Decoders | PNG/JPEG/GIF/WebP/TIFF/BMP via `image` with size limits before allocation; EXIF orientation + file ppi | COPY `PC/codecs/src/lib.rs:39-55`, `orientation.rs:161-198`, `VCe/fileio/ppi.rs` | 3.1 | no | dep review | no | M | 6000 px JPEG at its ppi, rotated right |
| 3.3 | Place, drop, paste | File ▸ Place ⇧⌘P (click or drag-to-size), Finder drop (`DroppedFile`), paste bitmap; Place sheet embed/link | ADAPT `VCe/place/mod.rs` | 3.2 | no | — | **yes** | M | drop a JPEG; ⌘V a screenshot |
| 3.4 | Image transform + crop | reuse `Xform`/opacity; on-canvas Crop box stored as the existing Clip group | ADAPT `VCe/menucmds.rs:110,532` | 3.1 | no | — | **yes** | M | rotate + crop; release |
| 3.5 | Links panel | relink/update/go to/embed/unembed, missing/modified badges, effective ppi | ADAPT `VCe/links.rs:38-56,962,1034` | 3.3 | no | — | **yes** | L | move a file → badge → Relink |
| 3.6 | Package | File ▸ Package: `Links/` + report (fonts when text exists) | ADAPT `VCe/package.rs:1-16` | 3.5 | no | — | yes | M | Package folder in Finder |
| 3.7 | Rasterize + ppi | Object ▸ Rasterize (ppi, bg) via varos-raster; Document Raster Effects ppi | ADAPT `VCe/menucmds.rs:92,370` | 3.1 | no | — | yes | S–M | rasterize a group at 300 ppi |
| 3.8 | Image Trace | presets + panel; quantise→despeckle→contours→DP→cubic fit | ADAPT `VC/trace/src/lib.rs:35-37,209-290` | 3.1, 4C | no | — | **yes** | M | trace a logo → paths |
| 3.9 | Optional codecs | HEIC via ImageIO; PSD flattened | IDEA `PC/heif`; ADAPT `PC/psd` composite | 3.2 | no | — | no | M | only when asked |

### Phase 4 — Tools waves (no bump; corner radius + shear baked for now)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 4A | Select/transform | Reflect (O), Shear (baked), Free Transform (E) shear, Rotate/Scale/Reflect/Shear dialogs (kit sheet, Copy, preview), Transform Each, Align to Key Object, Magic Wand (Y), Eyedropper options, Isolation mode (double-click group, breadcrumb, dim others), Layers menu family (Release to Layers, Collect, Merge, Flatten, Locate, Hide/Lock Others) | ADAPT `VC/tools/src/xform/transform.rs:24,91,168`, `free.rs:18`, `VCe/menucmds.rs:74,297`, `wand.rs:19`, `VCe/object.rs:146`, `VCe/layerpanel.rs:19-131` | 0.8 | no | — | **yes** | M–L | ⌥-click Rotate → dialog → Copy; isolation |
| 4B | Shapes + flyouts | Rounded Rect tool (↑↓), Polygon sides ↑↓, Star, Line (\), Arc, Spiral, Rect/Polar Grid + dialogs; rail flyouts (tech lead: Illustrator-style long-press flyout, mocked first) | COPY `VC/geom/src/shapes.rs:14-186` maths | — | no | — | **yes** | M | star with ↑↓; spiral dialog |
| 4C | Freehand | curve fitter → Simplify, Pencil (N) fidelity, Smooth, Path Eraser, Join tool, Curvature (⇧~) | ADAPT `VC/pathops/src/fit.rs:30-97`, `edit.rs:12-47,137`, `VC/tools/src/draw2/gesture.rs:43-49`, `curvature.rs` | 2.0 | no | — | yes | M–L | pencil, smooth, simplify |
| 4D | Cutting | Scissors (C), Knife, Eraser (⇧E), Divide Objects Below | ADAPT `VC/tools/src/draw2/anchor.rs:40`, `cut.rs`, `VC/pathops/src/planar.rs:359` onto i_overlay | 4E (Knife/Eraser) | no | — | no | M | scissors on a circle |
| 4E | Construction | planar faces on i_overlay → Pathfinder Divide/Trim/Merge/Crop/Outline/Minus Back → Shape Builder (⇧M) | ADAPT-ALGORITHM `VC/pathops/src/planar.rs:321`, `pathfinder.rs:29-87`, `VC/tools/src/builder.rs` | 0.3 | no | ADR note if linesweeper is proposed | yes | L | divide circles; drag-merge regions |
| 4F | Live Corners | corner widget on Direct (round/inverted/chamfer), bake-on-edit (stored params in Phase 10) | ADAPT `VC/geom/src/corners.rs:134,149`, `VC/tools/src/corners.rs` | — | no | — | yes | M | drag a corner widget |

### Phase 5 — Gradients and swatches (**v7**)
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 5.0 | S1 appearance view | `Appearance` read-view + `StackItem/EntryOpts/Look` types in memory; every reader goes through it; v4–v6 fixtures byte-identical | ADAPT `VC/doc/src/appearance.rs:306-1010` shape | 0.1 | no | ADR-0013 model sections | no | M | invariant tests |
| 5.1 | **v7 gradient + swatches** | `Paint::Gradient` (linear/radial, stops, midpoint, spread) + `Paint::SwatchRef` + swatch table; GPU LUT pipelines (dither), CPU, PDF axial/radial shading, SVG; hit-inside; eyedropper/Pathfinder/duplicate carry Paint by value; Bridge 1.2 | ADAPT `VC/color/src/gradient.rs`, `swatch.rs:9-45`; `PC/gpu/src/compose.wgsl:632-680` maths | 5.0, 2.1 | **v7** | ADR-0013 + VRS_FORMAT | no | L–XL | Bridge paints a gradient; PDF matches |
| 5.2 | Gradient UI | picker Gradient tab enabled (`a/ui/picker/panel.rs:59`), Gradient tool (G) annotator | IDEA `VC/tools/src/xform/gradient.rs:68-231` | 5.1 | no | — | **yes** | L | drag a gradient on canvas |
| 5.3 | Swatches panel | document/global/groups, `.ase`/`.gpl`, libraries; `PanelId::Swatches` real | ADAPT `VC/color/src/palette_io.rs`, `libraries.rs`, `VCe/swatch.rs:16-54` | 5.1 | no | — | **yes** | L | import `.ase`; global swatch recolours |
| 5.4 | Colour Guide + Recolor | harmony math to core; variation grid; Recolor Artwork | ADAPT `VC/color/src/harmony.rs`, `recolor.rs` | 5.3 | no | — | **yes** | L | recolor a poster to 3 colours |
| 5.5 | Gradient on stroke; freeform | within/along/across; freeform as baked texture, reported | ADAPT `VC/effects/src/stroke/gradient.rs`, `VC/color/src/freeform.rs` | 5.2 | no | — | yes | M + L | gradient along a stroke |

### Phase 6 — Appearance and masks (**next bump**; v8 is text since 2026-10-09) — after the owner design round
| # | Slice | What | Lift | Dep | Bump | ADR/spec | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|---|---|
| 6.0 | Design round + ADR-0013 final | Figma options: Appearance section (Properties home), Layers fx badge, masked row (chip + expand), "Add mask" gesture, drop-onto-thumbnail; LAYERS_VISION §3.2 amended | — | 5.0 | — | **ADR-0013 accepted** | **yes (several)** | M (design) | owner picks |
| 6.1 | **v8 stack + look + MaskAlpha** | extra `StackItem`s with per-entry opacity/visible/blend; `Node.look` (group opacity, isolate); `MaskAlpha` emitted; GPU layer stack + texture pool + depth cap; PDF SMask/transparency groups; SVG `<mask>`; Bridge 1.2 | ADAPT `VC/doc/src/appearance.rs`, `bake.rs`; PC mask maths | 6.0, 0.1 | **next bump** (was v8) | ADR-0013 | no | L | group at 50 % composites once; alpha mask |
| 6.2 | Appearance UI | stack list, add fill/stroke, per-entry opacity, reorder, fx badge, Expand Appearance | BUILD (kit) | 6.1 | no | — | from 6.0 | L | two strokes + two fills; PDF identical |
| 6.3 | Masks from the row | "Add mask" → mask group; several per row (nested); draw any vector inside; alpha/clip switch; drop-onto-thumbnail; ⌘7 row chip | LAYERS_VISION §3; PC mask shapes | 6.1 | no | — | from 6.0 | L | add mask → draw circle → masked; second mask |
| 6.4 | Graphic Styles | named appearance presets | IDEA `VCe/style.rs:29-120` | 6.2 | no | — | yes | M–L | save a style, apply ×10 |

### Phase 7 — Interchange import (no bump; separate crate, never in `.vrs` read)
| # | Slice | What | Lift | Dep | Figma | Size | Owner exit test |
|---|---|---|---|---|---|---|---|
| 7.1 | SVG/SVGZ import | usvg tree → Path/Image/gradient; Open + Place; loss report | ADAPT `VC/svg/src/import.rs`, `lib.rs:401-503` | 3.1, 5.1, 2.1 | yes (report) | M–L | open a Figma SVG |
| 7.2 | OS clipboard in | paste PDF/SVG/bitmap from Figma/Illustrator | ADAPT `VCe/clipboard/resources.rs` | 7.1, 1.9 | no | M | ⌘V from Figma |
| 7.3 | PDF / AI import | hayro-based crate; vector + images; text as outlines until text programme | ADAPT `VC/pdf/src/import_*.rs`, `VCe/fileio/pdfimport.rs` | 7.1 | yes | L–XL | open an `.ai` |
| 7.4 | DXF import | lines/arcs/polylines/splines; DWG refused | ADAPT `VC/cad/src/lib.rs:29` | 7.1 | no | M | open a DXF |

### Phase 8 — View depth (no bump)
8.1 Outline mode ⌘Y (renderer outline pass; dep 6.1; Figma yes; M) · 8.2 Pixel Preview + Snap to Pixel (reads `force_pixel_align`/`move_whole_px`; M) · 8.3 Navigator (proxy thumbnail, no idle repaint; Figma yes; M) · 8.4 Presentation mode, Trim view, canvas colour, transparency grid, smart-guide readouts (S–M).

### Phase 9 — Application (no bump)
9.1 Preferences v2 (`settings.json`: keyboard increment, units default, GPU, history depth, recovery, autosave, language; Figma yes; M) · 9.2 Command registry ADR-0015 (one id per behaviour shared by menu/shortcut/Bridge/CLI; `enabled`/`disabled_reason`; menu map toward Illustrator's 9 menus; L) · 9.3 Shortcut editor (⌥⇧⌘K, conflicts, parity default; Figma yes; L) · 9.4 History panel (uses `history_preview` + per-AI history A3-next; Figma yes; M) · 9.5 Actions/batch (record + CLI `apply`; M) · 9.6 Help menu + Quick Look for `.vrs` (S+M) · 9.7 Updates per ADR-0007 (M) · 9.8 Arabic UI per ADR-0012 T2→T3→catalog+RTL (needs ADR-0012 accepted; Figma yes; XL) · 9.9 Accessibility start (AccessKit roles; L).

### Phase 10 — Live objects A (**v9** = Live Corners; **v11** = Lane B effects/width profiles): typed `Effect::{Offset,ZigZag,Transform,Warp}`, per-corner params, `width_profile`; one evaluator for canvas/CPU/PDF/SVG/Expand; per-object cache (ADAPT `VC/effects/src/{distort,warp,stylize,util}.rs`, `VC/doc/src/live.rs:278-400`, `VC/effects/src/stroke/width.rs`; ADR-0016 "Live objects"; L) → 10.1 Effect menu + Appearance rows + Width tool ⇧W (Figma yes; L).

### Phase 11 — Live objects B (**v10**, **v11**): 11.0 `NodeKind::Live{Blend,Repeat,Envelope}` + Blend tool (W) + Free Transform distort (ADAPT `VC/doc/src/blend.rs:35-380`, `VCe/live.rs:27-117`; XL) · 11.1 Live Paint (K) on planar faces (XL) · 11.2 v11 symbols + brushes (calligraphic/scatter/art/pattern, Paintbrush B / Blob ⇧B, tablet pressure COPY `PC/tablet/src/{macos,appkit}.rs`) + patterns (XL) · 11.3 Perspective grid (COPY `VC/geom/src/projective.rs`; only if asked).

### Phase 12 — Colour management (**v12**, owner-committed): 12.0 ADR-0017 (Process RGB/CMYK/Gray, Spot, document mode, profiles via `moxcms`, explicit conversion only) · 12.1 v12 colour model (picker CMYK sliders, render through profile, PDF ICCBased/Separation/DeviceN, report; XL) · 12.2 PDF/X-4, OutputIntent, Overprint preview, Proof Colours (L).

### Phase 13 — Raster effects + blend modes (**v13**, last before text): `Effect::{Shadow,Blur,Glow}` in points; blend modes; renderer-only cache; PDF image XObjects at 300 ppi default, reported; CPU parity suite (ADAPT `PC/doc/src/effects.rs`, `PC/compose/src/effects.rs`, WGSL rebuilt from `compose.wgsl:1029-1178`; Figma yes; XL).

### Phase 14 — Text programme (placeholder, own plan later, **v14+**): ADR-0010 P1c → P2 TextBox → Latin point text → Arabic/RTL (kashida, our composer; lift only the every-line idea of `VC/text/src/composer.rs`) → area text → type on path → styles/OpenType → PDF text subsetting → Package fonts → Create Outlines. Planned when Phases 0–9 are closed.

### Parked / not planned (reason)
Rotate view (owner parked) · version history + iCloud coordination (track A5 FileStore) · Web/WASM (A6) · multiple windows/tile (shell rewrite) · saved views · EPS/EMF/WMF/DWG, DXF/TXT export, Slices/Save for Web (low Mac value) · Flare · liquify/puppet warp · gradient mesh (IDEA only) · symbolism tools (after symbols) · layered PSD · Windows runtime (compile-only) · MCP prompts/resources · undo structural sharing (only if 3.1 measurement demands) · target circle/layer colour (fx badge instead).

## Lanes and rules
| Lane | Owns | Typical slices |
|---|---|---|
| Codex Sol 6.1 (default implementer) | varos-core geometry/model/format/tools, render-wgpu, raster, cli | 0.1–0.3, 2.0–2.1, 3.1–3.2, 4B–4F, 5.0–5.1, 6.1, 7.x, 10.0, 13.0 |
| Opus (AppKit, design judgement, second reviews) | varos-app chrome/menus/sheets/panels, varos-pdf, bridge verbs, objc2 | 0.4–0.9, 1.x, 2.2–2.3, 3.3–3.6, 4A, 5.2–5.4, 6.2–6.4, 8.x, 9.x |
| Moderator (Fable) / Astra | ADRs, specs, Figma briefs, VRS_FORMAT, NOTICE, GATE_LOG/STATUS/PLAN, memory | ADR list below |
Parallel only when two slices share no file (safe pairs: 0.1‖0.4, 0.2‖0.6, 0.8‖1.1 after the `menus/` split, 3.1‖4B, 5.1‖4A, 6.1‖7.1, 9.x‖8.x); format slices never parallel with another writer-touching slice; author ≠ reviewer (Codex slice → Opus review and vice versa); every slice: workspace tests, clippy Mac + Windows target `-D warnings`, fmt, dep directions, ratchets unchanged, GATE_LOG entry, merge, push, install when Varos closed, owner hand test.

## ADRs and specs before code (in order)
1. S0 fix list + v5 ordering note (PLAN + ADR-0010 cross-note: no reservation, first merged writer takes v5). 2. Engineering note: panic guard, GPU health, no-panic ratchet. 3. Export sheet + Document Setup + PDF options + Autosave spec (short, Figma-backed). 4. ADR note: kurbo in varos-core + dep review (kurbo, proptest, image features). 5. StrokeStyle spec + ADR-0008 v5 amendment + VRS_FORMAT §5. 6. ADR-0014 raster object + links (+ v6). 7. Planar-faces note (i_overlay; linesweeper only via ADR). 8. ADR-0013 Appearance & masks: model sections before Phase 5, UI sections after 6.0; ADR-0008 v7/v8; ADR-0009 "1.2 opt-in, not-solid token". 9. Import spec (loss table; firewall; hayro review). 10. Settings spec v2 + ADR-0015 command registry. 11. ADR-0012 acceptance (owner) before 9.8. 12. ADR-0016 live objects (+ v9/v10/v11). 13. ADR-0017 colour management (+ v12). 14. ADR-0013 S7 raster annex (+ v13). 15. Text programme plan (last).

## Format bump schedule
Revised 2026-10-09 (wave-2 integration, merge order binding): v5 Phase 2 `Path.stroke_style` (stamped) · **v6 Phase 3 Image node + blobs + links — STAMPED 2026-10-09 (`integ/w2`)** · **v7 Phase 5 Gradient/SwatchRef/swatch table — STAMPED 2026-10-09** · **v8 TextBox — STAMPED 2026-10-09** · **v9 Live Corners + container Quick Look preview — STAMPED 2026-10-09 (current writer)** · later bumps in merge order: Phase 6 stack/look/MaskAlpha, Phase 10 effects/width_profile, Phase 11 live nodes / symbols+brushes+patterns, Phase 12 colour model, Phase 13 raster fx/blend. ADR-0010's "v5" and COLOR_PICKER_V3 §3's "v5 gradients" become "next bump". Per bump (ADR-0008 rule 3): named pure migration, frozen fixtures, refusal fixtures, old-reader harness, VRS_FORMAT rows; every new key `#[serde(default, skip_serializing_if)]`; plain object byte-identical; Bridge 1.0/1.1 fixtures untouched.

## Owner decisions (resolved this session)
Export/print before stroke · Place = embed default + Link · CMYK committed late (Phase 12) · **Autosave writes into the file** (slice 1.10) · rotate view stays parked · Appearance home + masked-row look decided in the Phase 6.0 Figma round.

## Top risks and guards
1 two writers claim one version → schedule + number at merge only · 2 ~30 `.solid()` sites → 5.0 read-view first, 5.1 end-to-end in one slice · 3 undo clones × image bytes → bytes outside clone stacks, RAM measured in 3.1 · 4 one offscreen layer + two clip bugs → 0.1 first, 6.1 layer stack with budget · 5 heat → caches keyed like `flatten.rs`, idle = Wait, pacing test per slice · 6 silent export loss → `ExportReport` from 0.1, every lossy path adds a row, gate · 7 Bridge drift → `skip_serializing_if`, 1.2 opt-in, contract tests · 8 borrowed code self-graded → our tests + fixtures first, attribution, never ui-egui/CPU renderer · 9 second engines (kurbo, linesweeper) → adapter + ADR · 10 import scope creep → separate crate/phase, refusals not repairs · autosave must never write mid-gesture/transaction or during a Bridge batch, and never to a file changed on disk by another app (existing warning path).

## Execution contract (how the moderator runs it after approval)
1. Housekeeping commit: copy the two gap inventories into `docs/reference/gap/`, rewrite `docs/PLAN.md` to this phase/slice map (keep the A/B rows that are done as history), update STATUS, memory.
2. Per slice: spec/ADR if listed → Figma options if marked (owner picks) → brief → Codex or Opus lane → moderator gates → independent review (other lane) → fix round → merge (no-ff) → push → install when Varos closed → GATE_LOG + PLAN row → short Arabic hand-off with what to test.
3. Owner is asked only for: Figma picks, ADR acceptances, hand-test results, and anything on the "owner decisions" list; otherwise the moderator decides and records why.
4. Order inside a phase may be reshuffled by the moderator for lane availability; phases are not reordered without the owner.

## Verification (plan-level)
- Each slice's "owner exit test" is the acceptance; moderator gates are the floor.
- Phase exits: 0 = both clip bugs fixed + SVG/PNG export used by the owner; 1 = a printed PDF with bleed/marks + autosave observed; 2 = dashed/arrowed stroke in PDF from a v4 file; 3 = a placed, cropped, linked photo in a Package; 4 = Shape Builder + Pencil used; 5 = gradient in picker + PDF; 6 = multi-stroke + row mask; 7 = `.ai` opened; 8–9 = Outline/Prefs/Shortcut editor/Arabic names; 10–13 as listed; 14 = its own plan.

## Summary for the owner (plain)
1 نصلّح بَجّي الماسك ونمنع الكراش. 2 إخراج SVG/PNG/JPEG + Save a Copy/Revert/Close All + أدوات الزوم واليد + قائمة Select + ⌘2/⌘3/⌘8/⌘J/⌘7. 3 الطباعة: bleed وDocument Setup وخيارات PDF وppi وعلامات القص و⌘P + Autosave في الملف. 4 محرك الاستروك (أطراف/زوايا/داشات/أسهم/داخل-خارج) = أول تغيير فورمات. 5 الصور: Place وسحب من Finder ولصق وCrop وLinks وPackage وImage Trace. 6 موجات التولز: التحويل والعزل، الأشكال والفلاي آوت، Pencil وCurvature، Scissors وKnife، Shape Builder، الزوايا الحية. 7 الجرادينت على الكانفس وفي البيكر + Swatches + Recolor. 8 بعد جولة Figma: الأبيرنس (كذا فيل/استروك) والماسك على الصف مع ⌘7. 9 فتح SVG/PDF/AI/DXF ولصق من Figma. 10 Outline وPixel preview وNavigator، ثم التفضيلات ومحرر الاختصارات والهيستوري والواجهة العربية. 11 الكائنات الحية (إفكتات، Width، Blend، Repeat، رموز، فرش)، ثم CMYK والمطابع، ثم الظل والبلر. 12 النص (عربي + إنجليزي) آخر حاجة كبرنامج مستقل. كل قطعة مرئية تبدأ بخيارات Figma تختارها، وكل شريحة تنتهي بتجربتك في النافذة الحقيقية.


## Progress (moderator-maintained)
| Slice | State | Commit | Notes |
|---|---|---|---|
| Phase 6 S4/S6 — Appearance stack + row masks (Lane A) | implemented (provisional UI, owner design review pending) | uncommitted `feat/w3-appearance` | Assigned writer v10; ordered base/extra paints, group look, clip/alpha mask groups, recursive GPU/CPU/PDF/SVG, Properties Appearance and Layers badges/thumbnail drop, API 1.2 progressive schemas + CLI. Native GPU, thermal and owner design acceptance pending; no GUI/install/commit/push. Gate counts and bounded fallback: worktree `REPORT.md`. Integrator rechains v10→v11→v12→v13→v14. |
| picker v3 (pre-plan) | done, owner-approved | `214e998` | Wheel/Sliders/Harmony/Mini |
| 0.1 S0 render fixes | merged + installed, owner hand test pending | `5d422d5` | Exclude engine defect found by 0.3 and fixed in the same branch |
| 0.3 boolean oracles | merged | `5d422d5` | 200 cases/op, all hard, 0 failures |
| Night-shift group 1 (0.2, 0.9, 0.8, 0.7, 0.4, 0.5, 1.1, 1.5, 1.6, 1.7, 1.2, 1.3, 1.9, 1.10) | merged + installed `6d03c90`, provisional UI, owner review pending | `6d03c90` | GATE_LOG 2026-10-09 night shift |
| Night-shift group 2 (2.0, 2.1 v5, 4A, 4B core, 4C core, 4D, 4E, 3.8 engine, 7.1, text programme P-composer/kashida) | merged + installed `5a09800`, provisional UI, owner review pending | `5a09800` | GATE_LOG 2026-10-09; API 1.2 progressive disclosure (ADR-0009 amendment proposed) |
| Wave 2 stage 1 (4B/4C tools + flyouts, Phase 8, text P2–P4 v8, 7.2–7.5) | merged + installed `0dee21e` | `0dee21e` | GATE_LOG 2026-10-09 |
| Wave 2 stage 2 (Phase 3 v6, Phase 5 v7, 1.8, 1.4, 2.3, 4F v9, 9.1–9.6) | merged + installed `8d21c1f` | `8d21c1f` | format chain v5→v9; 1.2 tools/list 121 B headroom |
| Wave 3 (6 v10, 10 v11, 12 v12, render layers/blur/blend, 11 v13, 9.8, 9.9+9.7, text P5–P8 v14) | running (8 lanes) | — | started 2026-10-09 ~22:00 |
| 0.6 file-menu basics | merged + installed, owner hand test pending | merge | Save a Copy ⌥⌘S, Revert F12, Close All ⌥⌘W, Export Selection, cancellable export + Show in Finder, tickets, menus/ split |
| Export sheet v2 design | owner pick D·Advanced (Figma 14:82) | `d4aaf4e` | contract EXPORT_SHEET_V2.md; build = 0.4/0.5/1.8 |
| 2.0 kurbo + adapter (Lane F) | implemented (core only; fix round complete, independent re-review pending) | uncommitted | kurbo 0.13.1, std only, no serde feature; immutable metadata sidecar preserves flags/IDs; pure MIT/Apache dependency already locked; WASM core check passed; 55 JSON fixture paths round-trip. Geometry only, no format/API changes. Gates: fmt/dep directions, workspace 1527 passed / 0 failed / 15 ignored, native + Windows Clippy clean; ratchets and 14 Bridge fixtures unchanged. |
| 4B shape maths (Lane F) | implemented (core only; fix round complete, independent re-review pending) | uncommitted | Rounded rect, polygon/star, line/arc/spiral, rect/polar grids; `GAP_2_TOOLS_PAINT_LAYERS_APP.md` rows 51–55. UI deferred to later slice; owner design review pending. |
| 4C fitter + edits (Lane F) | implemented (core only; fix round complete, independent re-review pending) | uncommitted | Schneider fitter, certified simplify, range smooth, exact subdivision, average/join; fix round certifies f32 fits, preserves post-close segments and caches join candidates; `GAP_2_TOOLS_PAINT_LAYERS_APP.md` freehand/Simplify rows. Commands/Bridge/CLI and provisional UI deferred as explicitly scoped for this core lane. |
| 2.1 v5 StrokeStyle + provisional 2.2 controls (Lane H) | implemented (provisional UI, owner design review pending) | uncommitted worktree `feat/p2-stroke` | First merged writer takes v5; original 29-head library; no GUI/install/commit/push. Moderator merges stroke_adapter with p2-geom. Gates and limitations: worktree REPORT.md. |
| 4E planar / full Pathfinder / Shape Builder | implemented (provisional UI, owner design review pending) | uncommitted Lane K | i_overlay faces, command/Bridge 1.2/CLI; see reference/PLANAR_CUTTING_4E_4D.md |
| 4D cutting tools | implemented (provisional UI, owner design review pending) | uncommitted Lane K | Scissors C, Knife, Eraser Shift+E, Divide Objects Below; native hand test pending |

| 4A select/transform (Lane I) | implemented (provisional UI, owner design review pending) | uncommitted `feat/p4-tools` | Reflect/Shear/Free Transform shear; preview + Copy sheets; Transform Each; Wand/Eyedropper options; isolation; Layers family; API 1.2 + CLI. Align/key object belongs to 0.8; distort/perspective deferred. Native hand test and independent moderator review pending. |
| 3.8 Image Trace engine | implemented (engine only; owner design review pending) | uncommitted Lane M | Pure RGBA8 → filled paths/holes + report; PNG CLI and API 1.2 trace_rgba. Provisional UI/presets deferred: this lane is core engine only; image object/shared fitter absent at base. |

| 7.1 SVG import | implemented (provisional UI, owner design review pending) | uncommitted lane Q | New foreign-format varos-import firewall; Open SVG/SVGZ → Untitled viewBox board; Place → undoable group; CLI import-svg; Bridge 1.2 import_svg with files-scope input guards and explicit losses |
| 0.2 Crash safety | implemented, pending review | uncommitted | Typed command panic rollback; Bridge staged rollback; runtime GPU health; core/Bridge no-panic ratchets 37/106; docs/CRASH_SAFETY.md |
| 0.9 Clipping mask ⌘7 | implemented, pending review | uncommitted | Core commands, API 1.2 clip/release_clip, CLI apply/attached edit, native/burger mirrors, undo and headless parity |
| 0.8 command wiring | implemented, pending review | uncommitted (`feat/p0-commands`) | Select, lock/hide, distribution/spacing/key, path/compound commands, layer creation/send + persisted Paste Remembers Layers, Expand Transform, Group Selection/Lasso/Anchor tools and rail, Lucide 1.8 assets, shortcut parity. 0.8 gates passed before 0.7; final combined gates recorded in the evidence note; evidence: `docs/reference/gap/COMMAND_WAVE_0_8_BATCH_1.md`. |
| 0.7 view quick wins | implemented, pending review | uncommitted (`feat/p0-commands`) | Fit All, typed zoom, Zoom/Hand rail tools, path guides and typed ruler-guide position, document grid spacing/subdivisions and visibility, artboard reorder arrows/fit/conversion; API 1.2 document view actions. Evidence: `docs/reference/gap/VIEW_QUICK_WINS_0_7.md`. |

| 0.4 SVG + export sheet v2 Minimal | implemented, pending review | uncommitted Lane C | Artboards/Selection checklist, folder, vector presets, report/Done/cancel; shared SVG planning, CLI and opt-in Bridge 1.2 |
| 0.5 raster export | implemented, pending review | uncommitted Lane C | PNG/JPEG/WebP/TIFF offline codecs; scale/ppi, PNG transparency, JPEG quality and report notes |
| 1.1 Document Setup + bleed | implemented (provisional UI, owner design review pending) | uncommitted lane D | ⌥⌘P kit sheet; units/ppi/asymmetric bleed/grid EditCommands, canvas bleed guide, Bridge 1.2 + CLI; evidence GAP_1 §A Document setup / §D bleed and transparency grid; spec PHASE1_DOCUMENT_BASICS.md |
| 1.5 Document Info | implemented (provisional UI, owner design review pending) | uncommitted lane D | Window row, shared Bridge counts/solid colours/artboard dimensions+ppi, links/fonts placeholders; evidence GAP_1 §A Document Info |
| 1.6 Templates | implemented (provisional UI, owner design review pending) | uncommitted lane D | Folder convention, Save/New menu rows, Start section; opens Untitled dirty, no path; Bridge 1.2 + CLI; evidence GAP_1 §A Templates |
| 1.7 Window memory | implemented (provisional UI, owner design review pending) | uncommitted lane D | Debounced window.json, quarantine/validation, monitor clamp, normal geometry + maximised/fullscreen; evidence GAP_1 §A Window size, STATUS:63; owner relaunch test pending |
| 1.2 PDF options (lane E) | implemented (provisional UI, owner design review pending) | uncommitted worktree | Presets, custom ppi, compression, bleed boxes, K marks, report; CLI and API 1.2 options. Evidence: GAP_1 §C PDF options/Bleed; spec `docs/reference/P1_PDFPRINT_PROVISIONAL.md`. Owner exception lifts this lane's Figma gate only. |
| 1.3 Print Cmd+P (lane E) | implemented (provisional UI, owner design review pending) | uncommitted worktree | PDF job + permitted Preview fallback; user chooses Print in Preview. Windows row disabled with reason; headless builder/CLI and API 1.2 verb. Evidence: GAP_1 §C Print. Direct NSPrintOperation and real printing unverified. |
| 1.9 OS clipboard out (lane E) | implemented (provisional UI, owner design review pending) | uncommitted worktree | Detached Varos flavour first, PDF/SVG/transparent 2x PNG; injectable NSPasteboard, Copy/Cut, API 1.2 + headless CLI bundle. Evidence: GAP_1 §C Copy to clipboard. Figma/Keynote paste test pending. |
| 1.10 Autosave to file | implemented (provisional UI, owner design review pending) | uncommitted lane G | Offline gates recorded in lane report; independent publication review and owner native acceptance pending |
| 4B shape tools + rail flyouts (Lane D) | implemented (provisional UI, owner design review pending) | `e2ba362` + uncommitted resume fixes | Rounded Rectangle/Polygon/Star/Line/Arc/Spiral/Rect/Polar Grid, numeric kit sheets, Shift/Option, arrows, remembered long-press/right-click kit groups; spec `docs/reference/4BC_DRAWING_PROVISIONAL.md`; no format keys; resume gates: root `REPORT.md` |
| 4C drawing tools (Lane D) | implemented (provisional UI, owner design review pending) | `e2ba362` + uncommitted resume fixes | Pencil + options and open endpoint continuation, selected Smooth/Path Eraser/Join, Curvature through-points; one undo; Bridge API 1.2 progressive schemas + CLI apply; Blob Brush deferred; resume gates: root `REPORT.md` |
| 8.1–8.4 View depth (Lane E) | implemented (provisional UI, owner design review pending) | `feat/w2-view` (WIP + uncommitted completion) | Command/API 1.2/CLI, Outline paths, pixel preview/snap, Navigator, screen modes, canvas prefs/readouts. Image-box outline awaits image model integration; native acceptance and moderator review pending. Evidence: worktree REPORT.md and reference/VIEW_DEPTH_8_LANE_E.md. |
| Text programme P2–P4 / Lane G | implemented (provisional UI, owner design review pending) | WIP 6296a54 + uncommitted completion | Data-only TextBox + next-version migration; text-layout adapter; Type tool and kit Properties; outline canvas/CPU/PDF/SVG; API 1.2 and CLI. See worktree REPORT.md for gate evidence and review limitations. |
| 7.2–7.5 Lane H | implemented (provisional UI, owner design review pending) | WIP `14db577` + uncommitted resume fixes | Bounded PDF/AI + ASCII DXF subset, clipboard vector input and canvas Place/drop; image/bitmap node prerequisite absent; restrictions in LANE_H_IMPORT_IMPLEMENTATION.md |
| 3.1–3.8 raster image lane w2-images | implemented (provisional UI, owner design review pending) | uncommitted resume of a205728 | Image metadata + sibling resources, next-version migration/frozen refusal fixtures, six codecs, resource-aware consumers/recovery/clipboard, Place/drop/paste/crop, Links, Package, Rasterize and Image Trace. Full gate evidence and runtime limits: worktree REPORT.md. Proposed image writer version 6; integrator assigns final number. Optional 3.9 deferred. |
| 5.0 S1 appearance read-view (lane B) | implemented (provisional UI, owner design review pending) | feat/w2-gradients, WIP + working tree | Borrowed base slots; old v4/v5 appearance bytes and Bridge 1.0/1.1 fixtures frozen. |
| 5.1 Gradients + swatches (lane B) | implemented (provisional UI, owner design review pending) | feat/w2-gradients | Provisional next-format 6, named pure migration and refusals; GPU LUT/dither, CPU, PDF axial/radial + alpha, SVG, hit-inside, owned copies, API 1.2/CLI. Integrator renumbers after images. |
| 5.2 Gradient picker/tool (lane B) | implemented (provisional UI, owner design review pending) | feat/w2-gradients | Enabled type/flip/rotate/spread/stops/opacity/midpoint; G annotator, radial aspect/focal (Alt selects coincident focal), one undo per gesture, Escape cancellation. |
| 5.3 Swatches panel (lane B) | implemented (provisional UI, owner design review pending) | feat/w2-gradients | Document/global/group table, library, GPL/ASE/native palette IO; unsupported palette data refused. Baked gradient references detach by value; solid globals remain linked. |
| 5.4 Colour Guide + Recolor (lane B) | implemented (provisional UI, owner design review pending) | feat/w2-gradients | Harmony maths in core; variation grid; Lab k-means reduction and delta E2000 palette matching, checked commands and API 1.2/CLI. |
| 5.5 Gradient stroke (lane B) | within implemented; optional along/across/freeform deferred | feat/w2-gradients | Within paints evaluated stroke coverage consistently across backends; no along/across/freeform UI or silent fallback. |
| 1.4 / 1.8 / 2.3 / 4F (w2-export-paths) | implemented (provisional UI, owner design review pending) | WIP `67b2f9b` + uncommitted completion | New Document categories/units/layout/bleed/ppi; Advanced card × row export/SVG options; Outline/Offset/Expand + scale strokes; persisted live corners. Next writer adds `Path.corners` (`radius`, `kind`); integrator assigns final version. Gates/evidence and merge notes: worktree REPORT.md. |
| 9.1 Preferences v2 (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Typed settings table, durable generation/source checks, v1 migration, existing consumer integration; see [implementation notes](foundation/work_orders/PHASE9_IMPLEMENTATION.md) and worktree REPORT.md. |
| 9.2 Command registry (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Incumbent table migration, aliases, native/burger projections, API 1.2 command index and enabled/reason; ADR-0015 moderator acceptance pending. |
| 9.3 Shortcut editor (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Search/rebind/unbind/conflicts/reset; additive durable overrides; held Space remains fixed. |
| 9.4 History (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Dockable History, human/agent labels, jump, bounded undo/redo, API 1.2 review/top-agent undo. |
| 9.5 Actions (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Dockable Actions; supported semantic recording, .vrs-actions load/save, atomic replay, offline CLI apply. |
| 9.6 Help + Quick Look fallback (Lane F) | implemented (provisional UI, owner design review pending) | WIP 48830d0 + uncommitted completion | Help/typed Bridge tool; cached PNG embedded in container; native signed QL extension packaging documented and pending. |

<!-- ---- Lane D wave 3 ---- -->
| Slice | State | Commit | Notes |
|---|---|---|---|
| Wave 3 renderer capability (Lane D) | implemented (provisional UI, owner design review pending) | uncommitted `feat/w3-render` | Renderer-only nested layer primitives, 16 blend modes, Gaussian blur, shadow/glow, CPU reference. No UI/model/format/API writes in this lane; appearance/raster-fx lanes emit the contract. Gates and native-GPU verification limits: worktree `REPORT.md`. |
<!-- ---- end Lane D wave 3 ---- -->
<!-- ---- Lane B w3-effects ---- -->
Phase 10 Lane B: implemented (provisional UI, owner design review pending);
ADR-0016 proposed. Appearance-stack integration deferred to integrator.
