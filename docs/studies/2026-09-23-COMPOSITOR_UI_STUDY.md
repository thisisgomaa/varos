> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Study: what Varos can borrow from Compositor's UI (and which pixel features fit)

- **Date:** 2026-09-23
- **Author:** Claude (study session). **Decision owner:** Ahmed (product owner). He judges every UI idea here with his own eye. This document only lays them out with evidence.
- **Authority:** none. Level-5 document under `docs/foundation/FOUNDATION_CHARTER.md` §3 (lines 29-39). It proposes and decides nothing. If anything here conflicts with `docs/UI_DIRECTION.md` (the visual law), the law wins. Every conflict is named in §3.
- **Sources read:**
  - **Compositor** (MIT, `LICENSE:1-3`), a shallow clone at commit `c802b19` (2026-09-22). Read: `README.md`, `docs/*.md`, all of `Compositor/UI/*.swift`, `ContentView.swift`, `Rendering/TransformOverlay.swift`, `Rendering/InlineTextEditor.swift`, and a skim of `Rendering/EditorCanvas.swift`. It was not built, because that needs Xcode 26 (`README.md:64-65`).
  - **Varos**, working tree at `61786a0` (clean).
- **Citation format:** Compositor paths are relative to its `Compositor/` source folder unless they start with `README`/`docs`. Varos code paths are relative to `varos/crates/`. Varos doc paths are relative to the repo root.

---

## 1. الملخص (بالمصري)

1. **الحقيقة الأول:** نص جمال Compositor جاي من ماك نفسه، زي أيقونات SF Symbols وأزرار وتولبار النظام. الحاجات دي مينفعش ناخدها أصلًا، لأن رخصة أيقونات أبل للأجهزة بتاعتها بس. اللي ينفع ناخده هو **السلوك**، يعني كل حاجة بتتصرف إزاي.
2. **أحسن ٥ حاجات نستلفها** (مرتبة بقيمتها لأوجاعك قصاد مجهودها):
3. ① **أدب خانات الأرقام:** ‏Esc يرجّع القيمة القديمة، Enter يثبّتها ويرجّع الكيبورد للكانفس. الرقم الصحيح يتكتب من غير "‎.00"، والأرقام تتكتب بخط ثابت العرض عشان متترقصش (ده بيحل A20 وA14).
4. ② **البار العايم يثبت من الشمال:** ارتفاعه ثابت، ويبدأ دايمًا باسم الأداة. كده مش هينط لما تحدد أو تلغي التحديد (بيحل P8 وA14).
5. ③ **شريط الحالة تحت يشرح الأداة اللي في إيدك:** يقولك Shift بيعمل إيه وAlt بيعمل إيه، بدل الجملة الثابتة اللي موجودة دلوقتي.
6. ④ **مقابض التحجيم:** حرف البوكس كله يتمسك، مش النقطة الصغيرة بس.
7. ⑤ **عيون الليرز:** تدوس على عين وتسحب لتحت، فكل العيون اللي عديت عليها تاخد نفس الحالة، وكلها Undo واحد.
8. **٣ حاجات بتخالف دستورنا:** (أ) الأزرار عندهم كبسولة مدوّرة وإحنا زوايانا 3px. (ب) حركات صغيرة: التابات بتبهت، والسلايدر بيتزحلق، وmarching ants. (ج) زينة: جريدينت على أطراف التابات، وزجاج ورا مؤشر التحميل، وضل تحت الماوس.
9. **الجملة الصريحة عن البيكسل:** Varos النهارده مبيعرضش ولا صورة واحدة. الحاجة البيكسل الوحيدة اللي v1 محتاجها هي **تصدير PNG**. تركيب الصور وقصّها وImage Trace مكانهم بعد v1. أما الفرش والـHealing والـContent-Aware فمش شغلانة Varos خالص، دي شغلانة Compositor نفسه وهو ببلاش.

---

## 2. Compositor UI anatomy — how its shell is built

**Read this first:** Compositor is a **SwiftUI + AppKit** app for macOS 26.5+ (`README.md:64`). Several of its best-feeling details are hacks inside Apple's own controls. One example is swapping a method inside `NSSliderCell` (`UI/SliderSnap.swift:13-23`). **None of its code can come across to Varos.** We draw every pixel ourselves in egui on wgpu (`docs/adr/ADR-0001-native-gpu-ui-stack.md`), so we can copy **behaviour** and never code. Its MIT licence is GPL-compatible, so porting an *algorithm* with attribution would be legal. For UI code that question never comes up, because there is nothing to port.

**What is *not* borrowable at all:** its icons are SF Symbols, loaded with `Image(systemName:)` at `ContentView.swift:277` and `UI/LayersPanel.swift:35-56`. Apple licenses those for Apple platforms only. Some of the "gorgeous" is also the macOS 26 toolbar material (`ContentView.swift:153-188`, `.sharedBackgroundVisibility(.hidden)` at `:169`). Our icon pain (A16, `docs/PAINS_LOG.md:227`) cannot be solved by borrowing their glyphs.

### 2.1 The shell, region by region

| Region | Where (evidence) | What it does, in one line |
|---|---|---|
| **Window toolbar** | `ContentView.swift:153-188` | Native macOS toolbar: "+" new canvas · project tabs · Fit / 100% · zoom ± on the right. |
| **Project tabs** | `UI/ProjectTabs.swift:21-60`, `:143-163` | Scrolling tab strip. A layer dragged onto another tab moves it to that project. An empty "new tab" slot appears only while dragging. |
| **Tool header** (their control bar) | `ContentView.swift:25-76`, `UI/ToolHeaderStyle.swift:3-8` | A full-width strip on top that swaps its contents per tool. Its height is fixed at 42 and every header starts with the tool's name in semibold. |
| **Tool rail** | `ContentView.swift:265-300` | 56 pt column of 36×36 buttons. It scrolls when the window is short, and the fg/bg colour swatches sit at its bottom (`UI/ColorPaletteControls.swift:10-57`). |
| **Canvas + rulers** | `ContentView.swift:84-102`, `UI/CanvasRulers.swift:174-179` | Rulers are optional. Ticks are about 70 pt apart on a 1-2-5 number ladder. |
| **Layers panel** (docked right) | `UI/LayersPanel.swift:9-70`, `ContentView.swift:104-105`, `:331-351` | Header with a count, then Blend + Opacity, then the list, then an icon footer. The left edge drags the width (202–352), and the width is remembered (`ContentView.swift:6`). |
| **Layer list** | `UI/NativeLayerList.swift` (1101 lines) | A real AppKit table: select on mouse-down, inline rename, eye swipe, modifier-aware cursors, thumbnails framed on the canvas. Details in §4.3. |
| **Status bar** | `ContentView.swift:306-328` | Left: zoom %, canvas size, colour space. Right: a **hint sentence for the current tool and mode**. Digits are drawn monospaced. |
| **Floating panels** | `UI/FloatingPanel.swift:12-14`, `:35-73` | Non-modal windows for *tuning* (Levels, Hue/Sat, Effects, Filters, Color Picker, Shortcuts). They never dim the editor, open centred on the canvas the first time, and reopen where you left them. Close means Cancel. |
| **Sheets** (modal) | `UI/PSDConversionSheet.swift:58-68`, `UI/JPEGExportSheet.swift` | Only for *decisions with consequences*: import a PSD, export a JPEG. |
| **Empty state** | `ContentView.swift:99`, `:301-305`; `UI/NewCanvasSheet.swift:18-44` | With no document, the canvas area *is* the New-canvas form: Open project · Import image · Create. |
| **Keyboard remap** | `UI/KeyboardShortcuts.swift:63-281` | One table of every shortcut, user overrides saved to disk, a conflict checker, and a searchable editor with a key recorder. |

### 2.2 The three "containers" and how they choose between them

Compositor has one consistent rule for *where a control lives*:

1. **Setting up a tool** goes **inline in the tool header**. See `UI/ShapeControls.swift:6-63`, `UI/TransformInspector.swift:9-50`, `UI/NavigationToolHeader.swift:10-37`.
2. **Tuning something with a live preview** goes in a **floating panel** with a *Preview* toggle and Cancel/OK. See `UI/FilterSheet.swift:3`, `:100-124`, `UI/LevelsSheet.swift:69`, `UI/HueSaturationSheet.swift:45`.
3. **Deciding something that changes the whole document** goes in a **modal sheet** that states the consequences first. See `UI/PSDConversionSheet.swift:27-28` ("Nothing is applied until you continue").

This maps cleanly onto our law. (1) is our floating **control bar** (`docs/UI_DIRECTION.md:73-75`). (2) is how our **Color Picker** already behaves: floating, no scrim, remembers its place (`varos-app/src/ui.rs:2325-2327`). (3) is where our future import/export dialogs would sit. The one mismatch is that Compositor floats panels as separate OS windows. Our law says homes are docked and only the two "hands" float (`docs/UI_DIRECTION.md:21-24`). Floating is fine for a *transient editing session* like the picker, and never fine for a *home*.

### 2.3 Their visual language (for contrast, not for copying)

- **Chrome:** neutral grey `Color(white: 0.14)` (`ContentView.swift:117`). Ours is the warm black `#141313` (`varos-app/src/shell/tokens.rs:11`). Keep ours.
- **Active tool:** a white 12 % fill plus a 14 % white border, corner 7 (`ContentView.swift:280-285`). No accent colour. Ours is azure (`docs/UI_DIRECTION.md:27`). Keep ours.
- **Controls:** capsules everywhere (`ContentView.swift:353-357`, `UI/BlendModePicker.swift:19-20`). This conflicts with our 3 px rule (§3, conflict C1).
- **Separation:** `Divider()` hairlines between every region (`ContentView.swift:29-107`), plus a one-device-pixel white-6 % line under every layer row (`UI/NativeLayerList.swift:1058-1062`). **Same philosophy as our rule 2.** This is part of why it reads calm.
- **Transform box:** a single 1 px accent line and nothing behind it. The comment says a dark under-line "read as a grey halo" (`Rendering/TransformOverlay.swift:257`). Same instinct as our no-shadow law.

---

## 3. Borrow list — ranked by (value to Ahmed's logged pains ÷ effort)

**Effort:** S = one region of one file, under a day, hand-testable alone · M = a new widget or several regions, a few days · L = multi-stage.
**Scheduling honesty:** the Foundation program is active. The current work order is P11.2 and F4.2 comes next (`docs/foundation/STATUS.md:13`). `flag.design-work-started` is `false` (`STATUS.md:25`). Ahmed's directive is "foundation before features, safety work, or UX repair" (`docs/foundation/FOUNDATION_CHARTER.md:6`, `:16`), and F5 will split `ui.rs` into modules (`FOUNDATION_CHARTER.md:71`). So **nothing below jumps that queue.** §6 says how to slot the first pieces.

### Summary table

| Rank | Borrow | Serves (logged pain / law gap) | Effort | Law conflict? |
|---|---|---|---|---|
| **1** | Number-field manners: Esc reverts, Enter commits and returns keys to the canvas, no `.00`, tabular digits | A20, A14 (Q2 / A14.4), P4 | S | none; *fulfils* rule 5 |
| **2** | Control bar anchored left, fixed height, tool name always first | **P8** (open decision), A14 | S | adapt: stays floating |
| **3** | Status-bar hints that change with the tool and its state | UI_DIRECTION status-bar spec; makes A8b/A9/A11/A12/A20 discoverable; P9 | S | none; typography only |
| **4** | Whole edges of the transform frame are grabbable, not just the dots | A12 (edge midpoints), general feel | S | none (canvas) |
| **5** | Eye swipe in Layers (and lock swipe), one undo step | LAYERS_VISION §2 gap | S | none |
| 6 | Slider behaviour kit (for Varos's first slider) | A19 stroke options, opacity | M | drop the 0.18 s glide |
| 7 | One shortcut table, later a remap sheet | "one truth" law; constitution #4 | M → L | none |
| 8 | Alt-cursor zones in Layers (clip strip vs duplicate) | LAYERS_VISION 7.6 (deferred) | M | none |
| 9 | Export sheet with live result + file size + remembered settings | PNG export (v1 finish line) | M | drop the glass blur |
| 10 | "What will change" report before import applies | SVG import (product sequence) | M | none |
| 11 | Hover-preview inside dropdown menus | future blend modes; A19 presets | M | none |
| 12 | Empty states that teach | A8a boardless start | S | none |
| 13 | Digit keys set opacity (1…0, two digits = exact %) | taste (Photoshop habit, not Illustrator) | S | none |

### 3.1 — Number-field manners  ·  S

- **What it is.** Four small rules, all in Compositor:
  - **Escape reverts.** In the layer rename, "Return keeps it, Escape leaves it as it was, as does clicking away" (`UI/NativeLayerList.swift:717`).
  - **Enter/Escape hand focus back to the canvas**, "so a tool's key works straight away instead of typing into the field" (`ContentView.swift:359-365`, used by every tool header, e.g. `UI/TransformInspector.swift:49`).
  - **Whole numbers print without decimals**, and fractions print two decimals (`UI/TransformInspector.swift:108-111`).
  - **Digits are monospaced** in the status bar and value readouts (`ContentView.swift:325`, `UI/JPEGExportSheet.swift:42`, `UI/LayersPanel.swift:14`).
- **Varos today.**
  - `num_field` commits whatever was typed on *any* focus loss, and there is no revert branch (`varos-app/src/ui.rs:1633-1648`).
  - Layer rename also commits on any focus loss (`ui.rs:4300`).
  - Values are drawn in proportional 13 px (`ui.rs:1578`, `ui.rs:1657`). That is the open A14.4 item: "numeric values → tabular mono … needs Ahmed's eye" (`docs/VISUAL_POLISH_PLAN.md:225-229`).
- **Serves.** A20 (number feel, `docs/PAINS_LOG.md:231`), A14 Q2 (`VISUAL_POLISH_PLAN.md:204`), and the rename flow from P4 (`PAINS_LOG.md:37`).
- **In our shell.** No visual change except the digits: the same field, now in `FontId::monospace` (rule 5, "tabular mono numerals", `docs/UI_DIRECTION.md:28`). Esc in a field puts back the value from before the edit. Enter commits and the next `V`/`P` press reaches the canvas.
- **Conflict.** None. The digits change *fulfils* rule 5.
- **Unknown to test by hand.** Whether egui already returns keyboard focus to our canvas after Enter. The code does not say either way; Ahmed's hand test decides.

### 3.2 — Control bar anchored left, fixed, named  ·  S (plus Ahmed's taste call)

- **What it is.** Compositor gives every tool a header of **identical height** ("Shared metrics keep tool switching from changing typography or canvas layout", `UI/ToolHeaderStyle.swift:3-8`). Each one **starts with the tool's name**: `UI/TransformInspector.swift:11`, `UI/ShapeControls.swift:8`, `UI/NavigationToolHeader.swift:12`. Even "no tool" keeps a header, "so the canvas doesn't jump" (`ContentView.swift:67-73`). Contents grow to the right from a fixed left edge.
- **Varos today.**
  - The bar is pinned to 36 px already (A14.2 done, `ui.rs:3568-3571`).
  - It is **pivoted on the board's centre** (`ui.rs:3555-3556`: `pivot(CENTER_TOP)` at `board.center().x`), so every width change moves *both* edges. That is exactly the P8 jump, and P8 is still an open taste decision: "(a) stay centred … or (b) fixed left edge like Illustrator" (`PAINS_LOG.md:149`).
  - The tool name appears only in the idle branch (`ui.rs:3676-3680`).
- **Serves.** **P8** (`PAINS_LOG.md:149`, `:197`) and A14 "bar design tired" (`PAINS_LOG.md:225`).
- **In our shell.** The bar stays a **floating hand with margins**, 8 px box corners, a 1 px hairline, no shadow (`docs/UI_DIRECTION.md:73-75`; `docs/reference/BOX_SYSTEM_PLAN.md` Stage 4). Only the anchor moves: left edge = board left + tool-rail width + one seam gap. A fixed first slot shows the tool name as a TEXT micro-title ("Selection", "Pen", "Artboard"), and the context controls follow it.
- **Conflict.** Compositor's header is docked edge-to-edge. We **keep it floating** (rule 1). No other conflict.
- **Needs Ahmed's eye.** This *is* P8 option (b). Compositor is evidence that (b) reads calm in a modern editor, but the choice is his.

### 3.3 — Living status-bar hints  ·  S

- **What it is.** Compositor's status bar shows a sentence for the **current tool and its current mode**. For the Marquee in rectangle mode it reads "Drag a rectangle · Shift add · Option subtract · Shift again mid-drag square …", and every tool and sub-mode gets its own line (`ContentView.swift:322`). It also names busy states ("Working…", "Importing images…", `:315-320`).
- **Varos today.** Three fixed hints: "V select · A direct · Alt+drag duplicates" (`ui.rs:3440-3451`), whatever tool is in hand. The law asks for "shortcut hints" in the status bar (`docs/UI_DIRECTION.md:82`).
- **Serves.** Many of Ahmed's pains were requests for *modifiers* that now exist but stay invisible unless you know them:
  - Shift = 45° on the pen: A8b (`PAINS_LOG.md:219`, done `:168`)
  - Space repositions while placing: A9 (`:220`, done `:54`)
  - Alt from centre: A11 (`:222`, done `:167`)
  - midpoint modifiers: A12 (`:223`)
  - scrub Shift ×5 / Ctrl fine: A20 (`:166`)

  The hint line is where a user *finds* them. It also extends P9 ("Drawing path…", `PAINS_LOG.md:144`) from the bar to the whole tool.
- **In our shell.** The same line and the same styling as now: keys in MUTED, prose in FAINT (`ui.rs:3439-3451`). Only the words change per `ToolKind` and state (idle / drawing / dragging).
- **Conflict.** None. Typography is the only decoration (rule 5).
- **Honesty rule.** Every hint must be true in code (`varos-app/src/main.rs:154-232` for keys). A hint that lies is worse than none. Once item 7 exists, a test can check that every hinted key is in the shortcut table.

### 3.4 — Whole-edge grab on the transform frame  ·  S

- **What it is.** "Entire edges are draggable, not just the small midpoint squares" (`Rendering/TransformOverlay.swift:294-303`). The text box uses "a band along each edge … rather than only the handle squares", about 10 screen points wide (`Rendering/InlineTextEditor.swift:268-275`).
- **Varos today.** Scale grabs only within **7 screen px of the 8 handle points** (`varos-core/src/editor.rs:687-698`). The rotate zone is a 22 px ring outside the corners (`editor.rs:699-708`).
- **Serves.** A12, which asks the edge midpoints to behave like corners (`PAINS_LOG.md:223`). Making the whole edge the target means you stop hunting for a 7 px dot.
- **In our shell.** Nothing new is drawn. It is a hit-test change plus the right resize cursor along each edge (`varos-app/src/cursors.rs` already has `ResizeH`/`ResizeV`, lines 24-25).
- **Conflict.** None visual. **Risk:** the band must not steal clicks that A31's occlusion rules give to other objects (`PAINS_LOG.md:157`). Keep the band on and just *outside* the frame line, and keep the corner-rotate ring working. This is a feel change, so Ahmed must hand-test it.

### 3.5 — Eye swipe (and lock swipe) in Layers  ·  S

- **What it is.** "Pressing it shows or hides the layer; keeping the button down and dragging up or down the list gives every eye passed over the same state, as in Photoshop." The whole swipe is **one undo step**: `beginVisibilitySwipe` … `endVisibilitySwipe` (`UI/NativeLayerList.swift:1067-1090`).
- **Varos today.** Eye and lock are plain toggles. The Layers interaction table has no swipe (`docs/LAYERS_VISION.md:48-60`), and `panel_layers` has no swipe state (`ui.rs:3992-4010`).
- **Serves.** No pain is logged for this yet. It fills a gap in the Layers table for anyone hiding ten rows. Cheap and self-contained.
- **In our shell.** No new visuals. The swipe reuses the existing eye/lock glyphs (`docs/VISUAL_POLISH_PLAN.md:48`).
- **Conflict.** None.

### 3.6 — Slider behaviour kit  ·  M

- **What it is.** The exact rules are in §4.1. The key ones: a click on the track jumps **instantly**, a double-click on the knob resets to the default, and a field next to the slider accepts values beyond the slider's range.
- **Varos today.** **There is no slider in the app crates.** A search for `Slider` in `varos-app` and `varos-core` finds nothing. Opacity and weight are `num_field`s.
- **Serves.** A19 "stroke needs big development … more options" (`PAINS_LOG.md:230`), a future opacity slider, and gradient stops later.
- **In our shell.** A 2 px track in LINE, a 10 px knob, hairline only, no shadow. An exact `num_field` sits beside it with its unit tight against the number (`UI/ToolHeaderStyle.swift:11-18`: 2 pt gap).
- **Conflict.** `UI/CameraRawSlider.swift:140-151` *animates* the knob for 0.18 s on a track click. **Drop that.** Their own `UI/SliderSnap.swift:4-9` exists to *remove* the system glide, which is our law exactly (`varos-app/src/shell/tokens.rs:60`, `animation_time = 0`). Coloured gradient tracks (`UI/CameraRawSlider.swift:4-48`) are fine **only when the colour is the meaning**, as in hue or saturation. That is content, not decoration.

### 3.7 — One shortcut table, later a remap sheet  ·  M (table) → L (editor)

- **What it is.**
  - Every shortcut is one row in `ShortcutDefinition.all`, with a title, a group, and the original chord (`UI/KeyboardShortcuts.swift:63-127`).
  - User overrides are saved (`:129-164`).
  - A checker refuses duplicates and reserved chords (`:165-180`).
  - Translation happens *only* at the canvas boundary, so text fields keep normal typing (`:182-201`).
  - The editor has search, a click-to-record button, "Restore Defaults", and a plain-prose list of the fixed mouse gestures (`:228-281`).
- **Varos today.** **Two truths.** The keys are hard-coded in a `match` (`varos-app/src/main.rs:154-232`), and the menu shows literal strings like `"Ctrl+N"` (`ui.rs:3295-3299`). They can drift apart silently.
- **Serves.** The "one source of truth" spirit (`CLAUDE.md`, ONE-HOME `docs/UI_DIRECTION.md:34-43`), constitution #4 "easy to add tools" (`docs/VAROS_CONSTITUTION.md:10`), and hint honesty for item 3.
- **In our shell.** Stage 1 is invisible: a table in code that the menu labels and `apply_key` both read. Stage 2 is a remap *section* in a box (not a floating window), drawn in our tokens.
- **Conflict.** None. **Timing:** it touches `main.rs` and `ui.rs` together, so it belongs **after F5/F6** (`FOUNDATION_CHARTER.md:71-72`).

### 3.8 — Alt-cursor zones in Layers (clip vs duplicate)  ·  M

- **What it is.**
  - With Option held over a row, the cursor already *tells you* the outcome before you press: the clipping cursor over a fixed **10 px strip at the bottom of the row**, the duplicate cursor everywhere else (`UI/NativeLayerList.swift:305-347`).
  - Click in the strip = clip. Drag anywhere = duplicate.
  - The cursor updates even when only the modifier key changes, with no mouse move (`:298-299`).
- **Varos today.** Our doc *rejected* Alt-click-between-rows clipping because it "collides with Alt-drag-duplicate". It is deferred to 7.6 "only if it earns a razor-thin (≤3px) zero-motion hit-zone with its own cursor" (`docs/LAYERS_VISION.md:120-122`, `:297-298`).
- **Serves.** It is **working evidence that 7.6 can be unambiguous**, because motion and cursor separate the two gestures.
- **In our shell.** Two new cursors in the same drawn style as `cursors.rs`. No panel visuals.
- **Conflict.** None. **Ahmed's call:** Compositor's strip is 10 px, while our doc asked for ≤3 px.

### 3.9 — Export sheet with a live result  ·  M

- **What it is.**
  - A real encoded preview, the pixel size, the **actual file size** (`UI/JPEGExportSheet.swift:28-58`).
  - Re-encoding is debounced by 200 ms (`:70-75`).
  - The last quality is remembered for next time (`:7-18`, `:62`).
  - Export stays disabled until the preview matches the settings (`:66`).
- **Varos today.** PNG export does not exist yet. It is on the v1 finish line (`docs/VAROS_CONSTITUTION.md:26`) and second in the product sequence (`FOUNDATION_CHARTER.md:116`).
- **Serves.** The v1 PNG export, when its turn comes.
- **In our shell.** A box-styled modal: scale ×1/×2/×3, px size, background (transparent / page colour), file size, remembered settings.
- **Conflict.** A `.regularMaterial` glass blur sits behind their spinner (`UI/JPEGExportSheet.swift:35`). Use a solid PANEL and the static "Updating preview…" text they already have (`:58`), with no spinner animation.

### 3.10 — "What will change" report before import  ·  M

- **What it is.**
  - The sheet opens **instantly in a "Reading…" state** "so the click feels answered" (`UI/PSDConversionSheet.swift:8-10`, `:27-35`).
  - It then lists each layer by name with what will happen to it (`:37-42`).
  - "Nothing is applied until you continue" (`:28`), and confirm stays disabled while reading (`:48-49`).
  - The README promises the same thing: "A conversion report is shown before anything is applied" (`README.md:57`).
- **Varos today.** SVG import is last in the product sequence (`FOUNDATION_CHARTER.md:116`). Our old roadmap already wanted an "Import fidelity report — what converted, what was rasterized, what was dropped" (`docs/history/DETAILED_ROADMAP.md:957`).
- **Serves.** SVG import trust. Illustrator files will always have things we cannot represent yet: gradients, text, effects.
- **Conflict.** None.

### 3.11 — Hover-preview inside dropdown menus  ·  M

- **What it is.** Moving the highlight through the blend-mode menu previews each mode on the canvas. Cancelling restores the original, and choosing commits. The preview is kept alive through the menu's close so the canvas never flashes back (`UI/BlendModePicker.swift:39-61`).
- **Varos today.** Blend modes are not built (`docs/history/DETAILED_ROADMAP.md:253`). The same pattern would fit stroke cap/join/dash presets later (A19).
- **Conflict.** None.

### 3.12 — Empty states that teach  ·  S

- **What it is.** The Layers panel with nothing in it shows "No layers yet · Import an image or add a blank layer" (`UI/LayersPanel.swift:20-31`). The status bar says "Ready when you are" (`ContentView.swift:313`). With no document, the canvas area *is* the New form (`ContentView.swift:99`).
- **Varos.** Since A8a the app opens with no artboard (`PAINS_LOG.md:57`). An empty Layers box could say "Draw with P (pen) or M (rectangle) · Shift+O makes an artboard", in FAINT micro-type.
- **Conflict.** None. Low value, very cheap.

### 3.13 — Digit keys set opacity  ·  S (taste)

- **What it is.** `1`…`9` set 10–90 %, `0` sets 100 %, and two digits typed within 0.6 s set the exact % (`Document/EditorSession+Brush.swift:195-209`).
- **Varos.** Plain digits are unused in `apply_key`; only Ctrl+1 is bound (`main.rs:166`). This is a Photoshop habit, not an Illustrator one. **Ahmed decides whether it belongs.**

### 3.14 — Conflicts with the visual law, all in one place

| # | Compositor does | Where | Our law | How we adapt |
|---|---|---|---|---|
| C1 | Capsule buttons and pop-ups everywhere | `ContentView.swift:353-357`, `UI/BlendModePicker.swift:19-20` | Rule 3: 3 px controls / 8 px boxes (`UI_DIRECTION.md:26`, `tokens.rs:41-42`) | Take the behaviour, draw it at `R` = 3. The only pill we own is the tab chip `RCAP` (`tokens.rs:43`). |
| C2 | Motion: tab-edge fade eased over 0.15 s, slider track-click glide 0.18 s, animated marching ants | `UI/ProjectTabs.swift:45-55`, `UI/CameraRawSlider.swift:143-151`, `Rendering/TransformOverlay.swift:196`, `Rendering/EditorCanvas.swift:1978` | "No animations" (`CLAUDE.md`; `tokens.rs:60`) | Instant only. Tabs clip at a hard edge with a scroll chevron (we already have them, `tokens.rs:43`). Selection outlines stay static. |
| C3 | Decoration: gradient fade masks on tabs, material (glass) blur behind a spinner, soft shadow under custom cursors | `UI/ProjectTabs.swift:45-55`, `UI/JPEGExportSheet.swift:35`, `Rendering/EditorCanvas.swift:137-138` | Rule 2 "not one shadow", rule 5 typography-only decoration (`UI_DIRECTION.md:25`, `:28`) | Solid tones and hairlines. Our cursors keep their current drawn outline. |
| C4 | Floating OS windows for editing panels | `UI/FloatingPanel.swift:87-96` | Rule 1: homes docked, only the hands float (`UI_DIRECTION.md:21-24`) | OK for a *transient session* (our Color Picker already floats, `ui.rs:2325`). Never for a home. |
| C5 | Accent colour on the transform box **and** on snap lines | `Rendering/TransformOverlay.swift:259`, `:177` | Azure only for selection/active/focus (`UI_DIRECTION.md:27`). Snap lines have their own reserved colour (`tokens.rs:28`, `GUIDE`) | Frame = azure (it *is* the selection). Guides stay `GUIDE`. |

---

## 4. Interaction details worth stealing exactly

### 4.1 Slider rules (for when Varos gets its first slider)

1. **A click on the track jumps the knob there instantly.** No glide. The value is set under the pointer *before* tracking starts, so a drag continues smoothly from that point (`UI/SliderSnap.swift:4-9`, `:27-41`).
2. **Pressing the knob itself does not jump it.** The drag starts from where the knob is (`UI/SliderSnap.swift:8-9`, `:31-32`; the knob hit area is inflated 2 px, `UI/CameraRawSlider.swift:166-172`).
3. **Double-click the knob to reset to the default** (`UI/CameraRawSlider.swift:50`, `:125-128`).
4. **A slider always comes with an exact field**, with the unit tight against it (`UI/FilterSheet.swift:140-155`; `UI/ToolHeaderStyle.swift:11-18`).
5. **The slider covers the common range. The field accepts more.** A typed value beyond the slider stays intact and "only the thumb is pinned to the end" (`UI/EffectsSheet.swift:179-183`). Example: a line width slider covering 1–100 while the field allows up to 5000 (`UI/ShapeControls.swift:20-24`).
6. **Size-like values use a logarithmic slider**, so the small values people use most get most of the travel (`UI/FilterSheet.swift:140-148`).
7. **One drag = one undo step** (`onEditingChanged` → `beginOpacityEdit` / `finishOpacityEdit`, `UI/LayerAppearanceControls.swift:17-19`).
8. **Right-to-left flips the direction** (`UI/SliderSnap.swift:37`, `UI/CameraRawSlider.swift:161`). This matters for our Arabic future.
9. **While you drag, outside updates do not fight the knob** (`isTrackingValue`, `UI/CameraRawSlider.swift:93`, `:120-121`).

### 4.2 Number fields: typing and scrubbing

**Honest finding:** Compositor has **no drag-to-scrub** on number fields. The only `DragGesture`s in `UI/` are on colour fields, curves and levels. Its numbers move by slider or by arrow keys. **Varos is already ahead here**: whole-box scrub with a resize cursor, Shift ×5, Ctrl ÷5 (`ui.rs:1650-1676`, A20 `PAINS_LOG.md:166`), plus arrow steps of 1 / 10 / 0.1 with a precise accumulator (`ui.rs:1586-1632`, FB4 `PAINS_LOG.md:115`). Keep all of that. What to add from Compositor:

1. **Up/Down = ±1, Shift = ±10** while focused (`ContentView.swift:376-386`). This matches ours (`ui.rs:1586-1609`), so nothing changes.
2. **Escape = put back the old value. Enter = commit.** Both hand focus back to the canvas (`ContentView.swift:359-365`, `UI/NativeLayerList.swift:717`, `:752`). *Varos commits on any blur* (`ui.rs:1633-1648`), so this is a change.
3. **A valid number previews live while typing** (`UI/TransformInspector.swift:94-96`). Today we apply on arrow keys or blur only. This one is optional: live typing into W could feel jumpy on big shapes, so Ahmed decides.
4. **A focused field is never overwritten by outside updates**, and a step writes the number it applied (`UI/TransformInspector.swift:92-103`). *Varos already keeps its own buffer while focused* (`ui.rs:1572`).
5. **Format: whole numbers without decimals, else two decimals** (`UI/TransformInspector.swift:108-111`).
6. **Clamp in the field's own binding**, so arrows can never push past range (`UI/NavigationToolHeader.swift:40-43`). *Ours clamps too* (`ui.rs:1620`).

### 4.3 Layers: drag, nest, Alt-duplicate, and the small stuff

What Varos **already has**: Alt+drag duplicates, a middle "Into" zone nests (never auto-clips), multi-selected rows drag together in one undo step, and double-click renames (`docs/LAYERS_VISION.md:50-60`, `:370-373`). Compositor's extras, each small:

| Rule | Compositor evidence | Varos status |
|---|---|---|
| Select on **mouse-down**, with no double-click delay | `UI/NativeLayerList.swift:4`, `:415`, `:942` | check by hand |
| Alt-drag offers **only Copy**, including whole folder trees | `:176-177` | we duplicate; folder trees to verify |
| A dragged folder brings its children, which are never counted twice | `:245-246` | model re-guards (`ui.rs:4055` comment) |
| After a drop, **the moved rows stay selected** so you can drag them on | `:237` | check |
| Multiple rows keep their order on drop (above vs into a folder) | `:215-236` | check |
| **Delete on a row in a multi-selection deletes the whole selection** | `:800` | check |
| With the Move tool, **arrow keys in the list move the object** instead of the row selection | `:449-453` | not present |
| **Eye swipe**, one undo step | `:1067-1090` | not present (item 5) |
| Alt shows the **clip vs duplicate cursor** by zone | `:305-347` | not present (item 8) |
| Row-height changes are **not animated** | `:80-81` | egui is instant, so this holds already |
| Selecting never rebuilds thumbnails | `:75`, `:90` | Varos keys thumbnails on geometry only (`ui.rs:361`, test `ui.rs:5716`) |
| Thumbnails framed on the **whole canvas** | `UI/CanvasThumbnail.swift:3-4` | a taste question; for vectors, framing on the object's own bounds is arguably clearer |
| Double-click the **thumbnail** opens the content; double-click the **name** renames | `:140-141` | we have name-rename only |
| A **contextual tooltip** says exactly what the trash will delete | `UI/LayersPanel.swift:57-58` | cheap to copy |
| Footer icons have **hit areas bigger than the glyph** | `UI/LayersPanel.swift:74-82` | relevant to the clipped-footer history (`PAINS_LOG.md:35`) |

### 4.4 Transform inspector fields

Compositor's Move-tool header (`UI/TransformInspector.swift:9-50`) contains: **X · Y · W · H · 🔗 lock ratio · Scale % · ° · Sampling · Flip H · Flip V · Cancel · Apply**.

- W/H respect the ratio lock with simple proportional maths (`:63-74`).
- **Scale %** scales about the centre, measured against the layer's 100 % size, so typing twice doesn't compound (`:26-31`, `:52-53`).
- **Rotation wraps at 360** (`:32`).
- **While the object is distorted, the number fields disable** ("the handles are the controls", `:41-42`).
- The field row **scrolls sideways** instead of wrapping when the window is narrow (`:18`, `:44`).

What fits our Transform home (`docs/UI_DIRECTION.md:41`: X/Y/W/H, rotate, flip, pivot, "more…"):

- **Take:** the **Scale %** field, the rotation wrap, the sideways scroll for a narrow bar, and the ratio-lock maths.
- **Don't take:** the **Apply/Cancel transform session**. That is Photoshop's model. Varos chose Illustrator's live, immediately-committed transform (A7, `PAINS_LOG.md:63-65`).
- **Don't take:** "Sampling" (smooth vs nearest). It only exists for pixel layers. It becomes relevant if placed images arrive (§5).

### 4.5 Tool header behaviour

1. **Fixed height (42 pt)** for every tool, so switching tools never moves the canvas (`UI/ToolHeaderStyle.swift:3-8`).
2. **Title first**, in 13 semibold, with 12 pt controls (`:6-7`).
3. **"No tool" still shows a header** ("Select a tool") (`ContentView.swift:67-73`).
4. **Units hug their numbers** (2 pt) while controls sit wider apart (12 pt) (`UI/ToolHeaderStyle.swift:11-18`, `UI/ShapeControls.swift:7`).
5. **Controls that make no sense right now are disabled, not hidden**, so the layout never shifts (`UI/TransformInspector.swift:42`, `UI/ShapeControls.swift:62`).
6. **Return/Escape in any header field gives the keyboard back to the canvas** (`ContentView.swift:359-365`).
7. **Hover help explains the gesture**, not just the name. Example: "Shift-U (or Tab) steps through Rectangle, Ellipse and Line" (`UI/ShapeControls.swift:16`).

Rules 1, 3, 4 and 5 fit our floating bar as-is. Rule 2 needs our micro-type sizes (`docs/VISUAL_POLISH_PLAN.md:205`).

### 4.6 The PSD "conversion report before applying" pattern

1. The click opens the sheet **immediately**, in a *Reading…* state (`UI/PSDConversionSheet.swift:8-10`, `:30-35`).
2. The report is **one row per affected layer**: layer name in bold, then plain words saying what happens to it (`:37-42`).
3. A sentence on top promises "Nothing is applied until you continue" (`:28`).
4. **Cancel is safe** (`:47`), and **Confirm is disabled until the report is complete** (`:48-49`).
5. The same promise appears in the product README (`README.md:57`).

For our SVG import, the rows would read like: "‹Layer 3› — gradient fill → flattened to its first colour", "‹Title› — live text → converted to outlines".

### 4.7 Remappable shortcuts

1. **One table** holds every command as title, group and default chord (`UI/KeyboardShortcuts.swift:63-127`).
2. **Overrides only** are saved, so defaults can improve later without breaking users (`:132`, `:140`).
3. **Conflicts are refused with a plain sentence** naming both commands (`:176`). Reserved OS chords are refused too (`:173-175`). Text-editing chords must carry a modifier (`:170-172`).
4. Translation happens **only at the canvas boundary**, so fields keep normal typing (`:182-201`).
5. The editor shows search, click-to-record, **Restore Defaults**, and Save disabled while there's a conflict (`:228-281`).
6. **Fixed mouse gestures are documented in prose** inside the editor (`:259-261`). That doubles as the gesture manual.
7. Menus read their labels **from the table** (`configuredKeyboardShortcut`, `:221-225`; used at `CompositorApp.swift:33-65`). This is what fixes our "two truths" problem (§3.7).

---

## 5. Raster / pixel features — what fits a vector editor

### 5.1 The honest starting point

- **Varos has zero image support today.** No image node exists in the model: a search of `varos-core/src` for image/raster/png finds nothing. The only PNG code is a debug dump in the app (`varos-app/src/main.rs:307`).
- **The v1 finish line has no images in it:** draw · colour · arrange · combine · simple text · export SVG/PNG (`docs/VAROS_CONSTITUTION.md:26`). "Build to this line, then ship — don't keep adding." "Don't drown" (`:33`).
- **The product order after Foundation** is Masks 3-6 → PNG export → SVG export → SVG import (`FOUNDATION_CHARTER.md:116`).
- **There is a technical gate before any image can enter a document:** undo keeps up to 200 full document copies. "Before embedded images … enter `Document`, heavy immutable bytes must be shared (`Arc` or content-addressed storage)" (`docs/audits/2026-07-11-CODEX-FULL-PROJECT-AUDIT.md:318-322`).
- **Placed images were always "Stage 3" / post-v1** in the old plans: images and raster fills at `docs/history/DETAILED_ROADMAP.md:252`, raster import needing a new node type at `:958-959`, image trace post-v1 at `docs/history/ROADMAP.md:62`.
- **Compositor proves how big "real raster" is.** Its brush alone needed a Metal compute kernel, 256×256 tiles, and shared snapshot tiles for undo to get mouse-up from ~1 s to ~9 ms on a 4K canvas (`docs/brush-performance.md:1-4`, table `:13-18`). That is a second engine, not a feature.

### 5.2 Fits / doesn't fit

| Feature | Fits Varos? | When | Effort | Why (evidence) |
|---|---|---|---|---|
| **PNG export** (rasterise the artwork) | **Yes, required** | v1 | M | On the finish line (`VAROS_CONSTITUTION.md:26`). The model is Compositor's export sheet (§3.9). |
| **Place / embed an image** as an object you can move, scale, rotate and flip, at full resolution | Yes | first post-v1 raster item, after the undo-sharing gate | L (new node type + asset storage + GPU texture + PDF embed) | Illustrator's baseline. Compositor keeps sources at full resolution under a separate transform (`README.md:22`, `docs/project-format.md:5-7`). Gated by the audit (`:322`). |
| **Drag an image file onto the canvas to place it** at the drop point | Yes, with Place | post-v1 | S on top of Place | `ContentView.swift:125-141` (drop at the document point). Also `docs/history/DETAILED_ROADMAP.md:953` (7.1.b). |
| **Image inside a shape** (clip an image with a vector) | Yes | with Place | S on top of Place + clip groups | Reuses our one clip form (`docs/LAYERS_VISION.md:93-107`). This is the most common real use. |
| **Opacity on a placed image** | Yes | with Place | S | Per-object opacity already exists (`docs/history/DETAILED_ROADMAP.md:253`). |
| **Crop image / sampling (smooth vs crisp)** | Yes, small | after Place | S–M | Illustrator has Crop Image. For sampling see Compositor's per-layer choice (`UI/TransformInspector.swift:33-37`). |
| **A few non-destructive adjustments on a placed image** (greyscale, a tint/duotone, brightness) | Maybe, small | later | M | Useful for designers doing duotone looks. Must stay *properties of the image object*, never pixel painting. Compositor's adjustment layers show the pattern but are far bigger (`README.md:15`). |
| **Blend modes** on objects and images | Yes (art feature) | post-v1 | M | Already on the old list (`docs/history/DETAILED_ROADMAP.md:253`). UI pattern from §3.11. |
| **Image Trace** (bitmap → vector paths) | Yes, and strategic | post-v1, after Place | L | This is where raster *serves* the vector mission: scanned Arabic calligraphy and lettering → editable paths, which ties to the Arabic moat (`CLAUDE.md`). Listed post-v1 (`docs/history/ROADMAP.md:62`). Candidate engines exist (potrace is GPL; the Rust `vtracer` is reportedly MIT). **Licences to be verified** before any choice. |
| **Artwork effects like drop shadow or blur on vectors** | Maybe | post-v1 | M–L | Note: the no-shadow law is for the **UI**, not for the user's artwork (`UI_DIRECTION.md:32`: "the only beautiful thing on screen is the user's artwork"). Needs the offscreen target we already have (`docs/history/DETAILED_ROADMAP.md:254`). |
| Raster layer masks painted with a brush | **No** (not now) | — | L | Needs a brush/raster engine. Already fenced as FUTURE (`docs/LAYERS_VISION.md:85-87`). |
| **Brushes / eraser / painting** | **No** | never in core | L+ | A second engine (`docs/brush-performance.md`). It collides with "don't drown" (`VAROS_CONSTITUTION.md:33`). |
| **Healing, clone stamp, content-aware fill, liquify, smudge** | **No** | never | L+ | Photo retouching. Compositor does this and is free (`README.md:34-41`). Point users there. |
| **Pixel selections** (marquee, lasso, magic wand, select subject) | **No** | never | L | Already judged vector-only: "Snap to pixel selection bounds — SKIP … no referent in Varos" (`docs/reference/SNAP_TRANSFORM_REFERENCE.md:72`). |
| **Levels / Curves / Camera Raw / Remove Background** | **No** | never | L | Photo-grading tools. Their own panels are Compositor's largest UI files (`UI/CameraRawColorControls.swift`, 485 lines). |

**One-sentence version:** the only pixel work Varos must do for v1 is **turn vectors into a PNG**. After v1 the right order is **place → clip → crop → trace**, all *objects*, never *painting*.

---

## 6. Recommended next 3 UI pieces (each small enough to hand-test alone)

**When:** not before P11.2 and F4.2 finish (`STATUS.md:13`). Two options for Ahmed:

- **(a) Do them just *after* F5** splits `ui.rs` (`FOUNDATION_CHARTER.md:71`). Each piece then lands in its own new module and never fights the split. **This is the recommended option.**
- **(b) Squeeze them in *between* work orders.** Each piece touches one region only, but any F5 branch open at the time would need a rebase.

Each piece is one branch, one hand-test, then the next. All three leave `tokens.rs` untouched.

### U1 — Number-field manners  (from §3.1)

- **Change.** Inside `num_field` (`ui.rs:1525-1699`) and the layer rename (`ui.rs:4288-4308`):
  - Esc puts back the old value.
  - Enter commits.
  - Whole numbers print without decimals.
  - Values draw in monospace digits (A14.4).
- **Ahmed's hand test (2 minutes):**
  1. Select a rectangle, type `500` in W, press **Esc** → W shows the old number and the shape did not change.
  2. Type `500`, press **Enter**, then press **P** → the Pen tool activates, meaning the keyboard came back to the canvas.
  3. Scrub W slowly → the digits do not jiggle sideways.
  4. Rename a layer, press **Esc** → the old name stays.
- **Done when:** Ahmed says the numbers "look engineered" and Esc feels right. `cargo test --workspace` and clippy are green.

### U2 — Anchored control bar  (from §3.2; this is Ahmed's P8 decision (b))

- **Change.** In `board_ctlbar` (`ui.rs:3541-3683`):
  - The pivot moves from board-centre to a fixed left edge (board left + rail width + one seam gap).
  - The tool name becomes the fixed first slot in *every* state, not only idle.
  - Height stays 36.
- **Ahmed's hand test (1 minute):**
  1. Click a shape, click empty canvas, click an artboard with the Artboard tool, and switch to the Pen → **the bar's left edge never moves**, only its right end grows or shrinks.
  2. Resize the window narrow → the bar never overlaps the tool rail.
- **Done when:** P8 is closed in `PAINS_LOG.md` by Ahmed. If he prefers centred after seeing it, the branch is dropped. That is a cheap, honest outcome.

### U3 — Living status-bar hints  (from §3.3)

- **Change.** `build_statusbar` (`ui.rs:3422-3488`) reads the hint from a small table keyed by tool and state (idle / drawing / dragging), in the existing MUTED-key / FAINT-prose style. Every hint is checked against `apply_key` (`main.rs:154-232`) and the tool code before it is written.
- **Ahmed's hand test (3 minutes):**
  - Pick each tool on the rail and read the line.
  - Try each modifier it mentions, for example Pen + Shift = 45° and Rectangle + Alt = from centre.
  - **Any hint that doesn't do what it says is a failed test.**
- **Done when:** every hint is true by Ahmed's hand, and the line never overlaps the right-side zoom/Fit/Artboard group at the minimum window width.

**Next in line after these three:** 3.4 whole-edge grab (one core hit-test plus a feel test), 3.5 eye swipe, then the shortcut table (3.7) once F6 lands.

---

## Appendix A — Compositor files read

`README.md` · `docs/project-format.md` · `docs/brush-performance.md` · `ContentView.swift` · `UI/FloatingPanel.swift` · `UI/ToolHeaderStyle.swift` · `UI/NavigationToolHeader.swift` · `UI/SliderSnap.swift` · `UI/CameraRawSlider.swift` · `UI/TransformInspector.swift` · `UI/LayersPanel.swift` · `UI/NativeLayerList.swift` (doc comments + drag, clip-strip, keyboard and eye-swipe sections) · `UI/BlendModePicker.swift` · `UI/LayerAppearanceControls.swift` · `UI/LayerMaskMenu.swift` · `UI/ColorPickerSheet.swift` · `UI/ColorPaletteControls.swift` · `UI/KeyboardShortcuts.swift` · `UI/PSDConversionSheet.swift` · `UI/JPEGExportSheet.swift` · `UI/ProjectTabs.swift` (strip) · `UI/CanvasRulers.swift` (step logic) · `UI/CanvasThumbnail.swift` (comments) · `UI/ShapeControls.swift` · `UI/NewCanvasSheet.swift` · `UI/FilterSheet.swift` / `UI/EffectsSheet.swift` / `UI/LevelsSheet.swift` / `UI/HueSaturationSheet.swift` (slider + preview sections) · `Rendering/TransformOverlay.swift` (handles, snap lines) · `Rendering/InlineTextEditor.swift` (comments) · `Rendering/EditorCanvas.swift` (doc-comment skim) · `Document/EditorSession+Brush.swift:195-209`.

## Appendix B — Varos files read

`CLAUDE.md` · `docs/UI_DIRECTION.md` · `docs/VAROS_CONSTITUTION.md` · `docs/PAINS_LOG.md` · `docs/VISUAL_POLISH_PLAN.md` · `docs/LAYERS_VISION.md` (skim) · `docs/reference/BOX_SYSTEM_PLAN.md` (skim) · `docs/foundation/STATUS.md` · `docs/foundation/FOUNDATION_CHARTER.md` (§1, §3, §5 table, §8) · `docs/audits/2026-07-11-CODEX-FULL-PROJECT-AUDIT.md:318-322` · `docs/history/DETAILED_ROADMAP.md` (image/import lines) · `docs/history/ROADMAP.md:55-66` · `varos-app/src/shell/tokens.rs` · `varos-app/src/ui.rs` (num_field, rename, control bar, status bar, colour modal header, menu rows) · `varos-app/src/main.rs:154-232` · `varos-core/src/editor.rs:683-708` · `varos-app/src/cursors.rs:10-40`.
