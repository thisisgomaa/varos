//! Disposable canvas coverage memoization. Export evaluation is deliberately separate.
use super::{evaluate::evaluate_capped, StrokeCoverage, StrokeError};
use crate::model::{Anchor, Document, Path, Xform};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

pub const ELEMENT_CAP: usize = 60_000;
const MIN_TOL: f64 = 0.01;
const MAX_TOL: f64 = 0.25;

pub fn tolerance(ppu: f32) -> f64 {
    let ppu = crate::flatten::bucket_ppu(crate::flatten::zoom_bucket(ppu));
    (0.025 / f64::from(ppu)).clamp(MIN_TOL, MAX_TOL)
}

fn geometry_hash(path: &Path) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    path.closed.hash(&mut hash);
    for contour in std::iter::once(&path.anchors).chain(path.holes.iter()) {
        contour.len().hash(&mut hash);
        for anchor in contour {
            for point in [Some(anchor.p), anchor.hin, anchor.hout] {
                point.is_some().hash(&mut hash);
                if let Some(point) = point {
                    point.map(f32::to_bits).hash(&mut hash);
                }
            }
        }
    }
    hash.finish()
}
struct Entry {
    geometry_hash: u64,
    anchors: Vec<Anchor>,
    holes: Vec<Vec<Anchor>>,
    closed: bool,
    style: super::StrokeStyle,
    width: f32,
    painted: bool,
    xform: Xform,
    tolerance: f64,
    coverage: Option<Arc<StrokeCoverage>>,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<u32, Entry>,
    evaluations: u64,
}

#[derive(Default)]
pub struct CanvasStrokeCache(Mutex<Cache>);
impl Clone for CanvasStrokeCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl CanvasStrokeCache {
    pub fn lookup(&self, path: &Path, xform: Xform, ppu: f32) -> Option<Arc<StrokeCoverage>> {
        let mut cache = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let tolerance = tolerance(ppu);
        let geometry_hash = geometry_hash(path);
        if let Some(entry) = cache.entries.get(&path.id) {
            if entry.geometry_hash == geometry_hash
                && entry.anchors == path.anchors
                && entry.holes == path.holes
                && entry.closed == path.closed
                && entry.style == path.stroke_style
                && entry.width == path.stroke_width
                && entry.painted == path.stroke.solid().is_some()
                && entry.xform == xform
                && entry.tolerance == tolerance
            {
                return entry.coverage.clone();
            }
        }
        let mut coverage = None;
        // Six bounded attempts, including the requested quality. Non-budget errors fail open too.
        for step in 0..6 {
            cache.evaluations += 1;
            match evaluate_capped(path, tolerance * 2f64.powi(step), ELEMENT_CAP, &|| false) {
                Ok(mut result) => {
                    for point in result.rings.iter_mut().flatten() {
                        *point = xform.apply(*point);
                    }
                    if super::evaluate::triangles_capped(&result.rings, ELEMENT_CAP).is_err() {
                        continue;
                    }
                    if step > 0 {
                        simplified(&mut result.report, path.id);
                    }
                    coverage = Some(Arc::new(result));
                    break;
                }
                Err(StrokeError::LimitExceeded) => (),
                Err(_) => break,
            }
        }
        cache.entries.insert(
            path.id,
            Entry {
                geometry_hash,
                anchors: path.anchors.clone(),
                holes: path.holes.clone(),
                closed: path.closed,
                style: path.stroke_style.clone(),
                width: path.stroke_width,
                painted: path.stroke.solid().is_some(),
                xform,
                tolerance,
                coverage: coverage.clone(),
            },
        );
        coverage
    }

    pub fn retain_live(&self, doc: &Document) {
        let mut cache = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let live: HashSet<_> = doc.paths.iter().map(|p| p.id).collect();
        cache.entries.retain(|id, _| live.contains(id));
    }

    /// Evaluation counter for headless cache/heat checks.
    pub fn evaluations(&self) -> u64 {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).evaluations
    }
}

pub(crate) fn simplified(report: &mut crate::ExportReport, id: u32) {
    report.notes.push(crate::ExportNote {
        kind: "stroke_simplified".into(),
        object_id: Some(id),
        message: "stroke simplified at this zoom".into(),
    });
}
