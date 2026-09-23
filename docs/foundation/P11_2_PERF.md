> **Status:** current — P11.2 items (1) and (2) design, measurement and implementation evidence, governed by `FOUNDATION_CHARTER.md`.
# P11.2 Performance Evidence — view culling, view clipping, flatten cache

Scope: work order P11.2 items **(1)** viewport culling at path level plus ring/edge clipping to the
view rect (owner symptom (d), 4000% zoom), and **(2)** a cross-frame flatten cache with zoom buckets.
Item (0), instant zoom, lives in the app crate and is a separate piece — **not done here**.

## Design (decided 2026-09-23)

### (1) Path-level culling plus ring-level view clipping

`scene::build_scene_in_view(ed, view, frame)` is what the app now calls. It computes the world rect the
frame shows and grows it by `VIEW_PAD_PX` = 32 screen px (converted to world units).

- **Cull test, per path:** does the path's control-point bbox (anchors, handles and hole anchors, through
  the unit `Xform`) overlap the view rect grown by half the stroke width? A cubic always stays inside its
  control points, and a rotation keeps that true, so this box contains every flattened point at every
  zoom. We never flatten the curve just to find out whether it is off screen.
  A culled path gets no geometry. The fill, stroke, mask, skeleton and snap highlight all skip it, the
  same way they already skip a hidden path. It is skipped **before** clip-group tracking, so the visible
  members of a clip group still form one run.
- **Ring and edge clipping:** a path that is only partly inside the rect has its fill rings, stroke runs,
  mask rings, skeleton and snap-highlight outline cut to that rect. The cut uses the artboard clippers
  that were already there (`clip_poly_rect` and `clip_polyline_rect`); no new geometry code was written.
  Sutherland–Hodgman against a convex rect keeps the winding number of every point inside the rect, and
  the rect is bigger than the frame. So the *geometry* of even-odd fills, holes and mask silhouettes is
  unchanged on screen. (Tests pin this for fills, holes and strokes; for masks, only the empty-ring case
  is pinned. See Known limits for how the renderer draws inside masks.)
  Stroke runs are cut at least half a stroke width plus 32 px beyond the frame, so the new end caps
  never show on screen.
- **Artboard clip together with the view:** the geometry is cut once, to the page rect intersected with
  the view rect. A clipped stroke keeps the **page** rect as its GPU scissor (the A2 contract does not
  change).
- **Handles and markers:** a handle line is kept if any part of it reaches the padded frame. A handle
  disc or an anchor marker is kept if its centre does; every such mark is 6 px or smaller, well under the
  32 px pad. Culling never changes *which* anchors show handles.
- **A path wholly inside the frame** is not clipped at all, so its primitives are byte-identical to
  `build_scene`. A test pins this.
- **Fail-open:** an empty frame or an invalid zoom turns culling off, and everything is drawn.

- **The treatment never depends on the view (review fix P1-2, 2026-09-23).** Whether an object becomes
  an isolated layer, a knockout, a folded-alpha prim or plain opaque is decided from its **uncut** fill
  and stroke. View clipping only ever removes geometry, so when a cut side comes out empty, the scene
  re-checks whether the uncut side exists. Before this fix, a 50%-opacity filled and stroked object whose
  stroke lay entirely in the off-screen margin flipped from isolated to folded, and back again when a pan
  of a few pixels brought the stroke into the pad. An object whose cut fill and cut stroke are both empty
  emits nothing.
- **Object boundaries survive culling (review fix P1-1, 2026-09-23).** The renderer paints consecutive
  translucent strokes of one colour with a single stencil coverage, so one object's outer and hole rings
  paint once. A run holds many objects, though. When the object between two same-colour translucent
  strokes was culled, the two strokes became adjacent and merged, and their crossing painted once (50%)
  instead of twice (75%). The scene now closes the opaque run at an object boundary whenever the next
  object's first prim would merge with the previous object's last one. Coverage batching therefore never
  crosses objects, whether the separator was culled, hidden or never there. **This also changes one
  pre-existing case:** two *adjacent* same-colour translucent strokes of different objects used to merge
  even without culling. They now overlap as two objects, which is the per-object behaviour the renderer
  comment always described.

Why a bbox check plus the existing clippers, rather than something smarter? The bbox check is
conservative, costs O(anchors) per path, and is cached along with the geometry. The clippers already
carry the artboard-clip contract and its tests, so reusing them adds no new geometry code that could be
wrong.

### (2) Cross-frame flatten cache (`varos-core/src/flatten.rs`)

- **Key:** path id, plus the exact geometry inputs, plus the zoom bucket. There is no per-path revision
  counter in the model, and `Editor::rev` is **not** a safe key: a live drag or preview moves anchors
  before any commit bumps `rev`. So the cache stores a copy of the inputs the flatten actually reads
  (outer anchors, hole anchors, `closed`, and the unit `Xform`) and compares them by value on every
  lookup. That comparison is O(anchors), much cheaper than the flatten it saves (up to 256 cubic
  evaluations per segment). It cannot go stale, because any change to any input is a miss.
- **Invalidation rule:** an entry is rebuilt whenever any geometry input differs, or the zoom bucket
  differs. Paint-only edits (fill, stroke colour, opacity) reuse the cached flatten. Entries for deleted
  paths are evicted (a length check each frame; the evicting pass only runs after a delete). Culled paths
  are never flattened.
- **Zoom buckets:** quarter octaves. Geometry is flattened at the bucket's **upper** edge
  (`bucket_ppu`), so a cached flatten is never coarser than an exact-zoom flatten and at most 19% finer.
  Whole-octave zooms (0.5, 1, 2, 4 …) are their own representatives, so their geometry is unchanged.
- **Where the cache lives:** `Editor::flatten_cache`, a `Mutex<FlattenCache>`. The mutex is locked once
  per scene build and never contended, and it keeps `Editor` `Send + Sync`. The cache is never
  serialized and is not part of undo. A poisoned lock clears the cache; a cold rebuild is always correct.
- Each path pays **one** `unit_xform` lookup per frame, the same as P11.1. A first version paid three and
  measurably slowed scene B; that was fixed before commit.

## Before / after (measured)

Harness: `cargo run -p varos-render-wgpu --release --example perf_harness -j 4` from `varos/`. The
harness gained scene **D** (the selected curved-150 path at ppu 40 = 4000%, view centred on its right
edge) and scene **E** (scene B's 500 rectangles at 400%, one corner visible). It also gained overlay
tessellation timing and vertex counts (commit `3acbeaf`). The "before" binary was built from `3acbeaf`,
which is the P11.1 core plus the new scenes. "After" is the branch tip. **cold** = one full CPU canvas
frame (scene build, content tessellation and overlay tessellation) with an empty flatten cache.
**warm** = the same frame with the cache populated; this is every frame where the scene signature missed
but the geometry did not change (pan, hover, selection change, an edit to another path). "Before" had no
cache, so its cold and warm are the same.

Machine: Apple M5, macOS, release build, 2026-09-23. Other agents were compiling on the same machine
(load average about 3), so the two binaries were run **alternately, 9 rounds each**, 15 iterations per
round. Figures are the median of the 9 per-round medians. These numbers are **not** comparable with the
P11.1 Windows table.

| Scene | before cold | after cold | after warm | fill / fg / overlay vertices before | after |
|---|---:|---:|---:|---:|---:|
| A selected curved-150, ppu 3 | 0.080 ms | 0.088 ms | 0.085 ms | 3,609 / 7,344 / 41,544 | 3,078 / 6,420 / 35,460 |
| B rectangles-500, ppu 0.3 (all visible) | 1.997 ms | 1.987 ms | 1.951 ms | 10,500 / 12,000 / 0 | 10,500 / 12,000 / 0 |
| C curves-100, ppu 1 (all visible) | 0.506 ms | 0.520 ms | 0.476 ms | 29,700 / 72,000 / 0 | 29,700 / 72,000 / 0 |
| **D selected curved-150, ppu 40 (4000%)** | 0.367 ms | **0.109 ms** | **0.085 ms** | 32,409 / 64,944 / 99,144 | **1,068 / 2,388 / 3,384** |
| **E rectangles-500, ppu 4 (partial)** | 2.435 ms | **1.213 ms** | 1.196 ms | 10,500 / 192,000 / 0 | **2,835 / 51,624 / 0** |

Scene-build time only (the core's share):

| Scene | before | after cold | after warm |
|---|---:|---:|---:|
| A | 0.022 ms | 0.036 ms | 0.033 ms |
| B | 1.851 ms | 1.830 ms | 1.793 ms |
| C | 0.250 ms | 0.262 ms | 0.218 ms |
| D | 0.042 ms | 0.095 ms | 0.071 ms |
| E | 1.814 ms | 1.089 ms | 1.069 ms |

What the numbers say:

- **Symptom (d), 4000%:** scene D emits **30× fewer fill vertices, 27× fewer stroke vertices and 29×
  fewer overlay vertices**, and the CPU frame is 3.4× faster (0.367 → 0.109 ms cold). The fill fan no
  longer carries thousands of off-screen edges, and that fan was the GPU overdraw the owner reported.
  This harness is CPU-only. **The GPU fill-rate gain is inferred from the vertex counts, not measured.**
  Confirming it needs the owner's hand test in the real window.
- **Many objects, partly visible (E):** CPU frame 2.0× faster, stroke vertices 3.7× fewer.
- **Everything visible (B, C):** culling cannot help here, and the table shows it: B and C are flat
  within noise when cold. The warm cache saves 0.04 ms of scene build on C (0.262 → 0.218). Scene B's
  remaining ~1.8 ms is **not** flattening. It is O(n²) per-path document lookups (`eff_hidden`,
  `unit_xform`, `clip_group_of` each scan the path or node lists linearly). That is outside this piece.
- **The cache's gain is small on these scenes**, because P11.1 already made flattening cheap: warm
  saves 4–25% of scene build. Its value grows with curve count and zoom, since steps per segment scale
  with ppu up to 256.
- **After the review fixes** (re-measured 2026-09-23 with the same alternating method, 9 rounds × 2 runs).
  The machine was under heavy load from other agents (load average about 10 against about 3 above), so
  every absolute time is roughly 2.5–3× the table above and only the ratios are meaningful. D cold went
  1.020 → 0.305 ms (3.3×) and 1.277 → 0.358 ms (3.6×). E cold went 6.384 → 3.150 ms and 7.990 → 3.990 ms
  (2.0× both). Vertex counts are **identical** to the table: D 1,068 / 2,388 / 3,384 and E 2,835 / 51,624.
  So the fixes did not give back any of the win.
- **Small cold costs, reported honestly:** A's cold frame rose 0.008 ms (+10%), and D's scene build rose
  from 0.042 to 0.095 ms while its whole frame fell 3.4×. Cold now also pays for the bbox, a copy of the
  anchors, clipping, and a flatten up to 19% finer (bucket upper edge).

## Verification

| Gate | Result |
|---|---|
| `cargo test -p varos-core -p varos-pdf -p varos-render-wgpu -j 4` (macOS) | PASS: 34 suites, 237 passed, 0 failed (includes the 16 new tests in `varos-core/tests/view_cull.rs` and 1 new CPU test in `tess.rs`) |
| `cargo clippy -p varos-core -p varos-pdf -p varos-render-wgpu --all-targets -j 4 -- -D warnings` (macOS) | PASS |
| `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -j 4 -- -D warnings` | PASS (type-checks and lints `varos-app` and its tests; nothing executed) |
| `cargo fmt --all -- --check` | PASS |

**`varos-app` does not build on macOS** (the `windows` crate fails with E0425 on this host), so the
workspace-wide `cargo test` cannot run here. The app's own tests, including `scene_signature_tests`,
were **compiled** for the Windows target but **not executed**. They must be run on Windows.

The new tests pin the following:
- **(a) Culling.** A path wholly outside the view is culled, while a partly visible one is kept and cut
  at the grown rect. For a 4000%-style compound shape with a hole, the even-odd fill coverage and the
  stroke-band distance of every in-frame sample point are identical before and after clipping. A view
  that shows everything gives exactly the scene `build_scene` gives. A page clip and the view clip
  compose correctly, and strokes keep the page scissor. An off-screen clip mask contributes no ring: the
  scene emits empty `mask_rings`. That is all the test asserts. It does **not** mean every member draws
  nowhere: a visible translucent-stroke or knockout member can still draw, because those renderer
  branches ignore the clip flag (pre-existing, see Known limits 2). This is the **only** mask case
  pinned; a partly visible mask's ring cut is covered by the winding argument, not by a test.
- **Review P1-1.** Two crossing 50%-red strokes with an off-screen opaque rectangle between them form
  two coverage batches both uncut and culled. This is pinned twice: in the core by the batching rule,
  and in `tess.rs` by counting real `Draw::StrokeCov` steps from `build_content`, CPU-only. Two adjacent
  same-colour strokes of different objects stay two batches, and one object's outer and hole rings stay
  one.
- **Review P1-2.** The review's masked 50%-opacity rectangle has its whole stroke cut away at pan 0 and
  one stroke run inside the pad at pan 70. It is `Isolated` in both, exactly as uncut. The same is
  checked without a mask at pans 0, 35 and 70, and a knockout object stays a knockout when its band is
  cut away.
- **These tests detect the bugs.** Run against the pre-fix `scene.rs`, all 4 new core tests and the new
  `tess.rs` test fail; with the fix they pass.
- **(b) Handles and markers.** Every handle disc, anchor marker and handle line of a selected path that
  lies in the frame survives culling. A handle that reaches into view from an off-screen anchor is kept.
- **(c) Flatten cache.** The cache equals a fresh `world_outline_px` flatten. It hits on the same zoom
  bucket and misses on a new one. It misses on an anchor move, a handle move, an added hole, an
  open/close, or a unit rotation, and it hits on a paint-only change. A live edit with **no** `rev` bump
  never draws stale geometry. Culled paths cost no flatten, and deleted paths are evicted.
- **Mutation check.** With the cache's input comparison forced to "always equal", the two invalidation
  tests fail. So the tests really do detect stale geometry.

## Known limits

- **Pre-existing renderer gaps inside clipping masks (found in review, NOT fixed here, logged
  2026-09-23).** These are not caused by P11.2, which now only guarantees that each object's
  **treatment** (isolated, knockout, folded alpha or plain opaque) does not change with the view. The
  scene geometry itself necessarily changes with the view, since culling and clipping are the point.
  1. **Group opacity is lost inside a mask.** `tess.rs` `build_content` builds a `Group::Clip`'s members
     with `group_draws`, which never applies an `Isolated` member's opacity. A 50%-opacity filled and
     stroked object inside a mask therefore renders fully opaque. Fixing it needs a masked offscreen
     layer: new GPU passes that must be hand-tested in the real window.
  2. **Some draws ignore the mask entirely.** In `lib.rs` `draw_steps`, only `Draw::Fill` and `Draw::Fg`
     honour the `clip` flag. `Draw::StrokeCov` (translucent strokes) and `Draw::Knockout` (fill with a
     translucent stroke) ignore it, so inside a mask they are not cut to the silhouette.
- **Known cost of the P1-1 fix (structural, not yet measured).** Because coverage batching no longer
  crosses object boundaries, 500 consecutive unfilled same-colour translucent rectangles now produce 500
  `StrokeCov` batches (1,000 draw calls) instead of 1. The harness has no such scene yet. Follow-up
  measurement item: **scene F — many same-colour translucent strokes**, before and after, CPU and draw
  count.
- **GPU cost is not measured.** Headless numbers cover CPU time and vertex counts only; the 4000% win on
  fill-rate is an inference that still needs the owner's hand test.
- **The frame size comes from `window.inner_size()`**, the same value the scene signature uses. If the
  renderer's surface ever lagged a resize by a frame, content in the extra strip past the old size plus
  32 px could be missing for that one frame. The next frame rebuilds, because the size is part of the
  signature.
- **Partly visible paths are still flattened whole** before clipping. At extreme zoom, a single huge
  path pays for its full flatten on a cold frame or a bucket change; the cache then removes that cost
  while panning. Segment-level culling inside the flattener was not done.
- **`build_scene` (uncut) now flattens at the bucket ppu too**, so at non-octave zooms its geometry is
  up to 19% finer than before. No existing test depended on this (all 220 pre-existing tests pass
  unmodified). Hit-testing still flattens at the exact ppu; the difference is sub-pixel.
- **Memory:** the cache keeps one copy of each path's anchors and one flattened geometry at one zoom
  bucket. This is bounded by the document, but it roughly doubles geometry memory.
- **Zooming back and forth across a bucket boundary re-flattens.** Only one bucket is kept per path.
- **Scene B's O(n²) lookups** (`eff_hidden`, `unit_xform` and friends scanning lists linearly) are
  untouched. They are the next measurable cost for documents with many objects.
- **`varos-app`** changed only at its two `build_scene` call sites (commit `b5a13b0`, kept separate so it
  rebases onto the macOS port).
