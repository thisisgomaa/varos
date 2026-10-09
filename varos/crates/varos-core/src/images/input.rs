//! Image selection and translation; pixels and previews remain outside document history.
use super::{image_hidden, image_locked, world_corners, ImageAffine};
use crate::{model::NodeKind, Editor, ToolKind};
#[derive(Clone, Debug)]
pub struct Gesture {
    pub id: u32,
    pub start: [f32; 2],
    pub xform: ImageAffine,
    pub opacity: f32,
}
pub fn hit(ed: &Editor, p: [f32; 2]) -> Option<u32> {
    let top_path = ed.path_under(p);
    for kind in super::paint_order(&ed.doc).into_iter().rev() {
        let NodeKind::Image(id) = kind else {
            if let NodeKind::Path(id) = kind {
                if Some(id) == top_path && !ed.doc.is_mask_source(id) {
                    return None;
                }
            }
            continue;
        };
        if !ed.in_isolation(id) || image_hidden(&ed.doc, id) || image_locked(&ed.doc, id) {
            continue;
        }
        if super::board_clips(&ed.doc, id)
            .is_some_and(|rects| !rects.iter().any(|r| p[0] >= r.0 && p[0] <= r.2 && p[1] >= r.1 && p[1] <= r.3))
        {
            continue;
        }
        let mut parent = ed.doc.node_of_path(id).and_then(|n| ed.doc.node(n)).and_then(|n| n.parent);
        let mut clipped = false;
        while let Some(nid) = parent {
            let Some(node) = ed.doc.node(nid) else { break };
            if let Some(mask) = node.mask_child.and_then(|m| ed.doc.node(m)) {
                if let NodeKind::Path(pid) = mask.kind {
                    if let Some(pi) = ed.doc.pidx(pid) {
                        let local = ed.doc.unit_xform(pid).inverse_apply(p);
                        if !ed.doc.point_in_path(pi, local) {
                            clipped = true;
                            break;
                        }
                    }
                }
            }
            parent = node.parent;
        }
        if clipped {
            continue;
        }
        let i = ed.doc.images.iter().find(|i| i.id == id)?;
        let c = world_corners(&ed.doc, i);
        let u = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
        let v = [c[3][0] - c[0][0], c[3][1] - c[0][1]];
        let d = u[0] * v[1] - u[1] * v[0];
        if d.abs() < 1e-12 {
            continue;
        }
        let q = [p[0] - c[0][0], p[1] - c[0][1]];
        let x = (q[0] * v[1] - q[1] * v[0]) / d;
        let y = (u[0] * q[1] - u[1] * q[0]) / d;
        if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
            return Some(id);
        }
    }
    None
}
pub fn gesture(ed: &Editor, p: [f32; 2]) -> Option<Gesture> {
    if ed.tool != ToolKind::Object {
        return None;
    }
    let id = hit(ed, p)?;
    let i = ed.doc.images.iter().find(|i| i.id == id)?;
    Some(Gesture { id, start: p, xform: i.xform, opacity: i.opacity })
}
/// Transform world delta back through ancestor rotations exactly once.
pub fn translated(ed: &Editor, g: &Gesture, p: [f32; 2]) -> ImageAffine {
    let mut a = g.start;
    let mut b = p;
    let mut chain = vec![];
    let mut node = ed.doc.node_of_path(g.id);
    while let Some(id) = node {
        let Some(n) = ed.doc.node(id) else { break };
        chain.push(n.xform);
        node = n.parent;
    }
    for x in chain.into_iter().rev() {
        a = x.inverse_apply(a);
        b = x.inverse_apply(b);
    }
    ImageAffine { e: g.xform.e + b[0] - a[0], f: g.xform.f + b[1] - a[1], ..g.xform }
}

/// Shared transform corner write-back through the same ancestor chain as world_corners.
pub fn write_world_corners(doc: &mut crate::model::Document, id: u32, mut corners: [[f32; 2]; 3]) {
    let mut chain = vec![];
    let mut node = doc.node_of_path(id);
    while let Some(nid) = node {
        let Some(n) = doc.node(nid) else { break };
        chain.push(n.xform);
        node = n.parent;
    }
    for xf in chain.into_iter().rev() {
        corners = corners.map(|p| xf.inverse_apply(p));
    }
    if let Some(i) = doc.images.iter_mut().find(|i| i.id == id) {
        let [p, x, y] = corners;
        let w = i.px_w as f32;
        let h = i.px_h as f32;
        let a = ImageAffine {
            a: (x[0] - p[0]) / w,
            b: (x[1] - p[1]) / w,
            c: (y[0] - p[0]) / h,
            d: (y[1] - p[1]) / h,
            e: p[0],
            f: p[1],
        };
        if a.valid() {
            i.xform = a;
        }
    }
}
