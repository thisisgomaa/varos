> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — UI Direction & Visual Constitution (LOCKED with Ahmed, 2026-07-03)

## Execution note — 2026-09-27

This document retains the visual direction. `BOX_SYSTEM_PLAN` is implementation background, not the current build queue. Later owner decisions (Home instead of the Mac burger, icons instead of text buttons, bundled fonts, instant chrome with the box glide as an accepted exception, per-user layout persistence under law L7 (2026-10-08)) are collected in [UI_SYSTEM](specs/UI_SYSTEM.md). The mockup illustrates layout; runtime values live in `shell/tokens.rs`. Follow [PLAN](PLAN.md) for execution order; no new visual choice is approved by this navigation update.

## Reconciliation — 2026-10-04 (UI System v3)

This text disagreed with the code and with later owner decisions in four places (app-bar cell, seams, tab radius, panel corners) and never recorded two later decisions (on looks, motion). Each is settled below by the owner's recorded decision or CLAUDE.md; where the CODE is wrong, the fix is a piece in [UI_SYSTEM §9](specs/UI_SYSTEM.md), not an edit of this law.

| Topic | Was here | Code on `main` (`221310f`) | Law now | Code change needed |
|---|---|---|---|---|
| App-bar left cell | ☰ app menu | Mac: kit Home chip (`ui.rs:3510-3520`); ☰ only on the compile-only Windows path | **Home** on Mac (owner, Sep 24/25); native menu bar owns File/Edit/…; Windows ☰ revisited with Windows | none |
| Seams | ~6 px | `SEAM_GAP = 12` (`tokens.rs:60`, owner 07-04 "boxes breathe") | **12 px** equal void; the mockup's 6 px is an illustration | none |
| Tab radius | doc tab 4 px; panel tabs "pill" | doc tab chip `RBOX` 8 (`ui.rs:3192`, BOX_SYSTEM_PLAN §3.5); panel tab pill `RCAP` 11 (`shell/boxtree.rs:584`, owner 07-08 "one capsule radius"); `tokens.rs:57` comment wrongly lists tabs under `R` 3 | **doc tab chip 8 · panel tab pill 11** | fix the `R` comment (P4) |
| Panel corners | "0–4 px max, controls only; panels sit square" | boxes `RBOX` 8 (`tokens.rs:58`, owner 07-04); 26 stray literals 2/3/4/5 in `ui.rs` | **three radii only: 3 controls & menu rows · 8 boxes, popup frames, doc tab chips · 11 capsules** (CLAUDE.md "3px controls / 8px boxes") | literals → tokens (P4) |
| "On" looks | (not stated) | `icon_toggle` on = ACCENT block (`ui.rs:1982`); `seg_btn` on = ACCENT + white text (`ui.rs:5172`, white on azure 3.53:1 fails text contrast) | tool = azure block · toggle = TOGGLE_WELL + TEXT (owner 2026-10-08) · tab/segment = SURFACE fill (owner, Sep 24/25) | P4 |
| Motion | (rule absent) | `animation_time = 0` (`tokens.rs:90`) | **Chrome appears instantly** (`animation_time = 0`: menus, popups, panels never fade or ease) and the **live tab drag is instant** (no time, no interpolation); the direction bar stays. **Accepted exception:** the box glide on dock/undock (owner 2026-07-05, reaffirmed 2026-10-06: not a defect) | none — do not spend effort removing the glide |

> ✅ **APPROVED by Ahmed 2026-07-03 ("برفكتو").** The living visual reference =
> **`docs/reference/UI_VISION_MOCKUP.html`** (open it in a browser — it demonstrates every rule below:
> void frame, box workspace with equal seams — 12 px at runtime, see Reconciliation —, chip tabs, floating hands, warm-black ramp).

> **This file is the visual law.** Any new panel/bar/control is measured against it.
> Supersedes the *floating-shell* look in `UI_FIGMA_SPEC.md` / `PANELS_PRO_SPEC.md` (their **palette stays valid**).
> Architecture unchanged: we still draw the whole UI ourselves on the GPU (egui on our wgpu surface).
> Execution timing: the wgpu spike is **DONE** (`451ca2a` — wgpu 29 / egui 0.35). The build order lives in
> **`docs/reference/BOX_SYSTEM_PLAN.md`** — the Control Bar is born in its Stage 4 (not first).

## Owner decision — agent presence on the canvas, 2026-10-08

Azure is human selection/focus; `AGENT` orange is anything an agent is doing.
`AGENT = #C76A20` reads on both white pages and the dark well. Presence lives on
artboards and objects, never in bands or panels; the box system is unchanged.
A flat 1.5-pt outline outside the working page and small title-style labels show
its agents after each committed edit (4-s hold); labels stack on the same page. A committed batch gets staggered
1-pt world-bounds outlines, with short fades allowed as a canvas-only motion
exception. No shadows. Human selection wins over agent outlines.
The host paints existing Bridge data: no new wire fields or agent calls.
The full document commits atomically as one undo step before the reveal begins;
feedback shows observed edits, never an agent's unobservable thinking time.

## Identity — "Son of Illustrator, raised by Claude"
**ابن اليستريتور، متربّي عند كلود.**
- Illustrator gives the SKELETON: density, completeness ("everything is there"), a serious workbench.
- Claude gives the SKIN: flat calm surfaces, typography-led hierarchy, zero decoration, quiet confidence.
- NOT the generic AI-app look (glass/glow/gradient/round-everything). NOT old-Adobe chrome. NOT a Figma clone.

## The 7 rules (الدستور)
1. **HOMES DOCKED, HANDS FLOAT** *(refined by Ahmed's sketch, 2026-07-03)*. Panels that OWN content
   (the right Properties/Layers wall, app bar, status bar) are solid docked parts of the screen.
   The two HANDS — the **tool rail** (floating, left) and the **contextual control bar** (floating, top of
   the board) — float near the work. Nothing else floats (tear-off comes later). *(البيوت مرصوصة والإيدين عايمة.)*
2. **Not one shadow in the whole app.** Separation = 1px hairlines + 2–3 surface tone steps. *(الفصل بخط شعرة، مش ضل.)*
3. **Near-sharp corners — three radii only.** 3 px controls and menu rows · 8 px boxes, popup frames and doc tab chips · 11 px capsules (panel tab pills, scroll chevrons, switch track). Nothing else (2026-10-04 reconciliation). *(الحدة = الجدية.)*
4. **Azure is a scalpel, not paint.** `#0c8ce9` appears ONLY on selection / active / focus. *(الأزرق مشرط.)*
5. **Typography is the only decoration.** Spaced uppercase micro-labels, tabular mono numerals, clear names. *(الخط هو الزينة.)*
6. **Illustrator density, breathing rhythm.** Everything at hand, on a fixed 4/8px spacing beat. *(كثافة بتتنفس.)*
7. **The warm black is the signature.** `#141313` is a warm black (red-leaning) — ALL grays ride the same warmth. Not the cold blue-dark of every AI tool. *(الأسود الدافي = البصمة.)*

**Philosophy over all:** the only beautiful thing on screen is the USER'S ARTWORK. The UI is the frame, never the painting.

## THE ONE-HOME RULE — قاعدة البيت الواحد (Ahmed, 2026-07-03 — the anti-Adobe law)
Adobe scatters one feature across control bar + menus + 3 panels because 30 years of legacy won't let them merge.
We are born clean, so:

- **Every domain has exactly ONE home (a "Section").**
  - **Color home** = picker + recent colors + color guide/harmony + gradient + Recolor — ALL in one place. (The shipped picker already lives this rule: recent + wheel + harmonies in one box.)
  - **Stroke home** = weight, caps, joins, dashes, align, its options button — with the stroke color pick right there.
  - **Transform home** = X/Y/W/H, rotate, flip, pivot, and a "more…" that reveals the deep options (Transform Each, etc.). No separate scattered Transform surfaces.
- **Menus and the Control Bar NEVER own features — they are MIRRORS/shortcuts to homes.** A control on the bar is a mirror of its home's top controls; its "…" jumps to the home. A feature with no home may not exist on any bar/menu. This single rule prevents Adobe's scatter from ever growing here.
- **Sections are the unit; panels are containers of sections.** Future customization (tear-off a section into its own floating panel, rearrange docks, workspaces) moves the SAME section between containers — one implementation, many placements. *(حتة "آخدها في بانل لوحدها" — مشروعة ومبنية في التصميم من اليوم.)*

## THE CONTAINER MODEL — "كل حاجة صندوق" (Ahmed, 2026-07-03 — the Blender law)
Ahmed's formulation: every region is a "menu" (a BOX) built the same one way — Properties box, Align box…
and **the BOARD ITSELF is just another box** sitting beside them, whose special privilege is that the two
floating hands live over it. Boxes stack under/above/beside each other freely.

- **One layout system:** the screen = a tree of uniform boxes (exactly Blender's editor-areas — Ahmed named
  Blender himself). No box is special-cased in code — the board is a box whose content is the canvas.
- **The ONLY fixed chrome = the APP BAR** (Ahmed 07-03): Home (Mac; ☰ only on the deferred Windows build) + doc tabs + global actions + window buttons.
  It has nothing to do with the board. Everything else lives inside boxes.
- **The app bar (and status strip) are NOT panels — they are the VOID itself** (Ahmed 07-03, from Brave's
  tab bar): background = the seam colour — **`#000000` since 4b (owner, 2026-10-05; was `#0e0d0d`)**: one flat
  backdrop for the band, every seam and the status line, no fill, no hairline, no step (MAC_CHROME.md §A′). **Doc tabs = chips floating in the
  void** — active tab = a filled block (4b, 2026-10-05: `SURFACE`, 3 px `R` radius — was `PANEL` / 8 px `RBOX`),
  inactive = bare muted text, hover = `HOVER`; 28 tall, 88–176 wide, gap 2 (MAC_CHROME.md §A′).
  So the whole shell reads as: one dark void, boxes floating in it, chips floating on its bar.
- **Seams: EQUAL GAPS everywhere** (12 px `SEAM_GAP` of the backdrop — `#000000` since 4b, was `#0e0d0d` — darker than the board) between ALL boxes —
  including around the board box. The equal rhythm is what makes it read as one system (his 3 reference
  layouts / the Claude-Code panes look). Never shadows.
- **Multiple panels in one box → they become TABS automatically** — Claude-style pill tabs
  ([ Chat | Cowork | Code ] pattern): active = filled `SURFACE` pill (11 px `RCAP`), inactive = muted text. One panel alone
  in a box = no tab row, just its content.
- **Boxes resize freely; the box adapts to its panel type** (min sizes per panel). 
- **Engineering consequence (build it this way from day ONE):** the STANDARD layout is laid out AS a box
  tree from the first build, and **drag-to-resize splitters ship with it** (native in egui — nearly free).
  The later customization wave adds move/split/join/tear-off + workspaces on the SAME tree — a switch,
  not a rewrite. This is the no-rework principle applied to layout.

## The STANDARD layout (now; user customization comes later)
```
TOP:    App bar, docked (Home on Mac · doc tabs · quick actions right · window buttons)
FLOAT:  CONTROL BAR — a floating strip at the TOP OF THE BOARD (margins, not edge-to-edge);
        contextual mirrors of the selection's homes (mini X/Y/W/H · fill/stroke chips ·
        weight · opacity · quick align · snap magnet · insertion target · "…"-to-home)
FLOAT:  TOOL RAIL — floating on the LEFT over the board (✅ LOCKED left + floating,
        Ahmed's sketch 07-03; width is the abundant dimension). Fill/Stroke swatch cluster
        + the paint-mode trio at its bottom (Illustrator DNA).
RIGHT:  Docked column — TWO stacked tab groups (Ahmed's Illustrator pattern):
        upper  [ Align | Pathfinder(+Shape Builder) ]
        lower  [ Properties | Layers ]  — Properties = stacked section homes (Transform · Appearance …)
BOTTOM: Status bar, docked (shortcut hints · artboard nav · Fit · zoom)
BOARD:  Everything else. Rulers on.
```

## Future systems this direction already reserves (logged, NOT now)
- **Tear-off & arrange:** any section → its own panel; docks rearrangeable (roadmap A2.9).
- **Themes:** shade variants of the warm black + a Light mode, all read from the token module (A1.6).
- **Workspaces:** saved layouts per task; the "standard" above is Workspace #1.

## Build hook (so this stays real, not poetry)
- **The build order lives in `docs/reference/BOX_SYSTEM_PLAN.md`** (2026-07-03): tokens → void frame → box-system
  sandbox with dummy panels → behaviors → migrate the real app in. The **Control Bar** (= Transform §2)
  is born in its Stage 4 as the second floating hand, built as *mirrors of sections*. (The wgpu spike is
  DONE — `451ca2a`: wgpu 29 / egui 0.35.)
- Every new system's panel = a **Section home** composed from the A-layer puzzle pieces, then mirrored to the bar.

Owner-approved 2026-10-09: the Colour picker v3 is a 240 pt modeless hand inside the Board hole. The Figma Wheel export is the geometry reference; the warm dark tokens remain authoritative for chrome. The contract specifies a counter-clockwise hue axis, HSB readout, 36/44 pt target cluster and 20 pt default squares, which differ from the illustrative export; these explicit values govern this slice. OK/Cancel are removed: colour drags commit one undo step on release, fields commit through K3, and Esc closes without reverting. Selection and external edits re-seed the open panel; its position, open state and drawer preferences live in the additive version-1 layout store. The box system is unchanged.
