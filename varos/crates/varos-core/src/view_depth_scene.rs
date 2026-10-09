//! Lane E: canvas-only presentation; export scene construction stays unchanged.
use crate::{
    editor::Editor,
    geom::View,
    scene::{Group, Prim, Scene, SceneStyle},
};
pub fn outlined(ed: &Editor, pid: u32) -> bool {
    if ed.view_depth.outline {
        return true;
    }
    let mut node = ed.doc.node_of_path(pid);
    for _ in 0..ed.doc.nodes.len() {
        let Some(id) = node else { break };
        if ed.view_depth.outline_nodes.contains(&id) {
            return true;
        }
        node = ed.doc.node(id).and_then(|n| n.parent);
    }
    false
}
pub fn present(ed: &Editor, view: View, frame: [u32; 2], style: SceneStyle, mut scene: Scene) -> Scene {
    if ed.view_depth.outline {
        scene.content.clear();
    }
    if ed.view_depth.outline || !ed.view_depth.outline_nodes.is_empty() {
        let mut prims = Vec::new();
        for (pi, p) in ed.doc.paths.iter().enumerate().filter(|(_, p)| !ed.doc.eff_hidden(p.id) && outlined(ed, p.id)) {
            for pts in std::iter::once(ed.doc.world_outline_px(pi, view.zoom))
                .chain(p.holes.iter().map(|h| ed.doc.world_ring_px(h, pi, view.zoom)))
            {
                prims.push(Prim::Stroke { pts, width: 1.0 / view.zoom.max(0.0001), color: style.outline, clip: None });
            }
        }
        scene.content.push(Group::Opaque(prims));
    }
    scene.preview_color = style.outline;
    scene.pixel_preview = (ed.view_depth.pixel_preview && !ed.view_depth.outline)
        .then(|| crate::view_depth::pixel_step(ed.doc.units.ppi));
    // Canvas colour is furniture beneath paper/artwork; never included in exported scenes.
    let a = view.s2w([0.0, 0.0]);
    let b = view.s2w([frame[0] as f32, frame[1] as f32]);
    scene.canvas_color = Some(style.canvas);
    let mut pixel_grid = Vec::new();
    if scene.pixel_preview.is_some() && view.zoom >= 6.0 && !ed.view_depth.presentation {
        let step = crate::view_depth::pixel_step(ed.doc.units.ppi);
        // Bounded by a screen-density floor, never allocate millions of lines at high ppi.
        if step * view.zoom >= 2.0 {
            for axis in 0..2 {
                let lo = a[axis];
                let hi = b[axis];
                let start = (lo / step).ceil() as i64;
                let count = ((hi - lo) / step).ceil().max(0.0) as usize;
                for i in 0..count.min(8192) {
                    let x = (start + i as i64) as f32 * step;
                    let (p, q) = if axis == 0 { ([x, a[1]], [x, b[1]]) } else { ([a[0], x], [b[0], x]) };
                    pixel_grid.push(Prim::Stroke {
                        pts: vec![p, q],
                        width: 1.0 / view.zoom,
                        color: style.outline,
                        clip: None,
                    });
                }
            }
        }
    }
    if ed.view_depth.trim || ed.view_depth.presentation {
        // Pixel furniture belongs inside the same artboard clip as the artwork.
        scene.content.push(Group::Opaque(pixel_grid));
        let content = std::mem::take(&mut scene.content);
        scene.content.push(Group::Clip { mask_rings: trim_rings(ed), members: content });
        scene.overlay.clear();
        scene.grid_step = None;
    } else {
        // Content strokes scale with zoom; editing overlay strokes do not.
        for prim in &mut pixel_grid {
            if let Prim::Stroke { width, .. } = prim {
                *width = 1.0;
            }
        }
        scene.overlay.extend(pixel_grid);
    }
    scene
}

/// Union of visible artboard rectangles as disjoint strips: overlapping boards never double-composite.
fn trim_rings(ed: &Editor) -> Vec<Vec<crate::geom::Pt>> {
    let rects: Vec<_> = ed.doc.artboards.iter().filter(|a| !a.hidden).map(|a| a.rect()).collect();
    let mut xs: Vec<_> = rects.iter().flat_map(|r| [r.0, r.2]).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let mut rings = Vec::new();
    for x in xs.windows(2) {
        let mut intervals: Vec<_> = rects.iter().filter(|r| r.0 < x[1] && r.2 > x[0]).map(|r| (r.1, r.3)).collect();
        intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for (a, b) in intervals {
            if let Some(last) = merged.last_mut().filter(|last| last.1 >= a) {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        for (a, b) in merged {
            rings.push(vec![[x[0], a], [x[1], a], [x[1], b], [x[0], b]]);
        }
    }
    rings
}

#[cfg(test)]
mod tests {
    use super::*;
    fn style() -> SceneStyle {
        SceneStyle { checkerboard: [[0.0; 4]; 2], outline: [0.5, 0.5, 0.5, 1.0], canvas: [0.1, 0.1, 0.1, 1.0] }
    }
    #[test]
    fn preview_threshold_and_signature() {
        let mut ed = Editor::new();
        let v = View::identity();
        let key = crate::scene::scene_signature(&ed, v, [100, 100]);
        ed.view_depth.pixel_preview = true;
        assert_ne!(key, crate::scene::scene_signature(&ed, v, [100, 100]));
        let low = present(&ed, v, [100, 100], style(), Scene::default());
        assert!(low.overlay.is_empty());
        let hi = present(&ed, View { zoom: 6.0, ..v }, [100, 100], style(), Scene::default());
        assert!(!hi.overlay.is_empty());
    }
    #[test]
    fn presentation_clips_and_removes_chrome() {
        let mut ed = Editor::new();
        ed.view_depth.presentation = true;
        let scene = present(&ed, View::identity(), [100, 100], style(), Scene::default());
        assert!(scene.overlay.is_empty());
        assert!(scene.content.iter().all(|g| matches!(g, Group::Clip { .. })));
    }
}
#[cfg(test)]
mod trim_tests {
    use super::*;
    #[test]
    fn overlapping_artboards_clip_once() {
        let mut ed = Editor::new();
        ed.doc.artboards = vec![
            crate::model::Artboard { x: 0.0, y: 0.0, w: 100.0, h: 100.0, ..Default::default() },
            crate::model::Artboard { x: 50.0, y: 0.0, w: 100.0, h: 100.0, ..Default::default() },
        ];
        let rings = trim_rings(&ed);
        let area: f32 = rings.iter().map(|r| (r[1][0] - r[0][0]) * (r[2][1] - r[1][1])).sum();
        assert_eq!(area, 15000.0);
    }
}
