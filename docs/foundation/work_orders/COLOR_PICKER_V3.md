> **Status:** approved by the owner 2026-10-09 («فل ابدأ») — design of record is the Figma file; this page is the behaviour contract and the build order.

# Colour picker v3 — "Wheel" panel, modeless, live

**Design of record:** https://www.figma.com/design/qYRabe9nDx0TyXvuZMwFcA — frames `Picker / Wheel` (2:2), `Picker / Sliders` (3:2), `Picker / Gradient` (3:104), `Picker / Mini` (3:206). The owner edited them by hand on 2026-10-09; read geometry, spacing and colours from Figma, not from the older HTML drafts. Dark mode only (the light Figma page background is not part of the design).

Owner's words that shape this piece: «البوكس ضخم أوي… عاوز البوكس يكون أصغر ما يكون بس احترافي» · «المثلث ده المطلوب فعلاً» · «الفيل والاستروك أيقون مش كتابة… زي Affinity» · «الدايرة دايماً في النص» · «بوكس وهمي» (no frame around the cluster) · «الكلر ويل تشتغل لايف وأقدر أستخدم البرنامج وهي مفتوحة».

## 1. Shell and lifetime (the big change)

- The picker is a **modeless floating panel** inside the box system (a hand/area like the control bar, not an egui modal): the canvas, tools, panels and shortcuts keep working while it is open. No scrim. No OK/Cancel.
- **Follows the selection.** It shows the Fill/Stroke of the current selection (or the tool defaults when nothing is selected). Selection changes update the panel; the panel never changes the selection.
- **Commits per gesture.** Every drag on the ring/triangle/slider is one transaction: `begin()` on press, live `paint_live` while dragging, `commit()` on release (= one undo step). Typing in a field commits on blur/Enter (K3). Clicking a swatch commits one step. Nothing is written when nothing changed (reuse the 2026-10-08 live-preview fixes: change-only writes, mixed selection writes nothing until the first change).
- **Esc** closes the panel (does not undo). **Enter** in a field commits that field. Closing never reverts.
- Bridge/agent edits that land while open simply re-seed the panel (no foreign-begin conflict; the K3 rule-5 risk in `docs/audits/2026-10-08-PICKER-RISK-REGISTER.md` is closed by this design).
- Opens from: double-click on any Fill/Stroke swatch (rail, Properties, control bar) → opens focused on that target; Window menu; shortcut per Illustrator (double-click only — no new keys beyond §5).
- Position and open/closed state persist with the layout (`layout.json`, law L7). Default position: near the rail swatch, inside the Board hole.

## 2. Geometry (from Figma `Picker / Wheel`)

- Width **240 pt**; height from content (~341 pt with the swatch row). Corners 8, fill PANEL, 1 pt LINE border, no shadow.
- **Header 32 pt:** tab strip of icon buttons (26×24, glyph 16): Wheel ◎ (default) · Sliders · Harmony · Gradient — ON = TOGGLE_WELL + TEXT ink (law L5); spacer; Eyedropper; ×. Tooltips: "Wheel", "Sliders", "Harmony", "Gradient — coming with the gradient engine", "Eyedropper (I)", "Close (Esc)".
- **Wheel area:** hue ring ⌀ ~200 pt, thickness ~14 pt, centred horizontally; angular gradient red→yellow→green→cyan→blue→magenta→red (0° = red at 3 o'clock, counter-clockwise like Figma's draft). Inside: the **SB triangle** (HSV triangle, Krita/GIMP style): hue vertex on the ring at the current hue, white vertex at hue+120°, black vertex at hue+240°; the triangle **rotates with the hue**. Markers: 12 pt ring marker on the ring, 10 pt marker inside the triangle (white 2 pt stroke, 1 pt black halo, fill = current colour).
- Top-right of the wheel area: **Default** button (two 20×20 squares black | white) — sets Fill black / Stroke white? No: Varos default = fill black, stroke none? Use the existing tool defaults (`cur_fill`/`cur_stroke` defaults in code); tooltip "Default colours (D)".
- Bottom-left: **live readout** three lines `H:293` `S:100` `L:29` in JetBrains Mono 11, MUTED label + TEXT value; follows the mode chosen in Sliders (HSB default → `H S B`; RGB → `R G B`; CMYK → four lines).
- Bottom-right, **no frame** («بوكس وهمي»): the Fill/Stroke cluster exactly as Figma: stroke ring (⌀ 36, 8 pt ring) behind top-left; fill circle (⌀ 44) in front bottom-right; curved swap arrow (16) top-right; ⌀ "none" (14) bottom-left. The focused target gets the 1.5 pt ACCENT stroke; click Fill → focus Fill, click Stroke → focus Stroke; arrow = swap (⇧X); ⌀ = set focused target to none (/).
- **Hex row** (last row of the wheel area): `# F2C94C` field (JetBrains Mono, K3), alpha slider (track 8 pt, 14 pt knob, gradient transparent→colour over a checker), `100 %` field.
- **Swatch row 28 pt:** 9 recent swatches 16×16 r3 + ▾ at the right that expands the drawer (Recent · Board · Document tabs; drawer open/closed state persisted). Mixed selection: the striped MUTED pattern on the target swatch (never the checkerboard).

## 3. Tabs

- **Sliders** (Figma 3:2): mode dropdown (HSB · HSL · RGB · CMYK · Lab · Web) + hex field on the left; the same cluster top-right; one row per channel: letter, gradient track (the track shows the colour range for that channel at the current values), knob, value (JetBrains Mono, right-aligned, editable on click, K3); alpha row last. CMYK/Lab: naive sRGB conversions, documented as not colour-managed.
- **Harmony:** the wheel with linked harmony markers (complementary, analogous, split, triad, tetradic, square, mono — the 8 original glyphs landed 2026-10-08) and the resulting swatches below; clicking a swatch applies it to the focused target.
- **Gradient** (3:104): the tab exists but is **disabled** with the tooltip above until the engine has gradient paints (separate piece: core model, GPU, PDF/SVG, format v5). Do not implement its body now.
- **Mini** (3:206, 192 pt): wheel + hex/alpha/eyedropper row + swatch row, no tabs, no cluster. A popover for picking ONE colour (future gradient stops, page colour chip). Implement as the same widget with a `Mini` config; wire it to the page-colour chip now so it is exercised.

## 4. Shortcuts (Illustrator parity — no others)

X toggle Fill/Stroke focus · ⇧X swap · D default colours · / none · I eyedropper. Esc closes the panel. These are canvas shortcuts that already exist or map 1:1; do not bind new keys.

## 5. Tokens, glyphs, laws

- Only tokens from `tokens.rs`; add what the Figma needs (e.g. `PICKER_W = 240`, ring/triangle sizes) there. No shadows. Azure only for the focused target stroke and focus rings.
- Tab/swap/none glyphs: the SVGs in session scratchpad `picker-v3/final1.html` are original drawings (moderator, 2026-10-08) — add them under `assets/icons/varos/` with the provenance header; Lucide first where an official glyph exists (`pipette` exists).
- Box system untouched; the picker is one more hand/area. Ratchets never raised; `picker.rs` will be split (wheel, sliders, cluster, drawer) under the registry pattern.

## 6. Heat and performance

- Repaint only during an active drag/hover (paced); idle = no repaint (test with the pacing plan). The ring and triangle are painted from cached meshes: the ring once per size, the triangle re-tessellated only when the hue changes (≤ 1 ms). The eyedropper keeps the 2026-10-08 raster-once-per-view behaviour.

## 7. Tests and gates

Headless: triangle ↔ HSV mapping round-trip (hue rotation, vertices), ring hit-test and angle math, per-gesture undo (drag = 1 step, no step without change), follows-selection and mixed state, K3 fields, X/⇧X/D// routing, drawer state persisted, Mini config renders without tabs/cluster, idle = no repaint, ratchets. Gates: workspace tests, clippy macOS + Windows target, fmt, dep directions. Independent review before merge; owner hand test after install.

## 8. Build order (slices)

1. Modeless shell + follows-selection + per-gesture commits + hex/alpha row + cluster + swatch row (reusing today's picker logic). 2. Wheel tab (ring + triangle + readout + default). 3. Sliders tab with modes. 4. Harmony tab + drawer + Mini on the page-colour chip. Gradient tab stays disabled.
