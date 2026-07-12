//! CPU-only measurement seam for the P11 performance harness.
//!
//! This deliberately exposes counts and elapsed time, not renderer internals. It needs no adapter,
//! surface, or window, so the same synthetic workload runs on developer machines and in CI.

use crate::tess::build_content;
use std::hint::black_box;
use std::time::{Duration, Instant};
use varos_core::geom::View;
use varos_core::scene::Scene;

#[derive(Clone, Copy, Debug)]
pub struct CpuContentProfile {
    pub elapsed: Duration,
    pub fill_vertices: usize,
    pub foreground_vertices: usize,
    pub opacity_vertices: usize,
    pub draw_groups: usize,
}

/// Tessellate one already-built scene without touching GPU state.
pub fn profile_content(scene: &Scene, view: View, width: f32, height: f32) -> CpuContentProfile {
    let start = Instant::now();
    let (fill, foreground, opacity, groups) =
        black_box(build_content(black_box(&scene.content), view, view.zoom, width, height));
    CpuContentProfile {
        elapsed: start.elapsed(),
        fill_vertices: fill.len(),
        foreground_vertices: foreground.len(),
        opacity_vertices: opacity.len(),
        draw_groups: groups.len(),
    }
}
