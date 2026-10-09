//! Lane B: placement follows geometry. A baked global gradient becomes an owned paint;
//! solid global refs remain linked. Live gestures always map their starting placement.
use crate::{
    geom::Pt,
    model::{Document, Paint, Path},
};
use std::collections::HashMap;
pub fn materialize(path: &mut Path, paints: [Paint; 2]) {
    for (slot, paint) in [&mut path.fill, &mut path.stroke].into_iter().zip(paints) {
        if matches!(paint, Paint::Gradient(_)) {
            *slot = paint;
        }
    }
}
pub fn map(doc: &mut Document, pid: u32, f: impl Fn(Pt) -> Pt) {
    let Some(i) = doc.pidx(pid) else { return };
    let p = &doc.paths[i];
    let paints = [p.appearance().fill().resolved(doc), p.appearance().stroke().resolved(doc)];
    materialize(&mut doc.paths[i], paints);
    doc.paths[i].map_gradient_placement(f);
}
pub fn live(
    doc: &mut Document,
    base: Option<&Document>,
    cache: &mut HashMap<u32, [Paint; 2]>,
    pids: &[u32],
    world: bool,
    f: impl Fn(Pt) -> Pt,
) {
    for pid in pids {
        let Some(i) = doc.pidx(*pid) else { continue };
        let source = base.filter(|b| b.pidx(*pid).is_some()).unwrap_or(doc);
        let Some(si) = source.pidx(*pid) else { continue };
        let path = &source.paths[si];
        let start = cache
            .entry(*pid)
            .or_insert_with(|| [path.appearance().fill().resolved(source), path.appearance().stroke().resolved(source)])
            .clone();
        let source_xf = source.unit_xform(*pid);
        let dest_xf = doc.unit_xform(*pid);
        let mapped = start.map(|p| match p {
            Paint::Gradient(g) => {
                Paint::Gradient(g.mapped(|p| if world { dest_xf.inverse_apply(f(source_xf.apply(p))) } else { f(p) }))
            }
            p => p,
        });
        // Keep a valid placement when a live scale crosses zero. The geometry may be degenerate
        // for a frame, but this must never make an invalid document persistable.
        if mapped.iter().all(|p| !matches!(p,Paint::Gradient(g) if g.validate().is_err())) {
            materialize(&mut doc.paths[i], mapped);
        }
    }
}

pub fn world_path(doc: &Document, path: &Path) -> Path {
    let mut owned = path.clone();
    materialize(&mut owned, [path.appearance().fill().resolved(doc), path.appearance().stroke().resolved(doc)]);
    owned.map_gradient_placement(|p| doc.unit_xform(path.id).apply(p));
    owned
}
