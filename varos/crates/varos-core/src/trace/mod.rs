// Adapted from VectorCraft crates/trace/src/lib.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
//! Deterministic, headless RGBA8 tracing. Coordinates are in input pixels, y down.
mod contour;
mod fit;
mod quantize;
use crate::model::Path;
use serde::{Deserialize, Serialize};

pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_ANCHORS: usize = 100_000;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TraceMode {
    BlackWhite,
    Grayscale,
    Color { colors: u8 },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TraceOptions {
    pub mode: TraceMode,
    pub threshold: u8,
    /// Percentage 0..100. Higher values retain more detail.
    pub paths_fidelity: f64,
    /// Percentage 0..100. Higher values retain more sharp corners.
    pub corners: f64,
    /// Components smaller than this area merge into neighbouring colours.
    pub noise_px: u32,
    pub ignore_white: bool,
}
impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            mode: TraceMode::BlackWhite,
            threshold: 128,
            paths_fidelity: 75.,
            corners: 60.,
            noise_px: 4,
            ignore_white: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TraceReport {
    pub palette: Vec<[u8; 3]>,
    pub paths: usize,
    pub holes: usize,
    pub anchors: usize,
    /// Pixels with alpha below 128 are omitted; others become opaque fills.
    pub transparent_pixels: usize,
}
// Private compatibility view for the adapted palette algorithm.
struct Raster<'a> {
    rgba: &'a [u8],
}
enum Mode {
    BlackAndWhite,
    Grayscale,
    Color,
}
struct TraceParams {
    mode: Mode,
    threshold: u8,
    colors: u32,
}

/// Reject malformed/oversized inputs before allocation. IDs are deterministic local identities;
/// `EditCommand::InsertTracedPaths` remaps them into the destination document's arena.
pub fn trace(rgba: &[u8], width: u32, height: u32, options: &TraceOptions) -> Result<(Vec<Path>, TraceReport), String> {
    let pixels = u64::from(width) * u64::from(height);
    if pixels == 0 || pixels > MAX_PIXELS || pixels * 4 != rgba.len() as u64 {
        return Err("trace requires an exact, nonempty RGBA8 buffer of at most 16 megapixels".into());
    }
    if !options.paths_fidelity.is_finite()
        || !(0. ..=100.).contains(&options.paths_fidelity)
        || !options.corners.is_finite()
        || !(0. ..=100.).contains(&options.corners)
        || matches!(options.mode, TraceMode::Color { colors: 0 })
    {
        return Err("trace fidelity/corners must be 0..100 and color count must be positive".into());
    }
    let (mode, colors) = match options.mode {
        TraceMode::BlackWhite => (Mode::BlackAndWhite, 2),
        TraceMode::Grayscale => (Mode::Grayscale, 8),
        TraceMode::Color { colors } => (Mode::Color, u32::from(colors)),
    };
    let mut q = quantize::quantize(&Raster { rgba }, &TraceParams { mode, threshold: options.threshold, colors });
    let (w, h) = (width as usize, height as usize);
    quantize::denoise(&mut q.labels, w, h, options.noise_px as usize);
    let mut paths = Vec::new();
    let mut report = TraceReport {
        palette: vec![],
        paths: 0,
        holes: 0,
        anchors: 0,
        transparent_pixels: rgba.as_chunks::<4>().0.iter().filter(|p| p[3] < 128).count(),
    };
    let mut id = 1u32;
    for (ci, color) in q.palette.iter().enumerate() {
        if options.ignore_white && color.iter().all(|v| *v >= 245) {
            continue;
        }
        let mask: Vec<bool> = q.labels.iter().map(|l| usize::from(*l) == ci).collect();
        let components = contour::trace_mask_within(&mask, w, h, (MAX_ANCHORS - report.anchors) / 3)?;
        if components.is_empty() {
            continue;
        }
        report.palette.push(*color);
        for component in components {
            let anchors = fit::fit(&component.outer.pts, options, &mut id);
            let fill = [color[0] as f32 / 255., color[1] as f32 / 255., color[2] as f32 / 255., 1.];
            let mut path = Path::new(id, anchors, true, Some(fill), None, 0.);
            id += 1;
            path.holes = component.holes.iter().map(|h| fit::fit(&h.pts, options, &mut id)).collect();
            report.holes += path.holes.len();
            report.anchors += path.anchors.len() + path.holes.iter().map(Vec::len).sum::<usize>();
            if report.anchors > MAX_ANCHORS {
                return Err("trace exceeds 100000 anchors; increase noise or lower fidelity/colors".into());
            }
            paths.push(path);
        }
    }
    report.paths = paths.len();
    Ok((paths, report))
}
#[cfg(test)]
mod tests;

/// Validate the complete result against the destination format limits before starting history.
pub(crate) fn check_insert(ed: &crate::editor::Editor, paths: &[Path]) -> Result<(), String> {
    let limits = crate::format::Limits::DEFAULT;
    let layer = ed.doc.node(ed.doc.active_layer).ok_or("unknown active layer")?;
    if layer.kind != crate::model::NodeKind::Layer || layer.hidden || layer.locked {
        return Err("trace destination must be a visible unlocked layer".into());
    }
    let mut parent = layer.parent;
    let mut remaining = ed.doc.nodes.len();
    while let Some(id) = parent {
        if remaining == 0 {
            return Err("trace destination has cyclic ancestors".into());
        }
        remaining -= 1;
        let ancestor = ed.doc.node(id).ok_or("unknown trace destination ancestor")?;
        if ancestor.hidden || ancestor.locked {
            return Err("trace destination must have visible unlocked ancestors".into());
        }
        parent = ancestor.parent;
    }
    if ed.doc.paths.len() + paths.len() > limits.max_paths || ed.doc.nodes.len() + paths.len() > limits.max_nodes {
        return Err("trace exceeds document path limit".into());
    }
    let count: usize =
        ed.doc.paths.iter().chain(paths).map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum();
    if count > limits.max_anchors {
        return Err("trace exceeds document anchor limit".into());
    }
    if u64::from(ed.allocation_floor()) + count as u64 + paths.len() as u64 + limits.max_nodes as u64
        >= u64::from(u32::MAX)
    {
        return Err("trace exhausts stable IDs".into());
    }
    for path in paths {
        if !path.closed || path.anchors.len() < 3 || path.holes.iter().any(|h| h.len() < 3) {
            return Err("trace requires closed valid rings".into());
        }
        if path
            .anchors
            .iter()
            .chain(path.holes.iter().flatten())
            .flat_map(|a| std::iter::once(&a.p).chain(a.hin.iter()).chain(a.hout.iter()))
            .flatten()
            .any(|v| !v.is_finite() || !(*v + *v).is_finite())
        {
            return Err("trace coordinates must be finite".into());
        }
        if !matches!(path.fill, crate::model::Paint::Solid(c) if c.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)))
            || path.stroke != crate::model::Paint::None
            || path.stroke_width != 0.
            || path.opacity != 1.
        {
            return Err("trace requires solid filled paths".into());
        }
    }
    Ok(())
}
