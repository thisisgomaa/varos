> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# BStudio — Editor Redesign (interactive prototype)

> Chosen direction: **Focus / Pro skin** — neutral Figma-style chrome, floating
> panels, single sky-blue accent (`#2f8fe0`), effects kept light.
> English LTR chrome · Arabic-first canvas · correct kashida engine.

Open `index.html` in a browser. No build step — React + Babel load from CDN and
the `.jsx` files are transpiled in-browser. (For production, precompile the JSX.)

## What's interactive (not a mockup)
- **Real objects** — the poster is broken into selectable layers (heading,
  subhead, divider, kicker, accent block, latin label). Click to select.
- **Drag + resize** — move any shape; drag corner handles to resize; the
  inspector updates live.
- **Two-way inspector** — edit X/Y/W/H, fill (swatch popover), corner radius,
  opacity (slider); text gets size / line-height / align / colour.
- **Working kashida (`editor-model.jsx → kashidaText`)** — the moat slider
  inserts tatweel (ـ, U+0640) on the connection *after* a base letter + its
  harakat cluster, never inside a لا ligature, never between a letter and its
  diacritic. This is the typographically-correct join point.
- **Tools** — Rect / Ellipse / Text draw on drag; Move / Hand; keyboard
  shortcuts (V/R/O/T/H, Delete, ⌘D, arrows to nudge, +/− zoom).
- **Menus** (File/Edit/Object/Type/View), Layers tab with show/hide,
  wheel-zoom + zoom-to-fit.
- **Tweaks panel** (host edit-mode): accent colour, dot grid, keyboard hints.

## Files
| File | Role |
|---|---|
| `index.html` | entry — loads deps, mounts `EditorPro`, wires Tweaks + fit-to-viewport scaler |
| `editor-model.jsx` | shape data, `newShape`, **`kashidaText`** engine, helpers |
| `shared.jsx` | `Icon` set + (legacy) `CanvasArt` poster used by the comparison file |
| `EditorPro.jsx` | the editor: state, pointer interaction, tools, menus, zoom |
| `Inspector.jsx` | live two-way inspector + layers list + draggable sliders |
| `tweaks-panel.jsx` | host-protocol Tweaks shell + form controls |
| `styles/editor-base.css` | tokens (lifted from `v0/web/css/tokens.css`) + shared canvas |
| `styles/dir-focus.css` | neutral Figma-style "Focus" chrome |
| `styles/dir-focus-skins.css` | three visual skins (modern / solid / **pro**) |
| `styles/editor-pro.css` | prototype-only: artboard, shapes, selection, sliders |

## Notes / next steps (deferred)
- Pen tool, rotate, multi-select + align/distribute, real undo/redo, AI command bar.
- The sibling comparison file (3 directions × 3 skins) lives in the design tool
  under the original exploration — this folder is just the locked Pro editor.

_Design exploration produced for Ahmed. Chrome is English; canvas content is Arabic._
