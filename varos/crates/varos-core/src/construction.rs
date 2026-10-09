// Adapted from VectorCraft crates/tools/src/{builder,cut,draw2/anchor}.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Face editing and cutting commands; gestures use the same command boundary.
use crate::{
    boolean::Shape,
    editor::{Drag, Editor, ToolKind},
    geom::{cubic, dist, Pt},
    model::{Anchor, DropPos, Paint, Path},
    planar::{self, PathfinderOp, Piece},
    EditCommand,
};
use std::{cell::RefCell, collections::HashSet, sync::Arc};
#[derive(Clone, Default)]
pub(crate) struct ConstructionCache {
    key: Option<(u64, Vec<u32>)>,
    faces: Arc<Vec<planar::Face>>,
    points: Vec<Pt>,
    hits: Vec<bool>,
    #[cfg(test)]
    builds: usize,
    #[cfg(test)]
    hit_checks: usize,
}
pub(crate) type SharedConstructionCache = RefCell<ConstructionCache>;
impl Editor {
    pub(crate) fn reset_construction_walk(&self) {
        let mut cache = self.construction_cache.borrow_mut();
        cache.points.clear();
        cache.hits.clear();
    }
    fn construction_paths(&self) -> Vec<usize> {
        let selected = self.pathfinder_objects();
        (0..self.doc.paths.len())
            .filter(|i| {
                selected.contains(&self.doc.paths[*i].id)
                    && self.doc.paths[*i].closed
                    && self.doc.paths[*i].anchors.len() >= 3
            })
            .collect()
    }
    fn construction_cuts(&self) -> Vec<crate::boolean::Ring> {
        let selected = self.pathfinder_objects();
        self.doc
            .paths
            .iter()
            .filter(|p| selected.contains(&p.id) && !p.closed)
            .map(|p| {
                let xf = self.doc.unit_xform(p.id);
                let mut line = Vec::new();
                for w in p.anchors.windows(2) {
                    for k in 0..32 {
                        let q = cubic(
                            xf.apply(w[0].p),
                            xf.apply(w[0].hout.unwrap_or(w[0].p)),
                            xf.apply(w[1].hin.unwrap_or(w[1].p)),
                            xf.apply(w[1].p),
                            k as f32 / 32.0,
                        );
                        line.push([q[0] as f64, q[1] as f64]);
                    }
                }
                if let Some(a) = p.anchors.last() {
                    let q = xf.apply(a.p);
                    line.push([q[0] as f64, q[1] as f64]);
                }
                line
            })
            .collect()
    }
    fn cached_construction_faces(&self) -> Arc<Vec<planar::Face>> {
        let mut ids: Vec<_> = self.pathfinder_objects().into_iter().collect();
        ids.sort_unstable();
        let key = (self.rev, ids);
        let mut cache = self.construction_cache.borrow_mut();
        if cache.key.as_ref() != Some(&key) {
            cache.faces = Arc::new(planar::faces_with_cuts(
                &self.construction_paths().iter().map(|i| planar::flatten(&self.path_to_segs(*i))).collect::<Vec<_>>(),
                &self.construction_cuts(),
            ));
            cache.key = Some(key);
            cache.points.clear();
            cache.hits.clear();
            #[cfg(test)]
            {
                cache.builds += 1;
            }
        }
        Arc::clone(&cache.faces)
    }
    pub fn construction_faces(&self) -> Vec<planar::Face> {
        self.cached_construction_faces().as_ref().clone()
    }
    fn construction_replace(
        &mut self,
        sources: &[Path],
        pieces: Vec<Piece>,
        remove: &HashSet<u32>,
        strip_stroke: bool,
    ) {
        if remove.is_empty() || remove.iter().any(|id| self.doc.is_mask_source(*id)) {
            return;
        }
        self.begin();
        for p in sources.iter().filter(|p| remove.contains(&p.id)) {
            self.bake_unit_of(p.id);
        }
        let mut new_ids = Vec::new();
        for piece in pieces {
            let Some(source) = sources.get(piece.owner) else { continue };
            let Some(outer) = piece.shape.first() else { continue };
            if outer.len() < if piece.closed { 3 } else { 2 } {
                continue;
            }
            let mut make_ring = |ring: &Vec<[f64; 2]>| -> Vec<Anchor> {
                ring.iter()
                    .map(|p| Anchor {
                        id: self.doc.nid(),
                        p: [p[0] as f32, p[1] as f32],
                        hin: None,
                        hout: None,
                        smooth: false,
                    })
                    .collect()
            };
            let anchors = make_ring(outer);
            let holes = piece.shape.iter().skip(1).map(&mut make_ring).collect();
            let mut path = source.clone();
            path.id = self.doc.nid();
            path.anchors = anchors;
            path.holes = holes;
            path.closed = piece.closed;
            if strip_stroke {
                path.stroke = Paint::None;
            }
            if !piece.closed {
                path.stroke = source.appearance().fill().clone();
                path.fill = Paint::None;
                if path.stroke_width <= 0.0 {
                    path.stroke_width = 1.0;
                }
            }
            let id = path.id;
            let target = self.doc.node_of_path(source.id);
            self.doc.paths.push(path);
            self.doc.sync_tree();
            if let Some(target) = target {
                self.doc.move_paths_to(&[id], target, DropPos::Before, false);
            }
            new_ids.push(id);
        }
        self.doc.paths.retain(|p| !remove.contains(&p.id));
        self.objsel.retain(|id| !remove.contains(id));
        self.objsel.extend(new_ids);
        self.selected.clear();
        self.dsel_path = None;
        self.active = None;
        self.group_sel.clear();
        self.dirty = true;
        self.commit();
    }
    pub fn planar_pathfinder(&mut self, op: PathfinderOp) {
        let indices = self.construction_paths();
        if indices.len() < 2 {
            return;
        }
        let sources: Vec<_> =
            indices.iter().map(|i| crate::gradient_transform::world_path(&self.doc, &self.doc.paths[*i])).collect();
        let shapes: Vec<_> = indices.iter().map(|i| planar::flatten(&self.path_to_segs(*i))).collect();
        let keys: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, p)| {
                sources[..i]
                    .iter()
                    .position(|q| {
                        q.appearance().fill().clone() == p.appearance().fill().clone() && q.opacity == p.opacity
                    })
                    .unwrap_or(i)
            })
            .collect();
        let pieces = planar::pathfinder(op, &shapes, &keys);
        self.construction_replace(
            &sources,
            pieces,
            &sources.iter().map(|p| p.id).collect(),
            matches!(op, PathfinderOp::Trim | PathfinderOp::Merge | PathfinderOp::Crop),
        );
    }
    pub fn shape_builder(&mut self, points: &[Pt], delete: bool) {
        if points.is_empty() || points.iter().flatten().any(|v| !v.is_finite()) {
            return;
        }
        let indices = self.construction_paths();
        let sources: Vec<_> =
            indices.iter().map(|i| crate::gradient_transform::world_path(&self.doc, &self.doc.paths[*i])).collect();
        let shapes: Vec<_> = indices.iter().map(|i| planar::flatten(&self.path_to_segs(*i))).collect();
        let faces = self.cached_construction_faces();
        let hit: Vec<_> = faces.iter().map(|f| walk_hits(f, points)).collect();
        if !hit.iter().any(|v| *v) {
            return;
        }
        // Only replace components incident to the hit; retain disjoint paths verbatim.
        let mut affected: HashSet<usize> =
            faces.iter().zip(&hit).filter(|(_, h)| **h).flat_map(|(f, _)| f.owners.iter().copied()).collect();
        loop {
            let old_len = affected.len();
            for f in faces.iter() {
                if f.owners.iter().any(|o| affected.contains(o)) {
                    affected.extend(&f.owners);
                }
            }
            if affected.len() == old_len {
                break;
            }
        }
        let mut pieces = Vec::new();
        let mut merge = Vec::new();
        let mut owner = 0;
        for (f, hit) in faces.iter().zip(hit) {
            if !f.owners.iter().any(|o| affected.contains(o)) {
                continue;
            }
            let Some(top) = f.owners.last().copied() else { continue };
            if hit {
                owner = owner.max(top);
                if !delete {
                    merge.push(f.shape.clone());
                }
            } else {
                pieces.push(Piece { shape: f.shape.clone(), owner: top, closed: true });
            }
        }
        pieces.extend(planar::union(&merge).into_iter().map(|shape| Piece { shape, owner, closed: true }));
        // A region with unchanged coverage and paint must retain geometry, IDs and undo.
        let unchanged = affected.iter().all(|&i| {
            let owned: Vec<_> = pieces.iter().filter(|p| p.owner == i).collect();
            owned.len() == 1
                && planar::subtract(&shapes[i], std::slice::from_ref(&owned[0].shape))
                    .iter()
                    .map(planar::area)
                    .sum::<f64>()
                    < 1e-5
                && planar::subtract(&owned[0].shape, std::slice::from_ref(&shapes[i]))
                    .iter()
                    .map(planar::area)
                    .sum::<f64>()
                    < 1e-5
        });
        if unchanged {
            return;
        }
        let remove = affected.into_iter().map(|i| sources[i].id).collect();
        self.construction_replace(&sources, pieces, &remove, false);
    }
    pub fn cut_fills(&mut self, points: &[Pt], radius: Option<f32>) {
        if points.is_empty()
            || points.iter().flatten().any(|v| !v.is_finite())
            || radius.is_some_and(|r| !r.is_finite() || r <= 0.0)
        {
            return;
        }
        let line = points.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
        let brush = radius.map(|r| planar::brush(points, r));
        let indices = self.construction_paths();
        let sources: Vec<_> =
            indices.iter().map(|i| crate::gradient_transform::world_path(&self.doc, &self.doc.paths[*i])).collect();
        let mut pieces = Vec::new();
        let mut remove = HashSet::new();
        for (owner, i) in indices.iter().enumerate() {
            let shape = planar::flatten(&self.path_to_segs(*i));
            let result =
                if let Some(brush) = &brush { planar::subtract(&shape, brush) } else { planar::knife(&shape, &line) };
            let changed = if radius.is_some() {
                (result.iter().map(planar::area).sum::<f64>() - planar::area(&shape)).abs() > 1e-5
            } else {
                result.len() > 1
            };
            if changed {
                remove.insert(sources[owner].id);
                pieces.extend(result.into_iter().map(|shape| Piece { shape, owner, closed: true }));
            }
        }
        self.construction_replace(&sources, pieces, &remove, false);
    }
    pub fn divide_objects_below(&mut self) {
        let selected = self.construction_paths();
        let Some(&cutter) = selected.last() else { return };
        let cutter_shape = planar::flatten(&self.path_to_segs(cutter));
        let indices: Vec<_> = (0..cutter)
            .filter(|i| {
                self.doc.paths[*i].closed
                    && !self.doc.eff_locked(self.doc.paths[*i].id)
                    && !self.doc.eff_hidden(self.doc.paths[*i].id)
            })
            .collect();
        let sources: Vec<_> =
            indices.iter().map(|i| crate::gradient_transform::world_path(&self.doc, &self.doc.paths[*i])).collect();
        let mut pieces = Vec::new();
        let mut remove = HashSet::new();
        for (owner, i) in indices.iter().enumerate() {
            let shape = planar::flatten(&self.path_to_segs(*i));
            let regions = planar::faces(&[shape, cutter_shape.clone()]);
            if !regions.iter().any(|f| f.owners == [0, 1]) {
                continue;
            }
            remove.insert(sources[owner].id);
            pieces.extend(regions.into_iter().filter(|f| f.owners.contains(&0)).map(|f| Piece {
                shape: f.shape,
                owner,
                closed: true,
            }));
        }
        if remove.is_empty() {
            return;
        }
        remove.insert(self.doc.paths[cutter].id);
        self.construction_replace(&sources, pieces, &remove, false);
    }
    pub fn scissors(&mut self, pid: u32, segment: usize, t: f32) {
        if !t.is_finite() || !(0.0..=1.0).contains(&t) || self.doc.eff_hidden(pid) || self.doc.eff_locked(pid) {
            return;
        }
        let Some(pi) = self.doc.pidx(pid) else { return };
        let p = &self.doc.paths[pi];
        // Opening compound contours cannot preserve the current even-odd fill semantics.
        if !p.holes.is_empty() || self.doc.is_mask_source(pid) {
            return;
        }
        let n = p.anchors.len();
        if n < 2 || segment >= if p.closed { n } else { n - 1 } {
            return;
        }
        self.begin();
        self.bake_unit_of(pid);
        let Some(pi) = self.doc.pidx(pid) else {
            self.commit();
            return;
        };
        let source = self.doc.paths[pi].clone();
        let mut anchors = source.anchors.clone();
        let a = &anchors[segment];
        let b = &anchors[(segment + 1) % n];
        let (p0, p1, p2, p3) = (a.p, a.hout.unwrap_or(a.p), b.hin.unwrap_or(b.p), b.p);
        let q0 = lerp(p0, p1, t);
        let q1 = lerp(p1, p2, t);
        let q2 = lerp(p2, p3, t);
        let r0 = lerp(q0, q1, t);
        let r1 = lerp(q1, q2, t);
        let point = lerp(r0, r1, t);
        anchors[segment].hout = Some(q0);
        anchors[(segment + 1) % n].hin = Some(q2);
        let cut = Anchor { id: self.doc.nid(), p: point, hin: Some(r0), hout: Some(r1), smooth: false };
        anchors.insert(segment + 1, cut.clone());
        let mut first = source.clone();
        first.closed = false;
        first.holes.clear();
        if source.closed {
            first.anchors = anchors[segment + 1..].iter().chain(&anchors[..=segment]).cloned().collect();
            let mut end = cut;
            end.id = self.doc.nid();
            first.anchors.push(end);
        } else {
            first.anchors = anchors[..=segment + 1].to_vec();
            let mut second = source.clone();
            second.id = self.doc.nid();
            second.anchors = anchors[segment + 1..].to_vec();
            second.anchors[0].id = self.doc.nid();
            second.holes.clear();
            if let Some(start) = second.anchors.first_mut() {
                start.hin = None;
            }
            let id = second.id;
            let target = self.doc.node_of_path(pid);
            self.doc.paths.push(second);
            self.doc.sync_tree();
            if let Some(target) = target {
                self.doc.move_paths_to(&[id], target, DropPos::After, false);
            }
            self.objsel.insert(id);
        }
        if let Some(start) = first.anchors.first_mut() {
            start.hin = None;
        }
        if let Some(end) = first.anchors.last_mut() {
            end.hout = None;
        }
        if let Some(pi) = self.doc.pidx(pid) {
            self.doc.paths[pi] = first;
        }
        self.objsel.insert(pid);
        self.dirty = true;
        self.commit();
    }
    pub(crate) fn scissors_at(&mut self, pos: Pt) {
        let Some(pid) = self.path_under(pos) else { return };
        let Some(pi) = self.doc.pidx(pid) else { return };
        let p = &self.doc.paths[pi];
        let xf = self.doc.unit_xform(pid);
        let n = p.anchors.len();
        let mut best = (f32::INFINITY, 0, 0.0);
        for i in 0..if p.closed { n } else { n.saturating_sub(1) } {
            let a = &p.anchors[i];
            let b = &p.anchors[(i + 1) % n];
            for k in 0..=128 {
                let t = k as f32 / 128.0;
                let q = cubic(
                    xf.apply(a.p),
                    xf.apply(a.hout.unwrap_or(a.p)),
                    xf.apply(b.hin.unwrap_or(b.p)),
                    xf.apply(b.p),
                    t,
                );
                let d = dist(q, pos);
                if d < best.0 {
                    best = (d, i, t);
                }
            }
        }
        if best.0.is_finite() {
            let a = &p.anchors[best.1];
            let b = &p.anchors[(best.1 + 1) % n];
            let distance = |t| {
                dist(
                    cubic(
                        xf.apply(a.p),
                        xf.apply(a.hout.unwrap_or(a.p)),
                        xf.apply(b.hin.unwrap_or(b.p)),
                        xf.apply(b.p),
                        t,
                    ),
                    pos,
                )
            };
            let mut lo = (best.2 - 1.0 / 128.0).max(0.0);
            let mut hi = (best.2 + 1.0 / 128.0).min(1.0);
            for _ in 0..20 {
                let l = lo + (hi - lo) / 3.0;
                let r = hi - (hi - lo) / 3.0;
                if distance(l) < distance(r) {
                    hi = r;
                } else {
                    lo = l;
                }
            }
            let t = (lo + hi) * 0.5;
            let d = distance(t);
            if d < best.0 {
                best.0 = d;
                best.2 = t;
            }
        }
        if best.0 <= 8.0 / self.ppu {
            self.execute_ui(EditCommand::Scissors { path: pid, segment: best.1, t: best.2 });
        }
    }
    pub(crate) fn finish_construction(&mut self, points: Vec<Pt>, delete: bool) {
        let command = match self.gesture {
            ToolKind::ShapeBuilder => EditCommand::ShapeBuilder { points, delete },
            ToolKind::Knife => EditCommand::Knife { points },
            ToolKind::Eraser => EditCommand::Eraser { points, radius: 8.0 / self.ppu },
            _ => return,
        };
        self.execute_ui(command);
    }
    pub fn construction_highlight(&self) -> Vec<Shape> {
        if self.tool != ToolKind::ShapeBuilder {
            return Vec::new();
        }
        let faces = self.cached_construction_faces();
        if let Drag::Construction { points, .. } = &self.drag {
            let mut cache = self.construction_cache.borrow_mut();
            let n = cache.points.len();
            let continues = n > 0
                && points.len() >= n
                && points.first() == cache.points.first()
                && points.get(n - 1) == cache.points.last();
            if !continues {
                cache.points.clear();
                cache.hits = vec![false; faces.len()];
            }
            let start = cache.points.len().saturating_sub(1);
            if points.len() > cache.points.len() {
                for (i, f) in faces.iter().enumerate() {
                    if !cache.hits[i] {
                        cache.hits[i] = walk_hits(f, &points[start..]);
                        #[cfg(test)]
                        {
                            cache.hit_checks += points.len() - start;
                        }
                    }
                }
                cache.points.extend_from_slice(&points[if continues { n } else { 0 }..]);
            }
            faces.iter().zip(&cache.hits).filter(|(_, hit)| **hit).map(|(f, _)| f.shape.clone()).collect()
        } else {
            let mut cache = self.construction_cache.borrow_mut();
            cache.points.clear();
            cache.hits.clear();
            faces.iter().filter(|f| f.contains(self.cursor)).map(|f| f.shape.clone()).collect()
        }
    }
}
fn lerp(a: Pt, b: Pt, t: f32) -> Pt {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
fn walk_hits(face: &planar::Face, points: &[Pt]) -> bool {
    if points.iter().any(|p| face.contains(*p)) {
        return true;
    }
    points.windows(2).any(|w| {
        face.shape.iter().any(|r| {
            (0..r.len()).any(|i| {
                let a = [r[i][0] as f32, r[i][1] as f32];
                let b = [r[(i + 1) % r.len()][0] as f32, r[(i + 1) % r.len()][1] as f32];
                let cross = |a: Pt, b: Pt, c: Pt| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                let s = cross(w[0], w[1], a);
                let t = cross(w[0], w[1], b);
                let u = cross(a, b, w[0]);
                let v = cross(a, b, w[1]);
                s * t < 0.0 && u * v < 0.0
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ShapeKind;
    #[test]
    fn arrangement_reuses_hover_and_drag_and_invalidates_revision_selection_undo() {
        let mut ed = Editor::new();
        let id = ed
            .try_execute_created(EditCommand::AddShape {
                kind: ShapeKind::Rect,
                bounds: [0., 0., 20., 20.],
                parent: None,
                fill: Some([1.; 4]),
                stroke: None,
                stroke_width: 0.,
                opacity: 1.,
                name: None,
            })
            .unwrap();
        ed.execute_ui(EditCommand::SelectPaths(vec![id]));
        ed.set_tool(ToolKind::ShapeBuilder);
        for k in 0..20 {
            ed.pointer_move([k as f32, 5.]);
            ed.construction_highlight();
        }
        assert_eq!(ed.construction_cache.borrow().builds, 1);
        ed.pointer_down([-100., -100.]);
        for k in 0..50 {
            ed.pointer_move([-100. + k as f32, -100.]);
            ed.construction_highlight();
            ed.construction_highlight(); // scene retries do no extra hit work
        }
        let cache = ed.construction_cache.borrow();
        assert_eq!(cache.builds, 1);
        assert!(cache.hit_checks <= 101, "only new segments checked: {}", cache.hit_checks);
        drop(cache);
        ed.pointer_up();
        ed.execute_ui(EditCommand::SelectPaths(vec![]));
        ed.set_tool(ToolKind::ShapeBuilder);
        ed.construction_highlight();
        assert_eq!(ed.construction_cache.borrow().builds, 2);
        ed.execute_ui(EditCommand::SelectPaths(vec![id]));
        ed.set_tool(ToolKind::ShapeBuilder);
        ed.construction_highlight();
        assert_eq!(ed.construction_cache.borrow().builds, 3);
        ed.execute_ui(EditCommand::Eraser { points: vec![[5., 5.]], radius: 2. });
        ed.construction_highlight();
        assert_eq!(ed.construction_cache.borrow().builds, 4);
        ed.execute_ui(EditCommand::Undo);
        ed.construction_highlight();
        assert_eq!(ed.construction_cache.borrow().builds, 5);
        ed.execute_ui(EditCommand::Redo);
        ed.construction_highlight();
        assert_eq!(ed.construction_cache.borrow().builds, 6);
        ed.replace_doc(crate::model::Document::default());
        ed.set_tool(ToolKind::ShapeBuilder);
        assert!(ed.construction_highlight().is_empty());
        assert_eq!(ed.construction_cache.borrow().builds, 7);
    }
}
