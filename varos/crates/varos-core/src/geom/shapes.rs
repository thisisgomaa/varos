// Adapted from VectorCraft crates/geom/src/shapes.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Deterministic shape geometry. Angles are radians, radii are canvas units.
//! Generated IDs are zero: callers must allocate document IDs before insertion.
use super::{
    kurbo::{anchor, path, point, pt},
    Pt,
};
use crate::model::Path;
use ::kurbo::Point;
use std::f64::consts::{FRAC_PI_2, TAU};
/// Required quarter-circle cubic approximation.
pub const KAPPA: f64 = 0.5523;
fn poly(points: impl IntoIterator<Item = Point>, closed: bool) -> Path {
    path(points.into_iter().map(anchor).collect(), closed)
}
fn bounds(b: [f32; 4]) -> [f64; 4] {
    [b[0].min(b[2]) as f64, b[1].min(b[3]) as f64, b[0].max(b[2]) as f64, b[1].max(b[3]) as f64]
}
/// Rounded bounds `[x0,y0,x1,y1]`; radii TL, TR, BR, BL, clamped to half the shorter side.
/// Zero-radius corners use one anchor, rounded corners two.
pub fn rounded_rectangle(b: [f32; 4], radii: [f32; 4]) -> Path {
    let [x0, y0, x1, y1] = bounds(b);
    let limit = ((x1 - x0).min(y1 - y0) / 2.0).max(0.0);
    let corners = [
        (Point::new(x0, y0), Point::new(0., -1.), Point::new(1., 0.)),
        (Point::new(x1, y0), Point::new(1., 0.), Point::new(0., 1.)),
        (Point::new(x1, y1), Point::new(0., 1.), Point::new(-1., 0.)),
        (Point::new(x0, y1), Point::new(-1., 0.), Point::new(0., -1.)),
    ];
    let mut anchors = Vec::new();
    for (i, (c, incoming, outgoing)) in corners.into_iter().enumerate() {
        let r = (radii[i] as f64).max(0.0).min(limit);
        if r == 0.0 {
            anchors.push(anchor(c));
            continue;
        }
        let start = c - incoming.to_vec2() * r;
        let end = c + outgoing.to_vec2() * r;
        let mut a = anchor(start);
        a.hout = Some(pt(start + incoming.to_vec2() * (r * KAPPA)));
        a.smooth = true;
        let mut z = anchor(end);
        z.hin = Some(pt(end - outgoing.to_vec2() * (r * KAPPA)));
        z.smooth = true;
        anchors.extend([a, z]);
    }
    path(anchors, true)
}
/// Regular polygon, first vertex up; sides are clamped to 3–1000.
pub fn polygon(centre: Pt, radius: f32, sides: usize, rotation: f32) -> Path {
    let n = sides.clamp(3, 1000);
    poly((0..n).map(|i| radial(centre, radius as f64, -FRAC_PI_2 + rotation as f64 + TAU * i as f64 / n as f64)), true)
}
/// Alternating outer/inner radii, first tip up. Points clamped to 2–1000.
pub fn star(centre: Pt, r1: f32, r2: f32, points: usize, rotation: f32) -> Path {
    let n = points.clamp(2, 1000) * 2;
    poly(
        (0..n).map(|i| {
            radial(
                centre,
                if i % 2 == 0 { r1 } else { r2 } as f64,
                -FRAC_PI_2 + rotation as f64 + TAU * i as f64 / n as f64,
            )
        }),
        true,
    )
}
fn radial(c: Pt, r: f64, angle: f64) -> Point {
    point(c) + ::kurbo::Vec2::new(angle.cos() * r, angle.sin() * r)
}
/// Open segment with two corner anchors.
pub fn line(a: Pt, b: Pt) -> Path {
    poly([point(a), point(b)], false)
}
/// How to close an elliptical arc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcClosure {
    Open,
    Pie,
    Chord,
}
/// Elliptical arc with signed sweep, split into cubics of at most a quarter turn.
/// Sweep is clamped to one revolution. A complete revolution has no duplicate endpoint.
pub fn arc(centre: Pt, radii: Pt, start: f32, sweep: f32, closure: ArcClosure) -> Path {
    let sweep = (sweep as f64).clamp(-TAU, TAU);
    if !start.is_finite() || !sweep.is_finite() {
        return path(Vec::new(), false);
    }
    let n = (sweep.abs() / FRAC_PI_2 - 1e-7).ceil().max(1.) as usize;
    let step = sweep / n as f64;
    let (rx, ry) = (radii[0].abs() as f64, radii[1].abs() as f64);
    let pos = |a: f64| point(centre) + ::kurbo::Vec2::new(rx * a.cos(), ry * a.sin());
    let tangent = |a: f64| ::kurbo::Vec2::new(-rx * a.sin(), ry * a.cos());
    let mut anchors = vec![anchor(pos(start as f64))];
    for i in 0..n {
        let a = start as f64 + i as f64 * step;
        let b = a + step;
        let k = 4. / 3. * (step / 4.).tan();
        anchors[i].hout = Some(pt(pos(a) + tangent(a) * k));
        anchors[i].smooth = true;
        let mut end = anchor(pos(b));
        end.hin = Some(pt(pos(b) - tangent(b) * k));
        end.smooth = true;
        anchors.push(end);
    }
    let full = (sweep.abs() - TAU).abs() < 1e-6;
    if full {
        if let Some(last) = anchors.pop() {
            anchors[0].hin = last.hin;
        }
    } else if closure != ArcClosure::Open {
        anchors[0].smooth = false;
        if let Some(last) = anchors.last_mut() {
            last.smooth = false;
        }
        if closure == ArcClosure::Pie {
            anchors.push(anchor(point(centre)));
        }
    }
    path(anchors, full || closure != ArcClosure::Open)
}
/// Logarithmic inward spiral; `decay` is the remaining radius per full turn (0 < decay <= 1).
/// Cubic Hermite handles use the analytic spiral tangent. Segments clamped to 1–16384.
pub fn spiral(centre: Pt, radius: f32, turns: f32, decay: f32, segments: usize) -> Path {
    let n = segments.clamp(1, 16384);
    let turns = (turns as f64).clamp(-1000., 1000.);
    let log = (decay as f64).clamp(0.0001, 1.).ln();
    let dt = 1. / n as f64;
    let mut anchors = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let t = i as f64 * dt;
        let a = TAU * turns * t;
        let r = radius as f64 * (log * turns.abs() * t).exp();
        let p = radial(centre, r, a);
        let dr = r * log * turns.abs();
        let da = TAU * turns;
        let d = ::kurbo::Vec2::new(dr * a.cos() - r * da * a.sin(), dr * a.sin() + r * da * a.cos()) * (dt / 3.);
        let mut z = anchor(p);
        z.hin = (i > 0).then(|| pt(p - d));
        z.hout = (i < n).then(|| pt(p + d));
        z.smooth = true;
        anchors.push(z);
    }
    path(anchors, false)
}
/// Grid of `rows × cols` cells, including perimeter lines; zero dimensions produce no paths.
/// Independent open lines must remain separate Paths (holes are closed rings).
pub fn rectangular_grid(rows: usize, cols: usize, b: [f32; 4]) -> Vec<Path> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let (rows, cols) = (rows.min(1000), cols.min(1000));
    let [x0, y0, x1, y1] = bounds(b);
    let mut out = Vec::new();
    for i in 0..=rows {
        let y = (y0 + (y1 - y0) * i as f64 / rows as f64) as f32;
        out.push(line([x0 as f32, y], [x1 as f32, y]));
    }
    for i in 0..=cols {
        let x = (x0 + (x1 - x0) * i as f64 / cols as f64) as f32;
        out.push(line([x, y0 as f32], [x, y1 as f32]));
    }
    out
}
/// Concentric elliptical rings (including outer ring) and centre-to-edge radial segments.
pub fn polar_grid(rings: usize, radials: usize, centre: Pt, radii: Pt) -> Vec<Path> {
    let (rings, radials) = (rings.min(1000), radials.min(1000));
    let mut out = Vec::new();
    for i in 1..=rings {
        let f = i as f32 / rings as f32;
        out.push(arc(centre, [radii[0] * f, radii[1] * f], 0., TAU as f32, ArcClosure::Chord));
    }
    for i in 0..radials {
        let a = TAU * i as f64 / radials as f64;
        out.push(line(
            centre,
            pt(point(centre) + ::kurbo::Vec2::new(radii[0] as f64 * a.cos(), radii[1] as f64 * a.sin())),
        ));
    }
    out
}
