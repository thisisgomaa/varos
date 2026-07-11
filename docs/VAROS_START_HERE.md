> **Status:** historical — Preserved project history; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Execution Brief (START HERE)

You are the **BUILDER** for "Varos". A separate advisor session designed the architecture and wrote this handoff. **Read this whole file before writing any code.** It is detailed on purpose — you have none of the prior conversation, so nothing here is assumed.

References (both in THIS folder, `D:\VAROS\`):
- **The detailed Illustrator behavior spec: `VECTOR_BUILD_SPEC.md`** — read **PART A** (exact Illustrator model) and **PART C** (simple Arabic goal summary). ⚠️ **IGNORE PART B** of that file — it describes the *abandoned Penpot fork* and no longer applies.
- Figma reference board (architecture, history, plan): https://www.figma.com/board/4Q7vrowHrzb0s1VPAtBI6V
- Optional deeper background (advisor's memory): `C:\Users\Gomaa\.claude\projects\D--My-work-Desgin-tool\memory\varos-leave-penpot-own-core.md`

---

## 1. What Varos is (context)
An open-source, **desktop**, **Arabic-first** alternative to **Adobe Illustrator** (NOT Figma). We compete with Illustrator's vector power — pen, paths, anchor editing — done so the muscle memory matches.

## 2. THE one principle you must internalize (why everything else failed)
**The tool is the mode. There is NO "edit mode" you enter.**
- Illustrator is **modeless**: the active *tool* decides what a click does. White arrow (Direct Selection) = you grab anchors of **any** path **from outside**, instantly, with **no double-click**, no "entering" the shape.
- Penpot (and Figma) are **mode-based**: you double-click to "enter" one shape's edit mode. This is the wrong model and it is *why two previous rebuilds failed*. Do **not** build an edit mode. Ever.

If you remember one sentence: **grab any anchor of any path from outside, at any time, because the white-arrow tool is active — no mode, no double-click.**

---

## 3. YOUR FIRST AND ONLY TASK: the Pen-Feel Spike
Build ONE **throwaway test page**. It is **NOT the product and NOT the core** — it is a cheap probe to answer ONE question: *does the modeless pen + direct-select FEEL like Illustrator in Ahmed's hand?*

**Constraints:**
- ONE static `.html` file + plain TypeScript/JavaScript (~300–600 lines). **NO build system, NO framework, NO WASM.** Ahmed double-clicks the file to open it in his browser.
- Draw on a plain **SVG** canvas.
- Bezier handle math: hand-rolled or a tiny single-file `bezier-js`. Nothing else.
- Study `https://bezier.method.ac/` to feel the target pen interaction — but build something **simpler** (free canvas, no lessons/scoring).

**OUT OF SCOPE — do NOT add:** Arabic/text, layers panel, export, GPU, save, boolean ops, multiple artboards, colors/UI chrome. Adding any of these now is how past attempts broke.

---

## 4. PRECISE behavior spec (this is the "feel" — get it exact)
*(Condensed from `VECTOR_BUILD_SPEC.md` PART A — read that for the full version.)*

### Anchor visuals (the canonical language)
- **Corner** anchor = small **square**. **Smooth** anchor = small **circle**.
- **Selected** anchor = **FILLED**. **Unselected but visible** = **HOLLOW**. (This filled-vs-hollow contrast is *the* "is this point selected?" signal — get it right.)
- A selected smooth anchor shows its two **direction handles** (a thin line ending in a round dot).

### PEN tool (key `P`)
- **Click** on empty canvas → place a **corner** anchor; straight segment from the previous anchor.
- **Click-and-drag** on empty canvas → place a **smooth** anchor; the drag pulls out **two symmetric, collinear handles**; the segment curves. `Shift` constrains the handle to 45°.
- Hover the path's **first anchor** while drawing → show a **close** indicator (○); click it to **close** the path.
- `Enter` / `Esc` / switching tool → finish the path (leave it open).
- *(Stretch, only after the core feels right)*: hover a segment → `+` add anchor (preserve shape via De Casteljau split); hover a mid anchor → `−` delete + reflow; `Alt` over an anchor → convert corner↔smooth.

### DIRECT SELECTION / white arrow (key `A`) — the hard, important part
- **Click an anchor of ANY path** (no double-click, no edit mode) → that anchor becomes **filled**, all others **hollow**.
- **Click a path's body/segment** → show **all** that path's anchors as **hollow** (path is now reshapeable).
- **Drag a selected anchor** → it follows the cursor; its handles ride along; the two neighbor segments reshape. Multiple selected anchors (Shift-click to add) move **together** by the same delta.
- **★ Drag a direction handle → ONLY that handle moves. The opposite handle does NOT move. No `Alt` needed.** This is the single most important behavior and the exact thing Penpot kept getting wrong. The white arrow breaks the tangent **freely**.
- `Alt`-click an anchor → convert corner↔smooth.
- Click empty canvas → deselect. Arrow keys → nudge selected anchor(s).
- *(Stretch — the headline "moat")*: **marquee** from empty canvas selects anchors across **multiple paths at once** (cross-object), then dragging moves them all together.

### Tool switching
`P` / `A` keys switch **instantly**. Selection **persists** across switches. You draw with `P`, press `A`, and immediately grab an anchor — **no re-click, no double-click, no "enter."**

---

## 5. Build order WITHIN the spike (small steps; Ahmed verifies EACH in his browser)
Ship each step to Ahmed; let HIM say "شغّال" before the next.
1. **Canvas + Pen corners:** click places corner anchors + straight segments; the path renders. → *Ahmed: can I draw a connect-the-dots shape?*
2. **Pen smooth curves:** click-drag = smooth anchor with symmetric handles; curve renders. → *Ahmed: can I draw curves?*
3. **Close path:** clicking the first anchor closes (and fills) the path. → *Ahmed: closes cleanly?*
4. **White arrow grabs anchors from outside:** click any anchor of any path (no double-click), drag it; filled/hollow visuals. → *Ahmed: grabbing from outside feels right, not "edit mode"?*
5. **★ Single-handle drag, no coupling:** drag one handle; the opposite stays put. → *Ahmed: THE feel test.*
6. **Alt-convert:** Alt-click toggles corner↔smooth. → *Ahmed.*
7. *(Stretch)* cross-object marquee select + multi-anchor move.

After step 5 it should already *feel* like Illustrator for basic editing — that is the core of the gate.

---

## 6. Acceptance gate (Ahmed decides, in HIS browser, by hand)
The pass/fail question, in his words:
> "Does grabbing anchors from outside, and dragging a single handle, feel like Illustrator and NOT like an edit mode?"
- **YES** → foundation validated; the advisor plans the real Rust core next.
- **NO** → we learned in a day, not a month. Report exactly what felt wrong.

---

## 7. Hard working rules (these caused past failures — do NOT repeat)
1. **VERIFY = Ahmed using it in his REAL browser.** NEVER claim "done" from a headless/automated pass — automated tests LIE about interactive feel (this exact mistake cost weeks).
2. Hand Ahmed **ONE tiny piece at a time**; let HIM say "شغّال" before the next.
3. Talk to Ahmed in **SIMPLE, SHORT Egyptian Arabic** — he is a **DESIGNER, not a coder**. No English/engineering jargon; explain with design analogies (read PART C of the spec for the right tone).
4. If Ahmed says "شيله / ابنيه من جديد", do it the **FIRST time** — don't keep patching.
5. **ONE owner** on this code. Build it isolated.

---

## 8. AFTER the spike passes (reference only — NOT now)
Build the real owned **core in Rust**: flat path data model; the modeless interaction (above); a **single schema** (one definition → the `.varos` file + the AI/plugin API + the inspector UI); a self-drawn **GPU canvas** (wgpu/Vello, **never the DOM**); **web panels in Tauri behind a hard seam** (so we can swap to fully-native later with no rewrite). All heavy math is **rented OSS**: Clipper2 (booleans), HarfBuzz + bidi-js (Arabic shaping → editable outlines). Details in the Figma board + advisor memory.
