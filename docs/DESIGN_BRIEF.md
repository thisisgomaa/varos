> ⚠️ **SUPERSEDED for colors + top-bar/layout by `UI_FIGMA_SPEC.md` (read that FIRST).**
> This brief's color hex values and its "Illustrator menu bar" top layout were WRONG vs Ahmed's
> actual Figma (`8aJmebD05sAg48Jk3obngb`, node `97:273`). Ahmed's design is Figma-style floating
> chrome with document tabs + a center control bar + Share button, and a darker `#141313` palette.
> Use `UI_FIGMA_SPEC.md` for the look. The notes below on "apply don't redesign" + incremental
> verification still hold.

# Varos — UI Design Brief (APPLY the existing design, don't design from scratch)

A complete **dark + azure, Illustrator-style editor UI** was already designed (for the BStudio era) — a full design system + a working React/CSS prototype + screenshots — and it maps **directly** onto Varos. **Apply it** to Varos's web (wry) panels. We are NOT designing the UI from scratch, and we are NOT far from the target — the target already exists.

## Reference files — `D:\VAROS\design-reference\`
- **`design-system.md`** — the exact tokens (colors, type, spacing, radii, shadows, layout). THE canonical reference.
- **`editor_shot.png`** + **`mockups/bstudio_*.png`** — screenshots of the actual editor look (the docked Illustrator-style dark UI + a real color picker + Layers).
- **`editor-redesign/`** — a working React/CSS prototype (the floating "Focus/Pro" variant): `EditorPro.jsx`, `Inspector.jsx`, `styles/`. Good component reference.
- **`mockups/`** — 4 HTML direction explorations (figma-like / illustrator-like / minimal-hybrid / dark-monochrome) + a README comparing them. Open them in a browser.

## The design language (apply these tokens to the web panels)
- **Theme: dark.** Surfaces: `--bg-app #1c1c1e` → `--bg-panel #202024` → `--bg-surface #26262b` → `--bg-float #232327` → hover `#2c2c33` → active `#34343a`. Borders: `#2a2a30 / #34343a / #48484f`. Text: `#f0f0f2` · muted `#a0a0a8` · faint `#6b6b72` · strong `#ffffff`.
- **Accent: azure `#0c8ce9`** — functional (selection, active tool, highlights). Keep it in sync with the canvas selection color. (Gold `#c9a961` = brand-only, e.g. logo — never for selection.)
- **Fonts:** UI = **Inter**; numbers/labels = **JetBrains Mono**. Base 13px, line-height 1.5.
- **Spacing:** 4-based micro-grid (2/4/6/8 intra-control · 10/12/14 sections · **16px edge inset** · 24–32 card padding).
- **Radii:** floating cards/panels = **12px** · buttons 5–8px · pills 10px · round 999px.
- **Shadow (floating elevation):** `0 8px 30px rgba(0,0,0,0.45), inset 0 1px 0 rgba(255,255,255,0.03)`.

## The layout — RECOMMENDED: docked Illustrator-style (like the screenshots)
Full-bleed wgpu canvas, web-panel chrome around it:
- **Top menu bar:** File · Edit · Object · Type · Select · Effect · View · Window.
- **Context/control bar** below: changes per tool (selection ops · align · boolean · position).
- **Left vertical tool rail:** Varos's tools with letter shortcuts shown (V/A/P/M/L/… from `ILLUSTRATOR_TOOLS_CATALOG.md`). Active tool = azure chip.
- **Right inspector ("DESIGN"):** Size (X/Y/W/H) · Fill (swatch + a real color picker) · Stroke · Opacity — collapsible sections.
- **Layers panel** (already started) under the inspector — keep the two-way selection sync.
- **Zoom pill** bottom-center · **status bar** bottom.
- *(Alternative: the floating-glass "Focus/Pro" variant in `editor-redesign/`. Ahmed confirms docked vs floating — he leans docked.)*

## Adapt — do NOT copy 1:1
- **LTR only** — drop ALL RTL / Arabic-first chrome decisions (Varos chrome is English/LTR; Arabic is deferred).
- **No collaboration UI** — drop avatars / share / inbox (Varos is offline desktop).
- Map the tool rail + inspector sections to Varos's actual tools/properties (from the catalog).
- The reference is React/Skia (BStudio); Varos's panels are the **wry webview** — port the design as plain HTML/CSS into the webview panels.

## How to apply (incremental — Ahmed verifies + refines each step; the look is HIS domain)
1. Drop the **design tokens** above into the web panels as CSS variables.
2. Restyle the **existing** web tools rail + Layers panel to match (dark + azure + the rail look + shortcuts).
3. Add the **top menu bar** + **context bar** + **inspector** (Size/Fill/Stroke/Opacity + color picker) as web panels.
4. Show each step to Ahmed in the real app; he refines the look.

**Don't redesign — APPLY.** Ahmed leads the look; you implement it on the wry web panels.
