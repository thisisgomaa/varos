//! Varos: area type's outer corner handles (owner, 2026-10-19). The bounding box's 8 handles on
//! area type resize its frame and the text reflows at its size (upstream). Four more handles, drawn
//! [`OFFSET_PX`] outside the box's corners along both of its axes as filled circles, scale the
//! whole object as a shape scales: frame, type size and leading together, always in proportion, and
//! about the centre with Alt. The Selection tool tries them before the frame's handles.

use vectorcraft_doc::{Document, NodeId, OrientedBox, Selection};
use vectorcraft_geom::{Point, Vec2};

use crate::ToolContext;
use crate::bbox::Handle;

/// How far outside each corner an outer handle sits, along each of the box's axes (screen points).
pub const OFFSET_PX: f64 = 10.0;
/// Radius of an outer handle's filled circle (screen points).
pub const RADIUS_PX: f32 = 3.0;
/// How near (screen points) the pointer must come to an outer handle to take it.
const HIT_PX: f64 = 5.0;

/// The corners that carry an outer handle.
pub const CORNERS: [Handle; 4] = [Handle::TopLeft, Handle::TopRight, Handle::BottomRight, Handle::BottomLeft];

/// Does a selection of `objects` show outer handles: it holds area type, whose 8 handles reshape
/// the frame instead of scaling.
pub fn applies(doc: &Document, objects: &[NodeId]) -> bool {
    objects.iter().any(|id| doc.node(*id).is_some_and(crate::select::is_area_type))
}

/// Where corner `h`'s outer handle sits from the corner, in the box's own frame, at `zoom` (screen
/// points per document point).
pub fn offset(h: Handle, zoom: f64) -> Vec2 {
    let d = OFFSET_PX / zoom;
    match h {
        Handle::TopLeft => Vec2::new(-d, -d),
        Handle::TopRight => Vec2::new(d, -d),
        Handle::BottomRight => Vec2::new(d, d),
        Handle::BottomLeft => Vec2::new(-d, d),
        _ => Vec2::ZERO,
    }
}

/// The outer handles of `bx`, the selection's bounding box, in document coordinates at `zoom`;
/// none unless the selection holds area type.
pub fn handles(doc: &Document, selection: &Selection, bx: &OrientedBox, zoom: f64) -> Vec<(Handle, Point)> {
    if !zoom.is_finite() || zoom <= 0.0 || !applies(doc, &selection.objects) {
        return Vec::new();
    }
    CORNERS.iter().map(|&h| (h, bx.to_doc() * (h.pos(bx.rect) + offset(h, zoom)))).collect()
}

/// The outer handle of `bx` under `p`, with its offset from its corner (box frame).
pub(crate) fn hit(cx: &ToolContext, bx: &OrientedBox, p: Point) -> Option<(Handle, Vec2)> {
    if !cx.zoom.is_finite() || cx.zoom <= 0.0 || !applies(cx.doc, &cx.selection.objects) {
        return None;
    }
    let (lp, tol) = (bx.to_local(p), cx.tol(HIT_PX));
    CORNERS.iter().find_map(|&h| {
        let d = offset(h, cx.zoom);
        ((h.pos(bx.rect) + d).distance(lp) <= tol).then_some((h, d))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::select::SelectionTool;
    use crate::testutil::*;
    use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool};

    /// The 120 × 40 frame of [`doc_with_area_type`] at (300, 300), selected: its bottom-right
    /// outer handle sits at (430, 350) at zoom 1.
    #[test]
    fn the_outer_handle_scales_whole_and_in_proportion() {
        let (d, text) = doc_with_area_type();
        let p = paint();
        let mut s = Selection::default();
        s.add(text);
        let mut cx = cx(&d, &s, &p);
        cx.smart_guides = false;
        cx.snap_to_point = false;
        let mut t = SelectionTool::default();
        assert_eq!(t.cursor(&cx, Point::new(430.0, 350.0), Mods::default()), Cursor::ResizeNwSe);
        let down = PointerEvent::new(PointerKind::Down, 430.0, 350.0);
        assert_eq!(t.pointer(&cx, &down), vec![Action::Begin("Scale".into())], "not Resize Type Area");
        // Dragged 60 right only: the box grows ×1.5 (120 → 180 wide) in both directions.
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 490.0, 350.0));
        let Some(Action::Preview(c, v)) = a.first() else { panic!("{a:?}") };
        assert_eq!(c, "object.transform");
        assert!(v.get("typeAreas").is_none(), "the type scales too: {v}");
        let m: Vec<f64> = v["matrix"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        assert!((m[0] - 1.5).abs() < 1e-9 && (m[3] - 1.5).abs() < 1e-9, "uniform ×1.5: {m:?}");
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 490.0, 350.0)), vec![Action::Commit]);
        // The frame's own corner still resizes the type area.
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 420.0, 340.0)), vec![Action::Begin("Resize Type Area".into())]);
    }
}
