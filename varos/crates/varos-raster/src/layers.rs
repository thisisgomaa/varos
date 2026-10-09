//! Renderer-level layer contract and deterministic premultiplied-float reference.
//! Adapted from PhotoCraft compose/src/{effects,psblend}.rs and color/src/blend.rs
//! @ 4cb7cf3, Copyright (c) 2026 ArtCraft Team and contributors, MIT OR Apache-2.0.
//! Gaussian (rather than PhotoCraft's tent) is intentional for the Varos contract.
use std::collections::VecDeque;

pub type Pixel = [f32; 4];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Blend {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}
impl Blend {
    pub const ALL: [Self; 16] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::ColorDodge,
        Self::ColorBurn,
        Self::HardLight,
        Self::SoftLight,
        Self::Difference,
        Self::Exclusion,
        Self::Hue,
        Self::Saturation,
        Self::Color,
        Self::Luminosity,
    ];
}
/// Mask coverage in canvas pixel order, applied once at LayerEnd, after effects.
#[derive(Clone, Debug)]
pub enum Prim {
    Draw(varos_core::Prim),
    LayerBegin {
        opacity: f32,
        blend: Blend,
        mask: Option<Vec<f32>>,
    },
    LayerEnd,
    /// revision identifies the complete current input layer (previous effects and pan included).
    /// Radius is Gaussian sigma in points; preflight refuses sigma >128 physical pixels.
    /// Draw geometry at bucket_zoom so a cached bucket never contains inconsistent geometry.
    Blur {
        object: u64,
        revision: u64,
        radius: f32,
    },
    /// Zero offset + outer=true is an outer glow; colour is straight RGBA.
    Shadow {
        object: u64,
        revision: u64,
        offset: [f32; 2],
        blur: f32,
        colour: Pixel,
        outer: bool,
    },
}
#[derive(Clone, Copy, Debug)]
/// Offscreen budget only: caller destination/geometry, encoder staging and pass uniforms excluded.
/// Reserve 4 RGBA surfaces per live layer and a quarter-budget result cache. CPU uses f32, GPU f16.
pub struct Limits {
    pub depth: usize,
    pub bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self { depth: 16, bytes: 256 * 1024 * 1024 }
    }
}
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub flattened: bool,
    pub peak_bytes: usize,
    pub passes: usize,
    pub cache_hits: usize,
}
/// Preflight, shared by both backends; malformed lists fail before rendering.
pub fn validate(prims: &[Prim], size: [u32; 2], zoom: f32) -> Result<(), String> {
    if size.contains(&0)
        || size.iter().any(|v| *v > i32::MAX as u32 / 2)
        || !zoom.is_finite()
        || zoom <= 0.0
        || !bucket_zoom(zoom).is_finite()
        || bucket_zoom(zoom) <= 0.0
    {
        return Err("invalid layer size/zoom".into());
    }
    let n = (size[0] as usize).checked_mul(size[1] as usize).ok_or("layer size overflow")?;
    n.checked_mul(64).ok_or("layer bytes overflow")?;
    let valid_radius = |r: f32| r.is_finite() && r >= 0.0 && r * bucket_zoom(zoom) <= 128.0;
    let mut depth = 0usize;
    for p in prims {
        match p {
            Prim::LayerBegin { opacity, mask, .. } => {
                if !opacity.is_finite()
                    || !(0.0..=1.0).contains(opacity)
                    || mask
                        .as_ref()
                        .is_some_and(|m| m.len() != n || m.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)))
                {
                    return Err("invalid layer opacity/mask".into());
                }
                depth += 1;
            }
            Prim::LayerEnd => {
                depth = depth.checked_sub(1).ok_or("unmatched LayerEnd")?;
            }
            Prim::Blur { .. } | Prim::Shadow { .. } if depth == 0 => {
                return Err("effects require an object layer".into())
            }
            Prim::Blur { radius, .. } if !valid_radius(*radius) => {
                return Err("invalid blur (sigma limit: 128 pixels)".into())
            }
            Prim::Shadow { offset, blur, colour, .. }
                if !valid_radius(*blur)
                    || offset
                        .iter()
                        .any(|v| !v.is_finite() || (v * bucket_zoom(zoom)).abs() > i32::MAX as f32 / 2.0)
                    || colour.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)) =>
            {
                return Err("invalid shadow".into())
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unclosed layer".into());
    }
    Ok(())
}
/// Encoder-independent pass plan, tested without constructing a GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Draw,
    Begin { isolated: bool },
    End { isolated: bool },
    Blur { enabled: bool },
    Shadow { enabled: bool },
}
pub fn plan(prims: &[Prim], size: [u32; 2], zoom: f32, limits: Limits) -> Result<Vec<Step>, String> {
    plan_storage(prims, size, zoom, limits, 16)
}
/// Same pass planner, charging the backend's actual RGBA storage (CPU f32 / GPU f16).
pub(crate) fn plan_storage(
    prims: &[Prim],
    size: [u32; 2],
    zoom: f32,
    limits: Limits,
    pixel_bytes: usize,
) -> Result<Vec<Step>, String> {
    validate(prims, size, zoom)?;
    let bytes = size[0] as usize * size[1] as usize * 4 * pixel_bytes;
    let mut active = 0usize;
    let mut stack = Vec::new();
    let mut steps = Vec::with_capacity(prims.len());
    for p in prims {
        steps.push(match p {
            Prim::Draw(_) => Step::Draw,
            Prim::LayerBegin { .. } => {
                let isolated = active < limits.depth
                    && active.saturating_add(1).saturating_mul(bytes) <= limits.bytes.saturating_sub(limits.bytes / 4);
                stack.push(isolated);
                if isolated {
                    active += 1;
                }
                Step::Begin { isolated }
            }
            Prim::LayerEnd => {
                let isolated = stack.pop().ok_or("unmatched LayerEnd")?;
                if isolated {
                    active -= 1;
                }
                Step::End { isolated }
            }
            Prim::Blur { radius, .. } => Step::Blur { enabled: *radius > 0.0 && stack.last() == Some(&true) },
            Prim::Shadow { .. } => Step::Shadow { enabled: stack.last() == Some(&true) },
        });
    }
    Ok(steps)
}
fn lum(c: [f32; 3]) -> f32 {
    c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11
}
fn sat(c: [f32; 3]) -> f32 {
    c.iter().copied().fold(f32::NEG_INFINITY, f32::max) - c.iter().copied().fold(f32::INFINITY, f32::min)
}
fn set_lum(mut c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    for v in &mut c {
        *v += d;
    }
    let l = lum(c);
    let n = c.iter().copied().fold(f32::INFINITY, f32::min);
    let x = c.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if n < 0.0 {
        for v in &mut c {
            *v = l + (*v - l) * l / (l - n);
        }
    }
    if x > 1.0 {
        for v in &mut c {
            *v = l + (*v - l) * (1.0 - l) / (x - l);
        }
    }
    c
}
fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let n = c.iter().copied().fold(f32::INFINITY, f32::min);
    let d = sat(c);
    c.map(|v| if d > 0.0 { (v - n) * s / d } else { 0.0 })
}
pub fn blend_rgb(mode: Blend, b: [f32; 3], s: [f32; 3]) -> [f32; 3] {
    match mode {
        Blend::Hue => set_lum(set_sat(s, sat(b)), lum(b)),
        Blend::Saturation => set_lum(set_sat(b, sat(s)), lum(b)),
        Blend::Color => set_lum(s, lum(b)),
        Blend::Luminosity => set_lum(b, lum(s)),
        _ => std::array::from_fn(|i| {
            let (b, s) = (b[i], s[i]);
            let hard = |b: f32, s: f32| if s <= 0.5 { 2.0 * b * s } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - s) };
            match mode {
                Blend::Multiply => b * s,
                Blend::Screen => b + s - b * s,
                Blend::Overlay => hard(s, b),
                Blend::HardLight => hard(b, s),
                Blend::Darken => b.min(s),
                Blend::Lighten => b.max(s),
                Blend::ColorDodge => {
                    if b == 0.0 {
                        0.0
                    } else if s >= 1.0 {
                        1.0
                    } else {
                        (b / (1.0 - s)).min(1.0)
                    }
                }
                Blend::ColorBurn => {
                    if b == 1.0 {
                        1.0
                    } else if s <= 0.0 {
                        0.0
                    } else {
                        1.0 - ((1.0 - b) / s).min(1.0)
                    }
                }
                Blend::SoftLight => {
                    if s <= 0.5 {
                        b - (1.0 - 2.0 * s) * b * (1.0 - b)
                    } else {
                        let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
                        b + (2.0 * s - 1.0) * (d - b)
                    }
                }
                Blend::Difference => (b - s).abs(),
                Blend::Exclusion => b + s - 2.0 * b * s,
                _ => s,
            }
        }),
    }
}
/// W3C source-over general blend equation; both inputs and output are premultiplied.
pub fn composite(mode: Blend, b: Pixel, mut s: Pixel, opacity: f32) -> Pixel {
    for v in &mut s {
        *v *= opacity;
    }
    let cb = std::array::from_fn(|i| if b[3] > 0.0 { b[i] / b[3] } else { 0.0 });
    let cs = std::array::from_fn(|i| if s[3] > 0.0 { s[i] / s[3] } else { 0.0 });
    let blend = blend_rgb(mode, cb, cs);
    let mut o = [0.0; 4];
    for i in 0..3 {
        o[i] = (1.0 - s[3]) * b[i] + (1.0 - b[3]) * s[i] + s[3] * b[3] * blend[i];
    }
    o[3] = s[3] + b[3] * (1.0 - s[3]);
    o
}
/// Zoom bucket is 1/64 octave; both backends use the bucket centre to avoid stale pixels.
pub fn zoom_bucket(zoom: f32) -> i32 {
    (zoom.log2() * 64.0).round() as i32
}
pub fn bucket_zoom(zoom: f32) -> f32 {
    2.0f32.powf(zoom_bucket(zoom) as f32 / 64.0)
}
pub fn kernel(radius: f32, zoom: f32) -> Vec<f32> {
    let sigma = radius * bucket_zoom(zoom);
    if sigma <= 0.0 {
        return vec![1.0];
    }
    let r = (3.0 * sigma).ceil() as i32;
    let mut w: Vec<_> = (-r..=r).map(|i| (-0.5 * (i as f32 / sigma).powi(2)).exp()).collect();
    let sum: f32 = w.iter().sum();
    for v in &mut w {
        *v /= sum;
    }
    w
}
pub fn gaussian(src: &[Pixel], size: [u32; 2], weights: &[f32]) -> Vec<Pixel> {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let r = weights.len() as i32 / 2;
    let mut input = src.to_vec();
    for vertical in [false, true] {
        let mut out = vec![[0.0; 4]; src.len()];
        for y in 0..h {
            for x in 0..w {
                for (k, weight) in weights.iter().enumerate() {
                    let (xx, yy) = if vertical {
                        (x as i32, y as i32 + k as i32 - r)
                    } else {
                        (x as i32 + k as i32 - r, y as i32)
                    };
                    if xx >= 0 && yy >= 0 && xx < w as i32 && yy < h as i32 {
                        for c in 0..4 {
                            out[y * w + x][c] += input[yy as usize * w + xx as usize][c] * weight;
                        }
                    }
                }
            }
        }
        input = out;
    }
    input
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub shadow: bool,
    pub object: u64,
    pub revision: u64,
    pub radius: u32,
    pub zoom: i32,
    pub size: [u32; 2],
}
impl CacheKey {
    pub fn new(object: u64, revision: u64, radius: f32, zoom: f32, size: [u32; 2]) -> Self {
        Self { shadow: false, object, revision, radius: radius.to_bits(), zoom: zoom_bucket(zoom), size }
    }
}
/// Bounded LRU shared by CPU and GPU; edited objects replace obsolete revisions.
pub(crate) struct EffectCache<T> {
    entries: VecDeque<(CacheKey, T)>,
}
impl<T> Default for EffectCache<T> {
    fn default() -> Self {
        Self { entries: VecDeque::new() }
    }
}
impl<T> EffectCache<T> {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&CacheKey, &T) -> bool) {
        self.entries.retain(|(k, v)| keep(k, v));
    }
    pub(crate) fn get(&mut self, key: &CacheKey) -> Option<&T> {
        let index = self.entries.iter().position(|(k, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        self.entries.push_back(entry);
        self.entries.back().map(|(_, value)| value)
    }
    /// Call before allocating the replacement value, so eviction precedes allocation.
    pub(crate) fn prepare(&mut self, key: CacheKey, capacity: usize) -> bool {
        self.retain(|k, _| *k != key && (k.object != key.object || k.revision == key.revision));
        if capacity == 0 {
            return false;
        }
        while self.len() >= capacity {
            self.entries.pop_front();
        }
        true
    }
    pub(crate) fn insert(&mut self, key: CacheKey, value: T) {
        self.entries.push_back((key, value));
    }
}
/// Residual pooled storage after reserving live scratch and cache, in whole surfaces.
pub(crate) fn pool_capacity(budget: usize, active_bytes: usize, cache_bytes: usize, surface_bytes: usize) -> usize {
    budget.saturating_sub(active_bytes).saturating_sub(cache_bytes) / surface_bytes
}
struct CpuFrame<'a> {
    opacity: f32,
    blend: Blend,
    mask: Option<&'a [f32]>,
    parent: Option<Vec<Pixel>>,
}
#[derive(Default)]
pub struct CpuLayers {
    cache: EffectCache<Vec<Pixel>>,
    pool: Vec<Vec<Pixel>>,
}
impl CpuLayers {
    /// Explicitly drop stale object entries after deletion, pan changes or caller invalidation.
    pub fn invalidate(&mut self, object: u64) {
        self.cache.retain(|k, _| k.object != object);
    }
    /// Empty lists do no allocation, cache lookup or pixel work. draw paints into the current layer.
    pub fn render(
        &mut self,
        prims: &[Prim],
        size: [u32; 2],
        zoom: f32,
        limits: Limits,
        dst: &mut [Pixel],
        mut draw: impl FnMut(&varos_core::Prim, &mut [Pixel]),
    ) -> Result<Report, String> {
        let steps = plan(prims, size, zoom, limits)?;
        let n = size[0] as usize * size[1] as usize;
        if dst.len() != n {
            return Err("invalid destination".into());
        }
        let mut report = Report::default();
        if prims.is_empty() {
            return Ok(report);
        }
        // Four surfaces per active layer reserve ping-pong, backdrop and effect scratch in both backends.
        let bytes = n.checked_mul(16).and_then(|v| v.checked_mul(4)).ok_or("layer bytes overflow")?;
        self.cache.retain(|k, _| k.size == size);
        if self.cache.len() * n * 16 > limits.bytes / 4 {
            self.cache.clear();
        }
        let cache_budget = limits.bytes / 4;
        self.pool.retain(|p| p.len() == n);
        while self.pool.len() * n * 16 > limits.bytes.saturating_sub(cache_budget) {
            self.pool.pop();
        }
        let mut stack: Vec<CpuFrame<'_>> = Vec::new();
        let mut active = 0;
        let mut current = dst.to_vec();
        for (p, step) in prims.iter().zip(steps) {
            match p {
                Prim::Draw(p) => draw(p, &mut current),
                Prim::LayerBegin { opacity, blend, mask } => {
                    let isolated = matches!(step, Step::Begin { isolated: true });
                    let parent = if isolated {
                        active += 1;
                        // One pooled surface becomes live; all remaining pool storage must
                        // fit alongside the full four-surface scratch reservation.
                        let blank = self.pool.pop();
                        self.pool.truncate(pool_capacity(
                            limits.bytes,
                            active * bytes,
                            self.cache.len() * n * 16,
                            n * 16,
                        ));
                        let mut blank = blank.unwrap_or_else(|| vec![[0.0; 4]; n]);
                        report.peak_bytes =
                            report.peak_bytes.max(active * bytes + (self.cache.len() + self.pool.len()) * n * 16);
                        blank.fill([0.0; 4]);
                        Some(std::mem::replace(&mut current, blank))
                    } else {
                        report.flattened = true;
                        None
                    };
                    stack.push(CpuFrame { opacity: *opacity, blend: *blend, mask: mask.as_deref(), parent });
                }
                Prim::LayerEnd => {
                    if let Some(CpuFrame { opacity, blend, mask, parent: Some(mut parent) }) = stack.pop() {
                        for i in 0..n {
                            parent[i] =
                                composite(blend, parent[i], current[i], opacity * mask.as_ref().map_or(1.0, |m| m[i]));
                        }
                        self.pool.push(std::mem::replace(&mut current, parent));
                        active -= 1;
                        report.passes += 1;
                    }
                }
                Prim::Blur { object, revision, radius } => {
                    if *radius == 0.0 {
                        continue;
                    }
                    if stack.last().is_none_or(|f| f.parent.is_none()) {
                        report.flattened = true;
                        continue;
                    }
                    let key = CacheKey::new(*object, *revision, *radius, zoom, size);
                    if let Some(hit) = self.cache.get(&key) {
                        current.clone_from(hit);
                        report.cache_hits += 1;
                    } else {
                        current = gaussian(&current, size, &kernel(*radius, zoom));
                        report.passes += 2;
                        if self.cache.prepare(key, cache_budget / (n * 16)) {
                            self.pool.truncate(pool_capacity(
                                limits.bytes,
                                active * bytes,
                                (self.cache.len() + 1) * n * 16,
                                n * 16,
                            ));
                            self.cache.insert(key, current.clone());
                            report.peak_bytes =
                                report.peak_bytes.max(active * bytes + (self.cache.len() + self.pool.len()) * n * 16);
                        }
                    }
                }
                Prim::Shadow { object, revision, offset, blur, colour, outer } => {
                    if stack.last().is_none_or(|f| f.parent.is_none()) {
                        report.flattened = true;
                        continue;
                    }
                    let mut key = CacheKey::new(*object, *revision, *blur, zoom, size);
                    key.shadow = true;
                    let blurred = if let Some(hit) = self.cache.get(&key) {
                        report.cache_hits += 1;
                        hit.clone()
                    } else {
                        let blurred = gaussian(&current, size, &kernel(*blur, zoom));
                        report.passes += 2;
                        if self.cache.prepare(key, cache_budget / (n * 16)) {
                            self.pool.truncate(pool_capacity(
                                limits.bytes,
                                active * bytes,
                                (self.cache.len() + 1) * n * 16,
                                n * 16,
                            ));
                            self.cache.insert(key, blurred.clone());
                            report.peak_bytes =
                                report.peak_bytes.max(active * bytes + (self.cache.len() + self.pool.len()) * n * 16);
                        }
                        blurred
                    };
                    let (w, h) = (size[0] as i32, size[1] as i32);
                    let [dx, dy] = offset.map(|v| (v * bucket_zoom(zoom)).round() as i32);
                    for y in 0..h {
                        for x in 0..w {
                            let i = y as usize * w as usize + x as usize;
                            let (sx, sy) = (x - dx, y - dy);
                            let mut a = if sx >= 0 && sy >= 0 && sx < w && sy < h {
                                blurred[sy as usize * w as usize + sx as usize][3]
                            } else {
                                0.0
                            };
                            if *outer {
                                a *= 1.0 - current[i][3];
                            }
                            let a = a * colour[3];
                            let shadow = [colour[0] * a, colour[1] * a, colour[2] * a, a];
                            current[i] = composite(Blend::Normal, shadow, current[i], 1.0);
                        }
                    }
                    report.passes += 1;
                }
            }
        }
        dst.copy_from_slice(&current);
        Ok(report)
    }
}

#[cfg(test)]
#[path = "layer_tests.rs"]
mod tests;
