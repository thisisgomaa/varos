//! Adapted from VectorCraft color/src/gradient.rs@a469568 (MIT OR Apache-2.0).
//! Lane B: bounded gradient data and midpoint interpolation; placement maps unit space to path space.
//! Persisted next-format gradient paint; shared interpolation for all backends.
use crate::geom::{Pt, Rgba};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spread {
    #[default]
    Pad,
    Reflect,
    Repeat,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stop {
    pub offset: f32,
    pub colour: Rgba,
    pub opacity: f32,
    pub midpoint: f32,
}
impl Stop {
    pub fn new(offset: f32, colour: Rgba) -> Self {
        Self { offset, colour, opacity: 1.0, midpoint: 0.5 }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<Stop>,
    pub spread: Spread,
    /// [a,b,c,d,e,f]: (x,y) -> (ax+cy+e,bx+dy+f). Linear x=0..1; radial unit circle.
    pub placement: [f32; 6],
    /// Radial focal point in unit gradient space (strictly inside the unit circle).
    pub focal: Pt,
}
impl Default for Gradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            stops: vec![Stop::new(0., [1.; 4]), Stop::new(1., [0., 0., 0., 1.])],
            spread: Spread::Pad,
            placement: [100., 0., 0., 100., 0., 0.],
            focal: [0.; 2],
        }
    }
}
impl Gradient {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=256).contains(&self.stops.len()) {
            return Err("gradient requires 2..256 stops".into());
        }
        let unit = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
        if self.stops.iter().any(|s| {
            !unit(s.offset)
                || !unit(s.opacity)
                || !s.colour.into_iter().all(unit)
                || !s.midpoint.is_finite()
                || !(0.01..=0.99).contains(&s.midpoint)
        }) {
            return Err("invalid gradient stop".into());
        }
        if self.stops.windows(2).any(|s| s[0].offset > s[1].offset) {
            return Err("gradient stops must be ordered".into());
        }
        let [a, b, c, d, _, _] = self.placement;
        if !self.placement.into_iter().all(f32::is_finite)
            || !(a * d - b * c).is_finite()
            || (a * d - b * c).abs() < 1e-8
        {
            return Err("singular gradient placement".into());
        }
        if !self.focal.into_iter().all(f32::is_finite) || self.focal[0].hypot(self.focal[1]) >= 0.999 {
            return Err("radial focal point must be inside gradient".into());
        }
        Ok(())
    }
    pub fn parameter(&self, p: Pt) -> f32 {
        let [a, b, c, d, e, f] = self.placement;
        let det = a * d - b * c;
        if det.abs() < 1e-8 {
            return 0.;
        }
        let p = [(d * (p[0] - e) - c * (p[1] - f)) / det, (-b * (p[0] - e) + a * (p[1] - f)) / det];
        match self.kind {
            GradientKind::Linear => p[0],
            GradientKind::Radial => {
                let q = [p[0] - self.focal[0], p[1] - self.focal[1]];
                let aa = q[0] * q[0] + q[1] * q[1];
                if aa < 1e-12 {
                    return 0.;
                }
                let bb = 2. * (q[0] * self.focal[0] + q[1] * self.focal[1]);
                let cc = self.focal[0] * self.focal[0] + self.focal[1] * self.focal[1] - 1.;
                2. * aa / (-bb + (bb * bb - 4. * aa * cc).max(0.).sqrt())
            }
        }
    }
    pub fn sample(&self, t: f32) -> Rgba {
        let t = match self.spread {
            Spread::Pad => t.clamp(0., 1.),
            Spread::Repeat => t.rem_euclid(1.),
            Spread::Reflect => {
                let u = t.rem_euclid(2.);
                if u > 1. {
                    2. - u
                } else {
                    u
                }
            }
        };
        self.sample_pad(t)
    }
    pub fn sample_pad(&self, t: f32) -> Rgba {
        let Some(first) = self.stops.first() else { return [0.; 4] };
        let colour = |s: &Stop| [s.colour[0], s.colour[1], s.colour[2], s.colour[3] * s.opacity];
        if t < first.offset {
            return colour(first);
        }
        for w in self.stops.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t < b.offset {
                let u = (t - a.offset) / (b.offset - a.offset).max(1e-8);
                let m = a.midpoint.clamp(0.01, 0.99);
                let v = if u < m { 0.5 * u / m } else { 0.5 + 0.5 * (u - m) / (1. - m) };
                let (a, b) = (colour(a), colour(b));
                return std::array::from_fn(|i| a[i] + (b[i] - a[i]) * v);
            }
        }
        self.stops.last().map_or([0.; 4], colour)
    }
    pub fn sample_point(&self, p: Pt) -> Rgba {
        self.sample(self.parameter(p))
    }
    pub fn lut(&self) -> Vec<Rgba> {
        (0..1024).map(|i| self.sample_pad(i as f32 / 1023.)).collect()
    }
    pub fn reverse(&mut self) {
        let mids: Vec<_> = self.stops.windows(2).map(|w| 1. - w[0].midpoint).rev().collect();
        self.stops.reverse();
        for (i, s) in self.stops.iter_mut().enumerate() {
            s.offset = 1. - s.offset;
            s.midpoint = mids.get(i).copied().unwrap_or(0.5);
        }
    }
    pub fn transformed(&self, xf: crate::model::Xform) -> Self {
        let mut g = self.clone();
        let [a, b, c, d, e, f] = g.placement;
        let o = xf.apply([e, f]);
        let x = xf.apply([e + a, f + b]);
        let y = xf.apply([e + c, f + d]);
        g.placement = [x[0] - o[0], x[1] - o[1], y[0] - o[0], y[1] - o[1], o[0], o[1]];
        g
    }
}

impl std::hash::Hash for Gradient {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(&self.kind).hash(state);
        std::mem::discriminant(&self.spread).hash(state);
        for v in self.placement.iter().chain(self.focal.iter()).chain(
            self.stops.iter().flat_map(|s| [&s.offset, &s.opacity, &s.midpoint].into_iter().chain(s.colour.iter())),
        ) {
            (if *v == 0.0 { 0 } else { v.to_bits() }).hash(state);
        }
        self.stops.len().hash(state);
    }
}

impl Gradient {
    pub fn mapped(&self, f: impl Fn(crate::Pt) -> crate::Pt) -> Self {
        let mut g = self.clone();
        let [a, b, c, d, e, ff] = g.placement;
        let o = f([e, ff]);
        let x = f([e + a, ff + b]);
        let y = f([e + c, ff + d]);
        g.placement = [x[0] - o[0], x[1] - o[1], y[0] - o[0], y[1] - o[1], o[0], o[1]];
        g
    }
}
