# Varos — EXACT UI Spec (pulled from Ahmed's Figma)

> 🔁 **Right-dock UPDATE (2026-06-26):** the inspector is no longer "Design | Layers" (§2 below).
> Ahmed evolved it toward **Illustrator's panel structure** — see **`PANELS_PRO_SPEC.md`** for the
> right dock (`Transform·Align·Pathfinder` + `Properties·Layers·Libraries`), the floating left toolbar
> with the Fill/Stroke swatch, the pro Color Picker, and pro number inputs. **Keep this file's
> palette / radii / shadow / fonts / cursor set; take the panel structure from PANELS_PRO_SPEC.**

> 🧱 **Layout is now SOLID DOCKED (2026-06-26), not floating-over-canvas.** Windows can't reliably make
> child WebView2 panels transparent over the GPU canvas (black rectangles) or draw popups outside their
> rect (clipped dropdowns). So panels are **solid, docked to the edges**; the canvas is native in the
> center. The "floating pills over the dot-grid" look (control bar / AI bar / floating cards) is **dropped
> for now** — render those as solid docked bars. Keep the colors, radii (on the solid panels), fonts, and
> cursor set. See `PANELS_PRO_SPEC.md §0.1`.

> ⚠️ **This file is the source of truth for the UI look. It SUPERSEDES the colors and the
> top-bar/layout described in `DESIGN_BRIEF.md` and `design-reference/design-system.md`.**
> Those two were close but **wrong** vs Ahmed's real Figma (different hex values, and they
> described an Illustrator menu-bar layout — Ahmed's actual design is a clean Figma-style chrome).
> When this file and the old brief disagree, **THIS file wins.** Match Ahmed's Figma, not the old brief.

## 0. Source — pull it directly, do not eyeball
Ahmed's Figma file: `https://www.figma.com/design/8aJmebD05sAg48Jk3obngb/BSudio`
- fileKey: `8aJmebD05sAg48Jk3obngb`
- Inspector / full editor frame: nodeId **`97:273`** (other 4 states: `97:1823`, `97:1396`, `97:1147`, `232:2`)
- Tool rail: nodeId **`102:548`**

**If you have the Figma MCP connected to Ahmed's account, call `get_design_context` on `97:273`
and `102:548` and read the real values yourself — do NOT approximate from a screenshot.**
If you do NOT have Figma access, use the exact values captured below (the advisor read them off the file).

---

## 1. Color tokens — REPLACE the current ones with these EXACT values
The current build (`varos-app/src/main.rs`) uses `#1c1c1e / #26262b / #f0f0f2 / #9a9aa2`.
**Those are wrong.** Ahmed's Figma uses:

```css
--bg-app:     #141313;  /* app background (warm near-black), with a faint dot grid */
--bg-app-2:   #1e1e1e;  /* secondary canvas/background tone */
--bg-panel:   #1f1f22;  /* floating panel body */
--bg-surface: #262627;  /* input fields / chips resting */
--bg-hover:   #2c2c2c;  /* field hover */
--bg-active:  #2e2e2e;  /* field active / pressed */
--border:     #2a2a2d;  /* hairline divider (subtle) */
--border-2:   #3a3b3d;  /* stronger border (focused field) */
--text:       #e6e6e6;  /* primary text */
--text-2:     #d2d2d2;  /* secondary text */
--muted:      #8a8a8a;  /* labels */
--faint:      #7c7c7c;  /* faint / placeholder */
--accent:     #0c8ce9;  /* azure — active tool, Share button, focus, selection. (ALREADY correct) */
```
Keep the canvas selection color in the wgpu renderer in sync with `--accent #0c8ce9`.

## 2. Layout concept — match the screenshot, NOT an Illustrator menu bar
Ahmed's chrome is **Figma-style floating**, dark, with a faint dot-grid canvas. Full-bleed wgpu
canvas, floating rounded panels on top. From the screenshot of `97:273`:

**Top-left — document tabs (NOT a File/Edit/Object menu bar):**
- A row of rounded file tabs ("File", "File", "File" in the mock). Active tab = lighter surface.
- ❌ Remove the current `File · Edit · Object · Type · Select · Effect · View · Window` menu bar.
- ❌ Remove the `V` logo + `pre-alpha` pill from the top bar (not in Ahmed's design).

**Top-center — floating control bar (contextual, changes per selection):**
- `Alignment` label + 3 align icon buttons  ·  divider  ·  `Position` `X [-114]` `Y [-712]`  ·  `Rotation` `[0°]`.
- Rounded pill container, `--bg-panel`, hairline border, the floating shadow.

**Top-right:**
- A green status dot + a small chevron dropdown.
- A solid **azure `Share` button** (`--accent`, white text, rounded ~8px).

**Left — vertical tool rail (floating rounded column):**
Order top→bottom (with dividers shown as `—`):
`Hand/Grab` · **Selection (V)** · `Frame/Artboard` — `Rectangle` · `Ellipse` · `Pen` · `Pencil` · `Text (T)` — `Hand` · `Comment`
- Active tool = azure filled chip (`--accent`, white icon). Resting icon = `--muted`, hover = `--text`.
- Icons are thin outline (Lucide-style ~1.7 stroke) — the current Lucide set is the RIGHT style, keep it.
- (Map to Varos's real tools from `ILLUSTRATOR_TOOLS_CATALOG.md`; show the letter shortcut in the tooltip.)

**Right — inspector panel, tabs `Design` | `Layers`** (current build says "Properties" — rename to **`Design`**):
Sections top→bottom (the current build is MISSING most of these — add them):
1. **Object header row** — icon + object name (e.g. "T Text") + 2 small action icons (component, fullscreen).
2. **Layout** → `Resizing` (3 toggle buttons) · `Dimensions` `W` `H` + a link/lock toggle.
3. **Appearance** → opacity `100 %` + corner-radius `0`, with eye + droplet icons on the section header.
4. **Typography** (when text selected) → font family (`Inter`) · weight (`Regular`) + size (`12`) · line-height (`Auto`) + letter-spacing (`0 %`) · alignment row (6 icons). *(Latin first; Arabic deferred.)*
5. **Fill** → swatch + hex (`FFFFFF`) + opacity `%` + eye + remove. `+` to add.
6. **Stroke** → collapsed by default, `+` to add (weight/caps/joins/dashes when present).
7. **Effects** → collapsed, `+`.
8. **Export** → collapsed, `+`.
- Section headers: `--muted`, ~11–12px, letter-spaced. Fields: `--bg-surface`, 1px `--border`, radius ~5–6px.

**Bottom-center — AI utility bar (floating pill):**
- 4 buttons each with a small sparkle icon: `edite image` · `Generate Vectors` · `Image Tracer` · `Make an image`, then an azure grid/apps icon on the right. (These can be stubs/disabled for now — but the bar should exist to match the design.)

**Bottom-right — zoom pill:** `129%` `−` `+` in a rounded pill.
**Bottom-left — a small rounded status/breadcrumb pill (can be empty for now).**

## 3. Shape / elevation tokens
- Floating panels & pills: radius **12px**; buttons 6–8px; inputs 5–6px; round toggles 999px.
- Panel inset from window edge: ~**16px**. Inner card padding: ~12–16px. Field height ~28–30px.
- Floating shadow: `0 8px 30px rgba(0,0,0,0.45), inset 0 1px 0 rgba(255,255,255,0.03)`.
- Fonts: UI = **Inter**; numbers = **JetBrains Mono**. Base **13px**, line-height 1.5. Section labels ~11–12px.
- Canvas: `--bg-app #141313` + a faint dot grid (low-opacity dots, ~24px spacing).

---

## 4. The CURSOR — stop using Lucide UI icons as cursors
Lucide glyphs are **centered UI icons with no pointer tip / hotspot** — that is why the cursor
"doesn't feel like Illustrator." Replace with **real pointer cursors**: small SVGs drawn to look
like Illustrator's, each set on the canvas via CSS `cursor: url(...) hotspotX hotspotY, fallback;`
(or the native winit cursor when the pointer is over the wgpu canvas).

> Don't ship Adobe's actual cursor bitmaps (copyright). **Re-draw equivalents as tiny SVGs** —
> the shapes are simple and standard. Match the silhouette + the hotspot.

Required set (hotspot = the pixel that actually points):
| Tool / state | Cursor | Hotspot |
|---|---|---|
| Selection (V) | solid **black arrow** (classic NW arrow, white outline) | tip (0,0) |
| Direct Selection (A) | **hollow/white arrow** | tip (0,0) |
| Pen (P) — drawing | pen **nib** | nib tip |
| Pen — start new path | nib + small **×** | nib tip |
| Pen — close path | nib + small **○** | nib tip |
| Pen — add anchor | nib + small **+** | nib tip |
| Pen — delete anchor | nib + small **−** | nib tip |
| Pen — convert anchor (Alt over anchor) | nib + small **^** (caret) | nib tip |
| Pencil | pencil glyph | tip |
| Text (T) | **I-beam** | center |
| Rectangle / Ellipse / shapes | **crosshair** (+ tiny shape badge) | center |
| Hand (Space) | **grab** ✋ / **grabbing** ✊ while dragging | center |
| Zoom (Z) | **magnifier** with `+` (Alt → `−`) | glass center |

Cursor size 32×32 (provide @2x for hi-dpi). The contextual pen variants must switch live based on
what the pointer is over (empty / first-anchor / segment / mid-anchor) — same logic already proven
in `pen-spike.html`.

---

## 5. How to apply (incremental — Ahmed verifies each in the real app)
1. Swap the color tokens in all 3 inline HTML blocks in `main.rs` (TOPBAR_HTML, TOOLS_HTML, PANEL_HTML) to §1. → show Ahmed.
2. Rebuild the **top bar** to §2 (document tabs + center control bar + green-dot + Share; remove menu bar & V/pre-alpha). → show Ahmed.
3. Rename inspector tab `Properties → Design`; add the missing sections (Layout/Resizing, Appearance, Typography, Effects, Export) per §2. → show Ahmed.
4. Add the bottom AI bar + zoom pill + bottom-left pill (stubs OK). → show Ahmed.
5. Replace the Lucide "cursors" with the real cursor set in §4. → show Ahmed (he tests the pen feel).

Ahmed leads the look and verifies every step in the real window. Match his Figma pixel-for-pixel —
when in doubt, open `97:273` and measure.
