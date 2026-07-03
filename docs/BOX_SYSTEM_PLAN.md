# Varos — The Box-System Staged Build Brief (for the Production session)

> Onboarding + build order for the fresh session that builds the UI shell. Written 2026-07-03.
> **Base standard = Blender's editor-areas (the box tree); borrowed skin = Brave/Claude (void + chip tabs).**
> The look is already **LOCKED**. This brief is the *how it gets built, staged, without ever breaking the app*.
> We build it as a **staged series with one Ahmed gate per stage** — never A-to-Z in one shot.

---

## 0. READ FIRST (the fresh session starts here)

You have zero prior context. Before writing one line of code, read these three, in order:

1. **`docs/UI_DIRECTION.md`** — the visual **law** (✅ approved by Ahmed, "برفكتو", 2026-07-03). Non-negotiable.
2. **`docs/UI_VISION_MOCKUP.html`** — open it in a browser. It is the **target picture** — every token and
   measurement in §3 below is lifted verbatim from its source. When in doubt, this file wins.
3. **This brief** — the architecture (§4), the tool decision (§5), the stages (§6), the build order (§7),
   the rules of engagement (§8).

### The hard rules, restated (memorize these five)
1. **7 rules of the constitution** (from UI_DIRECTION): homes docked / hands float · no shadows (1px hairlines
   only) · near-sharp corners (0–4px) · azure `#0c8ce9` is a scalpel (selection/active/focus ONLY) ·
   typography is the only decoration · Illustrator density on a 4/8px beat · the **warm black `#141313`** is
   the signature (R ≥ G ≥ B, all grays share the warmth).
2. **ONE-HOME rule** — every domain has exactly one home (a "Section"). Bars/menus are **mirrors** of homes,
   never owners of features. Sections are the unit; panels are containers of sections.
3. **CONTAINER model** — "كل حاجة صندوق". The screen is a tree of uniform boxes (Blender editor-areas).
   **The board is just another box.** The only fixed chrome is the **app bar**; the app bar and status strip
   are **the void itself** (seam colour), not panels.
4. **Tokens only** — no inline colour/spacing/radius literals anywhere. Everything from `tokens.rs` (§3).
   `egui` animation time stays **0 forever** — this is a work tool, instant, no fades on chrome.
5. **Ahmed verifies every stage by hand, in the real window** (or the sandbox bin). You never drive synthetic
   UI tests, never auto-proceed past a gate. Each gate is a human hand-off you write in plain Egyptian Arabic
   (what changed / what to test / what might look different).

---

## ✅ DECISIONS LOCKED — 2026-07-03 (Ahmed + planning team)

These eight rulings are settled. Build to them; do not re-litigate.

1. **Tokens come from the mockup, not from `ui.rs`.** The `:root` block of `UI_VISION_MOCKUP.html` is the
   source of truth for every colour/radius/spacing value. `ui.rs`'s current literals are *legacy* and may
   differ — the mockup is the law. (Transcribed for you in §3, so you never reverse-engineer the CSS.)

2. **We do NOT hand-roll the box tree — we adopt `egui_tiles`** (reviewed against `egui_dock` and a manual
   build; full reasoning in §5). Corollary, stated honestly: **egui has no free split-panel *tree*** —
   `SidePanel`/`TopBottomPanel` do not compose into a recursive serde tree. `egui_tiles` is what gives us that
   tree + drag-resize + drag-tabs + serde; we supply a custom `Behavior` for the look and the chip tabs.

3. **Motion is functional only.** Drag follows the cursor 1:1; a box collapses/expands. That is the whole
   vocabulary. **No fades, no eases, no springs, no effects.** (`ctx.style_mut().animation_time = 0.0`.)
   When Ahmed said "animations" in his step 3 he meant *smooth drag-resize + collapse/expand*, NOT decoration.

4. **Colour resolution — two separate commits in the real app.**
   - The **sandbox is born on the law's values** (mockup) from day one, in a new `tokens.rs`.
   - The **real app** migrates in two gated commits: **(0a)** mechanical extraction into `tokens.rs` using
     `ui.rs`'s **current** values → gate = **pixel-identical**; then **(0b)** flip the values to the warm ramp
     / mockup → gate = **Ahmed judges by eye**. This is how "pixel-identical" and "mockup-is-law" coexist:
     0a is a safe refactor, 0b is a deliberate, separately-approved visual change.

5. **`UI_DIRECTION.md` is already reconciled** by the planning team (it now marks the spike DONE, points here,
   and fixes the control-bar line to "born in Stage 4, not first"). **Rely on its current version — do not edit
   it.**

6. **Build order is reordered (see §7): sandbox FIRST, in an isolated worktree; the real app is touched LAST,
   as one coordinated wave.** Start with the sandbox in a separate worktree so it runs fully parallel to the
   current Layers work with **zero collision** (new files only; never touch `ui.rs` or `varos-core`). The
   stages that touch the real program are deferred into one integration wave, coordinated with Ahmed **after
   the current Layers wave finishes** — so the app is never "half-built" and two sessions never fight over
   `ui.rs`.
   ```
   git -C "D:\VAROS" worktree add "D:\VAROS-shell" -b shell/box-system
   ```

7. **"Click a box, choose what it hosts" (the `⌄` swap-content menu) is a PRIMARY, gated goal of Stage 2** —
   not a footnote. It is how the container model is *proven*. It must be in Stage 2's gate.

8. **`plan.html` and `DETAILED_ROADMAP.md` belong to `main`.** The shell branch does **not** edit them.
   Log your progress inside **this file** (§10, Progress log) on your branch; the rail/roadmap are updated on
   `main` only at merge time (checklist in §9).

---

## 1. Repo reality (verified 2026-07-03 — so you trust the ground)

- **Engine spike is DONE** — commit `451ca2a`: wgpu 29 · egui 0.35 · winit 0.30.13. The shell is built on the
  NEW base; zero rework. (Project uses `egui` + `egui-wgpu` + `egui-winit` directly, **not** `eframe`.)
- **`varos-app/src/ui.rs` = 2552 lines**, 3 files in `src/` (`main.rs`, `ui.rs`, `cursors.rs`). **No `tokens.rs`
  exists yet** — you create it. `ui.rs` has ~13 named colour consts + ~13 inline colours + ~88 `CornerRadius`
  + ~114 `vec2` spacing literals scattered (Stage 0's target — Wave 2, not now).
- **Layers system is mid-flight** in parallel: scene-graph in `varos-core` (`model.rs`/`editor.rs`); the Layers
  *panel* lives in `ui.rs`. **This is why Wave 1 must not touch `ui.rs`** — a Layers session is editing the
  same file. The two crates: `varos-core` = model, `varos-app` = UI.
- **Cargo:** `varos-app/Cargo.toml` currently declares one binary (`[[bin]] name="varos"`). Adding the sandbox
  bin is a one-line additive edit *in your branch* (§6, Stage 2).

---

## 2. Why staged (the point of the whole exercise)

Ahmed's four steps, in his words → mapped to stages (identities in §6, order in §7):

| # | Ahmed's step | Stage | Wave |
|---|---|---|---|
| 1 | solid/void background lands on the program | **Stage 1** (void frame) | Wave 2 |
| 2 | panel SYSTEM sandbox — click a box, choose what it hosts, watch panels resize/tab | **Stage 2** | **Wave 1 (now)** |
| 3 | more boxes incl. a dummy "board" box — behaviors, grow/shrink, "animations", fake content | **Stage 3** (+ board & fake content seeded in Stage 2) | **Wave 1** |
| 4 | only after the system is proven → drop the real program in and wire it | **Stage 4** (migration) | Wave 2 |

The system is proven on **fakes** (Wave 1) before the **real** program is ever touched (Wave 2). That is the
entire safety guarantee.

---

## 3. The tokens & measurements (transcribe these verbatim into `tokens.rs`)

> Source = `UI_VISION_MOCKUP.html` `:root` + its class rules. These ARE the spec. Do not eyeball the CSS.

### 3.1 Colour ramp — the warm black (R ≥ G ≥ B). Chrome uses ONLY these.
| token | hex | role |
|---|---|---|
| `bg` | `#141313` | board / base — the deepest warm black, the **signature** (rule 7) |
| `panel` | `#1b1919` | box / panel fill |
| `surface` | `#242121` | inset field / control fill |
| `hover` | `#2b2828` | hover-state fill |
| `line` | `#2c2929` | 1px hairline (borders, separators) |
| `line2` | `#3b3735` | stronger hairline (kbd, swatch borders) |
| `text` | `#e9e6e3` | primary text |
| `muted` | `#8f8a86` | secondary text / icons |
| `faint` | `#6e6a66` | tertiary / micro-labels |
| `accent` | `#0c8ce9` | **azure scalpel** — selection / active / focus ONLY (rule 4) |
| `guide` | `#ff54a8` | smart guides |
| `seam` | `#0e0d0d` | **the VOID** — app bar bg, status bg, and every seam between boxes (darker than `bg`) |

### 3.2 Secondary palette (content/samples — NOT chrome)
| token | value | role |
|---|---|---|
| `navy` | `#12263a` | logo / artboard art sample |
| `amber` | `#f0b429` | fill sample |
| `ruler_bg` | `#181616` | ruler strips |
| `close_red` | `#c42b1c` | window-close hover |
| `none_red` | `#e05c5c` | "None" swatch diagonal |
| `dot_grid` | `rgba(255,255,255,0.045)` | board dot pattern |
| `void_hover` | `rgba(255,255,255,0.04)` | inactive doc-tab hover on the void |

### 3.3 Radii & rhythm
| token | value | role |
|---|---|---|
| `r` | `3px` | controls: fields, chips, buttons, tabs |
| `rbox` | `4px` | boxes / panels / the active doc-tab block |
| seam gap | `6px` | equal void between ALL boxes (including around the board) |
| spacing beat | `4 / 8px` | everything snaps to this (rule 6) |

### 3.4 Fonts
- `ui` = `"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif`
- `mono` = `"Cascadia Code", Consolas, monospace` — **tabular numerals** for every number on screen.

### 3.5 Measurements (lifted from the mockup — the box-system needs these in Stage 2/4)
- **App bar (void chrome):** height `40`. Burger `36×40`. Doc tab: h`28`, pad `0 12`, gap `4`, radius `rbox`;
  active = filled `panel` block, inactive = bare `muted` text, hover = `void_hover`. Tab-add `32` wide.
  Right cluster: search pill pad `4 9` radius `r`; buttons pad `5 12` radius `r`; window caps `42` wide
  (close hover = `close_red`).
- **Workspace / seams:** `.mid` = the box tree region — gap `6`, padding `6`, background `seam`.
- **Board box:** 1px `line` border, radius `rbox`. Rulers: h/v `18`, corner `18×18`, bg `ruler_bg`.
  Dot grid: cell `22`, dot = `dot_grid`.
- **HAND 1 — control bar (over board):** anchored `top:30` from the board's top edge, horizontally centered;
  h`36`, pad `0 10`, gap `6`. Mini field h`24` pad `0 7` radius `r`. Colour chip `17×17`. Icon button `26×26`
  radius `r`; active = `10×2` accent underline. Vertical sep `1×16` `line`.
- **HAND 2 — tool rail (over board, left):** anchored `left:34`, `top:104`; width `44`, vertical pad `6`,
  gap `2`. Tool button `32×32` radius `r`; **active = `accent` fill, white icon**. Separator `22×1`.
  Fill/Stroke swatch cluster `32×32` + paint-mode trio `9×9` at the bottom (Illustrator DNA).
- **Right column:** width `274` (min-size target ≈ `260`), inner gap `6`. Tab row (`gtabs`): pad `6`, gap `4`,
  bottom 1px `line`; tab pad `5 12` radius `r`, active bg `surface`. Section (`sec`): pad `10 12 12`, bottom
  1px `line`. Micro-label `slabel`: `9.5px`, uppercase, letter-spacing `1.2`, colour `faint`. Field `fld`:
  h`25`, pad `0 7`, radius `r`. Swatch `18×18`.
- **Status bar (void chrome):** h`25`, bg `seam`, gap `14`, pad `0 10`, `11px` `faint`.

---

## 4. The architecture (WHAT is fixed; HOW stays your freedom)

### 4.1 The box tree is the single truth of layout
The layout is a **tree**, serde-serializable from day one. Two node kinds (this is exactly what `egui_tiles`
gives us — §5):
- **Split** `{ dir: H|V, shares[], children[] }` — a `Container::Linear`. `shares` = the ratios; drag a seam
  → adjust shares.
- **Leaf** `{ panels[], active }` — a `Container::Tabs`. One panel → no tab row, just content. N>1 panels →
  **chip tabs render automatically** (Brave/Claude pill pattern).

**No layout is hardcoded.** The standard layout is just a **default tree value** you construct once. The later
customization wave (move/split/join/tear-off/workspaces) mutates or (de)serializes the same tree — a switch,
not a rewrite.

### 4.2 The panel registry
Every panel is a **named entry** in a registry: `id` · `display name` · `min_size` · a `render(ui, ctx, state)`
hook. The tree stores only **which panel id sits where** (a lightweight `PanelId`), never the panel's state —
state lives in the app model. This keeps serde clean (a saved workspace = a tree of ids) and lets the same
panel appear in different boxes.

Panels are composed from **shared "puzzle-piece" widgets** (number box, swatch, toggle, micro-label, chip tab)
— the same pieces the design system extracts. Build the piece once, reuse everywhere.

**Registry seed (the standard layout's panels):**
- `Board` — the canvas (its privilege in §4.4).
- `Align`, `Pathfinder` (later `+Shape Builder`) — the upper-right box.
- `Properties` (stacked section homes: `Transform · Appearance · Shape`), `Layers` — the lower-right box.
- Sandbox dummies (Stage 2): `Dummy A/B/C` + `DummyBoard` (flat fill).

### 4.3 The standard layout as a tree (the default value)
The tree occupies the **`.mid` region only**. The **app bar (top) and status strip (bottom) are void chrome
outside the tree** — painted as `TopBottomPanel`s filled with `seam`, no hairline (they are the void itself).

```
Root = Split(Horizontal, shares[ board ≈ .80 , right ≈ .20 ])
├─ Leaf[ Board ]                     ← canvas; HAND 1 + HAND 2 float over it
└─ Split(Vertical, shares[ upper ≈ .32 , lower ≈ .68 ])
   ├─ Leaf[ Align | Pathfinder ]         active = Align
   └─ Leaf[ Properties | Layers ]        active = Properties   (the growing box)
```
Right column min pixel width ≈ `260`; the mockup pins `274`.

### 4.4 The board is a Leaf like any other
`Board` is a normal leaf. Its only privilege: the **two floating hands render over it**. In the board pane's
`render` you get the pane rect — draw the canvas, then draw HAND 1 (control bar) and HAND 2 (tool rail) as
`egui::Area`s **anchored inside that rect** (z above content, per the offsets in §3.5). The hands are **not**
tiles; they never enter the tree. On a tiny window they float over the board content (Stage 3 edge case).

### 4.5 Seams, splitters, chip tabs, serde
- **Seams = equal `6px` `seam`-colour void everywhere**, including around the board. Never a shadow.
  Implemented via the `Behavior`'s gap width; the void shows through behind the boxes.
- **Drag-resize splitters live in the seams** (adjust the Split's shares). Cursor = resize icon on hover.
- **Chip tabs** = Brave/Claude pills: active = filled `panel`/`surface` block (radius `r`), inactive = bare
  `muted` text, hover = `void_hover`. Rendered by our custom `Behavior`, not egui_tiles' default tab bar.
- **The `⌄` swap-content menu** sits in each leaf's tab bar (painted by the `Behavior`): click → popup lists
  registry panels → picking one mutates the tree (swap the leaf's panel / add a tab). **This is Stage 2's
  headline interaction.**
- **serde from day one:** the tree derives `Serialize`/`Deserialize` (egui_tiles default feature). Workspaces
  later = saved trees; a missing/corrupt file falls back to the standard-layout default constructor. No
  migration ever.

---

## 5. Tool decision — `egui_tiles` (reviewed vs `egui_dock` vs hand-rolled)

**Ahmed asked for this to be settled with reasons before any manual build. It is settled: adopt `egui_tiles`.**

**Versions verified 2026-07-03 (both compatible with the project's egui 0.35):**
- **`egui_tiles` 0.16.0** (2026-06-26) → depends on `egui = "0.35.0"`; `serde` in default features. By the egui
  authors (rerun-io).
- `egui_dock` 0.20.1 → depends on `egui = "0.35"`; `serde` optional.

| requirement | `egui_tiles` | `egui_dock` | hand-rolled |
|---|---|---|---|
| recursive Split/Leaf tree **as data** (= our single truth) | ✅ `Tree<P>` / `Container::{Linear,Tabs,Grid}` + `Shares` | ⚠️ `DockState`, dock-oriented, less "plain tree" | ✅ but you build & debug it all |
| **full custom** tab-bar + pane look (chip tabs, no chrome) | ✅ `Behavior` trait (`pane_ui`, custom tab rendering, `gap_width`, `simplification_options`) | ⚠️ `Style` + `TabViewer`, but imposes more of its own dock chrome | ✅ total control, total cost |
| drag-resize splitters | ✅ built-in | ✅ built-in | ❌ hand-roll hit-tests |
| drag tabs → reorder / split / join (the *later* wave, free now) | ✅ built-in | ✅ built-in | ❌ hand-roll |
| serde tree (= workspaces later) | ✅ default feature | ✅ optional | ✅ you write it |
| 6px void seams, tokens look | ✅ via `Behavior` gap + our paint | ⚠️ fights its own styling | ✅ |
| board pane + floating hands over it | ✅ pane rect in `pane_ui`, hands as our `Area`s | ✅ similar | ✅ |

**Why `egui_tiles` over `egui_dock`:** its `Tree` is the closest thing to our literal `Split/Leaf` model and it
exposes it **as data**; the `Behavior` trait hands us near-total control of rendering, so we paint the exact
Brave/Claude chip tabs and the token palette without fighting a built-in dock aesthetic. `egui_dock` is
excellent but more opinionated toward an IDE dock look — more to override.

**Why not hand-rolled:** it would re-implement the tree, drag-resize, tab drag-reorder, split/join, and serde —
exactly the mechanics the *later* customization wave needs — for no gain, since `Behavior` already gives the
custom look. Hand-rolling only wins if egui_tiles' opinions block the mockup; they don't (Behavior covers it).

**Honest caveats to plan around (not blockers):**
- egui_tiles' **default tab bar is not our look** — implement custom chip-tab rendering in `Behavior`. Expected.
- **Auto-simplification:** egui_tiles collapses single-child / empty containers by default. For a stable
  standard layout, tune `SimplificationOptions` (keep it from dissolving our default tree); re-enable fuller
  simplification for the customization wave. Verify the exact flags in Stage 2.
- **Min-size enforcement:** confirm egui_tiles' min-size mechanism in Stage 2 and clamp shares to the §3.5
  targets (right column ≈ 260, etc.). If its hook is insufficient, enforce in `Behavior`.
- The two hands are **ours**, drawn over the board pane — egui_tiles doesn't know about them.

Sources: [egui_tiles docs](https://docs.rs/egui_tiles/latest/egui_tiles/) · [egui_tiles repo](https://github.com/rerun-io/egui_tiles) · [egui_dock](https://crates.io/crates/egui_dock).

---

## 6. The stages (identities — numbers match `UI_DIRECTION.md`; they do NOT imply build order)

> Build order is §7. Here each stage is defined with its **gate** (what Ahmed checks by hand).

### Stage 0 — Tokens *(real app · Wave 2)*
Extract every colour/radius/spacing from `ui.rs` into a new `tokens.rs`, in **two commits**:
- **0a — mechanical.** Move to `tokens.rs` using `ui.rs`'s **current** values. No value changes.
  **Gate: the app is pixel-identical to before.**
- **0b — value flip.** Change the token values to the warm ramp / mockup (§3). **Gate: Ahmed approves the new
  look by eye** (hand-off names exactly what shifted).
- *(عربي: نطلّع كل الألوان والمقاسات في ملف tokens.rs — أول كوميت نقل حرفي بدون أي تغيير شكل، تاني كوميت نقلب
  القيم للأسود الدافي وأحمد يحكم بعينه.)*

### Stage 1 — The void frame *(real app · Wave 2)*
App bar → the void + Brave chip **doc tabs**; status strip → the void; workspace background = `seam`.
Existing panels stay untouched **inside**. **Gate: Ahmed sees the void frame; nothing broke.**
- *(عربي: الإطار الفاضي بيتحط على البرنامج الحقيقي — الشريط العلوي والسفلي يبقوا فويد، والبانلات القديمة زي ما
  هي جوه لحد مرحلة الهجرة.)*

### Stage 2 — The box system, in a SANDBOX *(isolated worktree · Wave 1 · STARTS NOW)*
A separate `shell-sandbox` bin sharing the crate + a new `tokens.rs` (on the **law's** values). Deliver:
- the box tree (egui_tiles) + panel registry;
- **2–3 dummy panels** + **one dummy board** (flat `bg` fill, optional dot grid);
- **chip tabs** auto-rendering on any N>1 leaf;
- **drag-resize with min-sizes** (equal 6px seams);
- **the `⌄` swap-content menu — a box's "click and choose what it hosts"** *(primary; in the gate)*.
- **Gate: Ahmed plays with it** — resizes boxes, swaps a box's panel via `⌄`, sees seams stay equal and tabs
  appear/disappear correctly.
- *(عربي: ساندبوكس لوحده — ٢-٣ بانلات وهمية + بورد وهمي، تابات تلقائية، سحب وتحجيم بحدود دنيا، وأهم حاجة: تدوس
  على بوكس وتختار يشيل إيه. أحمد يلعب بيه ويجرّبه بإيده.)*

### Stage 3 — Behaviors, in the sandbox *(Wave 1)*
- functional motion only (drag follows cursor 1:1; collapse/expand a box) — **no fades/springs/effects**;
- collapse states; **the two dummy hands** floating over the dummy board; min-size enforcement;
- edge cases (tiny window, everything collapsed).
- **Gate: it *feels* right to Ahmed** — resize is smooth and instant, collapse is clean, hands sit right.
- *(عربي: السلوك بس — السحب يتبع الماوس، البوكس يتطوي ويتفرد، الإيدين الوهميتين فوق البورد، وحالات الشباك الصغير.
  ممنوع أي أنيميشن زينة. أحمد يحس إنه مظبوط.)*

### Stage 4 — Migration: the real app moves in *(real app · Wave 2)*
One panel at a time, app working at **every** commit (keep-both-paths-until-parity):
- canvas → the **Board** leaf;
- new Layers panel + Properties sections → real panels in `[Align|Pathfinder]` + `[Properties|Layers]`;
- tool rail → the floating **HAND 2**;
- **the Control Bar is BORN here** as the floating **HAND 1** (= Transform §2, **mirrors-only** rule).
- **Gate: the real program looks like `UI_VISION_MOCKUP.html` and everything still works** — pen feel,
  save/open, Layers drag-drop.
- *(عربي: البرنامج الحقيقي يدخل الصندوق بانل بانل، والبرنامج شغّال في كل كوميت. الكنترول بار بيتولد هنا كإيد تانية.)*

### Stage 5 — OUT of this series *(logged so you don't invent it early)*
Move/split/join boxes, tear-off a section into its own panel, workspaces (saved trees), themes/Light. Most
mechanics come free with egui_tiles; this is a **later wave on the same tree**.

---

## 7. The build order (Ahmed's ruling — this is what you actually do, and when)

### WAVE 1 — The sandbox *(START NOW; fully isolated; parallel-safe with the Layers work)*
**Stage 2 → Stage 3.** In a dedicated worktree:
```
git -C "D:\VAROS" worktree add "D:\VAROS-shell" -b shell/box-system
```
- **New files only.** Suggested: `varos-app/src/tokens.rs`, `varos-app/src/boxtree.rs` (Behavior + tree),
  `varos-app/src/registry.rs`, `varos-app/src/bin/shell-sandbox.rs`. Add `egui_tiles = "0.16"` to
  `varos-app/Cargo.toml` and one `[[bin]] name="shell-sandbox" path="src/bin/shell-sandbox.rs"` block —
  the **only** edits to existing files, and they live in your branch.
- **NEVER touch `ui.rs` or `varos-core`.** A Layers session is in `ui.rs` right now; that is the collision file.
- Run with `cargo run -p varos-app --bin shell-sandbox`. Gate each stage with Ahmed in that window.
- **The sandbox survives afterward as the design-system gallery** — every new puzzle piece gets shown there.

### WAVE 2 — Integration into the real app *(LATER; coordinated with Ahmed AFTER the Layers wave finishes)*
**Stage 0 (0a → 0b) → Stage 1 → Stage 4.** This is the **only** wave that touches `ui.rs`. Doing it as one
coordinated block (once `ui.rs` is free) means the real app is **never half-built** and there is **no `ui.rs`
collision** with the Layers work. Ahmed gives the go for this wave explicitly.

> Why this order resolves the risks: the parallel-session hazard was that both a shell session and the Layers
> session edit `ui.rs`. Wave 1 sidesteps it entirely (new files, isolated worktree). Wave 2 waits until
> `ui.rs` is free. No conflict, no Frankenstein interim.

---

## 8. Rules of engagement

- **One stage at a time. Never break the running app.** Wave 2 keeps both paths alive until parity.
- **Tokens only** — no inline colour/spacing/radius literals; everything from `tokens.rs`. Animation time `0`.
- **Gates are human hand-offs, not self-checks.** At each gate you STOP and write Ahmed a short plain
  **Egyptian-Arabic** hand-off: *what changed / what to test / what might look different*. He verifies by hand
  in the real window (or the sandbox bin). You never drive the UI yourself or auto-advance.
- **Ownership / conflict etiquette:**
  - Wave 1 (shell branch): new files only; **never** `ui.rs` or `varos-core`.
  - The Layers session: **never** `tokens.rs`/`boxtree.rs`/the sandbox.
  - `plan.html` + `DETAILED_ROADMAP.md` = **`main`'s**; the shell branch never edits them — log progress in §10
    here instead.
- **Honest PASS/FAIL per gate.** If a gate fails, say so plainly with what's wrong; don't dress it up.

---

## 9. Companion updates — done on `main`, at merge time only (checklist)

When a wave merges to `main`, update (in the **same** merge):
- **`plan.html`** — the `ORDER` rail (array ≈ lines 179–195): flip `⚙ Engine spike (wgpu)` to **✅ done
  (`451ca2a`)**, and slot a **"Box-System series"** run of cards. `node --check` the extracted `<script>`
  (lines 177–570) before committing.
- **`DETAILED_ROADMAP.md`** — flip the wgpu/spike lines to ✅ done: Decision-7 (l.16), build-order (l.29),
  **C1.12** (l.255, still says "currently 0.19/egui 0.27"), Stage-1 gaps (l.326). Note the Control Bar lands in
  Stage 4 as HAND 1.
- **`UI_DIRECTION.md`** — already reconciled; leave as-is.

*(These are Wave-2/merge chores. Wave 1 does none of them — it only appends to §10 below.)*

---

## 10. Progress log (append here on the shell branch — this is your rail until merge)

- 2026-07-03 — Brief written. Spike DONE (`451ca2a`). Decision: `egui_tiles` 0.16 (egui 0.35). Current stage =
  **Wave 1 / Stage 2** (not started). Worktree `D:\VAROS-shell`, branch `shell/box-system`.
- _(next: Stage 2 — box tree + registry + tokens.rs + 3 dummies + dummy board + chip tabs + `⌄` swap menu…)_

---

## 11. Kickoff message for the fresh session (Ahmed pastes this to open it)

> **مهمتك: بناء قشرة الواجهة (البوكس-سيستم) لڤاروس، على مراحل.**
> اقرأ بالترتيب قبل أي كود: (١) `docs/UI_DIRECTION.md` — ده القانون. (٢) افتح `docs/UI_VISION_MOCKUP.html`
> في المتصفح — دي الصورة الهدف. (٣) `docs/BOX_SYSTEM_PLAN.md` — ده البريف الكامل (المعمارية + القرار التقني +
> المراحل + الترتيب).
>
> **إحنا في Wave 1 / Stage 2 (الساندبوكس).** اشتغل في وركتري منفصل خالص:
> `git -C "D:\VAROS" worktree add "D:\VAROS-shell" -b shell/box-system`
> **ملفات جديدة بس — ممنوع تلمس `ui.rs` أو `varos-core` نهائياً** (فيه سيشن تانية شغالة على الـLayers في نفس
> `ui.rs`). القرار متحسم: نستخدم `egui_tiles` 0.16 (متوافق مع egui 0.35)، مش هنبني الشجرة بإيدنا — التفاصيل
> والأسباب في §5.
>
> ابنِ Stage 2: شجرة البوكسات + سجل البانلات + `tokens.rs` بقيم الموكاب + ٢-٣ بانلات وهمية + بورد وهمي + تابات
> تلقائية + سحب/تحجيم بحدود دنيا + **منيو الـ`⌄` (تدوس على بوكس وتختار يشيل إيه — دي أهم حاجة)**.
> `cargo run -p varos-app --bin shell-sandbox`. لما تخلص، **قف واكتبلي هاند-أوف بالمصري** (اتغيّر إيه / أجرّب
> إيه / إيه اللي ممكن يبان مختلف) وأنا أجرّبه بإيدي. مرحلة مرحلة، براحتك، مفيش استعجال.
