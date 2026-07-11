> **Status:** reference — Reference material only; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
I'll write the full reference and build-spec document directly, since this is a documentation/architecture task and all the source material is provided. No codebase investigation is needed — the Penpot internals are already specified.

Here is the complete document.

---

# VAROS VECTOR EDITING — Illustrator Reference Model, Target Architecture, and Build Spec

*Modeless, tool-driven anchor editing on a Penpot fork. Reuse Penpot's geometry/bezier/render/undo primitives; replace its mode-based interaction layer.*

All Illustrator behavior below incorporates the fact-check corrections. The three myths we must NOT replicate are flagged where they bite.

---

## PART A — ILLUSTRATOR REFERENCE MODEL

### A.0 The one organizing principle

**The tool is the mode. Selection granularity is determined by the active tool, not by an editing mode you enter.** The same path is selectable as a whole object (black arrow), as a member of a group (Group Selection), or as individual anchors/segments (white arrow) — and which happens depends entirely on which arrow is active when you click. There is no state to enter, no container to "open," no double-click gate before you can touch geometry.

Three orthogonal pieces of state fully determine what any click does, and they are **decoupled and recomputed every frame**:

1. **Active tool** — what a primary click/drag does right now (always exactly one).
2. **Selection state** — which objects, and which of their anchors/handles, are selected. This is **document data, not a mode**, and it **persists across tool switches**.
3. **Transient modifiers** — Cmd/Ctrl, Alt/Option, Shift held this instant, which can temporarily *borrow* another tool without changing the active one.

There is no hidden "edit mode" variable. The "mode" a user might think they're in is never stored — it is recomputed from `(active tool, cursor hit-test, held keys)` continuously, and is always visible (toolbar highlight + live cursor glyph).

### A.1 The selection model — one persistent, document-level set

Selection is a property of the **document/scene**, not of any tool. Switching tools does **not** clear, alter, or "commit" the selection. Tools are *lenses* onto one persistent selection at different granularities:

- **Selection tool (V), solid black arrow** — sees selection at **object/top-group granularity**. Always selects the *largest selectable unit*: click a path → whole path; click anything in a group → whole top-level group. Shows a **bounding box with 8 handles** (4 corner scale, 4 mid-edge scale, outside-corner hover = rotate). Cannot touch an individual anchor. Marquee = **touch-intersect** (selects every object the rectangle contacts), at whole-object granularity.

- **Direct Selection tool (A), hollow white arrow** — sees the *same* selection at **anchor/segment granularity**. Reaches *through* objects and groups to raw geometry without entering or disturbing them. This is what makes "no edit mode" true.

- **Group Selection tool (white arrow + "+"), nested under A, also = Alt while A is active** — progressive group drill: first click = the single inner object; each subsequent click on the same spot climbs one group level up (object → parent group → grandparent …). Treats objects as whole units; never touches anchors.

**Implementation keystone:** selection lives in document/session state. Tool *activation* is inert toward selection (a tool may *refine* it on first interaction — e.g. V on empty canvas deselects — but picking up a tool never resets it). You draw with Pen → press A → the path is still selected and you immediately grab an anchor, no re-click, no "enter."

### A.2 The crucial fact — no edit mode / no isolation for anchor editing

With `A` active, **every anchor of every path is directly clickable and editable at any moment**, with zero preparatory step. You do NOT: double-click to enter, enter Isolation Mode, open a container, toggle a "vector/path edit" state, or ungroup.

Consequences to preserve faithfully:
1. **Cross-object anchor editing is frictionless** — one marquee grabs anchors from many unrelated paths and moves them together.
2. **The tool is the only state** — what you can do to geometry is fully predicted by the active arrow.
3. **No "trapped inside an object" failure state** — there is nothing to get out of.

### A.3 The four canonical clicks, per tool

(Default pref: *Object Selection by Path Only = OFF*, so clicking a fill selects.)

| Click target | Selection (V) | Direct Selection (A) | Group Selection |
|---|---|---|---|
| Empty canvas | Deselect all | Deselect all | Deselect all |
| Object fill | Whole object / top group | Whole path; **all anchors visible (hollow)**; reaches into groups | Inner object → group → outer (per repeat click) |
| Path edge / segment | Whole object / top group | That **segment** + its two endpoint anchors' handles | Inner object → group (per repeat) |
| Anchor point | Whole object / top group | **That single anchor (filled)**, others hollow; handles if smooth | Whole object → group (per repeat) |

### A.4 On-screen feedback — the three treatments map 1:1 to the three granularities

1. **Object/group (V):** bounding box + 8 handles enclosing the whole unit; anchors shown only as inert markers.
2. **Whole path, anchor-aware (A, clicked fill):** full path skeleton in layer color, **every anchor visible as HOLLOW squares**, no bounding box.
3. **Single anchor/segment (A):** **selected anchor(s) drawn SOLID/FILLED**, unselected ones **HOLLOW**. This filled-vs-hollow contrast is *the* canonical "is this point selected?" signal and must be preserved exactly. Selected smooth anchors show their direction lines (line ending in a round dot); corner/straight anchors show no handles (zero-length).

### A.5 Direct Selection mechanics (exhaustive) — incorporating the corrections

The tool is **modeless and live**: pick up `A`, hover (Illustrator highlights the anchor/segment and previews via cursor glyph), click/drag, release commits one undo step. Click empty / Esc / switch tool = deselect. You are never "inside" anything.

- **Click a segment:** path becomes active, **all anchors appear hollow**, the clicked segment's two endpoint handles show so it's immediately reshapeable.
- **Click an anchor:** that anchor turns **solid**, others stay hollow; smooth anchor shows its handles.
- **Marquee from empty canvas:** selects **all anchors inside the rectangle, across any number of separate objects** (collects points, ignores object boundaries).
- **Shift-click / Shift-marquee:** add/toggle membership.
- **Drag a selected anchor:** it follows the cursor; **its handles ride along** (offsets preserved); adjacent segments stretch. **Multiple selected anchors translate together by the same delta** (press on one of the *already-selected* anchors to keep the set; pressing an unselected anchor re-selects just it).
- **Drag a segment directly:** curved segment bulges to follow cursor (Illustrator adjusts the two endpoint handles under the hood; anchors stay put); straight segment translates, pulling both endpoint anchors.
- **Drag a direction handle:** repositions that one off-curve control. **★ CORRECTION (biggest myth):** with the **Direct Selection tool**, dragging one handle of a smooth point moves **ONLY that handle** — it does **NOT** rotate the opposite handle, and it does **NOT** require Alt to "break symmetry." Direct Selection breaks the tangent **freely**. The collinearity/symmetry coupling ("drag one handle, the partner rotates to stay collinear") is the **Anchor Point tool's** (and the Pen's) behavior, **not** Direct Selection's. Do not replicate smooth-handle coupling on the white arrow.
- **Alt-drag a path part with Direct Selection:** **duplicates the whole path** (copies the object, not a lone anchor). So Alt+white-arrow-drag is an object-duplication gesture, not a per-anchor one.
- **Shift while dragging:** constrains to 45° multiples (anchor moves and handle angles).
- **Arrow keys:** nudge the selected anchor set by the **Keyboard Increment** (Prefs ▸ General); **Shift+arrow = 10× the increment** per press.
- Handle visibility for multi-selected anchors is governed by Prefs ▸ Selection & Anchor Display (on by default).

### A.6 The unified Pen (P) — one context-aware tool

The Pen is a small state machine resolved **per pointer-move** from `(activePath?, liveEndpoint?, hoverTarget ∈ {empty, segment, anchor, endpoint, startAnchor}, modifiers, gesture)`. It never switches tools; it changes the **cursor glyph** to show the armed behavior and changes what mouse-down does.

**Two anchor types from gesture:** click = **corner** (no handles, straight segments between corners); click-drag = **smooth** (pulls out two collinear+symmetric handles → cubic curve). Shift constrains handle/segment to 45°. Space (held mid-drag, before release) repositions the whole anchor while still placing it.

**Resolution priority (run every frame):**

1. **Alt/Option over an anchor → Convert Anchor Point** (Alt-click smooth → corner / removes handles; Alt-drag corner → pull fresh handles; Alt-drag one handle of a smooth anchor → break the pair → independent cusp handles). Highest-priority over-anchor modifier.
2. **Ctrl/Cmd held → temporary Direct Selection** (borrow last-used selection tool to nudge anchor/handle mid-draw; release → back to Pen, path still live).
3. **Path live (open, active endpoint):**
   - over **start anchor** → **Close** (glyph: small circle ○; click = straight close, drag = curved close).
   - over **empty canvas** → **extend** (click = corner, drag = smooth; rubber-band previews the next segment from the last anchor).
4. **No live path, cursor over existing geometry** (Auto-Add/Delete on, Shift not suppressing):
   - over a **segment** → **Add Anchor Point** (glyph +); inserts an anchor **without changing the path shape** (recompute handles via De Casteljau split).
   - over an **existing anchor** → **Delete Anchor Point** (glyph −); removes it and **reflows** the neighboring segment (does NOT leave a gap the way Backspace on a selected anchor would).
   - over an **endpoint of a *selected* open path** → **Continue/resume** (glyph /).
5. **No live path, empty canvas → start a new path.**

**★ CORRECTION:** a **bare Pen-click on an existing anchor = Delete**, not Convert. Smooth→corner conversion requires **Alt** held. (The source table contradicted itself; Delete is correct for the unmodified click.)

**Cursor glyph table (the live state indicator):**

| Glyph | Armed action |
|---|---|
| Pen + × | start a new path (empty, no live path) |
| plain Pen | extend current path |
| Pen + ○ | over start anchor → **close** |
| Pen + / | over endpoint of selected open path → **continue** |
| Pen + + | over a segment → **add anchor** |
| Pen + − | over an existing anchor → **delete anchor** |
| convert caret ∧ | **Alt** over an anchor → **convert** |
| arrow | **Ctrl/Cmd** held → temporary Direct Selection |

**Ending an open path (leave open):** Enter/Return, Esc, Ctrl/Cmd-click empty canvas, or switch tools. **Join** (Object ▸ Path ▸ Join, Ctrl/Cmd+J) is a separate command: select two endpoints/paths with `A`, Join inserts a straight segment if non-coincident, or merges into one anchor (with Corner/Smooth dialog) if coincident; joining the two ends of one open path closes it. **Average** (Alt+Ctrl/Cmd+J) moves selected anchors to their mean (H/V/Both).

### A.7 The dedicated path tools (same functions, explicit modes)

| Tool | Default shortcut | Function |
|---|---|---|
| Pen | **P** | context-aware draw/add/delete/continue/close |
| Add Anchor Point | **none** (★ no default key; lives in Pen flyout) | click segment → add |
| Delete Anchor Point | **none** (★ no default key) | click anchor → delete + reflow |
| Anchor Point (Convert) | **Shift+C** | smooth↔corner; drag to pull/break handles |
| Direct Selection | **A** | anchors/segments/handles |
| Selection | **V** | whole objects/groups |
| Curvature | **★ no reliable default key** (do not ship `Shift+~`) | auto-smoothing point-by-point curves |
| Scissors | **C** | cut path at a point |

**★ Corrections:** Add/Delete Anchor have **no default keyboard shortcuts** (`+`/`−` are tool *names*, not keys). The Curvature tool has **no reliable default shortcut** — do not state `Shift+~` and do not ship it expecting parity. `Shift+C` (Anchor Point), `A`, `V`, `P`, `C` (Scissors), `N` (Pencil) are correct.

### A.8 Spring-loaded (held-modifier) tool borrowing

Two switch *lifetimes*, which a faithful model must distinguish:
- **Tap a shortcut (V/A/P…) = latching/permanent** switch; stays until changed; selection preserved.
- **Hold a modifier = momentary borrow** that pops on release, restoring tool + cursor + in-progress operation *exactly*. No commit, no exit cost.

- **Ctrl/Cmd** → borrow the **last-used selection tool** (track "last selection tool" as state; power users keep `A` last so Cmd gives Direct Selection). The whole "select → adjust handle → resume drawing" micro-loop happens with the path under construction staying live.
- **Alt/Option** → the **convert / duplicate / transform-from-center** variant of the current gesture.
- **Shift** → constrain (45°, proportional) / add-to-selection. Orthogonal; composes with the others.

The defining invariant: **on key release, tool + cursor + in-progress op return to exactly the pre-modifier state.** Nothing in the toolbar mutates.

### A.9 Isolation Mode — what it actually is, and why it is NOT anchor editing

Isolation Mode is Illustrator's one genuine modal state, for working **inside a container** (group, symbol, clip mask, compound path, single object): enter by double-clicking a group, or via Layers panel / context menu / control-bar isolate icon. Outside content dims and locks; a grey breadcrumb bar appears; the Selection tool now grabs individual objects *inside* the container; Esc / double-click-out / breadcrumb exits.

**It is object-level scoping, NOT geometry editing.** Entering isolation does not expose anchors — you still need `A` for anchors, and once you have `A` you never needed isolation, because `A` already ignores grouping. The common confusion: double-click-a-group enters isolation, and Figma/Sketch users assume double-click-to-enter is the path-editing gesture. It is not. **Isolation changes *which objects you can select*; the white arrow changes *whether you're selecting objects or anchors*. Only the latter touches geometry, and it requires no mode.**

### A.10 The geometric data model (cubic bezier — fully accurate, no corrections needed)

A **path** = ordered list of **anchors** + `closed` flag. Order is load-bearing (trace anchor[0]→[1]→…→back to [0] if closed; reversing reverses direction → matters for nonzero fill).

Each **anchor** carries three coplanar points: `P` (on-curve), `in` (incoming handle), `out` (outgoing handle), plus `type ∈ {CORNER, SMOOTH}`.
- `out` = first control of the segment **starting** here; `in` = second control of the segment **ending** here. A handle belongs to exactly one segment; no sharing.
- **CORNER:** `in`/`out` independent (any dir/length, either may be null).
- **SMOOTH:** `in`, `P`, `out` **collinear** (tangent-continuous / G1). *Symmetric* smooth = equal lengths, exact mirror (`out = 2P − in`), the default when dragging out a fresh handle; *collinear-only* smooth = one line through P but unequal lengths.

Each **segment** A→B is one cubic: `P0=A.P, P1=A.out, P2=B.in, P3=B.P`, `B(t)=(1−t)³P0+3(1−t)²t·P1+3(1−t)t²·P2+t³P3`. Straight segment ⟺ `A.out==A.P` and `B.in==B.P` (≡ `L`). Half-straight (one handle out, one coincident) is legal and normal.

**Open** path: n anchors → n−1 segments (first.in and last.out dangling). **Closed:** n→n segments; closing activates last.out + first.in (can be curved). **Compound path** = several subpaths + fill rule (EVEN-ODD = ray crossings parity, direction-agnostic; NONZERO = signed winding sum, direction-dependent → "reverse path direction" toggles a hole only under nonzero).

**SVG mapping:** `M` (anchor0) + per segment `C x1 y1 x2 y2 x y` (`x1y1`=A.out, `x2y2`=B.in, `xy`=B.P) or `L` for straight + `Z`. A smooth anchor M's `in` is the **trailing** control of the first C, its `out` the **leading** control of the next C; collinearity through M is the on-disk smooth signature. (Penpot's `:content` is the same family — see Part B.)

**The two edit deltas (decisive):**
- **Move an anchor by d:** `P += d; in += d; out += d` → **both neighbor segments shift bodily**, tangents unchanged.
- **Move a handle by d (anchor fixed, curve still passes through P):** on a **CORNER**, only that one control moves → **one segment** re-bends; on a **SMOOTH** anchor, the *editor's constraint* drags the partner too → **both** adjacent segments re-curve, join stays smooth. **(In our white-arrow tool we deliberately do NOT apply the smooth coupling — see A.5 correction. Coupling is reserved for the Pen/Convert path.)**

### A.11 Consolidated principles for reimplementation

1. Tool determines granularity; no geometry edit mode.
2. Selection is document-level, reinterpreted per tool; tool activation is inert toward it.
3. Three feedback treatments ↔ three granularities (bbox / all-anchors-hollow / filled-among-hollow).
4. White arrow reaches through groups to geometry without entering them.
5. Filled-vs-hollow is the canonical anchor-selected signal.
6. Marquee: V = whole objects touched; A = anchors inside region, cross-object.
7. Isolation = object-scoping container focus, separate from anchor editing.
8. White-arrow handle drag breaks tangents freely (no coupling, no Alt). Coupling = Pen/Convert only.
9. Pen = context state machine over `(target, path-state, modifiers, gesture)`; bare-click-on-anchor = Delete; Convert needs Alt.
10. Distinguish tap-to-latch from hold-to-borrow.

---

## PART B — TARGET ARCHITECTURE FOR THE PENPOT REBUILD

### B.0 The thesis: reuse Penpot's primitives, replace its interaction layer

Penpot already gives us everything *below* the interaction line, and it is the right family:
- **Geometry:** `:content` = ordered `move-to / line-to / curve-to {:c1x :c1y :c2x :c2y} / close-path` — identical cubic-bezier model to A.10. `c1` ≙ `A.out`, `c2` ≙ `B.in`, the curve-to endpoint ≙ `B.P`.
- **Render:** WASM/SVG path renderer already draws `:content`.
- **Undo:** Penpot's undo stack works on shape mutations.
- **Anchor/handle hit-testing & drawing:** the path editor already renders square=corner / circle=smooth anchors and round handle dots, and already hit-tests them.

What we must **replace** is the interaction layer Penpot built on top: `:edition` (one-shape isolation), `:edit-path/:edit-mode {:draw|:move}`, the phantom add-point, the undo-blocked-by-`:edition` constraint, and double-click-to-enter. These encode a **per-object modal** model — the exact opposite of Illustrator's modeless, cross-object, tool-driven model. **Incrementally patching the mode system keeps producing interaction bugs** (e.g. `:draw` made our Direct Selection behave like a pen) because we are fighting the fundamental shape of the system. The fix is to introduce a **modeless selection/interaction layer that sits beside Penpot's primitives and treats `:edition` as a private rendering/render-scope detail, never as user-visible state.**

### B.1 Where Penpot's model fights Illustrator's — and the bridge for each

| # | Penpot (mode-based) | Illustrator (modeless) | Bridge |
|---|---|---|---|
| 1 | `:edition` = the ONE shape being edited = isolation; entered by double-click | No isolation for anchor editing; any path's anchors editable anytime, across objects | Make our **anchor-selection** state the source of truth. `:edition` becomes an *internal, possibly-multi* render flag we set/clear silently to satisfy Penpot's renderer; it is never the user's mental model and never gated behind double-click. |
| 2 | `:edit-path :edit-mode {:draw \| :move}` — one global sub-mode | No sub-mode; behavior = (active tool, hit-test, modifiers) per frame | Drive behavior from `(active-tool, hover-target, modifiers, path-state)` resolved per pointer-move. Force `:move` whenever the white arrow is active (never `:draw`); `:draw` is reserved for the Pen's own internal extend. |
| 3 | Phantom hover-point (`is-new`) on the segment → click = add node, always on | Add-anchor is a **Pen** affordance (glyph +), and only when not actively gated; the white arrow must NOT add nodes | Show the phantom add-point **only for the Pen tool** (and only in its add branch). Hide it entirely for Direct Selection. |
| 4 | Global UNDO is a no-op while `:edition` is set | Every edit (anchor move, handle drag, add/delete) is one undoable step, always | Decouple undo from `:edition`. Either (a) clear `:edition` at commit boundaries so undo runs, or (b) lift the undo guard so it ignores our internal `:edition` flag. Undo must work mid-edit. |
| 5 | Pen self-arms a draw on tool-select; hover frozen during active draw | Pen starts a *new* path only on empty-canvas click; over geometry it shows add/delete/continue; hover never frozen except mid-segment-rubber-band | Don't auto-arm a committed draw on Pen select. Arm only the *resolution pass*; commit a new path on the first empty-canvas mouse-down. Keep hover-resolution live; freeze only the rubber-band preview during an in-progress segment drag. |
| 6 | Entry via double-click (`start-editing-selected`) | Tool change (keypress) is the only transition | Tool buttons + shortcuts (`A`, `P`, `V`) are the entry. Double-click on a path may *also* activate the white arrow as a convenience, but it is not the gate. |

### B.2 The new state model

Introduce a **modeless interaction state** that lives in `:workspace-local` beside Penpot's existing keys, and treat the old keys as derived/internal.

```clojure
:workspace-local
  :active-tool          ; #{:select :direct-select :pen :pen-add :pen-delete :convert ...}
  :last-selection-tool  ; #{:select :direct-select} — for Ctrl/Cmd borrow
  :anchor-selection     ; THE NEW SOURCE OF TRUTH — a set, independent of object selection:
                        ;   #{ {:shape-id ID :index i} ... }  ; cross-object by construction
  :handle-selection     ; optional: {:shape-id :index :which #{:in :out}}
  :hover-target         ; resolved per pointer-move:
                        ;   {:kind #{:empty :segment :anchor :handle :endpoint :start-anchor}
                        ;    :shape-id :index :t (param along segment) ...}
  :modifiers            ; {:cmd? :alt? :shift?} — transient, recomputed per frame
  :pen                  ; {:active-path-id :live-endpoint :rubber-band ...} when Pen drawing
```

Key design points:

- **`:anchor-selection` is independent of object selection and is a SET keyed by `{shape-id, index}`.** This single change is what unlocks Illustrator's cross-object editing: a marquee with the white arrow fills this set with anchors from *any number of shapes*; a drag translates all of them by the same delta. There is no "the one shape in `:edition`."
- **`:edition` is demoted to an internal render scope.** When `:anchor-selection` is non-empty, we set whatever Penpot's renderer needs (it may want each touched shape flagged) so the anchor/handle overlay draws — but we never *show* the user an isolation chrome, never dim other shapes, never require entry. If the renderer can only flag one shape at a time, we flag the shape(s) under the current interaction and accept that the *overlay* may currently draw for the active shape while selection data spans many; the data model is already correct and the overlay can be widened incrementally (see B.4 step 6).
- **Behavior is a pure function of `(active-tool, hover-target, modifiers, pen-state)` recomputed per pointer-move** — exactly the Illustrator resolution pass. No `:edit-mode` branch decides meaning; the tool does.

### B.3 The three tools, defined against Penpot

**White arrow — Direct Selection (`:direct-select`, shortcut `A`):**
- On pointer-move: resolve `:hover-target` (anchor / segment / handle / empty) and highlight it. Phantom add-point hidden.
- Click anchor → set `:anchor-selection` to just that `{shape,index}` (Shift-click toggles membership). Render it **filled**, siblings **hollow**.
- Click segment → select the segment's two endpoint anchors as the active path's visible set (all anchors of that shape hollow; show the segment's endpoint handles). Drag segment → reshape via endpoint handles (curved) or translate (straight), reusing Penpot's curve math.
- Marquee from empty → fill `:anchor-selection` with **all anchors inside the rect across all shapes**.
- Drag a selected anchor → translate **all** anchors in `:anchor-selection` by the delta (mutate each shape's `:content` `move-to`/`curve-to` point + ride its handles). One undo step on release.
- Drag a handle → move **only that one** `c1`/`c2`. **Do NOT couple the partner. Do NOT require Alt.** (A.5 correction — this is exactly the bug that mode-patching produced; the white arrow breaks tangents freely.)
- Always force Penpot `:edit-mode = :move`; never `:draw`. Force `path-drawing? = false` so activation isn't blocked (we already do this — keep it, but now as part of a coherent model, not a patch).
- Alt-drag a path → duplicate the whole shape (Illustrator parity; optional, later).
- Arrow keys → nudge `:anchor-selection` by keyboard increment (Shift = 10×).

**Pen — unified (`:pen`, shortcut `P`):**
- On select, **do not auto-arm a committed draw.** Arm only the resolution pass.
- Per pointer-move resolve the armed branch by the A.6 priority list and set the **cursor glyph**:
  - empty + no live path → start; empty + live path → extend (rubber-band preview);
  - over segment → **add** (this is where the phantom hover-point is shown — *only here*);
  - over anchor → **delete + reflow**;
  - over endpoint of selected open path → **continue**;
  - over start anchor of live path → **close**;
  - Alt over anchor → **convert** (smooth↔corner, break/pull handles — coupling lives here);
  - Cmd/Ctrl → temporarily borrow `:last-selection-tool`.
- Click = corner, click-drag = smooth (collinear+symmetric handles). Shift = 45°. Space mid-drag = reposition. Reuse Penpot's bezier insert/split for add (De Casteljau preserve-shape) and its node-remove + segment re-fit for delete.

**Black arrow — Selection (`:select`, shortcut `V`):**
- Whole-object/group selection + bounding-box transform (Penpot already does this well; leave it). Marquee = touch-intersect, whole objects. Clears `:anchor-selection` on activation-by-interaction-on-empty, but tool *switching* preserves whatever object selection exists.

**Mutual exclusivity & borrow:** tools are mutually exclusive (one `:active-tool`); track `:last-selection-tool` so Cmd/Ctrl borrows the right arrow and springs back on release (hold-to-borrow ≠ tap-to-latch).

### B.4 The phantom add-point, `:draw`/`:move`, and the undo constraint — explicit dispositions

- **Phantom add-point (`is-new` / `create-node-at-position`):** keep the machinery, **scope its visibility to the Pen's add branch only.** Under `:direct-select` it must be fully hidden (this is precisely why our white arrow wrongly added nodes). Its click handler stays wired to Penpot's `create-node-at-position`.
- **`:edit-mode :draw` vs `:move`:** `:draw` is Penpot's "click adds a node" sub-mode — it belongs ONLY to the Pen's internal extend, never to selection. Force `:move` whenever `:direct-select` or `:select` is active. Drive *add/delete/convert* through the **Pen's resolution branches**, not through a global `:edit-mode` flag.
- **Undo blocked while `:edition`:** this must be fixed at the source, not worked around. Two acceptable bridges: **(a)** lift the undo guard so it ignores our *internal* `:edition` render-flag (preferred — keeps `:edition` purely a render concern), or **(b)** clear `:edition` at every commit boundary (on mouse-up) so undo runs, re-deriving it from `:anchor-selection` on the next interaction. Either way, **every anchor move / handle drag / add / delete must be exactly one undoable step, available immediately**, with no `:edition` left set that silently no-ops undo.

### B.5 Build sequence — small, independently browser-testable steps

Each step is shippable and verified in a **real browser** (per our standing rule: verify in Ahmed's actual SVG-renderer browser on a release build, not headless; the looks-broken-mid-transformation caveat applies). Order is dependency-driven; each builds on the last.

**Step 0 — Introduce `:anchor-selection` as data only (no behavior change).**
Add the set to `:workspace-local`; populate it in parallel wherever the old single-shape edit selects an anchor. Render nothing new yet. *Test:* existing editing still works; inspect app-db and confirm `:anchor-selection` mirrors the old selection. *This is the spine; everything hangs off it.*

**Step 1 — White-arrow handle drag breaks tangents freely.**
In the Direct Selection handle-drag path, move only the dragged `c1`/`c2`; remove any partner-coupling and any Alt-to-break requirement. *Test:* drag one handle of a smooth node → opposite handle does not move; curve on the other side unchanged. (Fixes the single biggest fidelity bug.)

**Step 2 — Hide the phantom add-point under Direct Selection; force `:move`.**
Scope `is-new` rendering + its click to Pen only. Confirm `:edit-mode` is forced `:move` and `path-drawing? false` under `:direct-select`. *Test:* with white arrow, clicking a segment selects/reshapes — it never adds a node.

**Step 3 — Decouple undo from `:edition`.**
Apply B.4 bridge (a) or (b). *Test:* move an anchor, press Ctrl/Z → it reverts, mid-edit, every time.

**Step 4 — Single-anchor selection + filled/hollow rendering driven by `:anchor-selection`.**
Render selected anchors filled, others hollow, from the new set (not from `:edition`). *Test:* click anchors on a path → exactly the clicked one is filled; clicking another moves the fill; Shift-click adds.

**Step 5 — Anchor drag translates the whole `:anchor-selection`, one undo step.**
Press on a selected anchor → all selected anchors move by the same delta; press on an unselected anchor → re-select just it. *Test:* select 3 anchors (Shift-click), drag one → all 3 move together; undo reverts in one step.

**Step 6 — Cross-object marquee.**
Marquee from empty canvas fills `:anchor-selection` from **all shapes** under the rect; widen the overlay so anchors of every touched shape render. (If Penpot's overlay only draws the `:edition` shape, generalize the overlay to iterate the distinct `shape-id`s in `:anchor-selection`.) *Test:* two separate paths, marquee across both → anchors on both turn filled; drag → both move together. **This is the headline Illustrator capability.**

**Step 7 — Tool switching preserves selection; no double-click gate.**
Pressing `V`/`A`/`P` never clears `:anchor-selection` or object selection by mere activation. Remove double-click as a *requirement* (keep it as an optional convenience that activates the white arrow). *Test:* draw a path, press `A`, immediately grab an anchor — no re-click, no "enter." Press `V`, drag whole shape. Press `A`, anchors still there.

**Step 8 — Pen resolution pass + cursor glyphs.**
Implement the A.6 priority resolution and per-branch glyphs; show the phantom add-point only in the add branch. Don't auto-arm a committed draw on select; commit a new path on first empty-canvas down. *Test:* with Pen, hover empty (×/start), hover segment (+/add), hover anchor (−/delete), hover selected-open endpoint (/, continue), hover start anchor of live path (○/close) — each glyph and action correct. Bare-click an anchor = delete (not convert).

**Step 9 — Spring-loaded borrow (Ctrl/Cmd) + Alt convert.**
Hold Cmd/Ctrl in Pen → borrow `:last-selection-tool`, adjust, release → back to Pen with path still live. Alt over an anchor → convert (coupling/break here, the one place coupling lives). *Test:* mid-draw, Cmd-drag a just-placed handle, release, keep drawing — path never breaks.

**Step 10 — Arrow-key nudge + Shift-constrain + Alt-duplicate.**
Nudge `:anchor-selection` by increment (Shift 10×); Shift constrains drags to 45°; Alt-drag a path duplicates. *Test:* select anchors, arrow-nudge; Shift-drag constrains; Alt-drag copies.

After Step 7 we already *feel* like Illustrator for single-object editing; after Step 6 we have the cross-object moat; Steps 8–10 complete Pen parity.

### B.6 Explicit non-goals / guardrails (so we don't re-introduce the fight)

- Do **not** surface `:edition` as user state, dim other shapes, or require entry. It is render scope only.
- Do **not** couple smooth handles on the white arrow, and do **not** ship `+`/`−`/`Shift+~`/Curvature as working default shortcuts (no Illustrator default exists; shipping them claims false parity).
- Do **not** let any global `:edit-mode :draw` leak into selection tools.
- Do **not** block undo on any internal flag.
- Keep `:anchor-selection` the single source of truth; render and `:edition` derive from it, never the reverse.

---

## PART C — ملخّص بسيط للمصمّم (بالعامية المصري)

دي خلاصة اللي إحنا فاهمينه عن إزاي إليستريتور بيشتغل، واللي لازم نعمله زيّه بالظبط — من غير كلام تقني، عشان تأكد لنا إننا فهمنا الهدف صح:

- **الأداة هي اللي بتحدد بتعدّل إيه — مفيش "وضع تعديل" تدخله.** السهم الأسود (V) بيمسك الشكل كله، والسهم الأبيض (A) بيمسك النقط نفسها. مفيش دبل-كليك تدخل بيه جوّه الشكل الأول — أول ما تاخد السهم الأبيض، أي نقطة في أي مسار تقدر تمسكها على طول.

- **السهم الأبيض بيعدّل نقط أي شكل، في أي وقت، حتى من غير ما تفتح الجروب.** ودي أهم حاجة: تقدر تختار نقط من كذا شكل مختلف مع بعض وتحرّكهم سوا بحركة واحدة. ده اللي بنميزنا، والبرامج التانية (زي فيجما) مش بتعمله لأنها بتجبرك تدخل "وضع تعديل" لشكل واحد بس.

- **شكل النقطة بيقولك هي متظللة ولا لأ:** النقطة **المليانة** = مختارة، النقطة **الفاضية** = ظاهرة بس مش مختارة. ده العلامة اللي المصمم بيقرأها بسرعة، ولازم نحافظ عليها بالظبط.

- **لمّا تشدّ "إيد" (هاندل) بالسهم الأبيض، الإيد التانية متتحركش معاها.** ده فرق مهم: السهم الأبيض بيكسر النعومة بحرية، من غير ما تضغط Alt. (النسخة القديمة عندنا كانت غلط وبتحرّك الاتنين سوا — هنصلحها.)

- **قلم الرسم (Pen) أداة واحدة بتعرف لوحدها هتعمل إيه حسب مكان الماوس:** فوق فراغ = ابدأ مسار، فوق خط = ضيف نقطة، فوق نقطة موجودة = امسح نقطة، فوق نهاية مسار = كمّله، فوق أول نقطة = اقفل الشكل. وشكل المؤشر بيتغيّر عشان يقولك هيعمل إيه قبل ما تدوس.

- **مشكلتنا الأساسية:** بنبوت (Penpot) متبني على فكرة إنك "تدخل" شكل واحد عشان تعدّله (زي العزل/الـ isolation)، وده عكس إليستريتور تماماً. كل ما نحاول نرقّع النظام القديم بيطلع باجات في التفاعل.

- **الحل:** نسيب كل الحاجات الحلوة في بنبوت (الرياضة بتاعة المنحنيات، الرسم، التراجُع/الـ undo) زي ما هي، ونغيّر بس **طبقة التفاعل** — نخلّيها أداة-بأداة زي إليستريتور بدل الأوضاع. أهم خطوة إننا نخلّي "اختيار النقط" حاجة مستقلة بذاتها تقدر تشيل نقط من كذا شكل، بدل ما تكون مربوطة بشكل واحد متعزّل.

- **هننفّذها خطوة خطوة صغيرة، وكل خطوة بتتجرّب في المتصفح الحقيقي على طول:** الأول نصلّح شدّ الإيد، نخفي نقطة الزيادة الوهمية مع السهم الأبيض، نظبّط التراجُع، نظبّط المليان/الفاضي، بعدين نحرّك أكتر من نقطة سوا، بعدين أهم خطوة = نختار نقط من كذا شكل مع بعض، وأخيراً نكمّل القلم بكل حالاته.

- **الحاجات اللي هنوعد بيها بس مش حقيقية في إليستريتور هنبعد عنها:** يعني مفيش اختصارات كيبورد افتراضية لأدوات "ضيف/امسح نقطة" ولا أداة المنحنى — إليستريتور نفسه مالوش، فمش هندّعي حاجة مش موجودة.

- **الخلاصة:** الهدف إن المصمم يحس إنه بيشتغل في إليستريتور بالظبط — يمسك أي نقطة في أي شكل في أي وقت، من غير ما "يدخل" أو "يخرج" من أي حاجة، والأداة اللي في إيده هي اللي بتحدد كل حاجة.

---

*Document ends. This is the build spec — Part B B.5 is the ordered, browser-verifiable work plan; B.1 is the bridge table for every place Penpot fights Illustrator; B.6 are the guardrails that keep us from sliding back into the mode-based fight.*
