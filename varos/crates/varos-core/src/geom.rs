//! Pure 2D geometry: points, vector math, cubic beziers, hit-testing math.
//! No rendering, no platform deps.

pub type Pt = [f32; 2];
pub type Rgba = [f32; 4];

/// The canvas camera: screen = world * zoom + pan.
#[derive(Clone, Copy)]
pub struct View { pub pan: Pt, pub zoom: f32 }
impl View {
    pub fn identity() -> Self { View { pan: [0.0, 0.0], zoom: 1.0 } }
    pub fn s2w(&self, s: Pt) -> Pt { [(s[0]-self.pan[0])/self.zoom, (s[1]-self.pan[1])/self.zoom] }
    pub fn w2s(&self, w: Pt) -> Pt { [w[0]*self.zoom+self.pan[0], w[1]*self.zoom+self.pan[1]] }

    /// Frame a world-space rect (x,y,w,h) centred in a `win_w`×`win_h` screen, leaving `pad` of the
    /// shorter axis as margin (pad 0.9 ≈ 10% breathing room). Used for fit-artboard / fit-in-window.
    pub fn fit(x: f32, y: f32, w: f32, h: f32, win_w: f32, win_h: f32, pad: f32) -> View {
        if w <= 0.0 || h <= 0.0 || win_w <= 0.0 || win_h <= 0.0 { return View::identity(); }
        let zoom = ((win_w / w).min(win_h / h) * pad).clamp(0.02, 64.0);
        let (cx, cy) = (x + w * 0.5, y + h * 0.5);
        View { zoom, pan: [win_w * 0.5 - cx * zoom, win_h * 0.5 - cy * zoom] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fit_centres_and_scales() {
        // a 1000×500 board in an 800×800 window, 10% margin → zoom limited by width: 0.8*0.9 = 0.72
        let v = View::fit(0.0, 0.0, 1000.0, 500.0, 800.0, 800.0, 0.9);
        assert!((v.zoom - 0.72).abs() < 1e-4, "zoom {}", v.zoom);
        // the board centre (500,250) must land at the window centre (400,400)
        let c = v.w2s([500.0, 250.0]);
        assert!((c[0] - 400.0).abs() < 1e-3 && (c[1] - 400.0).abs() < 1e-3, "centre {:?}", c);
    }
    #[test]
    fn fit_degenerate_is_identity() {
        assert_eq!(View::fit(0.0, 0.0, 0.0, 100.0, 800.0, 600.0, 0.9).zoom, 1.0);
    }
}

pub fn sub(a: Pt, b: Pt) -> Pt { [a[0] - b[0], a[1] - b[1]] }
pub fn add(a: Pt, b: Pt) -> Pt { [a[0] + b[0], a[1] + b[1]] }
pub fn scale(a: Pt, k: f32) -> Pt { [a[0] * k, a[1] * k] }
pub fn dist(a: Pt, b: Pt) -> f32 { ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() }
pub fn length(v: Pt) -> f32 { (v[0] * v[0] + v[1] * v[1]).sqrt() }
pub fn norm(v: Pt) -> Pt { let m = length(v).max(1e-4); [v[0] / m, v[1] / m] }
pub fn mirror(p: Pt, q: Pt) -> Pt { [2.0 * p[0] - q[0], 2.0 * p[1] - q[1]] }

/// Rotate `p` around pivot `c` by `ang` radians.
pub fn rotate_about(p: Pt, c: Pt, ang: f32) -> Pt {
    let (s, co) = ang.sin_cos();
    let d = sub(p, c);
    [c[0] + d[0]*co - d[1]*s, c[1] + d[0]*s + d[1]*co]
}

pub fn cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    [u*u*u*p0[0] + 3.0*u*u*t*p1[0] + 3.0*u*t*t*p2[0] + t*t*t*p3[0],
     u*u*u*p0[1] + 3.0*u*u*t*p1[1] + 3.0*u*t*t*p2[1] + t*t*t*p3[1]]
}

/// Constrain a vector to the nearest 45° direction (keeps its length).
pub fn snap45(v: Pt) -> Pt {
    let a = v[1].atan2(v[0]);
    let step = std::f32::consts::FRAC_PI_4;
    let s = (a / step).round() * step;
    let m = length(v);
    [s.cos() * m, s.sin() * m]
}

/// Even-odd point-in-polygon test.
pub fn point_in_poly(poly: &[Pt], pt: Pt) -> bool {
    let m = poly.len();
    if m < 3 { return false; }
    let mut inside = false;
    let mut j = m - 1;
    for i in 0..m {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > pt[1]) != (b[1] > pt[1]) {
            let x = (b[0] - a[0]) * (pt[1] - a[1]) / (b[1] - a[1]) + a[0];
            if pt[0] < x { inside = !inside; }
        }
        j = i;
    }
    inside
}
