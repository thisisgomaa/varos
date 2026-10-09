// Adapted from VectorCraft geom/src/corners.rs:134,149 @ a469568 (MIT OR Apache-2.0).
// Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors. See NOTICE.
//! Live outer-ring corner parameters; evaluation never mutates authored anchors.
use crate::{
    model::{Anchor, Path},
    Editor,
};
use kurbo::{Point, Vec2};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Round,
    Inverted,
    Chamfer,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CornerParam {
    pub radius: f32,
    pub kind: Kind,
}
#[derive(Clone, Copy, Debug)]
pub struct Corner {
    pub index: usize,
    pub at: Point,
    pub u: Vec2,
    pub v: Vec2,
    pub max_radius: f64,
    cos: f64,
    sin: f64,
}
fn point(p: [f32; 2]) -> Point {
    Point::new(p[0] as f64, p[1] as f64)
}
fn pt(p: Point) -> [f32; 2] {
    [p.x as f32, p.y as f32]
}
pub fn corners(path: &Path) -> Vec<Corner> {
    let n = path.anchors.len();
    if !path.closed || n < 3 {
        return vec![];
    }
    (0..n)
        .filter_map(|i| {
            let a = &path.anchors[i];
            let prev = &path.anchors[(i + n - 1) % n];
            let next = &path.anchors[(i + 1) % n];
            if a.smooth || a.hin.is_some() || a.hout.is_some() || prev.hout.is_some() || next.hin.is_some() {
                return None;
            }
            let u = point(prev.p) - point(a.p);
            let v = point(next.p) - point(a.p);
            let lu = u.hypot();
            let lv = v.hypot();
            if lu < 1e-6 || lv < 1e-6 {
                return None;
            }
            let u = u / lu;
            let v = v / lv;
            let cos = u.dot(v).clamp(-1., 1.);
            let sin = u.cross(v).abs();
            if sin < 1e-6 {
                return None;
            }
            Some(Corner { index: i, at: point(a.p), u, v, cos, sin, max_radius: lu.min(lv) * 0.5 * sin / (1. + cos) })
        })
        .collect()
}
impl Corner {
    pub fn centre(self, radius: f32) -> [f32; 2] {
        pt(self.at + (self.u + self.v) * (f64::from(radius).min(self.max_radius) / self.sin))
    }
    pub fn drag_radius(self, delta: [f32; 2], initial: f32) -> f32 {
        (f64::from(initial)
            + Vec2::new(delta[0] as f64, delta[1] as f64).dot(self.u + self.v) * self.sin / (2. + 2. * self.cos))
            .clamp(0., self.max_radius) as f32
    }
}
pub fn validate(path: &Path, params: &[CornerParam]) -> Result<(), String> {
    if params.len() > path.anchors.len() {
        return Err("corner parameters exceed outer anchors".into());
    }
    let eligible = corners(path);
    for (i, p) in params.iter().enumerate() {
        if !p.radius.is_finite() || !(0.0..=1e6).contains(&p.radius) {
            return Err("corner radius must be finite and between 0 and 1000000".into());
        }
        if p.radius > 0. && !eligible.iter().any(|c| c.index == i) {
            return Err("live corners require straight polygon edges".into());
        }
    }
    Ok(())
}
fn handle(cos: f64) -> f64 {
    4. / 3. * ((1. - cos) * 0.5).max(0.).sqrt() / (1. + ((1. + cos) * 0.5).max(0.).sqrt())
}
pub fn evaluated(path: &Path) -> Path {
    if path.corners.is_empty() {
        return path.clone();
    }
    let eligible = corners(path);
    let mut out = path.clone();
    out.corners.clear();
    out.anchors.clear();
    for (i, a) in path.anchors.iter().enumerate() {
        let p = path.corners.get(i).copied().unwrap_or_default();
        let Some(c) = eligible.iter().find(|c| c.index == i) else {
            out.anchors.push(a.clone());
            continue;
        };
        let radius = f64::from(p.radius).max(0.).min(c.max_radius);
        if radius <= 1e-6 {
            out.anchors.push(a.clone());
            continue;
        }
        let setback = radius * (1. + c.cos) / c.sin;
        let start = c.at + c.u * setback;
        let end = c.at + c.v * setback;
        let (hout, hin) = match p.kind {
            Kind::Round => {
                let h = radius * handle(-c.cos);
                (Some(pt(start - c.u * h)), Some(pt(end - c.v * h)))
            }
            Kind::Inverted => {
                let h = setback * handle(c.cos);
                let a = (c.v - c.u * c.cos) / c.sin;
                let b = (c.u - c.v * c.cos) / c.sin;
                (Some(pt(start + a * h)), Some(pt(end + b * h)))
            }
            Kind::Chamfer => (None, None),
        };
        out.anchors.push(Anchor { id: a.id, p: pt(start), hin: None, hout, smooth: false });
        out.anchors.push(Anchor { id: 0, p: pt(end), hin, hout: None, smooth: false });
    }
    out
}
impl Editor {
    pub fn set_corners(&mut self, id: u32, params: Vec<CornerParam>) {
        let Some(i) = self.doc.pidx(id) else { return };
        if validate(&self.doc.paths[i], &params).is_err() || self.doc.paths[i].corners == params {
            return;
        }
        self.begin();
        self.doc.paths[i].corners = params;
        self.dirty = true;
        self.commit();
    }
}
/// Export-side resolution leaves the editable model (and its embedded blob) intact.
pub fn document(source: &crate::model::Document) -> std::borrow::Cow<'_, crate::model::Document> {
    if source.paths.iter().all(|p| p.corners.is_empty()) {
        return std::borrow::Cow::Borrowed(source);
    }
    let mut doc = source.clone();
    for p in &mut doc.paths {
        *p = evaluated(p);
    }
    std::borrow::Cow::Owned(doc)
}
