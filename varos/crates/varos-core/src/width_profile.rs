// Adapted from VectorCraft doc/src/appearance.rs width profiles (MIT OR Apache-2.0).
// Copyright (c) 2026 ArtCraft Team and contributors. See NOTICE.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidthProfile {
    pub points: Vec<(f64, f64, f64)>,
}

/// A built-in width profile (the Stroke panel's Profile list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePreset {
    /// Stable id used by `stroke.set {profile}`.
    pub id: &'static str,
    /// Menu label.
    pub label: &'static str,
    /// (t, left, right) width points.
    pub points: &'static [(f64, f64, f64)],
}

impl WidthProfile {
    /// (left, right) width factors at `t`, linear between points.
    pub fn at(&self, t: f64) -> (f64, f64) {
        Self::at_points(&self.points, t)
    }
    /// [`Self::at`] of a profile's points (a preset's or a saved profile's).
    pub fn at_points(p: &[(f64, f64, f64)], t: f64) -> (f64, f64) {
        if p.is_empty() {
            return (1.0, 1.0);
        }
        if t <= p[0].0 {
            return (p[0].1, p[0].2);
        }
        for w in p.windows(2) {
            if t <= w[1].0 {
                let u = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-9);
                return (w[0].1 + (w[1].1 - w[0].1) * u, w[0].2 + (w[1].2 - w[0].2) * u);
            }
        }
        p.last().map_or((1.0, 1.0), |l| (l.1, l.2))
    }
    /// The built-in profiles, in menu order. "uniform" is the plain stroke (no profile).
    pub const PRESETS: [ProfilePreset; 7] = [
        ProfilePreset { id: "uniform", label: "Uniform", points: &[(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)] },
        ProfilePreset { id: "lens", label: "Lens", points: &[(0.0, 0.0, 0.0), (0.5, 1.0, 1.0), (1.0, 0.0, 0.0)] },
        ProfilePreset { id: "taperStart", label: "Taper Start", points: &[(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)] },
        ProfilePreset { id: "taperEnd", label: "Taper End", points: &[(0.0, 1.0, 1.0), (1.0, 0.0, 0.0)] },
        // Full width at both ends, pinched to a quarter in the middle.
        ProfilePreset { id: "pinch", label: "Pinch", points: &[(0.0, 1.0, 1.0), (0.5, 0.25, 0.25), (1.0, 1.0, 1.0)] },
        // A round head a fifth of the way along, then a long taper to the end.
        ProfilePreset {
            id: "teardrop",
            label: "Teardrop",
            points: &[(0.0, 0.0, 0.0), (0.2, 1.0, 1.0), (1.0, 0.0, 0.0)],
        },
        // Two swells between narrow necks.
        ProfilePreset {
            id: "wave",
            label: "Wave",
            points: &[(0.0, 0.3, 0.3), (0.25, 1.0, 1.0), (0.5, 0.3, 0.3), (0.75, 1.0, 1.0), (1.0, 0.3, 0.3)],
        },
    ];
    /// The built-in profile with this id.
    pub fn preset(id: &str) -> Option<Self> {
        Self::PRESETS.iter().find(|p| p.id == id).map(|p| Self { points: p.points.to_vec() })
    }
    /// The id of the built-in profile these points match, if any.
    pub fn preset_id(&self) -> Option<&'static str> {
        Self::PRESETS.iter().find(|p| p.points == self.points.as_slice()).map(|p| p.id)
    }
    /// The id of a stroke's profile: "uniform" without one, "custom" when it matches no preset.
    pub fn id_of(p: Option<&Self>) -> &'static str {
        p.map_or(Some("uniform"), Self::preset_id).unwrap_or("custom")
    }
    /// The lens profile (thin ends, full width in the middle).
    pub fn lens() -> Self {
        Self::preset("lens").unwrap_or_default()
    }
    pub fn taper_end() -> Self {
        Self::preset("taperEnd").unwrap_or_default()
    }
    pub fn taper_start() -> Self {
        Self::preset("taperStart").unwrap_or_default()
    }
    /// The (left, right) factors just before and just after `t`: they differ only at a
    /// discontinuous point (two points at `t`, within `1e-7`).
    pub fn around(&self, t: f64) -> ((f64, f64), (f64, f64)) {
        let p = &self.points;
        if let Some(i) = p.iter().position(|q| (q.0 - t).abs() <= 1e-7) {
            let j = i + p[i..].iter().take_while(|q| (q.0 - p[i].0).abs() <= 1e-9).count() - 1;
            if j > i {
                return ((p[i].1, p[i].2), (p[j].1, p[j].2));
            }
        }
        let v = self.at(t);
        (v, v)
    }
}

impl WidthProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.points.len() < 2 || self.points.len() > 256 {
            return Err("width profile requires 2–256 points".into());
        }
        let mut last = -1.;
        for &(t, l, r) in &self.points {
            if !t.is_finite()
                || !(0.0..=1.).contains(&t)
                || t < last
                || !l.is_finite()
                || !r.is_finite()
                || !(0.0..=100.).contains(&l)
                || !(0.0..=100.).contains(&r)
            {
                return Err("width points require sorted fractions 0–1 and factors 0–100".into());
            }
            last = t;
        }
        Ok(())
    }
}
