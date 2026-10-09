> **Status:** approved by the owner 2026-10-09 («Export / D — Illustrator-like · Advanced») — design of record is the Figma file; this page is the behaviour contract for plan slices 0.4, 0.5 and the first half of 1.8.

# Export sheet v2 — like Illustrator's Export for Screens, Minimal by default, Advanced on demand

**Design of record:** https://www.figma.com/design/qYRabe9nDx0TyXvuZMwFcA — frames `Export / D — Illustrator-like · Minimal` (14:2), `… · Advanced` (14:82), `… Minimal · Selection tab` (14:236), `Export / Done state` (10:251). Owner's reference: Illustrator's Export for Screens (Artboards / Assets tabs with the thumbnail checklist — «الجزء اللي بختار منه… مهم جداً وقوي ومش محتاج افتكاسات»). Dark mode only. Read geometry, spacing and colours from Figma.

## 1. Shell
- One sheet (kit sheet, hand-painted, no shadow) opened by **File ▸ Export… (⌥⌘E, Illustrator's Export for Screens key)**, the Windows burger Export row, and the Bridge/CLI equivalents. It replaces the S6-C "Export PDF" sheet; `File ▸ Export ▸ PDF…` becomes a preset of this sheet (format PDF, 1×).
- Two states: **Minimal** (default) and **Advanced** (▸ Advanced toggle bottom-left; state persisted in `layout.json` additively). Advanced only adds rows; nothing moves.
- **Cancel** stops a running job (existing cancel flag); **Export N files** runs on the IO worker; the **Done state** shows the file name(s), the export report notes (what was rasterised / skipped / unsupported — never silent), **Show in Finder** and **Done**.
- Esc closes when idle; while a job runs Esc = Cancel. Live: the canvas stays usable behind the sheet? No — it is a sheet (modal to the tab) like today; the box system is untouched.

## 2. Left pane — what to export (the important part)
- Tabs **Artboards | Selection**, kit segmented control (ON = TOGGLE_WELL + TEXT).
- **Artboards:** a thumbnail grid (72×90 cards, real artboard thumbnails from the existing thumbs pipeline, name under each) with a **checkbox on every card** (bottom-left, azure when on); click toggles, ⇧-click ranges, double-click = only this one. Grid/list toggle (list = name · size · checkbox rows). Footer: `Selected N · M files` (mono) + **Clear**.
- **Selection:** the same grid for the current selection — each selected object or group is one asset card (its own bounds; 72×72). Empty selection = the tab is disabled with the reason.
- **Advanced adds:** `All | Range [1–2]` radio row, `Include bleed` (reads the document bleed; disabled until 1.1 lands with the reason "no bleed set"), `Include artboard colour` (page colour in the export; off = transparent where the format allows), `Whole board as one file` radio (= today's Artwork bounds scope).

## 3. Right pane — how
- **Export to:** folder field (mono) + `…` picker; remembered per document; default `~/Desktop/Export`.
- **Formats (Minimal):** `Format` dropdown (PDF · SVG · PNG · JPEG · WebP · TIFF) and `Scale` dropdown (1× · 2× · 3× · ppi… ; for PDF/SVG the row reads "vector").
- **Formats (Advanced):** the table **Scale · Suffix · Format · ×** with `+ Add scale`, presets `iOS` (1×,2×,3× PNG) · `Android` (mdpi…xxxhdpi PNG) · `Web` (1×,2× PNG + SVG); `Prefix` field; `Open folder after export`; `Create sub-folders` (by scale | by format); `PDFs as` (single file | one per artboard).
- Per-format options live in a small popover from the format cell (later slices): PNG transparent background/interlace, JPEG quality, PDF preset + image ppi (1.2), SVG styling/decimals/ids/minify (1.8). Slice 0.4/0.5 ship sensible defaults and the popover for PNG (transparent) + JPEG (quality) only.
- Button label counts the files: `Export 6 files`.

## 4. Behaviour rules
- Scopes map to the existing export plans: artboards checklist → per-artboard pages; `Whole board as one file` → ArtworkBounds; Selection → bounds of each selected asset. Unavailable combinations are disabled with the plain-English reason (today's rule).
- Every job returns an `ExportReport` (plan 0.1); the Done state lists its notes; the CLI prints them; the Bridge returns them under API 1.2.
- File names: `<prefix><artboard or asset name><suffix>.<ext>`; collisions get ` 2`, ` 3`…; the sheet never overwrites silently (same rule as save-as).
- Remembered per tab: last tab, checklist, folder, formats table, Minimal/Advanced.
- Heat: thumbnails come from the existing thumb cache; no per-frame rendering; the sheet repaints only on interaction.

## 5. Shortcuts (Illustrator parity)
⌥⌘E opens the sheet (Export for Screens). ⌘⇧E stays unbound (Illustrator's Export As has no key). Esc = close / cancel. ⌘A inside the grid = select all cards.

## 6. Slices
- **0.4** SVG export wired + sheet v2 **Minimal** (Artboards/Selection grid, folder, Format PDF/SVG, Scale "vector", Done state with report, Cancel/Show in Finder) + CLI `export-svg` + Bridge `export_svg`.
- **0.5** PNG/JPEG/WebP/TIFF via `varos-raster` + `image` (scale/ppi, transparent background, JPEG quality popover) + fidelity notes in the report.
- **1.8** Advanced state: formats table, presets, suffix/prefix, sub-folders, PDFs-as, Include bleed/colour, Range; SVG options popover.
- **1.2** PDF options popover (presets, image ppi, marks) plugs into the same format cell.

## 7. Tests
Headless: card toggling (click/⇧/double), tab disable reasons, file-name composition + collision rule, scope→plan mapping on fixtures, report rendering in the Done state, persisted state round-trip (layout.json additive), cancel flag honoured, Minimal/Advanced row sets, CLI/Bridge parity on the same document (bytes identical to the app's export).
