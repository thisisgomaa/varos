# VAROS LAYERS PANEL — DEFINITIVE SPEC & GAP ANALYSIS

> Synthesised 2026-07-03 from a 4-agent deep-research pass (Illustrator anatomy · Photoshop/Figma/Affinity ·
> visual design language · gap-analysis) after Ahmed's "we are still very far". The model (scene-graph node
> tree, Layers Stage A) STAYS; the panel UX + look are rebuilt to this. Staged plan in §5.

Anchored to the current build (`D:\VAROS\varos\crates\varos-app\src\ui.rs`, `build_layers` ~L1786–1944; tokens L16–26) and the three research briefs. Every pixel number below is a build target; the model (scene-graph node tree in `varos-core`) stays, the panel UX + look are rebuilt. Ahmed's verdict — "we are still very far" — is correct, and the reason is concentrated in three places: **no drag-drop, fake lock, and scroll that leaks to the canvas.** Everything else is polish on top.

---

## 1. GAP ANALYSIS — current panel vs the professional bar

Ranked by how badly it reads as "not a real tool." Blunt, because that's the ask.

| # | Area | Current build | Pro bar | Severity | Where |
|---|------|---------------|---------|----------|-------|
| **1** | **Drag & drop** | Does not exist. No reorder, no nest, no reparent, no insertion line. | Three-zone hit-test (before / into / after), insertion line for sibling + row-highlight for nest, cross-layer reparent, drag-the-selection-square-to-move-art, Alt=copy. | **BLOCKER** | not built |
| **2** | **Lock is fake** | `Op::LayerLock` flips a flag; canvas selection/move ignore `eff_locked`. Locked object still drags. | Lock blocks hit-test selection AND move/transform AND marquee pickup; cascades to descendants. | **BLOCKER** | editor.rs hit-test path |
| **3** | **Scroll leaks to canvas** | Wheel over the panel also pans/zooms the board — canvas input isn't gated by pointer-over-panel. | Panel is a hard scroll boundary; canvas zoom handler ignores wheel when pointer ∈ any panel rect; drag takes pointer capture. | **BLOCKER** | canvas input gate |
| **4** | **Position / anchor** | `Align2::RIGHT_BOTTOM`, grows UP (L1801). Floats as its own rounded card. | Docks UNDER Properties on the right rail, shares width, grows DOWNWARD, list absorbs flex; one continuous dock split by a 1px seam. | **HIGH** | L1798–1801 |
| **5** | **Zebra banding** | Per-row bottom hairline `from_gray(30)` on every row (L1917). | No sibling separators; rhythm + indentation carry structure. Only 2 hairlines total (header↓, footer↑). | **HIGH** (the #1 primitive tell) | L1917 |
| **6** | **Colour clash** | `HOVER #2e2e31` used for whole rows (too hot); identity palette is raw saturated hues; indent guides `from_gray(38)` too heavy. | Row hover `#2a2a2c`; identity ramp desaturated (S≤55%, L 62–70%); indent guides ~8% white. | **HIGH** | L1840, L1854, palette |
| **7** | **Always-on chrome** | Eye + lock drawn on every row unconditionally (L1848–1849). | Hover-reveal: eye/lock appear on row-hover; persist only when toggled OFF (hidden/locked). | **MED-HIGH** | L1848–1849 |
| **8** | **Row size** | `row_h = 28` — reads slack. | 26px (pro band 24–28). | **MED** | L1825 |
| **9** | **Folder glyph** | Container = folder-tab outline (L1880–1883) — reads OS file-tree. | Identity-colour chip / stacked-swatch; disclosure does the "container" job. | **MED** | L1880–1883 |
| **10** | **Auto-name hierarchy** | `<Path>` rendered same weight/colour as human names (`from_gray 200`). | Auto-names muted `#8a8a8a` + italic; flip to upright `#e6e6e6`/`#d0d0d0` on rename. | **MED** | L1899–1900 |
| **11** | **Target vs Select** | Both columns drawn, but `Op::LayerFocus` fires for BOTH target-ring click AND row-body click — target ≠ focus semantics collapsed. | Target (appearance recipient) and Select (canvas selection) are orthogonal; container-level effects apply to container. | **MED** | L1908, L1915 |
| **12** | **List height** | Fixed `max_height(380)` (L1826) — floats, doesn't dock. | Computed from dock height: `list_h = panel_h − 102`. | **MED** | L1826 |
| **13** | **Header/footer bands** | Header is bare `add_space(9)` + title; no defined band, no options/⋯ affordance. | 34px title band (+ ⋯ overflow) + 34px search band; 32px footer. | **LOW-MED** | L1806–1810 |
| **14** | **No auto-scroll-to-selection** | Canvas selection doesn't scroll the active row into view. | Locate-object / auto-scroll on selection change. | **LOW** | not built |
| **15** | **No keyboard reorder** | None. | Ctrl+[ / Ctrl+] send back/forward, Ctrl+Shift+[ / ] to back/front (Illustrator parity). | **LOW** | not built |
| **16** | **No panel menu / flyout** | None. Merge, Flatten, Collect, Release-to-Layers, Panel Options all absent. | Full flyout (defer most, but the ⋯ affordance + a few items ship). | **LOW** (defer) | not built |

**Verdict:** the model is genuinely done and tested — keep it entirely. The panel is ~30% of the way there visually and ~0% on interaction. The three BLOCKERS (drag-drop, lock, scroll) are what make it feel "very far"; fix those first, then the look.

---

## 2. EACH OWNER BUG → CONCRETE FIX + HOW THE PROS DO IT

### BUG 1 — Drag & drop does not work

**How the pros do it (converged across PS / Figma / Affinity / Atlassian):** a single drag gesture expresses reorder AND reparent via a **three-zone hit-test** on the row under the pointer, with two distinct visual indicators.

**The hit-zone model (implement exactly):**
For the row under the pointer, split its 26px height:
- **Top third (0–8.6px) → INSERT-BEFORE** — draw a horizontal insertion line in the gap *above* the row.
- **Bottom third (17.3–26px) → INSERT-AFTER** — insertion line in the gap *below*.
- **Middle third (8.6–17.3px) → DROP-INTO (nest)** — only if the row is a legal container (Layer/Group, not a leaf `Path`). Draw a **highlight box/ring around the whole row**, not a line. If the row is a leaf, the middle third snaps to before/after by centroid (north = before, south = after).

**Insertion line visual grammar:**
- 2px stroke, colour `ACCENT #0c8ce9`, drawn in the middle of the gap (never overlapping either row).
- **Left end indented to the target depth** — the line's x-start encodes which nesting level you'll land at (sibling of the row above vs one level shallower). This is the whole "feel."
- Optional 6px terminal dot bleeding ~4px past the left edge (Atlassian touch, cheap).

**Nest highlight visual grammar:**
- 1.5px `ACCENT` outline around the full row rect + `ACCENT @ 8%` background tint. Unmistakably different from the line.

**Drag ghost:**
- Source row stays in place at **40% opacity** (never remove it — removal makes the list jump).
- A drag preview follows the cursor offset +16x/+8y so it doesn't hide the drop line.
- Multi-drag: single stacked preview with an "N" count badge, not N ghosts.
- Cursor → grabbing hand during drag (already have the hand cursor asset per `varos-cursors` memory).

**Three draggable payloads (three columns, three meanings):**
1. **Drag the row body (name/thumbnail)** → move the NODE itself (reorder/reparent).
2. **Drag the selection square (right)** → move the current *canvas selection* to the drop target. **Alt = duplicate.** This is Illustrator's signature move: payload is the whole selection regardless of how scattered, so it gathers objects from many layers into one destination in one drag. On landing, art recolours to the destination layer's identity colour.
3. **Drag the target ring** → move the *appearance* (defer to a later stage; wire the hit-test now, no-op the drop).

**Affinity clip gesture (Ahmed's pick — layer in at Stage 5):** dropping onto a row's **name-zone = clip-into** (child clipped to parent bounds); onto its **thumbnail-zone = mask-by** (parent masked to the dropped object's silhouette). Two distinct indicators: nesting ring vs mask-badge over the thumbnail.

**Auto-scroll + auto-expand (deep-tree survival):**
- Pointer in the top/bottom edge zone of the scroll viewport during drag → auto-scroll, speed ramps with edge proximity.
- Hover a **collapsed** container with INTO for **500ms** → spring-expand it (stays expanded after drop). This single rule is the difference between "toy" and "tool."

**Forbidden drops (no-op, no indicator shown):** container into its own descendant (cycle); nest into a leaf `Path`; drop onto a locked/hidden container. `varos-core` already has the node tree — reparent is a `children` splice + re-flatten; add a `can_drop(src, dst, mode)` guard that walks ancestors to reject cycles.

**Modifier to suppress nesting (avoid Figma's over-eager reparent pain):** hold a key during drag = "reorder only, never reparent" (middle-third stops offering INTO).

---

### BUG 2 — Scroll leaks to the canvas

**Root cause:** the canvas zoom/pan handler consumes wheel deltas regardless of where the pointer is. egui's `Window` doesn't automatically stop canvas code that reads raw input.

**The fix (pointer-over-panel input gating — route by rect, not focus):**
1. Collect every panel rect this frame — the code already pushes them: `build_layers` and `build_dock` push `response.rect` into `rects` (L1943). Union them into a `Vec<egui::Rect>` of "chrome rects."
2. In the canvas input handler, **before** applying wheel/pan/zoom:
   ```
   let over_chrome = chrome_rects.iter().any(|r| r.contains(pointer_pos));
   if over_chrome { /* skip canvas zoom/pan this frame */ }
   ```
   Gate on `ctx.pointer_latest_pos()` ∈ any chrome rect, NOT on focus.
3. The panel's `ScrollArea` already claims the wheel delta when the pointer is over it (egui does this once the area actually scrolls) — the leak is purely that the canvas *also* reads it. Gating the canvas is the fix.
4. **During an active drag**, take pointer capture (`ctx.set_dragged_id` / a `dragging: bool` in panel state) so ALL move/scroll events belong to the drag until drop — nothing leaks mid-reorder.
5. Also gate **clicks** the same way (a click on a panel must not also hit-test canvas art) — likely already partially handled but verify against the same `chrome_rects` union.

This is low-effort, high-rage-if-wrong. Non-negotiable.

---

### BUG 3 — Lock is fake (object still moves after lock)

**Root cause:** `Op::LayerLock` toggles the flag and `eff_locked` cascades in the model (tested), but the **canvas interaction path doesn't consult `eff_locked`**. Lock is presentation-only right now.

**The fix (lock must gate three things, in `varos-core` editor / hit-test):**
1. **Selection hit-test:** when picking art under the cursor (click or marquee), **skip any path whose `eff_locked` is true.** A locked object is not selectable — the click falls through to whatever is behind it. (Illustrator: locked = unselectable, period.)
2. **Move / transform:** even if an object is somehow in the selection set (e.g. locked after selection), the transform/drag handler must **exclude `eff_locked` paths** from translation, scale, rotate, and handle edits. Belt-and-braces with #1.
3. **Marquee pickup:** rubber-band selection must not add `eff_locked` paths.
4. **Cascade is already correct** — `eff_locked` propagates parent→child in the model. Ensure the hit-test reads `eff_locked` (effective), not `locked` (direct), so locking a Group locks every descendant path.
5. **Panel feedback:** locked rows show the padlock persistently; the row is not dimmed (locked ≠ hidden), but the cursor over locked art on canvas should not show move handles.

Same treatment for **hidden**: `eff_hidden` art is already dimmed in the panel; ensure it's also unselectable and unrendered/undraggable on canvas (hidden implies locked-out of interaction).

**Concrete:** find the hit-test in `editor.rs` (the function that maps a canvas point → path index) and add `if path.eff_locked || path.eff_hidden { continue; }`. Add a math test in `varos-core/tests` (allowed per `varos-math-test-suite` memory): lock a group → assert its child path is excluded from `hit(pt)` and from `translate_selection`.

---

### BUG 4 — Colours don't match the app tokens (kill the clash)

Stay strictly inside the locked tokens (L16–26). Three fixes:

1. **Row hover is too hot.** Current whole-row hover uses `HOVER #2e2e31` (L1840) — that's the *chip* hover, too bright for a full row. Introduce a dedicated **row-hover `#2a2a2c`** (a hair above panel `#1f1f22`). Reserve `#2c2c2c`/`#2e2e31` for clickable *chips* (footer buttons) only.

2. **Identity palette is raw/saturated → rainbow clash.** Replace the "rotating raw hues" with a **fixed 12-colour desaturated ramp**: **saturation ceiling ~55%, lightness pinned 62–70%.** Muted teal / rose / amber / periwinkle / sage family. This is the single change that moves it from "toy" to "tool." The identity colour appears in exactly three tiny places — the 3px bar, the selection square, and a 60%-tint on the thumbnail border — **never as a fill.**

3. **Structure whispers.** Indent guides `from_gray(38)` (L1854) → **~8% white** (≈`from_gray(30)` at alpha 20, or `Color32::from_white_alpha(20)`). Column rules `BORDER` → `BORDER @ 55%` (≈`#232326`). And **delete the per-row separator entirely** (L1917) — that zebra banding is the loudest clash of all.

State ladder, all from tokens:
| state | background | left edge |
|---|---|---|
| rest | transparent (`panel #1f1f22`) | none |
| hover | `#2a2a2c` | none |
| selected (art) | `surface #262627` | none |
| active (target) | `ACCENT @ 12%` (current `alpha 30/255` ✓) | 2px `ACCENT` |

Active-row **text stays `#e6e6e6`** — never tint text blue (amateur move).

---

### BUG 5 — Size is weird

Exact numbers (developer builds from these):
- **Panel width 258px** (keep — matches inspector-dock family; Layers is a hair wider for its 2 gutters). Min 240, max useful 320.
- **Row height 26px** (from 28). Thumbnails-off compact mode 22px.
- **Column x-positions (26px row):**
  ```
  eye:        x 0,    w 26
  lock:       x 26,   w 22
  rule L:     x 48    (BORDER @55%)
  colorbar:   x 52,   w 3   (inset 4 top/bottom)
  body/indent:x 62 →  (indent step 13px per depth)
  name:       flexes to (gutter_x − 4)
  rule R:     x w−44 = 214
  target ○:   center x w−30 = 228
  select ▢:   center x w−15 = 243,  11×11
  right margin: 10px to panel edge
  ```
- **Indent step 13px** (from 14 — buys one more visible level before truncation).
- **Thumbnail 18×18**, radius 2, checker 4.5px (all ✓ — keep).

---

### BUG 6 — Docking UNDER Properties, growing DOWNWARD

**Current:** `Align2::RIGHT_BOTTOM, vec2(-16,-16)`, `CornerRadius::same(12)` — floats bottom-right, grows up, all four corners rounded (L1798–1801).

**Target layout (anchor/layout approach):**
1. **Right-edge dock column.** Layers is the **bottom member** of a vertical dock; the Inspector (`build_dock`, currently `Align2::RIGHT_CENTER`) is the top member. They **share X and width** (258; the inspector's inner 214 + chrome ≈ same rail).
2. **Anchor to the inspector's bottom, not the screen.** Compute the layers panel's `min.y` = `inspector_rect.bottom()` (read from the `rects` the dock already pushes). Its `min.x` = `inspector_rect.left()`. Pin `bottom` to workspace bottom − 16.
   - In egui: instead of `.anchor(RIGHT_BOTTOM)`, use a fixed rect via `egui::Area::new("layers").fixed_pos(pos)` where `pos = inspector.left_bottom()`, and set the panel height = `workspace_bottom − inspector.bottom() − 16`.
3. **Grows DOWNWARD:** top edge pinned under inspector, bottom edge pinned to workspace floor. **The list scroll region absorbs all the flex** — header (34+34+1) and footer (1+32) are fixed → `list_height = panel_height − 102`. Kill `max_height(380)` (L1826); compute it.
4. **One continuous dock, not two cards.** Corner-specific radius:
   - Inspector: rounded **top** corners (12px), **square bottom.**
   - Layers: **square top**, rounded **bottom** (12px).
   - Seam between them = a single shared 1px `BORDER #2a2a2d` divider. Reads as one dock split by a line — the way Illustrator/Figma stack Properties over Layers.
5. **Resize:** horizontal drag on the shared left dock edge resizes the whole column width (both stay equal); vertical drag on the Inspector/Layers seam reallocates height between them (defer the vertical-seam drag; ship fixed 50/50 or content-sized inspector + flex layers first).

---

## 3. THE LOCKED VISUAL SPEC (developer builds from this alone)

All values logical px @1.0. Palette = tokens only.

### Panel
| token | value |
|---|---|
| Width | **258px** (min 240, max 320) |
| Fill | `panel #1f1f22` |
| Border | 1px `border #2a2a2d` |
| Corner radius | Layers docked: **top square, bottom 12px**. (Standalone fallback: all 12.) |
| Shadow | one soft low GPU shadow (match inspector; no glass) |
| Height | computed: `workspace_bottom − inspector_bottom − 16` |
| `list_height` | `panel_height − 102` (header 69 + footer 33) |

### Row rhythm
| token | value |
|---|---|
| Row height | **26px** (compact/thumbs-off 22px) |
| Full-bleed | edge to edge, CornerRadius 0 |
| Content | vertically centered, text optical center −0.5px |
| Indent step | **13px** / depth |
| Sibling separators | **NONE** (delete L1917) |
| Horizontal hairlines | exactly 2: header↓list, list↑footer — 1px `border #2a2a2d` |
| Vertical column rules | 2: after lock gutter, before target gutter — 1px `border @55%` (≈`#232326`), list-height only |

### Column geometry (x, width)
```
① eye        x 0    w 26   (18px icon box → 14px glyph)
② lock       x 26   w 22   (13px glyph)
  rule L     x 48          border@55%
③ colorbar   x 52   w 3    inset 4 T/B, radius 1, identity colour
④ disclosure x 62+dep*13   8–9px filled triangle, 16×26 hit
⑤ thumbnail  after disc, 18×18, radius 2
⑥ name       after thumb → gutter_x−4, flexes
  rule R     x 214         border@55%
⑦ target ○   center x 228  r 4.5, stroke 1.3
⑨ select ▢   center x 243  11×11 (+3 hit), radius 2
  right margin 10px
```

### Colours (tokens only)
| element | value |
|---|---|
| Panel bg | `#1f1f22` |
| Row hover | **`#2a2a2c`** (new; not `#2e2e31`) |
| Row selected (art) | `surface #262627` |
| Row active (target) | `ACCENT @ 12%` (`rgba 0c8ce9, 30/255`) + 2px `ACCENT` left edge |
| Chip hover (footer) | `#2c2c2c` |
| Column rules | `border @55%` ≈ `#232326` |
| Indent guides | **~8% white** (`from_white_alpha(20)`) — from `from_gray(38)` |
| Identity ramp | **fixed 12-colour, S≤55%, L 62–70%** (desaturated); on hidden/cascade ×0.42 alpha |
| Thumb border | 1px `border2 #3a3b3d @80%` |
| Checker | 4.5px squares, `#2a2a2a`/`#323232` (low-contrast) |

### Typography (Inter/system)
| element | size | weight | colour |
|---|---|---|---|
| Layer name | 12.5 | **600** | `#e6e6e6` |
| Group/sublayer name | 12.5 | 400 | `#e6e6e6` |
| Object name (user-named) | 12 | 400 | `#d0d0d0` |
| Auto-name `<Path>` | 12 | 400 **italic** | **`muted #8a8a8a`** (flips upright `#e6e6e6` on rename) |
| Header "Layers" | 12.5 | 600 | `#e6e6e6` |
| Footer "N Layers" | 11 | 400 | `faint #7c7c7c` |
| Empty state | 12 | 400 | `#7c7c7c` |

Truncate single-line with `…` at `name_rect.right − 4`; full-name tooltip after 500ms; never wrap; name yields before the gutter.

### Iconography (Lucide, stroke 1.6px @ native)
| icon | box | glyph | rest | hover/active |
|---|---|---|---|---|
| eye / eye-off | 26×26 | 14px | reveal rule | `text` |
| lock / unlock | 22×26 | 13px | reveal rule | `text` |
| disclosure | 16×26 hit | 8–9px filled | `muted` | `text` |
| target ring | 16×26 hit | r4.5, 1.3 | `muted` | `text` / `ACCENT` if active |
| select square | 11×11 (+3 hit) | 1px stroke rest / fill when selected | `muted` outline on hover | identity colour when selected |

**Reveal-on-hover (critical pro cue):**
- Eye: drawn only when (visible AND row-hovered). If hidden → eye-off **always** shown at `#8a8a8a`. Visible + not-hovered → **not drawn.**
- Lock: identical. Locked → padlock always; unlocked → only on hover.
- Target ring: always faintly present at `muted` (it's the Appearance handle); `text` on hover, `ACCENT` when active.
- **Direct vs inherited state must look different:** direct-hidden = solid eye-off `#8a8a8a`; inherited-hidden (`eff_hidden && !hidden`) = row dimmed to 42% + ghosted/hollow eye. Same for lock.

### Thumbnails
- 18×18, radius 2, border 1px `border2 @80%`.
- Object (leaf): actual path geometry, fill+stroke from object, 1px inset; checker only when the object has transparency (opaque fill skips checker — calmer).
- **Container: NOT a folder glyph.** Use identity-colour filled 18×18 rounded chip (or 2–3 overlapped stacked swatches at 60/40/25% alpha); disclosure carries the "it's a container" meaning.

### Header (69px total)
- **Title band 34px:** "Layers" 12.5/600 `text`, left inset 10; right-aligned `⋯` overflow (14px `muted`, 28×28 hit) → panel-options menu.
- **Search band 34px:** field 26px, fill `surface #262627`, border 1px `border`, radius 6, focus border → `border2` (no glow). Search glyph 13px `muted` at inset 14; placeholder "Search" `faint`; text `#e6e6e6`; add a filter-funnel button at field right (14px `muted`).
- **1px `border` divider** below.

### Footer (33px)
- 1px `border` divider above; band 32px.
- Left: "N Layer/Layers" 11px `faint`, inset 12, pluralized.
- Right (RTL): New Layer `+` · New Sublayer · Delete 🗑. Each 28×24 hit, glyph 15px, rest `muted`, hover = 5px-radius `#2c2c2c` chip + glyph→`text`. Right inset 9, 0–2px gap. Trash glyph may tint toward `close_red #e81123 @70%` on hover (optional).
- 6px breathing below buttons before the rounded bottom corner.

### Scrollbar
Thin ~6px, auto-hide, thumb `#3a3b3d`, transparent track. Active row auto-scrolls into view on canvas selection change.

---

## 4. INTERACTION SPEC

### Target vs Select (the crux — keep orthogonal)
Two independent model flags per node: `selected` (in canvas selection) and `targeted` (Appearance/Effect recipient).
- Selecting all objects on a layer ≠ targeting the layer. Select-all + drop-shadow = one shadow per object; **target the layer** + drop-shadow = one shadow for the merged silhouette. Container-level effects apply to the container node.
- Targeting always also selects; selecting never auto-targets. **Fix current L1908/L1915:** target-ring click = `Op::LayerTarget` (target + select); row-body click = `Op::LayerFocus` (select + make active layer). Don't collapse both to `LayerFocus`.

### Click map
| gesture | zone | action |
|---|---|---|
| Click | eye | toggle visibility (`Op::LayerEye`) |
| Ctrl/Cmd-click | eye | toggle Preview↔Outline for that item (defer) |
| Alt-click | eye | solo (hide all others); Alt-click again restores (defer) |
| Click-drag vertical | eye column | scrub visibility across rows (defer) |
| Click | lock | toggle lock (`Op::LayerLock`) — now REAL |
| Click-drag vertical | lock column | scrub lock across rows (defer) |
| Click | disclosure | expand/collapse |
| Alt-click | disclosure | expand/collapse all descendants recursively |
| Click | row body | select art + make active layer (`Op::LayerFocus`) |
| Shift-click | row body | extend selection (range) |
| Ctrl/Cmd-click | row body | toggle individual in selection |
| Double-click | name | inline rename |
| Double-click | layer thumbnail/empty-right | Layer Options dialog (defer) |
| Click | target ring | **target** (+select) — `Op::LayerTarget` |
| Click | select square | select that item's art (`Op::LayerSelectArt`) |
| Shift-click | select square | add/remove from selection |
| Click select square (container) | — | select ALL art in container |
| Right-click | row | context menu: Duplicate / Delete / Rename / Merge / colour-label (defer menu, stub) |

### Drag map (three payloads)
| drag from | payload | drop = line (before/after) | drop = INTO (middle third, container) | Alt |
|---|---|---|---|---|
| row body | the node | reorder at that depth | nest as child | — |
| **select square** | current selection | move art to z-pos | move art into container | **duplicate** |
| target ring | appearance | — | move appearance to item | copy (defer) |

Drop-on-name-zone = clip-into; drop-on-thumbnail-zone = mask-by (Affinity gesture, Stage 5). Forbidden drops (cycle / into-leaf / into-locked) show no indicator and no-op. 500ms auto-expand on hover over collapsed container; edge auto-scroll; hold-modifier = reorder-only (suppress nest).

### Keyboard (Illustrator parity — verify each before binding per `shortcuts-match-illustrator`)
- **Ctrl+[ / Ctrl+]** — send backward / bring forward (reorder within siblings).
- **Ctrl+Shift+[ / Ctrl+Shift+]** — send to back / bring to front.
- **F2 or Enter** on active row — rename.
- **Delete/Backspace** — delete selected rows (+ descendants; warn if layer holds art).
- **Esc** — cancel rename / cancel active drag.
- "Move to…" action for accessible reparent (defer).

### Two-way selection sync
- Canvas → panel: selecting art lights the select square on its row AND every ancestor row instantly; no forced scroll except on Locate-Object. Any descendant selected = square shown (no tri-state).
- Panel → canvas: select-square click selects; Shift extends; container square selects all descendants.
- Targeting is independent of ordinary canvas selection.

---

## 5. STAGED REBUILD PLAN (egui, hand-painted)

**Keep (do not touch):** the entire `varos-core` scene-graph model — node tree, `children` front-first ordering, z = pre-order traversal, re-flattened `Vec<Path>`, `eff_hidden`/`eff_locked` cascade, groups nesting. It's tested and correct.

**Rebuild:** everything in `build_layers` (panel UX + look) and the canvas input gate.

Order chosen so the three BLOCKERS land first, each stage is independently shippable + hand-testable by Ahmed.

---

**Stage 0 — Scroll containment + real lock (the two silent rage bugs). ~½ day.**
- Union all panel rects (already pushed to `rects`) into `chrome_rects`; gate canvas wheel/pan/zoom AND click hit-test on `pointer ∈ chrome_rects`.
- In `editor.rs` hit-test + transform: `continue` past `eff_locked`/`eff_hidden` paths. Add `varos-core` test: locked group → child excluded from `hit()` and `translate_selection`.
- **Ships:** wheel over panel no longer moves the board; locked objects truly immovable.
- **Ahmed tests:** scroll the list (board still), lock a layer, try to drag its object (can't).

**Stage 1 — Dock under Properties, grow downward, kill zebra. ~1 day.**
- Re-anchor: read inspector rect, pin Layers `top = inspector.bottom()`, `bottom = workspace − 16`; compute `list_h = panel_h − 102`; drop `max_height(380)`.
- Corner-specific radius (top square / bottom round); inspector bottom → square; shared 1px seam.
- **Delete L1917** per-row separator. Row hover → `#2a2a2c`.
- **Ships:** panel sits where Ahmed wants it, grows the right way, no more banding.
- **Ahmed tests:** resize window — Layers fills down to the floor under Properties; looks like one dock.

**Stage 2 — Colour + type + icon discipline (the "reads pro" pass). ~1 day.**
- Desaturated 12-colour identity ramp (S≤55%, L 62–70%).
- Indent guides → 8% white; column rules → `border@55%`.
- Auto-names → muted italic; real names upright.
- Row height 28→26; indent step 14→13.
- Eye/lock **hover-reveal** with direct-vs-inherited distinction.
- Container thumb → identity chip (drop folder glyph).
- Header 34+34 bands + `⋯` affordance; keep footer.
- **Ships:** the panel now looks like a tool, inside our tokens.
- **Ahmed tests:** does it match the rest of the app; is the rainbow gone.

**Stage 3 — Drag-drop reorder + nest (THE feature). ~2–3 days.**
- Panel drag state machine: pick-up (row body), three-zone hit-test, insertion line (indented, `ACCENT`) vs nest-highlight box, 40% source ghost + offset preview.
- `can_drop(src,dst,mode)` cycle/leaf/locked guard in `varos-core`; reparent = children splice + re-flatten (undoable `Op`).
- Edge auto-scroll + 500ms auto-expand; hold-modifier suppresses nest.
- **Ships:** full reorder + nest + cross-layer reparent by drag.
- **Ahmed tests:** drag a row between two (line, becomes sibling); drag onto a group (highlight, nests); drag out; deep tree.

**Stage 4 — Drag-the-selection-square-to-move-art + target/select split. ~1–2 days.**
- Select-square drag payload = canvas selection; drop moves art (Alt = duplicate); recolour to destination identity colour.
- Split target-ring click (`LayerTarget`) from row-body click (`LayerFocus`); orthogonal `selected`/`targeted` flags; container-level target routing.
- Auto-scroll active row into view on selection change (Locate).
- **Ships:** Illustrator's signature move + correct target semantics.
- **Ahmed tests:** select scattered objects, drag the little square onto another layer — all move + recolour.

**Stage 5 — Affinity clip gesture + keyboard + panel menu. ~2 days (partly deferrable).**
- Drop-on-name = clip-into; drop-on-thumbnail = mask-by; two indicators.
- Ctrl+[ /] reorder shortcuts (verify against Illustrator first).
- ⋯ flyout: Duplicate / Delete / Merge Selected / Collect in New Layer / Panel Options (Show Layers Only, Row Size). Release-to-Layers and Flatten deferred.
- **Ships:** clipping-by-drag, keyboard reorder, the pro flyout.

**Deferred beyond v1 (named so the gap is explicit, per `keep-plan-html-updated`):** Preview/Outline per-item toggle, solo (Alt-eye), scrub-toggle down a column, mask second-thumbnail + link chip (PS), colour-labels, filter-by-kind toggle row, Release-to-Layers Sequence/Build, Flatten Artwork, Template layers, virtualization (needed only at 1000s of rows — collapsed subtrees already bound cost).

---

**Relevant file:** `D:\VAROS\varos\crates\varos-app\src\ui.rs` — `build_layers` L1786–1944, tokens L16–26; canvas input gate + `editor.rs` hit-test/transform for the lock fix; `varos-core` node tree for reparent + `can_drop`. Hand-off to Ahmed in plain Egyptian Arabic per standing rule; land plan.html + DETAILED_ROADMAP.md updates in the same commits.