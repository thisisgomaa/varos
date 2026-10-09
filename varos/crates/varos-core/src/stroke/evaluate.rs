//! One transient coverage evaluator for canvas, hit-testing and all exporters.
use super::{heads, ArrowAlign, StrokeAlign, StrokeCap, StrokeJoin};
use crate::{geom::stroke_adapter, model::Path, ExportNote, ExportReport, Pt};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use kurbo::{BezPath, Cap, Join, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Point, Vec2};

const MAX_RUNS: usize = 100_000;
const MAX_ELEMENTS: usize = 1_000_000;
const ARC: f64 = 0.000001;
#[derive(Clone, Debug, PartialEq)]
pub enum StrokeError {
    Invalid(String),
    Numeric,
    LimitExceeded,
    Cancelled,
}
impl std::fmt::Display for StrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(e) => f.write_str(e),
            Self::Numeric => f.write_str("stroke numeric overflow"),
            Self::LimitExceeded => f.write_str("limit_exceeded: stroke geometry budget"),
            Self::Cancelled => f.write_str("stroke cancelled"),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrokeCoverage {
    /// Normalized even-odd region: nonzero kurbo shaft unioned with filled heads once.
    pub rings: Vec<Vec<Pt>>,
    pub report: ExportReport,
    pub generated_elements: usize,
}
fn note(report: &mut ExportReport, id: u32, kind: &str) {
    report.notes.push(ExportNote { kind: kind.into(), object_id: Some(id), message: kind.replace('_', " ") });
}
fn checked(p: [f64; 2]) -> Result<Pt, StrokeError> {
    if p.iter().any(|v| !v.is_finite() || v.abs() > f64::from(f32::MAX)) {
        Err(StrokeError::Numeric)
    } else {
        Ok([p[0] as f32, p[1] as f32])
    }
}
fn flat(
    path: &BezPath,
    tolerance: f64,
    cancelled: &dyn Fn() -> bool,
    generated: &mut usize,
    element_cap: usize,
) -> Result<Vec<Vec<[f64; 2]>>, StrokeError> {
    let mut rings = Vec::new();
    let mut ring = Vec::new();
    let mut point = Point::ZERO;
    for el in path.elements() {
        if cancelled() {
            return Err(StrokeError::Cancelled);
        }
        let curve = match *el {
            PathEl::MoveTo(p) => {
                if !ring.is_empty() {
                    rings.push(std::mem::take(&mut ring));
                }
                point = p;
                ring.push([p.x, p.y]);
                None
            }
            PathEl::LineTo(p) => Some(kurbo::CubicBez::new(point, point, p, p)),
            PathEl::QuadTo(p1, p2) => Some(kurbo::QuadBez::new(point, p1, p2).raise()),
            PathEl::CurveTo(p1, p2, p3) => Some(kurbo::CubicBez::new(point, p1, p2, p3)),
            PathEl::ClosePath => {
                if ring.len() > 2 {
                    rings.push(std::mem::take(&mut ring));
                } else {
                    ring.clear();
                }
                None
            }
        };
        if let Some(curve) = curve {
            point = curve.p3;
            let mut pending = vec![(curve, 0u8)];
            while let Some((c, depth)) = pending.pop() {
                if cancelled() {
                    return Err(StrokeError::Cancelled);
                }
                // Control points inside a tolerance tube guarantee the whole convex-hull curve is inside it.
                let chord = c.p3 - c.p0;
                let distance = |p: Point| {
                    if chord.hypot() == 0.0 {
                        (p - c.p0).hypot()
                    } else {
                        let t = ((p - c.p0).dot(chord) / chord.hypot2()).clamp(0.0, 1.0);
                        (p - (c.p0 + chord * t)).hypot()
                    }
                };
                if distance(c.p1).max(distance(c.p2)) <= tolerance {
                    *generated += 1;
                    if *generated > element_cap {
                        return Err(StrokeError::LimitExceeded);
                    }
                    checked([c.p3.x, c.p3.y])?;
                    ring.push([c.p3.x, c.p3.y]);
                } else {
                    if depth == 48 || pending.len() > element_cap {
                        return Err(StrokeError::LimitExceeded);
                    }
                    pending.push((c.subsegment(0.5..1.0), depth + 1));
                    pending.push((c.subsegment(0.0..0.5), depth + 1));
                }
            }
        } else {
            *generated += 1;
            if *generated > element_cap {
                return Err(StrokeError::LimitExceeded);
            }
        }
    }
    if !ring.is_empty() {
        rings.push(ring);
    }
    for p in rings.iter().flatten() {
        checked(*p)?;
    }
    Ok(rings)
}
fn normalized(rings: &[Vec<[f64; 2]>], rule: FillRule) -> Vec<Vec<[f64; 2]>> {
    let empty: Vec<Vec<[f64; 2]>> = Vec::new();
    rings.overlay_as::<i64>(&empty, OverlayRule::Union, rule).into_iter().flatten().collect()
}
fn append_seg(out: &mut BezPath, seg: PathSeg) {
    match seg {
        PathSeg::Line(s) => out.line_to(s.p1),
        PathSeg::Quad(s) => out.quad_to(s.p1, s.p2),
        PathSeg::Cubic(s) => out.curve_to(s.p1, s.p2, s.p3),
    }
}
fn metrics(p: &BezPath) -> Result<Vec<(PathSeg, f64)>, StrokeError> {
    p.segments()
        .filter_map(|s| {
            let len = s.arclen(ARC);
            (len > 0.0 || !len.is_finite()).then_some((s, len))
        })
        .map(|(s, l)| if l.is_finite() { Ok((s, l)) } else { Err(StrokeError::Numeric) })
        .collect()
}
fn slice(segs: &[(PathSeg, f64)], from: f64, to: f64) -> BezPath {
    let mut p = BezPath::new();
    let mut at = 0.0;
    for &(seg, len) in segs {
        if from < at + len && to > at {
            let t0 = seg.inv_arclen((from - at).max(0.0), ARC);
            let t1 = seg.inv_arclen((to - at).min(len), ARC);
            let s = seg.subsegment(t0..t1);
            if p.is_empty() {
                p.move_to(s.eval(0.0));
            }
            append_seg(&mut p, s);
        }
        at += len;
    }
    p
}
fn tangent(seg: PathSeg, start: bool) -> Option<Vec2> {
    let c = seg.to_cubic();
    let points = if start { [c.p1 - c.p0, c.p2 - c.p0, c.p3 - c.p0] } else { [c.p3 - c.p2, c.p3 - c.p1, c.p3 - c.p0] };
    points.into_iter().find(|p| p.hypot() > 0.0).map(|p| p / p.hypot())
}
fn point_at(segs: &[(PathSeg, f64)], mut distance: f64) -> (Point, Vec2) {
    for &(seg, len) in segs {
        if distance <= len {
            let t = seg.inv_arclen(distance, ARC);
            let c = seg.to_cubic();
            let derivative = (c.p1 - c.p0) * (3.0 * (1.0 - t).powi(2))
                + (c.p2 - c.p1) * (6.0 * t * (1.0 - t))
                + (c.p3 - c.p2) * (3.0 * t * t);
            return (
                seg.eval(t),
                if derivative.hypot() > 0.0 {
                    derivative / derivative.hypot()
                } else {
                    tangent(seg, true).unwrap_or(Vec2::new(1.0, 0.0))
                },
            );
        }
        distance -= len;
    }
    segs.last()
        .map(|(s, _)| (s.eval(1.0), tangent(*s, false).unwrap_or(Vec2::new(1.0, 0.0))))
        .unwrap_or((Point::ZERO, Vec2::new(1.0, 0.0)))
}
/// Evaluation is checked and cancellable; no cache is stored in Document.
pub fn evaluate(path: &Path, tolerance: f64, cancelled: &dyn Fn() -> bool) -> Result<StrokeCoverage, StrokeError> {
    evaluate_capped(path, tolerance, MAX_ELEMENTS, cancelled)
}

/// Checked evaluation with a caller-owned per-path cap; exports retain their original cap.
pub fn evaluate_capped(
    path: &Path,
    tolerance: f64,
    element_cap: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<StrokeCoverage, StrokeError> {
    path.stroke_style.validate(path.id).map_err(|e| StrokeError::Invalid(e.to_string()))?;
    if !tolerance.is_finite() || tolerance <= 0.0 || !path.stroke_width.is_finite() || path.stroke_width < 0.0 {
        return Err(StrokeError::Numeric);
    }
    if element_cap < MAX_ELEMENTS
        && path.anchors.len().saturating_add(path.holes.iter().map(Vec::len).sum::<usize>()) > element_cap / 4
    {
        return Err(StrokeError::LimitExceeded);
    }
    let mut result = StrokeCoverage::default();
    if path.stroke_width == 0.0 || path.stroke.solid().is_none() {
        return Ok(result);
    }
    if cancelled() {
        return Err(StrokeError::Cancelled);
    }
    let s = &path.stroke_style;
    let align = if !path.closed && s.align != StrokeAlign::Center {
        note(&mut result.report, path.id, "stroke_align_open_center");
        StrokeAlign::Center
    } else {
        s.align
    };
    if path.closed && (s.arrows.start.is_some() || s.arrows.end.is_some()) {
        note(&mut result.report, path.id, "stroke_arrows_closed_ignored");
    }
    let width = f64::from(path.stroke_width) * if align == StrokeAlign::Center { 1.0 } else { 2.0 };
    let cap = match s.cap {
        StrokeCap::Butt => Cap::Butt,
        StrokeCap::Round => Cap::Round,
        StrokeCap::Square => Cap::Square,
    };
    let join = match s.join {
        StrokeJoin::Miter => Join::Miter,
        StrokeJoin::Round => Join::Round,
        StrokeJoin::Bevel => Join::Bevel,
    };
    let style = kurbo::Stroke::new(width).with_caps(cap).with_join(join).with_miter_limit(f64::from(s.miter_limit));
    let mut shaft = Vec::new();
    let mut heads_region = Vec::new();
    let mut runs_count = 0usize;
    let mut generated = 0usize;
    for (index, (anchors, closed)) in
        std::iter::once((&path.anchors, path.closed)).chain(path.holes.iter().map(|h| (h, true))).enumerate()
    {
        let curve = stroke_adapter::contour(anchors, closed);
        if !curve.is_finite() {
            return Err(StrokeError::Numeric);
        }
        let mut segs = metrics(&curve)?;
        if closed && s.align_dashes_to_corners && !s.dash.is_empty() && segs.len() > 1 {
            let is_corner = |a: PathSeg, b: PathSeg| {
                tangent(a, false)
                    .zip(tangent(b, true))
                    .is_some_and(|(a, b)| a.cross(b).atan2(a.dot(b)).abs() > 0.000001)
            };
            // An authored seam in a smooth run is not a fitting corner. Rotate only transient
            // metrics to a real corner so the run across that seam gets one period fit.
            if !is_corner(segs[segs.len() - 1].0, segs[0].0) {
                if let Some(corner) = segs.windows(2).position(|pair| is_corner(pair[0].0, pair[1].0)) {
                    segs.rotate_left(corner + 1);
                }
            }
        }
        let length: f64 = segs.iter().map(|(_, l)| l).sum();
        if !length.is_finite() {
            return Err(StrokeError::Numeric);
        }
        let mut trim = [0.0; 2];
        if index == 0 && !closed {
            if segs.is_empty() && (s.arrows.start.is_some() || s.arrows.end.is_some()) {
                note(&mut result.report, path.id, "stroke_arrow_no_tangent");
            }
            for (end, head, scale) in [(0, s.arrows.start, s.arrows.scale_start), (1, s.arrows.end, s.arrows.scale_end)]
            {
                if let (Some(head), Some((seg, _))) = (head, if end == 0 { segs.first() } else { segs.last() }) {
                    let dir = tangent(*seg, end == 0).ok_or(StrokeError::Numeric)? * if end == 0 { -1.0 } else { 1.0 };
                    let endpoint = seg.eval(if end == 0 { 0.0 } else { 1.0 });
                    let (geometry, inset) = heads::geometry(head);
                    let size = f64::from(path.stroke_width) * f64::from(scale);
                    let inset = inset * size;
                    if s.arrows.align == ArrowAlign::Tip {
                        trim[end] = inset + if cap == Cap::Butt { 0.0 } else { width * 0.5 };
                    }
                    let placed = heads::place(
                        geometry,
                        endpoint,
                        dir,
                        size,
                        if s.arrows.align == ArrowAlign::Extend { inset } else { 0.0 },
                    );
                    heads_region.extend(normalized(
                        &flat(&placed, tolerance, cancelled, &mut generated, element_cap)?,
                        FillRule::EvenOdd,
                    ));
                }
            }
        }
        if length == 0.0 {
            if let Some(anchor) = anchors.first() {
                use kurbo::Shape;
                let x = f64::from(anchor.p[0]);
                let y = f64::from(anchor.p[1]);
                let r = width * 0.5;
                if !closed && cap != Cap::Butt {
                    let shape = if cap == Cap::Round {
                        kurbo::Circle::new((x, y), r).to_path(tolerance)
                    } else {
                        kurbo::Rect::new(x - r, y - r, x + r, y + r).to_path(tolerance)
                    };
                    shaft.extend(flat(&shape, tolerance, cancelled, &mut generated, element_cap)?);
                }
            }
            continue;
        }
        if trim[0] + trim[1] >= length {
            continue;
        }
        let mut sections = vec![(0.0, length)];
        if s.align_dashes_to_corners && !s.dash.is_empty() {
            sections.clear();
            let mut from = 0.0;
            let mut at = 0.0;
            for pair in segs.windows(2) {
                at += pair[0].1;
                if let (Some(a), Some(b)) = (tangent(pair[0].0, false), tangent(pair[1].0, true)) {
                    if a.cross(b).atan2(a.dot(b)).abs() > 0.000001 {
                        sections.push((from, at));
                        from = at;
                    }
                }
            }
            sections.push((from, length));
        }
        let mut intervals = Vec::new();
        let mut dots = Vec::new();
        if s.dash.is_empty() {
            intervals.push((trim[0], length - trim[1]));
        } else {
            let period: f64 = s.dash.iter().map(|v| f64::from(*v)).sum();
            for (lo, hi) in sections {
                let span = hi - lo;
                let n = (span / period + 0.5).floor().max(1.0);
                let scale = if s.align_dashes_to_corners { span / (n * period) } else { 1.0 };
                let pattern: Vec<f64> = s.dash.iter().map(|v| f64::from(*v) * scale).collect();
                let per = period * scale;
                if !per.is_finite()
                    || per <= 0.0
                    || span / per * (pattern.len() / 2) as f64 > MAX_RUNS.min(element_cap / 8) as f64
                {
                    return Err(StrokeError::LimitExceeded);
                }
                let phase =
                    if s.align_dashes_to_corners { pattern[0] * 0.5 } else { f64::from(s.dash_phase).rem_euclid(per) };
                let mut at = lo - phase;
                while at <= hi {
                    for pair in pattern.as_chunks::<2>().0 {
                        let a = at.max(lo).max(trim[0]);
                        let b = (at + pair[0]).min(hi).min(length - trim[1]);
                        if pair[0] == 0.0 && at >= lo && at <= hi && at >= trim[0] && at <= length - trim[1] {
                            dots.push(at);
                        } else if b > a {
                            intervals.push((a, b));
                        }
                        at += pair[0] + pair[1];
                        runs_count += 1;
                        if runs_count > MAX_RUNS.min(element_cap / 8) {
                            return Err(StrokeError::LimitExceeded);
                        }
                    }
                    if cancelled() {
                        return Err(StrokeError::Cancelled);
                    }
                }
            }
        }
        // Zero gaps join intervals instead of introducing artificial dash caps.
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (a, b) in intervals {
            if let Some(last) = merged.last_mut() {
                if a <= last.1 + 1e-10 {
                    last.1 = last.1.max(b);
                    continue;
                }
            }
            merged.push((a, b));
        }
        let mut curves = Vec::new();
        if closed
            && merged.len() > 1
            && merged.first().is_some_and(|x| x.0 == 0.0)
            && merged.last().is_some_and(|x| x.1 == length)
        {
            let first = merged.remove(0);
            if let Some(last) = merged.pop() {
                let mut wrap = slice(&segs, last.0, length);
                let first_curve = slice(&segs, 0.0, first.1);
                wrap.extend(first_curve.elements().iter().copied().skip(1));
                curves.push(wrap);
            }
        }
        for (a, b) in merged {
            let mut p = slice(&segs, a, b);
            if closed && a == 0.0 && b == length {
                p.close_path();
            }
            curves.push(p);
        }
        for p in curves {
            if cancelled() {
                return Err(StrokeError::Cancelled);
            }
            let outline = kurbo::stroke(p, &style, &kurbo::StrokeOpts::default(), tolerance);
            shaft.extend(flat(&outline, tolerance, cancelled, &mut generated, element_cap)?);
            if shaft.iter().map(Vec::len).sum::<usize>() > element_cap {
                return Err(StrokeError::LimitExceeded);
            }
        }
        for distance in dots {
            if cap == Cap::Butt {
                continue;
            }
            let (p, t) = point_at(&segs, distance);
            let mut dot = BezPath::new();
            dot.move_to(p);
            dot.line_to(p + t * 1e-10);
            shaft.extend(flat(
                &kurbo::stroke(dot, &style, &kurbo::StrokeOpts::default(), tolerance),
                tolerance,
                cancelled,
                &mut generated,
                element_cap,
            )?);
        }
    }
    let mut region = normalized(&shaft, FillRule::NonZero);
    if align != StrokeAlign::Center {
        let inside = normalized(
            &flat(&stroke_adapter::path(path), tolerance, cancelled, &mut generated, element_cap)?,
            FillRule::EvenOdd,
        );
        region = region
            .overlay_as::<i64>(
                &inside,
                if align == StrokeAlign::Inside { OverlayRule::Intersect } else { OverlayRule::Difference },
                FillRule::EvenOdd,
            )
            .into_iter()
            .flatten()
            .collect();
    }
    if !heads_region.is_empty() {
        // Normalize heads independently as even-odd (holes), then union with the shaft.
        region = region
            .overlay_as::<i64>(&heads_region, OverlayRule::Union, FillRule::NonZero)
            .into_iter()
            .flatten()
            .collect();
    }
    if cancelled() {
        return Err(StrokeError::Cancelled);
    }
    if region.iter().map(Vec::len).sum::<usize>() > element_cap {
        return Err(StrokeError::LimitExceeded);
    }
    result.rings = region.into_iter().map(|r| r.into_iter().map(checked).collect()).collect::<Result<_, _>>()?;
    generated = generated.saturating_add(result.rings.iter().map(Vec::len).sum::<usize>());
    generated = generated.saturating_add(triangles_capped(&result.rings, element_cap)?.len().saturating_mul(3));
    if generated > element_cap {
        return Err(StrokeError::LimitExceeded);
    }
    result.generated_elements = generated;
    Ok(result)
}
/// Disjoint trapezoids of a normalized even-odd polygon region, for positive coverage triangles.
/// Horizontal bands preserve holes; no fan triangle can leak outside a concave contour.
pub fn triangles(rings: &[Vec<Pt>]) -> Result<Vec<[Pt; 3]>, StrokeError> {
    triangles_capped(rings, MAX_ELEMENTS)
}

pub(crate) fn triangles_capped(rings: &[Vec<Pt>], element_cap: usize) -> Result<Vec<[Pt; 3]>, StrokeError> {
    if rings.iter().flatten().flatten().any(|value| !value.is_finite()) {
        return Err(StrokeError::Numeric);
    }
    let mut ys: Vec<f64> = rings.iter().flatten().map(|p| f64::from(p[1])).collect();
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    let edges: Vec<_> = rings
        .iter()
        .flat_map(|r| r.iter().zip(r.iter().cycle().skip(1)).take(r.len()))
        .filter(|(a, b)| a[1] != b[1])
        .collect();
    if ys.len().saturating_mul(edges.len()) > 20_000_000 {
        return Err(StrokeError::LimitExceeded);
    }
    let mut out = Vec::new();
    for y in ys.windows(2) {
        let mid = (y[0] + y[1]) * 0.5;
        let x = |a: Pt, b: Pt, y: f64| {
            f64::from(a[0])
                + (y - f64::from(a[1])) * (f64::from(b[0]) - f64::from(a[0])) / (f64::from(b[1]) - f64::from(a[1]))
        };
        let mut active: Vec<_> = edges
            .iter()
            .copied()
            .filter(|(a, b)| mid > f64::from(a[1].min(b[1])) && mid < f64::from(a[1].max(b[1])))
            .collect();
        active.sort_by(|(a, b), (c, d)| x(**a, **b, mid).total_cmp(&x(**c, **d, mid)));
        for pair in active.as_chunks::<2>().0 {
            let (a, b) = pair[0];
            let (c, d) = pair[1];
            let q = [
                checked([x(*a, *b, y[0]), y[0]])?,
                checked([x(*c, *d, y[0]), y[0]])?,
                checked([x(*c, *d, y[1]), y[1]])?,
                checked([x(*a, *b, y[1]), y[1]])?,
            ];
            out.extend([[q[0], q[1], q[2]], [q[0], q[2], q[3]]]);
            if out.len() * 3 > element_cap {
                return Err(StrokeError::LimitExceeded);
            }
        }
    }
    Ok(out)
}

/// Filled coverage plus the existing screen-space pick slop around its boundary.
pub fn contains(rings: &[Vec<Pt>], point: Pt, slop: f32) -> bool {
    let inside = rings.iter().filter(|r| crate::geom::point_in_poly(r, point)).count() % 2 == 1;
    inside
        || rings.iter().any(|ring| {
            ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()).any(|(&a, &b)| {
                let d = [b[0] - a[0], b[1] - a[1]];
                let l = d[0] * d[0] + d[1] * d[1];
                let t = if l > 0.0 {
                    (((point[0] - a[0]) * d[0] + (point[1] - a[1]) * d[1]) / l).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                crate::geom::dist(point, [a[0] + d[0] * t, a[1] + d[1] * t]) <= slop
            })
        })
}

/// Aggregate budget shared by a checked edit or export job, including intermediate outlines.
#[derive(Default)]
pub struct StrokeBudget {
    generated: usize,
}
impl StrokeBudget {
    pub fn charge(&mut self, coverage: &StrokeCoverage) -> Result<(), StrokeError> {
        self.generated = self.generated.saturating_add(coverage.generated_elements);
        if self.generated > MAX_ELEMENTS {
            Err(StrokeError::LimitExceeded)
        } else {
            Ok(())
        }
    }
}

/// Intersect evaluated coverage with the same even-odd mask rings used by the scene.
pub fn intersect(rings: &[Vec<Pt>], mask: &[Vec<Pt>]) -> Result<Vec<Vec<Pt>>, StrokeError> {
    let convert = |rings: &[Vec<Pt>]| {
        rings
            .iter()
            .map(|ring| ring.iter().map(|p| [f64::from(p[0]), f64::from(p[1])]).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let result: Vec<Vec<[f64; 2]>> = convert(rings)
        .overlay_as::<i64>(&convert(mask), OverlayRule::Intersect, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .collect();
    if result.iter().map(Vec::len).sum::<usize>() > MAX_ELEMENTS {
        return Err(StrokeError::LimitExceeded);
    }
    result.into_iter().map(|ring| ring.into_iter().map(checked).collect()).collect()
}

/// Whether the authored outer subpath has length, for native stroker eligibility.
/// Nondefault degenerate styles use explicit coverage instead of backend-specific zero-line rules.
pub fn has_length(path: &Path) -> bool {
    let segment_has_length = |a: &crate::model::Anchor, b: &crate::model::Anchor| {
        a.p != b.p || a.hout.is_some_and(|q| q != a.p) || b.hin.is_some_and(|q| q != a.p)
    };
    path.anchors.windows(2).any(|pair| segment_has_length(&pair[0], &pair[1]))
        || (path.closed
            && path.anchors.len() > 1
            && path
                .anchors
                .first()
                .zip(path.anchors.last())
                .is_some_and(|(first, last)| segment_has_length(last, first)))
}
