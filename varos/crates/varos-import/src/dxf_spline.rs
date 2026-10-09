//! Original nonrational B-spline conversion: each knot span is a polynomial of degree <= 3.
//! Interpolation at thirds reconstructs its cubic Bezier controls; no sampling approximation.
use varos_core::model::{Anchor, Document};
fn evaluate(points: &[[f64; 2]], knots: &[f64], degree: usize, span: usize, t: f64) -> Result<[f64; 2], String> {
    let mut work = points[span - degree..=span].to_vec();
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = span - degree + j;
            let denominator = knots[i + degree + 1 - r] - knots[i];
            if denominator <= 0. {
                return Err("Degenerate DXF spline knot span".into());
            }
            let alpha = (t - knots[i]) / denominator;
            let previous = work[j - 1];
            for (current, previous) in work[j].iter_mut().zip(previous) {
                *current = (1. - alpha) * previous + alpha * *current;
            }
        }
    }
    Ok(work[degree])
}
pub(crate) fn convert(
    doc: &mut Document,
    points: &[[f64; 2]],
    knots: &[f64],
    degree: usize,
) -> Result<Vec<Anchor>, String> {
    if !(1..=3).contains(&degree)
        || points.len() <= degree
        || knots.len() != points.len() + degree + 1
        || points.len() > 50_000
        || knots.iter().any(|v| !v.is_finite())
        || knots.windows(2).any(|w| w[0] > w[1])
    {
        return Err("Unsupported/malformed DXF spline degree, controls or knots".into());
    }
    let start = knots[degree];
    let end = knots[points.len()];
    if start >= end || knots[..=degree].iter().any(|k| *k != start) || knots[points.len()..].iter().any(|k| *k != end) {
        return Err("DXF splines require clamped endpoints".into());
    }
    let mut previous = start;
    let mut repeated = 0;
    for knot in &knots[degree + 1..points.len()] {
        repeated = if *knot == previous { repeated + 1 } else { 1 };
        previous = *knot;
        if repeated > degree {
            return Err("Discontinuous DXF spline unsupported".into());
        }
    }
    let native = |p: [f64; 2]| -> Result<[f32; 2], String> {
        let out = [p[0] as f32, p[1] as f32];
        if p.iter().zip(out).any(|(a, b)| !a.is_finite() || (a - b as f64).abs() > 0.002) {
            return Err("DXF spline exceeds 0.01 point conversion precision".into());
        }
        Ok(out)
    };
    let mut anchors: Vec<Anchor> = Vec::new();
    for span in degree..points.len() {
        crate::control::checkpoint()?;
        let a = knots[span];
        let b = knots[span + 1];
        if a == b {
            continue;
        }
        let p0 = evaluate(points, knots, degree, span, a)?;
        let p3 = evaluate(points, knots, degree, span, b)?;
        let q1 = evaluate(points, knots, degree, span, a + (b - a) / 3.)?;
        let q2 = evaluate(points, knots, degree, span, a + 2. * (b - a) / 3.)?;
        let mut c1 = [0.; 2];
        let mut c2 = [0.; 2];
        for axis in 0..2 {
            c1[axis] = (18. * q1[axis] - 9. * q2[axis] - 5. * p0[axis] + 2. * p3[axis]) / 6.;
            c2[axis] = (-9. * q1[axis] + 18. * q2[axis] + 2. * p0[axis] - 5. * p3[axis]) / 6.;
        }
        if anchors.is_empty() {
            anchors.push(crate::anchor(doc, native(p0)?));
        }
        let last = anchors.last_mut().ok_or("DXF spline has no starting point")?;
        if last.p != native(p0)? {
            return Err("Discontinuous DXF spline unsupported".into());
        }
        last.hout = Some(native(c1)?);
        let mut next = crate::anchor(doc, native(p3)?);
        next.hin = Some(native(c2)?);
        anchors.push(next);
    }
    if anchors.len() < 2 {
        return Err("DXF spline has no positive knot spans".into());
    }
    Ok(anchors)
}
