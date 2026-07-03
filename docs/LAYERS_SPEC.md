# Varos — Layers / Structure SPEC (the D2 model system)

The real scene-graph. Follows DETAILED_ROADMAP §Layers (points 1–21) — this spec locks the load-bearing
engineering decisions + the staged build; the roadmap stays the feature source of truth.

**Locked upstream:** [D2] Layers is a MODEL system, not a panel — build the tree, migrate selection/
render/tools, THEN the panel. 🔒 Ahmed (07-02, from Affinity): dragging a row ONTO another row = it
clips/nests INSIDE it. Illustrator-class rows: EVERY object appears as a leaf row; layers = top-level
containers; sublayers nest without limit; top of list = front of canvas.

**Current baseline (verified in tree):** `Document { paths: Vec<Path> (flat, vec order IS z),
groups: Vec<Group> + group_of: HashMap<u32,u32> (registry side-table, nests via parent), … }`.
Renderer builds z-ordered Groups from the flat order; `hidden/locked` already live per-Path; arrange/
group/undo/serde all assume the flat list.

---

## 1. The one load-bearing decision — how the tree lives

**An ORDERED NODE TREE over the flat path storage — `Path` structs and their construction sites stay
untouched.** `Vec<Path>` becomes pure storage (id → geometry/appearance); a new tree owns STRUCTURE:

```rust
pub struct Node { pub id: u32, pub kind: NodeKind, pub children: Vec<u32> /* node ids, front-first */ }
pub enum NodeKind { Layer { name, color, … }, Group, Clip /*Stage C*/, Path(u32 /*path id*/) }
// Document += tree: Vec<Node> (arena) + roots: Vec<u32> (the Layers, top-first) — serde #[default];
// a legacy file with an empty tree wraps ALL paths into "Layer 1" in flat order on load. group_of/groups
// migrate into the tree and retire.
```

- **Z-order = pre-order traversal** of the tree (replaces "vec order = z"): one `doc.z_iter()` feeds
  scene build, hit-testing (reverse), arrange (within the parent's children), export.
- Eye/lock live on nodes too; **effective state = own ∨ any ancestor** (cascade), Path.hidden/locked
  become the leaf case. Selection keeps `objsel` (path ids) + gains structural selection (node ids).
- Why not a full recursive enum owning Paths: every tool, test and serde site iterates `doc.paths`
  today; moving storage into the tree is a big-bang rewrite with zero user-visible gain. The arena
  keeps undo cheap (Document clone stays shallow-ish) and the panel is a plain indented VIEW of it.

## 2. Product decisions — LOCKED (Ahmed, 2026-07-03, voice review vs the Illustrator panel)

1. **Panel home (TEMPORARY): under the Properties dock on the right.** The final home comes with the
   panel-arrange system later — don't over-invest in placement now.
2. **New objects land on the ACTIVE layer** (Illustrator standard — he defers to the standard).
3. **NO delete-confirmation dialogs** — undo covers it ("في undo فممكن ترجع عادي"). The last layer
   still can't be deleted (engineering floor, silent).
4. **Selection/highlight = the brand azure accent everywhere.** Look = STANDARD and restrained —
   explicitly: not too many rounded corners; the real look pass is a later conversation.
5. **Illustrator behaviour is the default answer to every rule** (incl. cross-layer grouping pulls
   to the top-most member's layer).

**Ahmed's must-have panel features (from his AI screenshot — all "مهم جدًا"):** Search + filter row ·
per-row THUMBNAILS (vector-drawn mini previews, no textures needed) · per-layer colour strip on rows ·
eye + LOCK columns from day one (layer AND object level) · the right-side TARGET/selection column —
clicking the row highlights the row, clicking the selection column SELECTS the artwork on canvas (two
different acts, both needed) · footer buttons (new layer, new sublayer, delete; export-related slot
later) · the "N Layers" counter bottom-left · inline rename.

## 3. Staged build (one excellent piece at a time)

- **Stage A — the tree (model only, no UI):** Node arena + roots in Document (serde-default +
  legacy-wrap migration); `z_iter()` replaces flat-order everywhere (scene, path_under, arrange,
  group/ungroup re-parent in-tree, delete); eye/lock cascade; headless tests (migration, z, arrange
  scope, cascade, undo). The canvas must look/behave EXACTLY as before with one implicit "Layer 1".
- **Stage B — the panel MVP:** hand-painted left panel — rows (disclosure ▸, name, eye, lock),
  indentation, auto-names (`<Path>`, `<Group>`, `<Rectangle>`…), double-click rename, two-way
  selection sync, footer (+ layer, 🗑), active-layer highlight, new-objects-to-active-layer.
- **Stage C — drag-drop restructuring:** drag rows to reorder (insertion line) / BETWEEN = sibling,
  **ONTO = nest inside (Ahmed's gesture — into group/layer; onto a leaf = clip, Stage D)**,
  cross-layer re-parent, multi-select drag later.
- **Stage D — masks & compound:** Clipping Mask (Ctrl+7 / release Alt+Ctrl+7) as `Clip` nodes +
  drop-onto-leaf = clip; Compound Path (Ctrl+8) rides the existing even-odd/holes engine.
- **Stage E — depth:** isolation mode (double-click + breadcrumb), thumbnails, target dot,
  Select-Same, locate/search, merge/flatten, paste-remembers-layers (roadmap 13–20).

## 4. Acceptance feel (Stage A+B together)

Open Varos → one "Layer 1" holding everything, canvas identical to today. Draw → a `<Rectangle>` row
appears under the active layer, top of its stack. Click a row → selects on canvas and back. Eye hides
(and its children), lock refuses selection, rename inline. + adds "Layer 2" above; drawing lands there.
Ctrl+G shows a `<Group>` row wrapping its children. Undo covers every structural change. Save → reopen:
the whole tree returns.
