//! Lane E: transient canvas modes and document-pixel mathematics. No persisted keys.
use crate::{editor::Editor, geom::Pt};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ViewDepth {
    pub outline: bool,
    pub pixel_preview: bool,
    pub trim: bool,
    pub presentation: bool,
    pub outline_nodes: BTreeSet<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepthAction {
    Outline,
    CanvasColor { rgb: [u8; 3] },
    TransparencyGrid,
    PixelPreview,
    SnapPixel,
    MoveWholePixel,
    Trim,
    Presentation,
    ExitPresentation,
    OutlineNode { id: u32 },
    NavigatorPan { center: Pt },
    NavigatorZoom { percent: f32 },
}
/// A document unit is a point: one document pixel is 72 / ppi points.
pub fn pixel_step(ppi: f32) -> f32 {
    if ppi.is_finite() && ppi > 0.0 {
        72.0 / ppi
    } else {
        1.0
    }
}
pub fn quantize(p: Pt, ppi: f32) -> Pt {
    let s = pixel_step(ppi);
    p.map(|v| (v / s).round() * s)
}
impl Editor {
    pub fn depth_command(&mut self, action: DepthAction) {
        match action {
            DepthAction::CanvasColor { rgb } => self.requested_canvas = Some(rgb),
            DepthAction::TransparencyGrid => {
                self.execute_ui(crate::EditCommand::SetTransparencyGrid(!self.doc.transparency_grid))
            }
            DepthAction::Outline => self.view_depth.outline = !self.view_depth.outline,
            DepthAction::PixelPreview => self.view_depth.pixel_preview = !self.view_depth.pixel_preview,
            DepthAction::SnapPixel => self.doc.snap.force_pixel_align = !self.doc.snap.force_pixel_align,
            DepthAction::MoveWholePixel => self.doc.snap.move_whole_px = !self.doc.snap.move_whole_px,
            DepthAction::Trim => self.view_depth.trim = !self.view_depth.trim,
            DepthAction::Presentation => self.view_depth.presentation = !self.view_depth.presentation,
            DepthAction::ExitPresentation => self.view_depth.presentation = false,
            DepthAction::OutlineNode { id } => {
                if self.doc.nodes.iter().any(|n| n.id == id) && !self.view_depth.outline_nodes.remove(&id) {
                    self.view_depth.outline_nodes.insert(id);
                }
            }
            DepthAction::NavigatorPan { center } => {
                if center.iter().all(|v| v.is_finite()) {
                    self.requested_pan = Some(center);
                }
            }
            DepthAction::NavigatorZoom { percent } => {
                if percent.is_finite() && (2.0..=6400.0).contains(&percent) {
                    self.requested_zoom = Some(percent);
                }
            }
        }
    }
    pub fn pixel_point(&self, p: Pt) -> Pt {
        if self.doc.snap.enabled && self.doc.snap.force_pixel_align {
            quantize(p, self.doc.units.ppi)
        } else {
            p
        }
    }
    pub fn pixel_motion(&self, origin: Pt, d: Pt) -> Pt {
        if !self.doc.snap.enabled {
            return d;
        }
        let d = if self.doc.snap.move_whole_px { quantize(d, self.doc.units.ppi) } else { d };
        if self.doc.snap.force_pixel_align {
            let p = quantize([origin[0] + d[0], origin[1] + d[1]], self.doc.units.ppi);
            [p[0] - origin[0], p[1] - origin[1]]
        } else {
            d
        }
    }
}

pub fn validate(ed: &Editor, action: DepthAction) -> Result<(), String> {
    match action {
        DepthAction::NavigatorPan { center } if center.iter().any(|v| !v.is_finite()) => {
            Err("Invalid navigator center".into())
        }
        DepthAction::NavigatorZoom { percent } if !percent.is_finite() || !(2.0..=6400.0).contains(&percent) => {
            Err("Zoom must be 2–6400 percent".into())
        }
        DepthAction::OutlineNode { id } if !ed.doc.nodes.iter().any(|n| n.id == id) => {
            Err("Unknown outline node".into())
        }
        _ => Ok(()),
    }
}

/// Whole-board proxy camera. The viewport uses the real canvas rectangle, including its offset.
pub fn navigator_bounds(ed: &Editor) -> [f32; 4] {
    let mut bounds = ed.artwork_bounds(false);
    for ab in ed.doc.artboards.iter().filter(|a| !a.hidden) {
        let b = ab.rect();
        bounds = Some(bounds.map_or(b, |a| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))));
    }
    let b = bounds.unwrap_or((0.0, 0.0, 100.0, 100.0));
    [b.0, b.1, (b.2 - b.0).max(1.0), (b.3 - b.1).max(1.0)]
}
pub fn navigator_camera(bounds: [f32; 4], size: Pt) -> crate::geom::View {
    crate::geom::View::fit(bounds[0], bounds[1], bounds[2], bounds[3], size[0], size[1], 0.9)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepthCheck {
    Outline,
    PixelPreview,
    SnapPixel,
    MoveWholePixel,
    Trim,
    Presentation,
}
impl DepthCheck {
    pub fn read(self, ed: &Editor) -> bool {
        match self {
            Self::Outline => ed.view_depth.outline,
            Self::PixelPreview => ed.view_depth.pixel_preview,
            Self::SnapPixel => ed.doc.snap.force_pixel_align,
            Self::MoveWholePixel => ed.doc.snap.move_whole_px,
            Self::Trim => ed.view_depth.trim,
            Self::Presentation => ed.view_depth.presentation,
        }
    }
}

pub fn drawing_readout(start: Pt, end: Pt) -> (Pt, String) {
    let d = crate::geom::sub(end, start);
    (end, format!("{:.1}°   {:.2} pt", (-d[1]).atan2(d[0]).to_degrees(), crate::geom::length(d)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pixel_math() {
        assert_eq!(quantize([0.37, -0.37], 144.0), [0.5, -0.5]);
        assert_eq!(pixel_step(f32::NAN), 1.0);
        let mut ed = Editor::new();
        ed.doc.units.ppi = 144.0;
        ed.doc.snap.move_whole_px = true;
        assert_eq!(ed.pixel_motion([0.1, 0.1], [0.37, -0.37]), [0.5, -0.5]);
        ed.doc.snap.force_pixel_align = true;
        assert_eq!(ed.pixel_point([0.37, -0.37]), [0.5, -0.5]);
        ed.doc.snap.enabled = false;
        assert_eq!(ed.pixel_point([0.37, -0.37]), [0.37, -0.37]);
    }
    #[test]
    fn modes_are_transient_commands() {
        let mut ed = Editor::new();
        let before = serde_json::to_value(&ed.doc).unwrap();
        for action in [DepthAction::Outline, DepthAction::PixelPreview, DepthAction::Trim, DepthAction::Presentation] {
            ed.execute_ui(crate::EditCommand::View(crate::editor::view_commands::ViewAction::Depth(action)));
        }
        assert!(
            ed.view_depth.presentation && ed.view_depth.outline && ed.view_depth.trim && ed.view_depth.pixel_preview
        );
        assert_eq!(before, serde_json::to_value(&ed.doc).unwrap());
        ed.depth_command(DepthAction::ExitPresentation);
        assert!(!ed.view_depth.presentation);
    }
}
#[cfg(test)]
mod navigator_tests {
    use super::*;
    #[test]
    fn navigator_pan_round_trip() {
        let ed = Editor::new();
        let b = navigator_bounds(&ed);
        let camera = navigator_camera(b, [224.0, 126.0]);
        let center = [b[0] + b[2] / 2.0, b[1] + b[3] / 2.0];
        let p = camera.w2s(center);
        assert!((p[0] - 112.0).abs() < 0.001 && (p[1] - 63.0).abs() < 0.001);
        let q = camera.s2w(p);
        assert!((q[0] - center[0]).abs() < 0.001);
    }
}
