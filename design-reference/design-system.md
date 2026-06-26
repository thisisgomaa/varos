# BStudio Design System

> **Locked principle: Figma is the single source of truth.** Every visual decision
> — palette, spacing, the floating-chrome layout, each screen — is authored in the
> BStudio Figma file first, then hand-ported to React/CSS. When code and Figma
> disagree, **Figma wins**; the code is brought back into line, not the reverse.
> The CSS comments in `index.css` repeatedly cite this ("Figma source-of-truth",
> "matches the Figma stack", "BARE, no enclosing pill — the design target").

This doc describes the live editor skin as implemented in
`v0/web/bstudio/src/index.css` (the `:root` token block, ~lines 48–82) and the
companion decision record `COLORS.md` (repo root). It is the canonical reference
for tokens, type scale, spacing, radii, shadows, the floating-chrome layout, and
the Figma to React file mapping.

---

## 1. Two color systems (and which is live)

There are **two** color systems in the repo, and it matters which one you touch:

1. **`COLORS.md` (repo root) — the Phase-1 research/decision record.** It adopts
   **Radix Colors** (`slate-dark` neutrals + a custom 12-step *BStudio Gold*
   scale anchored on `#c9a961` at step 9) and prescribes a canonical token file
   at `v0/web/css/tokens.css` using `var(--slate-2)`, `var(--pivot-gold-9)`, etc.
   This is the *aspirational* token grammar (Radix step semantics: 1–2 bg, 3–5
   surfaces, 6–8 borders, 9–10 solid fills, 11–12 text).

2. **`v0/web/bstudio/src/index.css` `:root` — the live editor skin (2026-06-08
   "Figma-style dark UI" restyle).** This is what the running React app actually
   uses. It is a *flat hand-tuned hex palette* mapped onto short semantic variable
   names ("Mapped onto the existing variable names so nothing downstream breaks").
   The accent here is **Figma blue `#0c8ce9`**, not the gold — gold is demoted to
   a brand-only cue (the avatar).

> Practical rule: to restyle the **editor chrome today**, edit the `:root` tokens
> in `index.css`. `COLORS.md` / `tokens.css` describe the longer-term Radix target
> and the light-theme / user-pickable-accent roadmap (Phase 2).

---

## 2. Color tokens — the live `:root` (index.css ~48–82)

### Surfaces (dark, layered lightest-on-top)
| Token | Value | Role |
|---|---|---|
| `--bg-app` | `#1c1c1e` | App background / full-bleed canvas (`.shell`, `.viewport`) |
| `--bg-panel` | `#202024` | Panel body (Inspector card `.props-panel`) |
| `--bg-surface` | `#26262b` | Fields, sub-surfaces, brand-tag chip |
| `--bg-float` | `#232327` | **Floating chrome** surface (`.floatbar`, zoom pill) |
| `--bg-hover` | `#2c2c33` | Hover state on buttons/tools |
| `--bg-active` | `#34343a` | Active / pressed state |

### Borders
| Token | Value | Role |
|---|---|---|
| `--border-subtle` | `#2a2a30` | Default hairline on floating cards / dividers |
| `--border` | `#34343a` | Standard component border |
| `--border-strong` | `#48484f` | Emphasised / hover border |

### Text
| Token | Value | Role |
|---|---|---|
| `--text-faint` | `#6b6b72` | Disabled / `.file-ext` / tertiary |
| `--text-muted` | `#a0a0a8` | Secondary labels, idle tool icons |
| `--text` | `#f0f0f2` | Default body text |
| `--text-strong` | `#ffffff` | Headings, active tab; also the **UI text-selection band** (`::selection { background: var(--text-strong); color: var(--bg-app) }`) |

### Glass overlays
| Token | Value | Role |
|---|---|---|
| `--glass` | `rgba(255,255,255,0.035)` | Translucent panel fill (placeholder card) |
| `--glass-strong` | `rgba(255,255,255,0.06)` | Stronger glass overlay |

### Accents — the load-bearing distinction
| Token | Value | Role |
|---|---|---|
| `--accent` | **`#0c8ce9`** | **Functional accent (Figma blue).** Selection / highlight, active tool chip, brand-mark square, primary "+" in the AI bar. Comment in source: "reads on light + dark". Also drives canvas selection in `SkiaCanvas.tsx` (hardcoded `#0c8ce9`) and the text-edit selection bands (`rgba(12,140,233,0.32/0.45)` in `.text-sel-band` / `::selection`). |
| `--gold` | **`#c9a961`** | **Brand accent only.** Used for the account avatar (`.avatar { background: linear-gradient(135deg, var(--gold), #b08833) }`) and brand cues. Per `COLORS.md` this is the historical brand color (Radix BStudio-Gold step 9). |

> The 2026-06-08 restyle **flipped the accent**: gold was the old functional
> accent; the Figma skin makes **blue `#0c8ce9` functional** and keeps gold as a
> brand identity touch. Do not reintroduce gold as a selection/active color.

`color-scheme: dark` is set on `:root`; there is no light theme yet (Phase 2 in
`COLORS.md`).

---

## 3. Typography

Set on `:root` and inherited everywhere:

- **UI font:** `'Inter', system-ui, -apple-system, sans-serif`
- **Mono font:** `'JetBrains Mono', ui-monospace, monospace` (eyebrows, `.brand-tag`)
- **Base size:** `13px`; **base line-height:** `1.5`
- **Smoothing:** `-webkit-font-smoothing: antialiased; -moz-osx-font-smoothing: grayscale`

**Arabic engine fonts** (loaded via `@font-face` at the top of `index.css`, files
in `src/fonts/`) make the inline edit textarea show the *same* letterforms the
WASM canvas engine paints. Family names + order mirror `EDIT_FONT` in
`TextOverlay.tsx` and the engine `FontFamily` indices:
`Amiri (0) · Cairo · IBM Plex Sans Arabic · Tajawal · Noto Sans Arabic (4)`.
Cairo + Noto are variable (`font-weight: 100 900`); the others are 400 only.

### Effective type scale (sizes actually used across the chrome)
| px | weight | Used for |
|---|---|---|
| 9–10 | 500–600 | Micro-labels, eyebrows, tool sublabels, kbd hints |
| 11–11.5 | 500 | Field labels, secondary rows |
| 12 | 500–600 | Body controls, file title, zoom %, link text |
| 12.5 | 500 | AI chip text |
| 13 | 500–600 | Default body, brand name, inspector header, section titles |
| 14 | 600–700 | Brand-mark glyph, prominent buttons |
| 18 / 22 | 600 | Placeholder-card display headings |

Letter-spacing tightens slightly on brand/heading text (`-0.01em` to `-0.02em`).

---

## 4. Spacing

No `--space-*` tokens exist; spacing is literal px, but it follows a consistent
**~2px-step micro grid** clustering on a 4-based scale:

- **2 / 4 / 6 / 8** — intra-control gaps (icon to label, chip internals, button rows).
  Common: `gap: 8px` (brand, AI chips, sections), `gap: 6px` (tool rail / file bar),
  `gap: 4px` (rail buttons), `gap: 2px` (zoom buttons, file-name).
- **10 / 12 / 14** — section gaps, field-group margins (`.field-group { margin-bottom: 14px }`).
- **16** — the **canonical chrome inset**: every floating bar sits `16px` from its
  viewport edge (`top/left/right: 16px`; AI bar `bottom: 16px`).
- **24 / 28 / 32** — card padding (`.placeholder-card { padding: 28px 32px }`),
  viewport padding.

Floating-bar control heights: file bar `32px`, top-right row `36px`, context bar
`44px`, AI bar `56px`, zoom pill / share row `36px` — a small set of fixed rhythms.

---

## 5. Radii

Token-backed: `--radius-float: 12px` (all floating chrome cards).
Literal radii in use form this scale:

| Radius | Use |
|---|---|
| `3px` | Tiny chips / kbd hints / swatch corners |
| `4px` | Small buttons, brand-tag, titlebar links |
| `5–6px` | Medium buttons, zoom buttons, flyout items |
| `7px` | Tool buttons, brand-mark square, file img buttons |
| `8px` | Larger buttons |
| `10px` | Pills (zoom pill, popovers) |
| **`12px`** | **Floating cards** (`--radius-float`): inspector, tool-rail card, placeholder card, context bar |
| `999px` | Fully round (avatar, save dot, status dots) |

---

## 6. Shadows

- `--shadow-float`: `0 8px 30px rgba(0,0,0,0.45), 0 1px 0 rgba(255,255,255,0.03) inset`
  — the signature floating-chrome elevation (drop shadow + 1px inset top
  highlight). Applied via `.floatbar` and the zoom pill.
- Inspector card uses a slightly softer `0 8px 24px rgba(0,0,0,0.35)`.
- Avatar ring: `box-shadow: 0 0 0 1.5px rgba(255,255,255,0.18)` (thin light outer ring).

There are no elevation *tokens* beyond `--shadow-float`; other shadows are inline.

---

## 7. Floating-chrome layout (the `.shell`)

The editor is **full-bleed canvas with all chrome floating on top** — there is no
CSS grid splitting the viewport; every bar is absolutely/fixed-positioned over a
single edge-to-edge canvas. This is deliberate: the comment in `index.css` notes
the canvas "ALWAYS resolves to the full viewport (`inset:0`) so the engine's box
is never zero and pointer to world math stays correct."

```
.shell  (position:relative; 100vw x 100vh; overflow:hidden; user-select:none; cursor:default)
 |- .viewport (position:absolute; inset:0; z-index:0)        <- full-bleed WASM/Skia canvas + overlays
 |- .titlebar (top:16 left:16, z:30, bare)                   <- LogoMenu + "Untitled.bs" + save dot
 |- .contextbar.floatbar (top:14 left:50% centred, h:44)     <- ControlBar (history/align/ops/pos)
 |- .topright (top:16 right:16, z:30, bare)                  <- Share button + gold .avatar
 |- .tools-palette -> .tools-rail (left:16, vert-centred, z:30)  <- ToolsPalette vertical rail
 |- .props-panel (top:64 right:16, w:300, z:30)              <- PropertiesPanel (Inspector card)
 |- .ai-bar (bottom:16 left:50% centred, h:56, z:30)         <- AiBar discrete chips
 \- .zoom-pill (bottom:14 right:14, h:36, z:30)              <- ZoomPill
```

Key layout facts (all from `index.css`):
- `.shell` **disables text selection chrome-wide** (`user-select:none`) and
  re-enables it only on real text-entry surfaces (`.shell input/textarea/[contenteditable]`,
  `.text-edit-ta`). Owner request: dragging must not highlight menu/label text.
- `.floatbar` is the generic floating surface: `--bg-float` + `--border-subtle`
  + `--radius-float` + `--shadow-float`, `z-index: 30`.
- The **file bar and top-right row are intentionally "bare"** (transparent, no
  enclosing pill) at a `16px` inset — matching the Figma file-bar look. Only the
  centre context bar, inspector, tool-rail card, AI chips and zoom pill paint a
  surface.
- The **tool rail** wrapper `.tools-palette` is `pointer-events:none` and only its
  inner card receives events — so the empty rail column is click-through to canvas.
- The active tool is the only rail button that paints (a blue `--accent` chip);
  resting icons float bare on the dark canvas ("Figma source-of-truth").
- Inspector `.props-panel` is `height: fit-content`, `max-height: calc(100vh-80px)`,
  scrolls internally — a self-contained floating card, not a docked column.

`.shell` is assembled in `App.tsx` (~lines 43–110): `<main className="viewport">`
holds `SkiaCanvas / ImageOverlay / TextOverlay / FrameOverlay / ReviewLayer`,
followed by the floating bars in source order above.

---

## 8. Figma to code mapping

Figma is the source of truth; these are the React files that port each Figma node.
Node IDs are from the BStudio Figma file.

| Figma node | Screen | React file | Status |
|---|---|---|---|
| `144:223` | **Editor** (full app shell) | `v0/web/bstudio/src/App.tsx` + `index.css` `.shell` | Live — the assembled floating-chrome editor |
| `146:408` | **Inspector** | `v0/web/bstudio/src/PropertiesPanel.tsx` (`.props-panel`) | Live — Design/Layers tabs, collapsible Fill/Stroke/Effects sections |
| `137:203` | **Home** (dashboard / recent files) | *(not yet ported)* | Figma-only — no `Home`/dashboard component exists in `src/` |
| `148:100` | **New Document** | *(not yet ported)* | Figma-only — no New-Document component exists in `src/` |
| `150:108` | **AI Chat** | `v0/web/bstudio/src/AiBar.tsx` (`.ai-bar` / `.ai-chip`) | Live (bottom-centre chips); full chat surface may still be expanding |
| `156:2` | **Boot Splash** | *(not yet ported)* | Figma-only — no splash component exists in `src/` |

Supporting chrome components (not separately node-mapped above but part of node
`144:223` Editor): `ToolsPalette.tsx` (left rail), `ControlBar.tsx` (centre context
bar), `MenuBar.tsx` / `LogoMenu` (file menus), `ZoomPill.tsx`, `ContextMenu.tsx`,
`ReviewPanel.tsx` / `ReviewLayer.tsx`, plus canvas overlays
(`TextOverlay.tsx`, `ImageOverlay.tsx`, `FrameOverlay.tsx`) and the renderer
(`renderer/SkiaCanvas.tsx`).

> **Gotcha:** the exact node IDs (`144:223`, etc.) do **not** appear anywhere in
> the source tree — they live in Figma, not in code. The mapping is maintained by
> convention. Verify against the Figma file before relying on a node ID.

---

## 9. Gotchas & conventions

- **Two palettes, one live.** Don't assume `COLORS.md` / `var(--slate-*)` is what
  renders — the editor reads the flat hex `:root` in `index.css`. `tokens.css`
  (Radix) is the Phase-2 direction, not the current chrome.
- **Accent is blue, not gold.** `--accent #0c8ce9` is functional; `--gold #c9a961`
  is brand-only (avatar). The hardcoded `#0c8ce9` also appears in `SkiaCanvas.tsx`
  for canvas selection — keep them in sync if the accent ever changes.
- **No spacing/elevation token scale** beyond `--radius-float` and `--shadow-float`;
  most spacing/radii are literal px. Follow the 4-based micro grid and the `16px`
  edge inset when adding chrome.
- **`.bakN` files everywhere.** `src/` is littered with `*.tsx.bakN` / `index.css.bakN`
  snapshots. Only the non-`.bak` files are live; ignore the backups when reading.
- **`.shell` swallows selection.** New interactive text inputs must live inside
  `input/textarea/[contenteditable]` (or carry `.text-edit-ta`) or the user won't
  be able to select their text — that's by design.
- **Figma wins.** When porting, match the Figma node pixel-for-pixel; the CSS
  comments record where the design overrode an "obvious" engineering choice
  (bare bars, no rail card, blue active chip only).

---

## 10. Where this connects

- **Renderer:** the chrome floats over `renderer/SkiaCanvas.tsx` (WASM/Skia). The
  blue accent and selection bands are shared between CSS chrome and the canvas
  drawing layer.
- **Engine fonts:** the `@font-face` block ties the DOM text-edit overlay to the
  Rust/WASM text engine's `FontFamily` indices — a design-system concern because
  the *visible letterforms must match* between typing and committed canvas text.
- **Roadmap (`COLORS.md` Future section):** light theme (swap `--slate-*` to
  `slate-light`), user-pickable accent (swap `--pivot-gold-*`), and a
  document-level color picker (shape fills are *not* tied to the chrome palette).
- **Unbuilt screens:** Home (`137:203`), New Document (`148:100`), Boot Splash
  (`156:2`) are designed in Figma and await React ports; the editor (`144:223`)
  and Inspector (`146:408`) are the shipped surfaces.
