> **Status:** reference: a gap inventory made 2026-10-09 on `main` `b1de865`. It is not a plan. [STATUS](../../foundation/STATUS.md) and [PLAN](../../PLAN.md) keep their authority.

# Gap 1: files, import/export, canvas and view, raster objects

**Method.** Every Varos claim was checked in the code at `b1de865` and cited as `file:line`. `core/` = `varos/crates/varos-core/src/`, `app/` = `varos/crates/varos-app/src/`, `pdf/` = `varos/crates/varos-pdf/src/`, `raster/` = `varos/crates/varos-raster/src/`. Prior art is the local clones only (borrowing rules: [EXTERNAL_PRIOR_ART](../EXTERNAL_PRIOR_ART.md)). `VC/` = `reference/artcraft/vectorcraft/crates/` @ `a469568`. `PC/` = `reference/artcraft/photocraft/crates/` @ `4cb7cf3`. Their code is agent-written, so a citation shows the code exists, not that it is correct. Their `ROADMAP.md` parity table (lines 192–243) supplied the list of rows only.

**Liftable:** COPY = small pure code we can take almost as is, with attribution. ADAPT = port the algorithm or model onto our types. IDEA = learn the behaviour, write our own (eframe/krilla/UI code). BUILD = no usable prior art.
**Size:** S ≤ 1 day · M 2–5 days · L 1–3 weeks · XL > 3 weeks (one agent, including tests).
**HAVE** = in the code (owner-seen only where STATUS says so). **PARTIAL** = some of the behaviour, or code without a UI. **MISSING** = not found (the grep used is named where the absence is the evidence).

Two facts behind most of the gaps:
1. The scene graph has only `Layer | Group | Path` (`core/model.rs:260-264`), paint is only `None | Solid` (`core/model.rs:136-140`), and render primitives are only vector fills and strokes (`core/scene.rs:29-39`). **No image or text object exists.**
2. The user-facing export is PDF only: the File ▸ Export submenu has one row (`app/chrome.rs:561-565`). SVG export exists in core but has no UI. PNG rendering exists, but only the Bridge and CLI use it.

## A. Files

| Capability | Varos today | Illustrator reference | Prior art | Liftable | Depends on | Size |
|---|---|---|---|---|---|---|
| New (free board / size preset) | **HAVE.** `NewBoard` and `NewWithPreset` (`app/app_command.rs:65-69`). Presets Square/Portrait/Story/A4 (`core/board.rs:218-237`). ⌘N (`app/chrome.rs:554`). No dialog for units, bleed, colour mode or number of artboards. | New Document dialog: category presets, size, units, artboards, bleed, colour mode, raster ppi | `VC/engine/src/cmd/newdoc.rs:19` (categories), `:125` (bleed) | ADAPT (preset table plus fields) | bleed UI, colour mode | M |
| Open `.vrs` | **HAVE.** ⌘O (`app/chrome.rs:555`). Dialog filters `.vrs` and `.pdf` (`app/file_ports.rs:118-124`). Bounded reader (`pdf/read.rs:24-27`). Owner-seen (STATUS:12). | Opens .ai/.pdf/.eps/.svg directly | `VC/engine/src/cmd/fileio/load.rs` (328 lines) | n/a (ours) | — | — |
| Open a foreign file as a document (PDF without our model, SVG, AI, EPS) | **MISSING.** A PDF without the embedded model is refused: "not a Varos document" (`core/format/error.rs:89`). | Open any supported format | see section B | — | B6, B7 | — |
| Save / Save As | **HAVE** for `.vrs`. ⌘S and ⇧⌘S (`app/chrome.rs:559-560`; `app/app_command.rs:84-87`). Atomic tmp+rename (`core/file.rs:35-38`). Save gate checks the file can be reopened (`pdf/lib.rs:47-53`). Runs on the background worker (STATUS:30). Save As offers no other format. | Save As AI/PDF/EPS/SVG/template, each with options | `VC/engine/src/cmd/fileio/save.rs` (610) | n/a | — | — |
| Save a Copy | **PARTIAL.** Only the Bridge `save_as` keeps the tab's file, dirty state and Recent as they were (test `app/lifecycle.rs:1017`). No menu row or `AppCommand` (`app/app_command.rs:51-108`). | ⌥⌘S writes a copy and stays on the original | `VC/…/fileio/save.rs:55` | ADAPT | — | S |
| Revert | **MISSING** (no `AppCommand`, no menu row). | File ▸ Revert | `VC/…/fileio/save.rs:511` | IDEA | — | S |
| Close / Close All / quit guards | **PARTIAL.** ⌘W closes the tab with an unsaved-changes guard; Quit runs the quit flow over all tabs (`app/app_command.rs:93-95`; `app/chrome.rs:558`). No Close All. | Close, Close All (⌥⌘W) | `VC/engine/src/cmd/docmenu.rs:203` | IDEA | — | S |
| Multi-document tabs | **HAVE.** Activate/next/previous/reorder (`app/app_command.rs:97-105`; `app/workspace.rs`; STATUS:25). No floating or tiled document windows. | Tabbed or floating documents, Arrange ▸ Tile | `VC/engine/src/cmd/tabs.rs` (116) | — | D14 | — |
| Crash recovery (Data Recovery) | **HAVE.** A copy 30 s after the first change (`app/storage/scheduler.rs:9,36`). 2 generations, model-only blobs, per-session lock (`app/storage/recovery.rs:1-20,42-50`). Card and Review (`app/recovery_host.rs`). Owner saw restore after Force Quit (STATUS:28). | Data Recovery at a set interval | `VC/engine/src/cmd/recovery.rs` (923) | none (ours is equal or better) | — | — |
| Autosave into the document file | **MISSING** by design: only recovery copies; ⌘S already saves off-thread. | Local documents: Data Recovery only (cloud documents autosave) | — | — | owner decision | S |
| Versions / version history | **MISSING.** Undo only. Per-AI history is planned (PLAN A3-next). | Version History (cloud documents) | none found in the VC command list | BUILD | Bridge history, FileStore (PLAN A5) | L |
| Templates (Save as Template / New from Template) | **MISSING.** | `.ait`: opens as Untitled | `VC/…/fileio/save.rs:56`, `load.rs:281`; `template` flag in `VC/doc/src/lib.rs:529` | ADAPT | format flag (version bump) or a file convention; Start page | S–M |
| Warn when the file changed on disk | **HAVE.** "was changed by another app" (`app/file_ports.rs:106`). | Same warning | — | — | — | — |
| Package (collect fonts and linked images) | **MISSING.** There is nothing to collect yet: no images, no text (`core/model.rs:260-264`). | File ▸ Package: Links/ and Fonts/ folders plus a report | `VC/engine/src/cmd/package.rs:1-16` (skips fonts whose fsType forbids embedding; zip fallback) | ADAPT | B1, B12, text engine | M |
| Document setup: units, ppi, bleed, colour mode | **PARTIAL.** Units dropdown (`app/ui/panels/document.rs:16-24`). `DocUnits{ppi,display}` (`core/units.rs:113-121`), but no UI sets ppi (no `.ppi` hit in `app/`). The `bleed` field exists with the comment "UI is later" (`core/model.rs:376-379`); it is never drawn or exported. RGB only (`Rgba` paints). | Document Setup (units, bleed, transparency grid, paper); Document Color Mode RGB/CMYK | `VC/engine/src/cmd/docsetup.rs:1-2`, `docmenu.rs:205` (colour mode) | ADAPT fields; BUILD CMYK | CMYK = colour-model format bump plus CMS | M / XL |
| Document Info | **PARTIAL.** Board name, description and tags (`core/model.rs:536-547`, Board section `app/ui/panels/document.rs:11`); the Bridge `describe` gives counts. No Info panel (objects, colours, fonts, links). | Document Info panel | `VC/engine/src/cmd/docinfo.rs:16` (418) | ADAPT | — | S |
| Recent files and Start | **HAVE.** 20 recents (`app/storage/recents.rs:31`). Native Open Recent shows 10 (`app/chrome.rs:556,638-650`). Start v2 has thumbnails, tags, grid/list (`app/start.rs:6-7`). Owner acceptance pending (STATUS:27). | Home screen and recents | — | — | — | — |
| File association (Finder, Dock, `open`) | **HAVE** on Mac: `.vrs` declared (`tools/mac/bundle.sh:23,127-160`), `openURLs` hook (`app/mac_open.rs:1-14`). Finder double-click owner-seen (STATUS:29). Not on Windows. No Quick Look generator (grep `QuickLook`: none). | Registers .ai and others, Quick Look thumbnails | none (eframe app) | BUILD (Quick Look) | — | M |
| iCloud / cloud folders | **MISSING** as a feature. Plain `fs::write` + `rename` (`core/file.rs:35-38`), no `NSFileCoordinator` (grep: none). An iCloud Drive path works as an ordinary local folder; untested. | Cloud documents plus local files | none | BUILD | PLAN A5 `FileStore` | L |
| Window size/position and layout memory | **PARTIAL.** `layout.json` for panels (PLAN B-layout). Window geometry still pending (STATUS:63). | Remembered | — | — | — | S |

## B. Import / Place

| Capability | Varos today | Illustrator reference | Prior art | Liftable | Depends on | Size |
|---|---|---|---|---|---|---|
| **B1** Raster image object in the model | **MISSING** (`core/model.rs:260-264`; `core/scene.rs:29-39` has no image primitive). PLAN B4 is "proposed". | Placed/embedded images | `VC/doc/src/node.rs:326-340` (`ImageObject{key,w,h,xf,link,placement}`), `VC/doc/src/lib.rs:512-520` (`ImageBlob{mime,bytes,proxy}`) | ADAPT model | **format bump (v5)**, scene and wgpu textures (E10), PDF/SVG image output (E9) | L |
| **B2** Decode PNG/JPEG/GIF/WebP/TIFF/BMP | **MISSING.** `image` is built with `png` only, for icons (`varos-app/Cargo.toml:20`). | Place any common raster | `PC/codecs/src/lib.rs:39-55` (decode, size limits checked before allocating), `format.rs:7-26`; VC uses `image 0.25` with these features (`VC/../Cargo.toml:56`) | COPY (the `image` crate) + ADAPT the PC limits | B1 | M |
| B3 HEIC | **MISSING** | Place HEIC | `PC/heif/src/lib.rs:137` (pins `heic-rs =0.1.1`, a young crate) | IDEA (macOS ImageIO may be safer) | B1 | M |
| B4 PSD (flattened or as layers) | **MISSING** | Place PSD, layer comps | `PC/psd/src/lib.rs:71-82` (10,988-line reader/writer) | ADAPT later (flattened composite first) | B1 | L |
| B5 EXIF orientation and file ppi | **MISSING** | Placed size follows the file's ppi | `PC/codecs/src/orientation.rs:161-198`; `VC/engine/src/cmd/fileio/ppi.rs:1-3` | COPY (small, pure) | B2 | S |
| **B6** SVG / SVGZ import | **MISSING** (PLAN B3: "import still to do"; `core/svg.rs` writes only). | Open or Place SVG | `VC/svg/src/lib.rs:492-503` (via `usvg`), `import.rs:30` (1,164 lines), SVGZ `lib.rs:401-406` | ADAPT (usvg tree → our Path/Node; report what is dropped) | C14; gradient and stroke model for full fidelity | M–L |
| **B7** PDF / AI (PDF-compatible part) import | **MISSING** (refused, see A) | Open/Place PDF and AI | `VC/pdf/src/lib.rs:72` + `import_*.rs`, using `hayro-interpret` (pure Rust, see `VC/pdf/Cargo.toml`); `VC/engine/src/cmd/fileio/pdfimport.rs` (173) | ADAPT (keep separate from our own `.vrs` read, SAVE_EXPORT_PLAN §3) | B1, text, gradients | L–XL |
| B8 EPS import | **MISSING** | Open EPS | `VC/eps/src/import/` (PostScript interpreter, crate about 10k lines) | IDEA / later ADAPT | B1 | XL |
| B9 DXF import; DWG | **MISSING** | DXF/DWG import | `VC/cad/src/lib.rs:29` (DXF). VC also refuses DWG (`VC/cad/src/import.rs:113`). | ADAPT (DXF); none for DWG | — | M |
| B10 EMF/WMF import | **MISSING** | Windows metafiles | `VC/metafile/src/lib.rs:56,104` | IDEA (low priority on Mac) | — | M |
| **B11** File ▸ Place (one undo, real size) | **MISSING** (no row in `app/chrome.rs:552-566`) | Place: click or drag to size | `VC/engine/src/cmd/place/mod.rs:1-8` (raster at physical size; SVG/DXF as a group) | ADAPT | B1, B2 | M |
| **B12** Link vs Embed, Links panel (relink, update, go to, embed/unembed) | **MISSING** | Links panel | `VC/engine/src/cmd/links.rs:1-8` (`LinkInfo`: absolute and relative path, size, mtime, hash, low-res proxy), `:38-56` check/update/relink, `:962` embed, `:1034` unembed; `VC/doc/src/links.rs:22` | ADAPT | B1, format bump, Package | L |
| B13 Drag a file from Finder onto the canvas | **MISSING.** No `DroppedFile` or `HoveredFile` handling in `app/` (grep: none). A drop on the Dock icon opens a `.vrs` (`app/mac_open.rs:1-2`). | Drop places the file | `VC/ui-egui/src/lib.rs:863-869`, `place.rs:388` | IDEA (winit `WindowEvent::DroppedFile`) | B11 | S |
| B14 Paste from other apps (PDF/SVG/PNG/text flavours) | **MISSING.** The clipboard works only inside Varos (`core/clipboard.rs:3-6`); text fields get plain text only (`app/ui/menus.rs:153`). | Pastes PDF/AICB/SVG/bitmap | `VC/engine/src/cmd/clipboard/flavours.rs:1-7`, `resources.rs` (695); `arboard` | ADAPT (NSPasteboard; `objc2` is already a dependency, `varos-app/Cargo.toml:46-48`) | B2, B6 | M |

## C. Export / Output

| Capability | Varos today | Illustrator reference | Prior art | Liftable | Depends on | Size |
|---|---|---|---|---|---|---|
| PDF export (plain PDF, no Varos model inside) | **HAVE.** Scopes: all visible / active / artwork bounds (`pdf/export.rs:19-26,106-150`). Export sheet (`app/export_ui.rs:1-9`). Runs off-thread. Owner-seen (STATUS:12). CLI `export-pdf --artboard` (`varos-cli/src/main.rs:185-216`). Bridge `export_pdf` (`varos-bridge/src/lib.rs:31`). | Save Adobe PDF | — | — | — | — |
| PDF presets and options (compatibility, compression, image ppi, marks, security) | **MISSING.** The sheet offers page scope only (`app/export_ui.rs:1-5`). | Presets, Compression (downsample ppi), Marks & Bleeds, Security | `VC/pdf/src/presets.rs:56-83`, `settings.rs` (515), `marks.rs` (104), `encrypt.rs` | IDEA (VC writes PDF with krilla, we use pdf-writer) | images (nothing to downsample yet), bleed | M |
| PDF/X | **MISSING** | PDF/X-1a, X-4 | `VC/pdf/src/pdfx.rs` (272) | IDEA | CMYK/ICC (A, E8) | L |
| Bleed in the PDF (TrimBox/BleedBox) | **MISSING.** Only `media_box` is written (`pdf/write.rs:274`). | Bleed boxes, crop marks | `VC/pdf/src/marks.rs` | BUILD (small) | bleed UI | S |
| SVG export | **PARTIAL.** Core code: per artboard / active / whole board, names and ids (`core/svg.rs:1-4,14-27,97,128`, commit `fa19282`); checked with resvg (`varos-pdf/tests/export_svg.rs`). **Not reachable from any menu, the CLI or the Bridge** (grep: only tests call it). | SVG Options (styling, decimals, ids, images, outline text, minify, responsive) | `VC/svg/src/lib.rs:191-216` (`ExportOptions`), `:322` (`export_with_report`) | ADAPT the options list | Export sheet design (STATUS:35) | S (wire) + M (options) |
| PNG export | **PARTIAL.** `rasterize_artboard` renders the page rect at its page colour, up to a maximum size (`raster/lib.rs:85-112`), and `encode_png` exists (`raster/lib.rs:23`). Used only by the Bridge snapshot (`varos-bridge/src/dto.rs:434-463`) and the CLI `snapshot`, which draws a dark well with a grid and is not an export (`varos-cli/src/main.rs:176-184`). | Export As PNG (ppi or scale, background, anti-aliasing) | `VC/engine/src/cmd/fileio/encode.rs:434-449` | ADAPT (keep our rasterizer; add scale/ppi and transparent background) | — | S–M |
| JPEG/WebP/TIFF/GIF/BMP export | **MISSING** | Export As formats | `PC/codecs` encoders + `fidelity_warnings` (`PC/codecs/src/fidelity.rs:160`); VC `encode.rs:434-449` | COPY/ADAPT through `image` / `jpeg-encoder` / `image-webp` | PNG export | M |
| Export for Screens (artboards or assets, 1x/2x, suffixes, folders) | **MISSING** | Export for Screens and the Asset Export panel | `VC/engine/src/cmd/fileio/screens.rs:1-5` (468) | ADAPT the job shape {source × scale × format × suffix} (SAVE_EXPORT_PLAN §8 stage 2) | SVG, PNG and raster-format export | M |
| Slices / Save for Web | **MISSING** | Legacy Save for Web | `VC/engine/src/cmd/slices.rs` (717), `webexport.rs` (1,077) | IDEA (low priority) | Export for Screens | L |
| EPS export | **MISSING** (SAVE_EXPORT_PLAN §8: "EPS skipped unless demanded") | Save As EPS | `VC/eps/src/lib.rs:209` | IDEA | — | M |
| DXF/EMF/WMF/TXT export | **MISSING** | Export As | `VC/engine/src/cmd/fileio/encode.rs:417-429` | IDEA | — | M |
| Print (page setup, tiling, marks, separations) | **MISSING** (no Print row, `app/chrome.rs:552-566`) | Print dialog | `VC/engine/src/cmd/print.rs:38`, `printtiling.rs`, `VC/pdf/src/print/` | IDEA (Mac: send our PDF to the system print panel) | bleed/marks | M |
| Copy as SVG/PNG/PDF to the OS clipboard | **MISSING** (`core/clipboard.rs:3-6`) | Copy publishes PDF/AICB/SVG | `VC/engine/src/cmd/clipboard/flavours.rs:1-7` | ADAPT | SVG/PNG export, NSPasteboard | S–M |
| Export report (what was rasterised or dropped) | **PARTIAL.** Refusal reasons only (`pdf/export.rs:56-70`). Today nothing can be lost on export, because the model holds nothing that can't be exported. | Warnings, flattener report | `VC/svg/src/lib.rs:322`, `VC/pdf/src/lib.rs:71` (`*_with_report`); `PC/codecs` `fidelity_warnings` | ADAPT | must land before any lossy path | S |
| Export Cancel, Show in Finder | **PARTIAL.** The API has a cancel flag (`pdf/export.rs:137`); the Cancel button and Show in Finder are deferred (STATUS:30). | Both | — | — | — | S |
| Export selection only | **MISSING** (scopes are artboards or bounds) | Export Selection… | `VC/engine/src/cmd/fileio/export.rs:168` | IDEA | — | S |

## D. Canvas and view

| Capability | Varos today | Illustrator reference | Prior art | Liftable | Depends on | Size |
|---|---|---|---|---|---|---|
| Zoom: steps, fit, actual size, limits, % readout | **HAVE.** ⌘0/⌘1/⌘=/⌘− (`app/chrome.rs:612-615`). Fit = active page, or the art on a free canvas (`app/main.rs:396-410,622-626`). 5%–4000% (`app/gestures.rs:5-6`). Pinch and smart zoom (`app/gestures.rs:24-45`). Status bar % and Fit (`app/ui/bar.rs:768-780`). | Plus Fit All (⌥⌘0), typed zoom %, Zoom tool (Z) | `VC/ui-egui/src/menus.rs:182` (fitAll); `VC/tools/src/catalog.rs:136` | IDEA | — | S |
| Zoom tool / Hand tool | **MISSING.** `ToolKind` has neither (`core/editor.rs:39-52`). Space and middle-button pan work (`app/main.rs:62`; `app/host.rs:412-423`), as does trackpad pan. | Z and H tools | `VC/tools/src/catalog.rs:132,136` | IDEA | — | S |
| Rotate view | **MISSING.** Parked by the owner (PLAN B5, `docs/foundation/work_orders/VIEW_ROTATION.md`). | Rotate View (⇧H) | `VC/ui-egui/src/canvas.rs:75,555` | IDEA | owner decision | M |
| Rulers, origin, units | **HAVE.** ⌘R (`app/chrome.rs:617`). Rulers with drag-to-set origin and double-click reset (`app/ui/canvas_overlay.rs:18-26`). `ruler_origin` is saved in the document (`core/model.rs:587-590`). | Plus artboard vs global rulers, video rulers | — | — | — | — |
| Guides | **PARTIAL.** Drag out of the rulers, move, delete, hide ⌘;, lock ⌥⌘; (`core/model.rs:529-533,593-596`; `core/editor.rs:3006-3083,4714`; `app/chrome.rs:618-619`; `core/tests/guides.rs`). Missing: Make/Release Guides from paths, Clear Guides, typed guide position. | All of those | `VC/engine/src/cmd/docmenu.rs:123` (make), `:150` (clear); `VC/tools/src/rulerguide.rs` (524) | ADAPT | — | S |
| Smart Guides | **HAVE.** ⌘U and the alignment/geometric toggles (`app/chrome.rs:620-623`). Snap engine draws lines and equal spacing (`core/editor.rs:2728-2840`). | Plus angle and distance readouts while drawing | `VC/tools/src/guides.rs:1-6` | IDEA | — | S |
| Snapping (point/guide/page/bounds/geometry/grid/pixel) | **HAVE** except pixel. `SnapConfig` (`core/model.rs:448-485`); guides `core/editor.rs:2629`, grid `:2831`, points/segments/geometry `:3107-3245`; menu `app/chrome.rs:625-626`. **Snap to Pixel:** the `force_pixel_align` and `move_whole_px` fields (`core/model.rs:455-456`) are never read (grep). | Snap to Pixel / Pixel-perfect | none (VC's ROADMAP lists it missing there too) | BUILD | pixel preview | S–M |
| Document grid | **PARTIAL.** An adaptive base-5 dot grid on the board (`core/editor.rs:2645-2653`, rulers `app/ui.rs:560`), with Snap to Grid. `grid_spacing` = 72 pt (`core/model.rs:485,519`) is never read; snapping uses the adaptive step (`core/editor.rs:2832`). No Show Grid toggle and no grid preferences. | Show Grid ⌘', spacing, subdivisions, grids in back | `VC/doc/src/lib.rs:416` (`GridPrefs`), `VC/ui-egui/src/menus.rs:197-198` | ADAPT | — | S |
| Pixel grid / Pixel Preview | **MISSING** (grep: none) | Pixel Preview ⌥⌘Y | `VC/ui-egui/src/canvas.rs:223,335` | IDEA (our wgpu renderer must do it) | — | M |
| Outline mode ⌘Y | **MISSING** (grep: none) | Outline / Preview, also per layer | `VC/ui-egui/src/state.rs:162`, `canvas.rs:206`, `menus.rs:170` | IDEA | renderer | M |
| Overprint preview / separations / proof colours | **MISSING** | View ▸ Overprint Preview, Proof Colors | `VC/engine/src/cmd/colormgmt.rs:95` | IDEA | CMYK | L |
| GPU/CPU preview | **PARTIAL** by design: GPU only (ADR-0001). A CPU rasterizer exists for thumbnails and snapshots (`raster/lib.rs:1,32`). | GPU/CPU toggle | — | — | — | — |
| Screen modes | **PARTIAL.** Native full screen only (`app/chrome.rs:628`). | Presentation mode, Trim view | — | IDEA | — | S |
| Multiple windows / New Window / Arrange / split view | **MISSING.** One window: "Close Window = Quit (one window)" (`app/app_command.rs:50`). | New Window, Tile, Consolidate | VC has only a menu row (`VC/ui-egui/src/menus.rs:254`) | BUILD | shell box system | L |
| Navigator | **MISSING.** The status bar shows "Artboard i/n" (`app/ui/bar.rs:768`). | Navigator panel | `VC/ui-egui/src/chrome.rs:458` (artboard navigator only) | IDEA | — | M |
| Saved views | **MISSING** | New View… | `VC/engine/src/cmd/views.rs:16` | IDEA | format field | S |
| Artboards: create, resize, move, duplicate, delete, count, name, presets, orientation, colour, clip, hide, lock | **HAVE.** `core/editor.rs:2180-2559` (`ab_down`…`ab_set_count`), `:5192-5203` (hide/lock). Panel presets and actions (`app/ui/panels/artboard.rs:3-10,35-91`). Stable ids (format 4, `core/model.rs:363-370`). | Artboard tool and panel | — | — | — | — |
| Artboard reorder, rearrange, fit to art/selection, convert to artboards | **PARTIAL.** Reorder exists only through the Bridge (`core/bridge.rs:939`). The panel's "Fit" icon fits the view, not the artboard (`app/ui/icon_actions.rs:81-82`). | Rearrange All, Fit to Artwork Bounds, Convert to Artboards | `VC/engine/src/cmd/layer.rs:142` (fitToArt), `menucmds.rs:155` (rearrange), `panelcmds.rs:13` (reorder) | ADAPT | — | S–M |
| Export per artboard with its name | **PARTIAL.** PDF: active or all pages (`pdf/export.rs:19-26`); CLI `--artboard`. SVG has per-artboard names (`core/svg.rs:21-27`) but no UI. No per-artboard PNG. | Use Artboards / per-artboard export | see section C | — | SVG/PNG export | — |
| Bleed shown on the canvas | **MISSING.** `bleed` is read nowhere outside the model (grep). | Red bleed guide | — | BUILD | bleed UI | S |
| Canvas colour, transparency grid | **PARTIAL.** Fixed warm-black board with a dot grid (shell tokens). A transparent page shows a ghost paper (`raster/lib.rs:89-92`, `scene::AB_GHOST`). No checkerboard toggle. | Canvas colour preference, Show Transparency Grid | `VC/engine/src/cmd/views.rs:35` | IDEA | — | S |

## E. Raster objects inside the document

| Capability | Varos today | Illustrator reference | Prior art | Liftable | Depends on | Size |
|---|---|---|---|---|---|---|
| Image object (model, save, undo) | **MISSING** (see B1) | Embedded or linked image | `VC/doc/src/node.rs:326-340`, `VC/doc/src/lib.rs:512-520` | ADAPT | format v5 | L |
| GPU drawing of images (textures, mip levels, memory budget) | **MISSING** (`core/scene.rs:29-39` has no image primitive) | — | none for wgpu (VC draws on the CPU with `vello_cpu`) | BUILD | image object | M |
| Transform and opacity of images | **MISSING.** Would reuse `Xform` (`core/model.rs:46-50`) and the opacity groups (`core/scene.rs:54-58`). | Free transform, opacity, blend | — | BUILD on what exists | image object | S |
| Crop image | **MISSING.** The clip model exists (`core/scene.rs:58`). | Crop Image (on-canvas box) | `VC/engine/src/cmd/menucmds.rs:110,532` | ADAPT | image object | M |
| Link vs embed | **MISSING** (see B12) | — | `VC/engine/src/cmd/links.rs` | ADAPT | B12 | — |
| Image Trace | **MISSING** | Image Trace and its presets | `VC/trace/src/lib.rs:35-37,209-290` (1,511 lines, 26 tests); commands `VC/engine/src/cmd/buildcmds.rs:119-137` | ADAPT (pure; depends only on `image`) | image object | M |
| Rasterize selection | **MISSING.** The rasterizer exists (`raster/lib.rs`, tiny-skia). | Object ▸ Rasterize | `VC/engine/src/cmd/menucmds.rs:92,370` | ADAPT | image object | S–M |
| Resolution handling (effective ppi, raster effects ppi) | **MISSING** | Links panel ppi, Document Raster Effects Settings | `VC/engine/src/cmd/fileio/ppi.rs`; newdoc `rasterEffectsPpi` (`VC/engine/src/cmd/newdoc.rs:19`) | COPY/ADAPT | image object | S |
| Colour profiles (image ICC, document profile) | **MISSING** | Assign Profile, colour settings | `PC/codecs/src/image.rs:232-233` (keeps ICC); `PC/cms`; SAVE_EXPORT_PLAN §4 (moxcms) | ADAPT / BUILD | CMYK decision | L |
| Images in PDF and SVG export | **MISSING** (no images to write; pdf-writer can write image XObjects) | — | `VC/pdf/src/images.rs` (325); SVG `ImageMode` (`VC/svg/src/lib.rs:196`) | ADAPT | image object | M |

## 1. Missing basics that may surprise

The owner asked about export, image import, the basics, save options and Package.

1. **No image can enter a document at all.** There is no image object, no Place, no drag-and-drop and no paste of a picture (`core/model.rs:260-264`; no `DroppedFile` in `app/`). For an Illustrator alternative this is the biggest gap.
2. **The only user export is PDF.** SVG export is written and tested in core (`core/svg.rs`), but no menu, CLI or Bridge reaches it. **No PNG or JPEG export**, although the page renderer that would do it is already there (`raster/lib.rs:85`).
3. **The PDF export has no options:** no presets, quality or compatibility setting, no bleed/trim boxes (only MediaBox, `pdf/write.rs:274`), no marks.
4. **Bleed is a saved field that nothing uses:** no UI, nothing on the canvas, nothing in the PDF (`core/model.rs:376-379`).
5. **Save options are thin:** no Save a Copy (the Bridge has it, the menu doesn't), no Revert, no Close All, no templates, and Save As writes `.vrs` only.
6. **Package can't exist yet**, because nothing in a document is external: no linked images, no fonts.
7. **Copy/paste stays inside Varos.** Copy into Figma/Illustrator/Keynote, or paste from them, is not possible (`core/clipboard.rs:3-6`).
8. **The document can't open anything but its own `.vrs`.** A normal PDF, SVG or AI file is refused (`core/format/error.rs:89`).
9. **No Print** command, and **no Outline view (⌘Y)**, Hand/Zoom tools, Pixel Preview or Snap to Pixel (the pixel snap fields exist but are inert, `core/model.rs:455-456`).
10. **The grid is decoration:** an adaptive dot grid with no spacing setting or Show Grid toggle, and the saved `grid_spacing` is unused.
11. **ppi has no UI** (`core/units.rs:113-121`), so "px" always means 72 ppi. **Colour is RGB only:** no CMYK for print, which is the main MENA deliverable (SAVE_EXPORT_PLAN §7).

## 2. Suggested dependency order inside these areas

1. **Export first (no format bump):** wire SVG export into File ▸ Export, the CLI and the Bridge → PNG export through `rasterize_artboard` with scale/ppi and a transparent background → an export report seam (C14) → JPEG/WebP via `image`. Small pieces; the owner sees the result right away.
2. **Small file basics:** Save a Copy (reuse the Bridge path), Revert, Close All, typed guide position and Clear/Make Guides, Fit Artboard to Art, artboard reorder in the UI, Zoom/Hand tools, Fit All.
3. **Print-ready PDF:** bleed UI → TrimBox/BleedBox in PDF → bleed on canvas → a PDF options sheet (presets, crop marks) → Print through the system panel. (PDF/X waits for CMYK.)
4. **Image object (format v5, the one big bump):** model + save/load + wgpu texture + PDF/SVG image output → decode PNG/JPEG/WebP/GIF/TIFF (+EXIF orientation, ppi) → Place and drag-drop → crop, opacity and transform reuse → OS paste of bitmaps.
5. **Links:** linked images (path, hash, proxy) → Links panel (relink/update/embed) → Package (images; fonts after the text engine).
6. **Interchange import:** SVG import (usvg, with a loss report) → paste SVG/PDF from other apps → PDF/AI import (hayro) → DXF; EPS/EMF/DWG only if asked for.
7. **Image tools:** Image Trace (VC `trace` port), Rasterize, resolution warnings; HEIC/PSD as needed.
8. **View depth:** Outline mode, Pixel Preview + Snap to Pixel, a real document grid, Navigator, rotate view (when the owner unparks it), multiple windows.
9. **Colour management** (CMYK document mode, ICC, overprint preview, PDF/X): its own ADR; it touches the format, the picker and every exporter.

## 3. Summary

- Files are mostly solid: tabs, safe save/open off-thread, crash recovery, Start/Recent and Finder opening all work.
- Missing in files: Save a Copy (Bridge only), Revert, Close All, templates, Package, Document Info, ppi/bleed/colour-mode setup, cloud storage.
- Import is **completely missing**: no images, no SVG/PDF/AI/EPS/DXF, no Place, no drag-and-drop, no paste from other apps.
- Export is **PDF only** for the user. SVG is built but not wired; the PNG renderer exists but has no command.
- The PDF has no options, bleed, marks or PDF/X; there is no Print.
- Canvas is good on zoom, rulers, guides, smart guides, snapping and artboards.
- Canvas gaps: Outline, Pixel Preview, Snap to Pixel, a real grid, Hand/Zoom tools, Navigator, rotate view, multiple windows.
- The prior art covers almost every row. Best candidates to lift: `VC/trace` (Image Trace), `PC/codecs` (decode limits, orientation, fidelity warnings), the VC `ImageObject`/`LinkInfo` model, VC SVG import, and Package. Their PDF code is krilla-based, so it gives ideas only.
- The order that pays off fastest: wire SVG + PNG export → small file basics → bleed and print PDF → the image object (format v5) → links/Package → SVG/PDF import.
- One format bump (v5: image object + links) unlocks about half of the missing rows.
