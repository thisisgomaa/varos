//! Disposable canvas coverage memoization. Export evaluation is deliberately separate.
use super::{evaluate::evaluate_capped, StrokeCoverage, StrokeError};
use crate::model::{Anchor, Document, Path, Xform};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

pub const ELEMENT_CAP: usize = 60_000;
const MIN_TOL: f64 = 0.01;
// At 100% this is at most 0.1 px deviation, a tenth of a thin 1 px stroke.
const MAX_TOL: f64 = 0.1;
// Offscreen paths keep coverage briefly for panning, then release it.
const UNSEEN_BUILD_LIMIT: u64 = 8;

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
    last_seen: u64,
    geometry_hash: u64,
    anchors: Vec<Anchor>,
    holes: Vec<Vec<Anchor>>,
    closed: bool,
    // integration w2: Live Corners change the evaluated outline without touching anchors
    corners: Vec<crate::live_corners::CornerParam>,
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
    build: u64,
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
        let build = cache.build;
        if let Some(entry) = cache.entries.get_mut(&path.id) {
            if entry.geometry_hash == geometry_hash
                && entry.anchors == path.anchors
                && entry.holes == path.holes
                && entry.closed == path.closed
                && entry.corners == path.corners
                && entry.style == path.stroke_style
                && entry.width == path.stroke_width
                && entry.painted == path.stroke.solid().is_some()
                && entry.xform == xform
                && entry.tolerance == tolerance
            {
                entry.last_seen = build;
                return entry.coverage.clone();
            }
        }
        let mut coverage = None;
        // Stop immediately after the ceiling fails; never evaluate the same ceiling twice.
        for attempt_tol in retry_tolerances(tolerance) {
            cache.evaluations += 1;
            match evaluate_capped(path, attempt_tol, ELEMENT_CAP, &|| false) {
                Ok(mut result) => {
                    for point in result.rings.iter_mut().flatten() {
                        *point = xform.apply(*point);
                    }
                    if super::evaluate::triangles_capped(&result.rings, ELEMENT_CAP).is_ok() {
                        if attempt_tol > tolerance {
                            simplified(&mut result.report, path.id, false);
                        }
                        coverage = Some(Arc::new(result));
                        break;
                    }
                }
                Err(StrokeError::LimitExceeded) => (),
                Err(_) => break,
            }
        }
        cache.entries.insert(
            path.id,
            Entry {
                last_seen: build,
                geometry_hash,
                anchors: path.anchors.clone(),
                holes: path.holes.clone(),
                closed: path.closed,
                corners: path.corners.clone(),
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
        cache.build += 1;
        let build = cache.build;
        let live: HashSet<_> = doc.paths.iter().map(|p| p.id).collect();
        cache.entries.retain(|id, entry| live.contains(id) && build - entry.last_seen <= UNSEEN_BUILD_LIMIT);
    }

    /// Evaluation counter for headless cache/heat checks.
    pub fn evaluations(&self) -> u64 {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).evaluations
    }
}

pub(crate) fn simplified(report: &mut crate::ExportReport, id: u32, native: bool) {
    report.notes.push(crate::ExportNote {
        kind: "stroke_simplified".into(),
        object_id: Some(id),
        message: if native {
            "stroke simplified at this zoom: dashes/arrows/alignment not shown"
        } else {
            "stroke simplified at this zoom"
        }
        .into(),
    });
}

// Clamp every attempt, including retries, and include MAX_TOL exactly once.
fn retry_tolerances(initial: f64) -> impl Iterator<Item = f64> {
    (0..6).scan(false, move |at_max, step| {
        if *at_max {
            return None;
        }
        let tol = (initial * 2f64.powi(step)).clamp(MIN_TOL, MAX_TOL);
        *at_max = tol == MAX_TOL;
        Some(tol)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_retry_at_100_percent_respects_quality_ceiling() {
        let attempts: Vec<_> = retry_tolerances(tolerance(1.)).collect();
        assert_eq!(attempts, vec![0.025, 0.05, MAX_TOL]);
        assert!(attempts.iter().all(|t| (MIN_TOL..=MAX_TOL).contains(t)));
    }

    #[test]
    fn unseen_live_paths_expire_but_recent_paths_remain() {
        let cache = CanvasStrokeCache::default();
        let p = Path::new(1, vec![], false, None, None, 1.);
        let doc = Document { paths: vec![p.clone()], ..Default::default() };
        cache.retain_live(&doc);
        cache.lookup(&p, Xform::default(), 1.);
        for _ in 0..UNSEEN_BUILD_LIMIT {
            cache.retain_live(&doc);
        }
        assert_eq!(cache.0.lock().unwrap().entries.len(), 1);
        cache.retain_live(&doc);
        assert!(cache.0.lock().unwrap().entries.is_empty());
        cache.lookup(&p, Xform::default(), 1.);
        for _ in 0..UNSEEN_BUILD_LIMIT * 2 {
            cache.retain_live(&doc);
            cache.lookup(&p, Xform::default(), 1.);
        }
        assert_eq!(cache.0.lock().unwrap().entries.len(), 1);
        cache.retain_live(&Document::default());
        assert!(cache.0.lock().unwrap().entries.is_empty());
    }
}
