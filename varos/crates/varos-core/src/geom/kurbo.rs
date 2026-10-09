// Adapted from VectorCraft crates/geom/src/path.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Absolute-handle conversion. Hole rings become independent closed subpaths.
//! Plain BezPath carries geometry only; use [`KurboPath`] for lossless model round trips.
use crate::model::{Anchor, Path};
use ::kurbo::{BezPath, PathEl, Point};

pub(crate) fn point(p: [f32; 2]) -> Point {
    Point::new(p[0] as f64, p[1] as f64)
}
pub(crate) fn pt(p: Point) -> [f32; 2] {
    [p.x as f32, p.y as f32]
}
pub(crate) fn anchor(p: Point) -> Anchor {
    Anchor { id: 0, p: pt(p), hin: None, hout: None, smooth: false }
}
pub(crate) fn path(anchors: Vec<Anchor>, closed: bool) -> Path {
    Path::new(0, anchors, closed, None, None, 1.0)
}

/// Geometry plus the original model metadata, which BezPath cannot encode.
/// Immutable geometry avoids accidentally applying stale anchor metadata after topology edits.
#[derive(Clone, Debug)]
pub struct KurboPath {
    geometry: BezPath,
    source: Path,
}
impl KurboPath {
    pub fn new(source: &Path) -> Self {
        Self { geometry: to_bez_path(source), source: source.clone() }
    }
    pub fn geometry(&self) -> &BezPath {
        &self.geometry
    }
    /// Exact model round trip, including smooth flags, IDs, paints and inactive endpoint handles.
    pub fn into_path(self) -> Path {
        let mut restored = self.source;
        let mut elements = self.geometry.elements().iter();
        restore_ring(&mut restored.anchors, restored.closed, &mut elements);
        for ring in &mut restored.holes {
            restore_ring(ring, true, &mut elements);
        }
        restored
    }
}

// Topology metadata distinguishes a repeated endpoint from the explicit closing segment,
// and preserves None handles, smooth flags and otherwise unrepresentable empty rings.
fn restore_ring<'a>(ring: &mut [Anchor], closed: bool, elements: &mut impl Iterator<Item = &'a PathEl>) {
    if ring.is_empty() {
        return;
    }
    if let Some(PathEl::MoveTo(p)) = elements.next() {
        ring[0].p = pt(*p);
    }
    let n = ring.len();
    for i in 0..if closed { n } else { n - 1 } {
        let next = (i + 1) % n;
        match elements.next() {
            Some(PathEl::LineTo(p)) => {
                ring[next].p = pt(*p);
            }
            Some(PathEl::CurveTo(a, b, p)) => {
                if ring[i].hout.is_some() {
                    ring[i].hout = Some(pt(*a));
                }
                if ring[next].hin.is_some() {
                    ring[next].hin = Some(pt(*b));
                }
                ring[next].p = pt(*p);
            }
            _ => {}
        }
    }
    if closed {
        let _ = elements.next();
    }
}

fn append(out: &mut BezPath, anchors: &[Anchor], closed: bool) {
    let Some(first) = anchors.first() else { return };
    out.move_to(point(first.p));
    let n = anchors.len();
    for i in 0..if closed { n } else { n - 1 } {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % n];
        if a.hout.is_none() && b.hin.is_none() {
            out.line_to(point(b.p));
        } else {
            out.curve_to(point(a.hout.unwrap_or(a.p)), point(b.hin.unwrap_or(b.p)), point(b.p));
        }
    }
    if closed {
        out.close_path();
    }
}
/// Convert a single contour using the same absolute-handle rules as full paths.
pub fn contour(anchors: &[Anchor], closed: bool) -> BezPath {
    let mut out = BezPath::new();
    append(&mut out, anchors, closed);
    out
}
/// Convert outer and hole contours; handles in the model are absolute, never offsets.
pub fn to_bez_path(source: &Path) -> BezPath {
    let mut out = BezPath::new();
    append(&mut out, &source.anchors, source.closed);
    for hole in &source.holes {
        append(&mut out, hole, true);
    }
    out
}

/// Decode each subpath separately (the model cannot represent multiple open rings in one Path).
/// Quadratics are elevated exactly to cubics. Smoothness is inferred from collinear handles;
/// use [`KurboPath`] when the original explicit flags must survive.
pub fn from_bez_path(source: &BezPath) -> Vec<Path> {
    let mut out = Vec::new();
    let mut anchors: Vec<Anchor> = Vec::new();
    let mut post_close = None;
    for el in source.elements() {
        if matches!(el, PathEl::LineTo(_) | PathEl::QuadTo(_, _) | PathEl::CurveTo(_, _, _)) {
            if let Some(p) = post_close.take() {
                anchors.push(anchor(p));
            }
        }
        match *el {
            PathEl::MoveTo(p) => {
                post_close = None;
                if !anchors.is_empty() {
                    out.push(finish(std::mem::take(&mut anchors), false));
                }
                anchors.push(anchor(p));
            }
            PathEl::LineTo(p) => {
                if !anchors.is_empty() {
                    anchors.push(anchor(p));
                }
            }
            PathEl::QuadTo(h, p) => {
                if let Some(last) = anchors.last_mut() {
                    last.hout = Some(pt(point(last.p).lerp(h, 2.0 / 3.0)));
                    let mut a = anchor(p);
                    a.hin = Some(pt(p.lerp(h, 2.0 / 3.0)));
                    anchors.push(a);
                }
            }
            PathEl::CurveTo(h1, h2, p) => {
                if let Some(last) = anchors.last_mut() {
                    last.hout = Some(pt(h1));
                    let mut a = anchor(p);
                    a.hin = Some(pt(h2));
                    anchors.push(a);
                }
            }
            PathEl::ClosePath => {
                if !anchors.is_empty() {
                    post_close = anchors.first().map(|a| point(a.p));
                    if anchors.len() > 1 && anchors.first().map(|a| a.p) == anchors.last().map(|a| a.p) {
                        if let Some(last) = anchors.pop() {
                            anchors[0].hin = last.hin;
                        }
                    }
                    out.push(finish(std::mem::take(&mut anchors), true));
                }
            }
        }
    }
    if !anchors.is_empty() {
        out.push(finish(anchors, false));
    }
    out
}
fn finish(mut anchors: Vec<Anchor>, closed: bool) -> Path {
    for a in &mut anchors {
        if let (Some(i), Some(o)) = (a.hin, a.hout) {
            let u = point(a.p) - point(i);
            let v = point(o) - point(a.p);
            a.smooth = u.dot(v) > 0.0 && u.cross(v).abs() <= 1e-6 * u.hypot() * v.hypot();
        }
    }
    path(anchors, closed)
}
/// Combine a closed outer contour and closed holes. Reject unrepresentable open extra subpaths.
pub fn from_compound_bez_path(source: &BezPath) -> Result<Path, &'static str> {
    let mut rings = from_bez_path(source).into_iter();
    let mut outer = rings.next().unwrap_or_else(|| path(Vec::new(), false));
    for ring in rings {
        if !outer.closed || !ring.closed {
            return Err("compound paths require closed contours");
        }
        outer.holes.push(ring.anchors);
    }
    Ok(outer)
}
