//! Minimal geometry-only adapter for Lane H; unify with p2-geom at merge.
use crate::model::{Anchor, Path};
use kurbo::{BezPath, Point};
fn point(p: [f32; 2]) -> Point {
    Point::new(f64::from(p[0]), f64::from(p[1]))
}
/// No IDs or metadata are rewritten; holes remain separate closed subpaths.
pub fn contour(anchors: &[Anchor], closed: bool) -> BezPath {
    let mut out = BezPath::new();
    let Some(first) = anchors.first() else { return out };
    out.move_to(point(first.p));
    let mut segment = |a: &Anchor, b: &Anchor| {
        if a.hout.is_none() && b.hin.is_none() {
            out.line_to(point(b.p));
        } else {
            out.curve_to(point(a.hout.unwrap_or(a.p)), point(b.hin.unwrap_or(b.p)), point(b.p));
        }
    };
    for pair in anchors.windows(2) {
        segment(&pair[0], &pair[1]);
    }
    if closed {
        if let Some(last) = anchors.last() {
            segment(last, first);
        }
        out.close_path();
    }
    out
}
pub fn path(path: &Path) -> BezPath {
    let mut out = contour(&path.anchors, path.closed);
    for hole in &path.holes {
        out.extend(contour(hole, true).elements().iter().copied());
    }
    out
}
