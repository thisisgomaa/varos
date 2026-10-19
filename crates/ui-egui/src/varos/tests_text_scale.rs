//! Area type's outer corner handles (`vectorcraft_tools::varos_text_scale`), through the
//! Selection tool and the engine: an outer handle scales type and frame together; a corner
//! handle reshapes the frame only.

use serde_json::json;
use vectorcraft_doc::{NodeId, NodeKind, TextKind, TextObject};
use vectorcraft_engine::{Session, ViewInfo};
use vectorcraft_geom::{Point, Rect, Vec2};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind};

use crate::VectorcraftApp;

fn view() -> ViewInfo {
    ViewInfo { smart_guides: false, snap_to_point: false, ..Default::default() }
}

/// A 120 × 80 frame of 12 pt type at (40, 40), selected, with the Selection tool.
fn app() -> (VectorcraftApp, NodeId) {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
    let id = app
        .session
        .execute("text.create", &json!({"x": 40, "y": 40, "size": 12, "area": {"width": 120, "height": 80}, "text": "Area type that wraps"}))
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    app.run("select.set", json!({"ids": [id]})).unwrap();
    app.select_tool("selection");
    (app, NodeId(id))
}

fn text(app: &VectorcraftApp, id: NodeId) -> TextObject {
    match &app.session.active().unwrap().doc.node(id).unwrap().kind {
        NodeKind::Text(t) => (**t).clone(),
        k => panic!("not text: {k:?}"),
    }
}

/// The type's size as seen on the page (its transform applied), in points.
fn size(t: &TextObject) -> f64 {
    let st = &t.runs[0].style;
    (t.xf * Point::new(0.0, st.size * st.v_scale / 100.0) - t.xf * Point::ZERO).hypot()
}

/// The frame on the page.
fn frame(t: &TextObject) -> Rect {
    match &t.kind {
        TextKind::Area { frame } => t.xf.transform_rect_bbox(frame.bounds().unwrap()),
        k => panic!("not area type: {k:?}"),
    }
}

fn drag(app: &mut VectorcraftApp, from: Point, by: Vec2, mods: Mods) {
    let to = from + by;
    for (kind, p) in [(PointerKind::Down, from), (PointerKind::Drag, from + by * 0.5), (PointerKind::Drag, to), (PointerKind::Up, to)] {
        app.session.pointer(&PointerEvent::new(kind, p.x, p.y).with_mods(mods), view()).unwrap();
    }
}

/// The bottom-right corner of the selection's box.
fn corner(app: &mut VectorcraftApp) -> Point {
    let b = app.selection_box().unwrap();
    assert_eq!(b.angle, 0.0);
    Point::new(b.rect.x1, b.rect.y1)
}

#[test]
fn an_outer_handle_scales_type_and_frame_together() {
    let (mut app, id) = app();
    let (before, at) = (text(&app, id), corner(&mut app));
    assert!((size(&before) - 12.0).abs() < 1e-9);
    let f0 = frame(&before);
    // 10 points out from the corner on both axes (zoom 1): the outer handle. Dragged unevenly, it
    // still scales in proportion.
    let outer = at + Vec2::new(10.0, 10.0);
    assert_eq!(app.session.cursor(outer, Mods::default(), view()), vectorcraft_tools::Cursor::ResizeNwSe, "the scale cursor");
    drag(&mut app, outer, Vec2::new(60.0, 20.0), Mods::default());
    let after = text(&app, id);
    let f1 = frame(&after);
    let k = f1.width() / f0.width();
    assert!(k > 1.2, "the frame grew: {f0:?} → {f1:?}");
    assert!((f1.height() / f0.height() - k).abs() < 1e-6, "in proportion: {f0:?} → {f1:?}");
    assert!((size(&after) - 12.0 * k).abs() < 1e-6, "the type grew with it: {} for ×{k}", size(&after));
    assert!((f1.x0 - f0.x0).abs() < 1e-6 && (f1.y0 - f0.y0).abs() < 1e-6, "about the opposite corner");
    // One undo step.
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(text(&app, id), before);
}

#[test]
fn alt_on_an_outer_handle_scales_about_the_centre() {
    let (mut app, id) = app();
    let f0 = frame(&text(&app, id));
    let outer = corner(&mut app) + Vec2::new(10.0, 10.0);
    drag(&mut app, outer, Vec2::new(30.0, 30.0), Mods { alt: true, ..Default::default() });
    let f1 = frame(&text(&app, id));
    assert!(f1.width() > f0.width() + 1.0, "{f0:?} → {f1:?}");
    assert!((f1.center() - f0.center()).hypot() < 1e-6, "the centre stays: {f0:?} → {f1:?}");
}

#[test]
fn a_corner_handle_reshapes_the_frame_only() {
    let (mut app, id) = app();
    let before = text(&app, id);
    let f0 = frame(&before);
    let at = corner(&mut app);
    drag(&mut app, at, Vec2::new(60.0, 20.0), Mods::default());
    let after = text(&app, id);
    let f1 = frame(&after);
    assert!((f1.width() - f0.width() - 60.0).abs() < 1e-6 && (f1.height() - f0.height() - 20.0).abs() < 1e-6, "{f0:?} → {f1:?}");
    assert!((size(&after) - 12.0).abs() < 1e-9, "the type keeps its size: {}", size(&after));
}

/// The outer handles show only for area type: a rectangle's corners keep upstream's rotate zone.
#[test]
fn only_area_type_has_outer_handles() {
    let (mut app, id) = app();
    let b = app.selection_box().unwrap();
    let st = app.session.active().unwrap();
    let handles = vectorcraft_tools::varos_text_scale::handles(&st.doc, &st.selection, &b, 2.0);
    assert_eq!(handles.len(), 4);
    let (_, top_left) = handles[0];
    assert!((top_left - Point::new(b.rect.x0 - 5.0, b.rect.y0 - 5.0)).hypot() < 1e-9, "10 screen points out at zoom 2: {top_left:?}");
    let rect = app.run("shape.rectangle", json!({"x": 300, "y": 300, "width": 50, "height": 40})).unwrap()["id"].as_u64().unwrap();
    app.run("select.set", json!({"ids": [rect]})).unwrap();
    let b = app.selection_box().unwrap();
    let st = app.session.active().unwrap();
    assert!(vectorcraft_tools::varos_text_scale::handles(&st.doc, &st.selection, &b, 1.0).is_empty());
    let outer = Point::new(b.rect.x1 + 10.0, b.rect.y1 + 10.0);
    assert_ne!(app.session.cursor(outer, Mods::default(), view()), vectorcraft_tools::Cursor::ResizeNwSe, "a rectangle's corner rotates there");
    let _ = id;
}

/// The type's leading as seen on the page, in points.
fn leading(t: &TextObject) -> f64 {
    let l = t.runs[0].style.leading.expect("an explicit leading");
    (t.xf * Point::new(0.0, l) - t.xf * Point::ZERO).hypot()
}

/// An explicit leading scales with the type and the frame.
#[test]
fn an_outer_handle_scales_an_explicit_leading_too() {
    let (mut app, id) = app();
    app.run("text.setStyle", json!({"leading": 20})).unwrap();
    let before = text(&app, id);
    assert!((leading(&before) - 20.0).abs() < 1e-9);
    let f0 = frame(&before);
    let outer = corner(&mut app) + Vec2::new(10.0, 10.0);
    drag(&mut app, outer, Vec2::new(60.0, 60.0), Mods::default());
    let after = text(&app, id);
    let k = frame(&after).width() / f0.width();
    assert!(k > 1.2);
    assert!((leading(&after) - 20.0 * k).abs() < 1e-6, "leading {} for ×{k}", leading(&after));
    assert!((size(&after) - 12.0 * k).abs() < 1e-6);
}

/// Rotated area type: the outer handles sit off the turned box's corners, and dragging one
/// scales along the type's own axes, in proportion, keeping the angle.
#[test]
fn rotated_area_type_scales_along_its_own_axes() {
    let (mut app, id) = app();
    app.run("object.rotate", json!({"angle": 30})).unwrap();
    let b0 = app.selection_box().unwrap();
    assert!((b0.angle.abs() - 30.0).abs() < 1e-6, "{b0:?}");
    let st = app.session.active().unwrap();
    let (handle, outer) = vectorcraft_tools::varos_text_scale::handles(&st.doc, &st.selection, &b0, 1.0)[2];
    assert_eq!(handle, vectorcraft_tools::bbox::Handle::BottomRight);
    // 10 points out along each of the box's own axes.
    let corner = b0.to_doc() * Point::new(b0.rect.x1, b0.rect.y1);
    assert!(((outer - corner).hypot() - 10.0 * std::f64::consts::SQRT_2).abs() < 1e-9);
    let cursor = app.session.cursor(outer, Mods::default(), view());
    assert!(
        matches!(
            cursor,
            vectorcraft_tools::Cursor::ResizeH
                | vectorcraft_tools::Cursor::ResizeV
                | vectorcraft_tools::Cursor::ResizeNwSe
                | vectorcraft_tools::Cursor::ResizeNeSw
        ),
        "a scale cursor, not the rotate one: {cursor:?}"
    );
    // Out along the box's diagonal.
    let out = b0.to_doc() * Point::new(b0.rect.x1 + 50.0, b0.rect.y1 + 50.0) - b0.to_doc() * Point::new(b0.rect.x1, b0.rect.y1);
    drag(&mut app, outer, out, Mods::default());
    let b1 = app.selection_box().unwrap();
    let k = b1.rect.width() / b0.rect.width();
    assert!(k > 1.2, "{b0:?} -> {b1:?}");
    assert!((b1.rect.height() / b0.rect.height() - k).abs() < 1e-6, "in proportion: {b0:?} -> {b1:?}");
    assert!((b1.angle - b0.angle).abs() < 1e-6, "the angle stays: {}", b1.angle);
    assert!((size(&text(&app, id)) - 12.0 * k).abs() < 1e-6, "the type grew with it");
}
