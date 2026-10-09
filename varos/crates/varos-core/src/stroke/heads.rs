//! Original geometric head library. Coordinates are width multiples, outward tip at (0, 0).
//! Positive x points outward. Hollow marks contain real inner contours; no artwork is borrowed.
use super::ArrowHead;
use kurbo::{BezPath, Point, Shape};
fn polygon(points: &[(f64, f64)]) -> BezPath {
    let mut p = BezPath::new();
    if let Some(&first) = points.first() {
        p.move_to(first);
        for &point in &points[1..] {
            p.line_to(point);
        }
        p.close_path();
    }
    p
}
fn rect(x: f64, y: f64, w: f64, h: f64) -> BezPath {
    polygon(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h)])
}
fn regular(n: usize, radius: f64, cx: f64) -> BezPath {
    polygon(
        &(0..n)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / n as f64;
                (cx + radius * a.cos(), radius * a.sin())
            })
            .collect::<Vec<_>>(),
    )
}
fn append(p: &mut BezPath, other: BezPath) {
    p.extend(other.elements().iter().copied());
}
/// Returns filled even-odd head geometry and the shaft attachment inset.
pub fn geometry(kind: ArrowHead) -> (BezPath, f64) {
    use ArrowHead::*;
    let mut p = match kind {
        Triangle | TriangleOpen => polygon(&[(0.0, 0.0), (-4.0, 2.0), (-4.0, -2.0)]),
        Circle | CircleOpen | Target | DotOnBar => kurbo::Circle::new((-2.0, 0.0), 2.0).to_path(0.001),
        Square | SquareOpen => rect(-4.0, -2.0, 4.0, 4.0),
        Bar | DoubleBar => rect(-0.5, -2.0, 0.5, 4.0),
        Diamond => polygon(&[(0.0, 0.0), (-2.0, 2.0), (-4.0, 0.0), (-2.0, -2.0)]),
        Arrow | ArrowOpen | Chevron | DoubleArrow => polygon(&[(0.0, 0.0), (-4.0, 2.0), (-2.8, 0.0), (-4.0, -2.0)]),
        Barbed => polygon(&[(0.0, 0.0), (-4.0, 2.0), (-3.0, 0.0), (-4.0, -2.0)]),
        HalfArrowLeft => polygon(&[(0.0, 0.0), (-4.0, -2.0), (-3.0, 0.0)]),
        HalfArrowRight => polygon(&[(0.0, 0.0), (-3.0, 0.0), (-4.0, 2.0)]),
        Concave => polygon(&[(0.0, 0.0), (-4.0, 2.0), (-2.0, 0.0), (-4.0, -2.0)]),
        Feather => polygon(&[(0.0, 0.0), (-2.0, 2.0), (-5.0, 2.0), (-3.0, 0.0), (-5.0, -2.0), (-2.0, -2.0)]),
        Star => polygon(
            &(0..10)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 2.0 } else { 0.9 };
                    (-2.0 + r * a.cos(), r * a.sin())
                })
                .collect::<Vec<_>>(),
        ),
        Cross => polygon(&[
            (0.0, 1.5),
            (-0.5, 2.0),
            (-2.0, 0.5),
            (-3.5, 2.0),
            (-4.0, 1.5),
            (-2.5, 0.0),
            (-4.0, -1.5),
            (-3.5, -2.0),
            (-2.0, -0.5),
            (-0.5, -2.0),
            (0.0, -1.5),
            (-1.5, 0.0),
        ]),
        Plus => polygon(&[
            (0.0, -0.4),
            (0.0, 0.4),
            (-1.6, 0.4),
            (-1.6, 2.0),
            (-2.4, 2.0),
            (-2.4, 0.4),
            (-4.0, 0.4),
            (-4.0, -0.4),
            (-2.4, -0.4),
            (-2.4, -2.0),
            (-1.6, -2.0),
            (-1.6, -0.4),
        ]),
        Hexagon | HexagonOpen => regular(6, 2.0, -2.0),
        Tag | TagOpen => polygon(&[(0.0, 0.0), (-1.0, 2.0), (-4.0, 2.0), (-4.0, -2.0), (-1.0, -2.0)]),
        HalfCircle => polygon(
            &(0..=24)
                .map(|i| {
                    let a = std::f64::consts::PI / 2.0 + i as f64 * std::f64::consts::PI / 24.0;
                    (2.0 * a.cos(), 2.0 * a.sin())
                })
                .collect::<Vec<_>>(),
        ),
        Drop => {
            let mut p = BezPath::new();
            p.move_to((0.0, 0.0));
            p.curve_to((-2.0, 3.0), (-5.0, 2.0), (-5.0, 0.0));
            p.curve_to((-5.0, -2.0), (-2.0, -3.0), (0.0, 0.0));
            p.close_path();
            p
        }
    };
    let inner = match kind {
        TriangleOpen => Some(polygon(&[(-1.2, 0.0), (-3.4, -1.1), (-3.4, 1.1)])),
        CircleOpen | Target => Some(kurbo::Circle::new((-2.0, 0.0), 1.3).to_path(0.001)),
        SquareOpen => Some(rect(-3.4, -1.4, 2.8, 2.8)),
        HexagonOpen => Some(regular(6, 1.3, -2.0)),
        TagOpen => Some(polygon(&[(-0.8, 0.0), (-1.4, -1.3), (-3.4, -1.3), (-3.4, 1.3), (-1.4, 1.3)])),
        ArrowOpen => Some(polygon(&[(-1.1, 0.0), (-3.2, -1.1), (-2.3, 0.0), (-3.2, 1.1)])),
        _ => None,
    };
    if let Some(inner) = inner {
        append(&mut p, inner);
    }
    match kind {
        DoubleBar => append(&mut p, rect(-2.0, -2.0, 0.5, 4.0)),
        DoubleArrow => {
            let other = polygon(&[(-3.0, 0.0), (-7.0, 2.0), (-5.8, 0.0), (-7.0, -2.0)]);
            append(&mut p, other);
        }
        Target => append(&mut p, kurbo::Circle::new((-2.0, 0.0), 0.5).to_path(0.001)),
        DotOnBar => append(&mut p, rect(-4.5, -2.5, 0.5, 5.0)),
        _ => {}
    }
    let inset = match kind {
        Bar => 0.5,
        DoubleBar => 2.0,
        HalfCircle => 2.0,
        DoubleArrow => 7.0,
        Drop => 5.0,
        _ => 4.0,
    };
    (p, inset)
}
/// Place a head in document space using an outward tangent and an explicit endpoint.
pub fn place(head: BezPath, endpoint: Point, tangent: kurbo::Vec2, size: f64, advance: f64) -> BezPath {
    kurbo::Affine::new([
        tangent.x * size,
        tangent.y * size,
        -tangent.y * size,
        tangent.x * size,
        endpoint.x + tangent.x * advance,
        endpoint.y + tangent.y * advance,
    ]) * head
}
