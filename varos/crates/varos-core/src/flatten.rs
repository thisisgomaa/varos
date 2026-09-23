//! P11.2 cross-frame flatten cache (docs/foundation/P11_2_PERF.md).
//!
//! Adaptive curve subdivision (`Document::world_outline_px` / `world_ring_px`) is the expensive part of
//! scene building. P11.1 made it once-per-frame; this makes it once-per-EDIT: a path's flattened world
//! geometry is kept across frames and reused while its geometry inputs and the zoom bucket are unchanged.
//!
//! **Key** = (path id, exact geometry inputs, zoom bucket). The "geometry revision" is not a counter but
//! the inputs themselves — outer anchors, hole anchors, `closed`, and the unit's live `Xform` — compared
//! by value on every lookup. `Editor::rev` alone is NOT a safe key: live gestures (drags, previews) move
//! anchors before any commit bumps it, and there is no per-path revision. Value comparison is O(anchors),
//! a small fraction of the subdivision it saves (up to 256 cubic evaluations per segment), and it cannot
//! go stale: any change to anything the flatten reads is a miss by construction.
//!
//! **Zoom buckets**: the flatten depends on `ppu` (steps per cubic ≈ on-screen chord length). Geometry is
//! flattened at the bucket's representative ppu (`bucket_ppu`), which is the bucket's UPPER edge — so a
//! cached flatten is never coarser than the exact-ppu one (at most 2^(1/4) ≈ 19% finer). Integer
//! octaves (ppu 1, 2, 4, 0.5 …) are their own representatives, so their geometry is unchanged.
//!
//! Pure Rust, no GPU/window deps (the core seam).

use crate::geom::Pt;
use crate::model::{Anchor, Document, Path, Xform};
use std::collections::hash_map::Entry as MapEntry;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

/// An axis-aligned world rect `(x0, y0, x1, y1)`.
pub type Rect = (f32, f32, f32, f32);

/// One path's flattened WORLD geometry: the outer outline and each hole ring.
#[derive(Clone, Debug, PartialEq)]
pub struct PathGeometry {
    pub outline: Vec<Pt>,
    pub holes: Vec<Vec<Pt>>,
}

/// Zoom buckets per doubling of zoom (quarter octaves).
pub const BUCKETS_PER_OCTAVE: i32 = 4;
/// 2^(k/4) for k = 0..3 — exact representatives inside one octave.
const OCTAVE_STEPS: [f32; 4] = [1.0, 1.189_207_1, std::f32::consts::SQRT_2, 1.681_792_8];

/// The zoom bucket of `ppu`: the smallest `b` with `bucket_ppu(b) >= ppu` (up to a 1e-4 octave
/// tolerance, so ppu 1.0000001 stays in bucket 0). A non-finite or non-positive ppu maps to bucket 0.
pub fn zoom_bucket(ppu: f32) -> i32 {
    if !(ppu.is_finite() && ppu > 0.0) {
        return 0;
    }
    (ppu.log2() * BUCKETS_PER_OCTAVE as f32 - 1e-4).ceil() as i32
}

/// The representative (flatten) ppu of a bucket: exactly 2^(b/4), exact at integer octaves.
pub fn bucket_ppu(bucket: i32) -> f32 {
    let octave = bucket.div_euclid(BUCKETS_PER_OCTAVE);
    let step = bucket.rem_euclid(BUCKETS_PER_OCTAVE) as usize;
    2f32.powi(octave) * OCTAVE_STEPS[step]
}

/// A fresh (uncached) flatten of path `pi` for `ppu`, at the bucket's representative ppu, built through
/// the model's own `world_outline_px` / `world_ring_px`. This is the REFERENCE the cache must equal
/// (tests compare them); the cache itself flattens via `flatten_with` so it looks the unit transform up
/// once per path per frame instead of once per ring.
pub fn flatten_path(doc: &Document, pi: usize, ppu: f32) -> PathGeometry {
    let fppu = bucket_ppu(zoom_bucket(ppu));
    PathGeometry {
        outline: doc.world_outline_px(pi, fppu),
        holes: doc.paths[pi].holes.iter().map(|hole| doc.world_ring_px(hole, pi, fppu)).collect(),
    }
}

/// `world_outline_px` / `world_ring_px` with the unit transform already in hand — the same steps in the
/// same order (flatten locally, then map through `xf` unless it is the identity), so the result is
/// bit-identical to `flatten_path`.
fn flatten_with(path: &Path, xf: Xform, fppu: f32) -> PathGeometry {
    let world = |ring: Vec<Pt>| -> Vec<Pt> {
        if xf.is_identity() {
            ring
        } else {
            ring.into_iter().map(|p| xf.apply(p)).collect()
        }
    };
    PathGeometry {
        outline: world(Document::ring_px(&path.anchors, path.closed, fppu)),
        holes: path.holes.iter().map(|hole| world(Document::ring_px(hole, true, fppu))).collect(),
    }
}

/// World-space bounding box of a path's CONTROL points (anchors + handles, outer ring and holes) through
/// its unit transform. A cubic lies inside its control hull and a rigid transform maps hulls to hulls, so
/// this box contains every flattened point at every zoom. Empty path ⇒ an inverted (empty) rect.
pub fn control_bbox(doc: &Document, pi: usize) -> Rect {
    let path = &doc.paths[pi];
    control_bbox_with(path, doc.unit_xform(path.id))
}

fn control_bbox_with(path: &Path, xf: Xform) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for anchor in path.anchors.iter().chain(path.holes.iter().flatten()) {
        for q in [Some(anchor.p), anchor.hin, anchor.hout].into_iter().flatten() {
            let q = xf.apply(q);
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
    }
    (x0, y0, x1, y1)
}

struct Entry {
    closed: bool,
    xform: Xform,
    anchors: Vec<Anchor>,
    holes: Vec<Vec<Anchor>>,
    bbox: Rect,
    flat: Option<(i32, Arc<PathGeometry>)>,
}

impl Entry {
    fn fresh(path: &Path, xform: Xform) -> Entry {
        Entry {
            closed: path.closed,
            xform,
            anchors: path.anchors.clone(),
            holes: path.holes.clone(),
            bbox: control_bbox_with(path, xform),
            flat: None,
        }
    }
    fn matches(&self, path: &Path, xform: Xform) -> bool {
        self.closed == path.closed && self.xform == xform && self.anchors == path.anchors && self.holes == path.holes
    }
}

/// The cross-frame cache. One entry per path id (its latest geometry inputs + one zoom bucket).
/// Memory is bounded by the document: entries for deleted paths are evicted by `retain_live`.
#[derive(Default)]
pub struct FlattenCache {
    entries: HashMap<u32, Entry>,
    hits: u64,
    misses: u64,
}

impl FlattenCache {
    /// THE lookup `build_scene` makes, once per path per frame: validate path `pi`'s entry against its
    /// current geometry inputs (rebuilding it on any difference), hand `want` the world control bbox, and
    /// — only if `want` says yes (the path is visible) — return its flattened geometry at `ppu`'s zoom
    /// bucket, reused when the bucket is unchanged. A culled path is never flattened.
    pub fn lookup(
        &mut self,
        doc: &Document,
        pi: usize,
        ppu: f32,
        want: impl FnOnce(Rect) -> bool,
    ) -> (Rect, Option<Arc<PathGeometry>>) {
        let path = &doc.paths[pi];
        let xform = doc.unit_xform(path.id); // the one unit lookup for this path this frame
        let entry = match self.entries.entry(path.id) {
            MapEntry::Occupied(slot) => {
                let entry = slot.into_mut();
                if !entry.matches(path, xform) {
                    *entry = Entry::fresh(path, xform);
                }
                entry
            }
            MapEntry::Vacant(slot) => slot.insert(Entry::fresh(path, xform)),
        };
        let bbox = entry.bbox;
        if !want(bbox) {
            return (bbox, None);
        }
        let bucket = zoom_bucket(ppu);
        if let Some((cached_bucket, geometry)) = &entry.flat {
            if *cached_bucket == bucket {
                self.hits += 1;
                return (bbox, Some(Arc::clone(geometry)));
            }
        }
        let geometry = Arc::new(flatten_with(path, xform, bucket_ppu(bucket)));
        entry.flat = Some((bucket, Arc::clone(&geometry)));
        self.misses += 1;
        (bbox, Some(geometry))
    }

    /// World control-point bbox of path `pi` (validated like `lookup`; never flattens).
    pub fn bbox(&mut self, doc: &Document, pi: usize) -> Rect {
        self.lookup(doc, pi, 1.0, |_| false).0
    }

    /// Flattened world geometry of path `pi` at `ppu`'s zoom bucket — reused when the path's geometry
    /// inputs and the bucket are unchanged, otherwise flattened now (identical to `flatten_path`).
    pub fn geometry(&mut self, doc: &Document, pi: usize, ppu: f32) -> Arc<PathGeometry> {
        self.lookup(doc, pi, ppu, |_| true).1.expect("want = true always yields geometry")
    }

    /// Evict entries whose path no longer exists. Call it AFTER `lookup` touched every live
    /// path (as `build_scene` does): every live id then has an entry, so a stale entry exists exactly
    /// when there are more entries than paths — a length check, O(1) on frames where nothing was deleted.
    pub fn retain_live(&mut self, doc: &Document) {
        if self.entries.len() > doc.paths.len() {
            let live: HashSet<u32> = doc.paths.iter().map(|path| path.id).collect();
            self.entries.retain(|id, _| live.contains(id));
        }
    }

    /// `(hits, misses)` of `geometry` since the cache was created or cleared.
    pub fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Number of cached paths.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop everything (and reset the counters) — the perf harness uses this to measure cold frames.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }
}

/// The cache as the `Editor` holds it: `build_scene` takes `&Editor`, so the cache needs interior
/// mutability. A `Mutex` (uncontended: one lock per scene build) keeps `Editor` `Send + Sync`.
#[derive(Default)]
pub struct SharedFlattenCache(Mutex<FlattenCache>);

impl SharedFlattenCache {
    /// Lock the cache. A poisoned lock (a panic mid-update) is recovered by CLEARING the cache — a cold
    /// rebuild is always correct.
    pub fn lock(&self) -> MutexGuard<'_, FlattenCache> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                self.0.clear_poison();
                let mut guard = poisoned.into_inner();
                guard.clear();
                guard
            }
        }
    }
}
