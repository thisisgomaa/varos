//! Color flyouts fit the full picker and its drawer before falling back to scrolling.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;
use crate::varos::picker;

impl Harness {
    /// Color alone, docked right as an icon column with its flyout open; 18 recent picker colours.
    fn color_flyout() -> (Self, u64) {
        let mut layout = Layout::new_workspace();
        layout.apply(&Action::Move { source: Source::Panel("color".into()), target: Target::Float { position: [100.0, 100.0] } }).unwrap();
        let host = layout.owner("color").unwrap().id;
        layout.apply(&Action::Resize { host, position: [100.0, 100.0], width: 300.0, height: 180.0 }).unwrap();
        layout.apply(&Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } }).unwrap();
        let group = layout.group_of("color").unwrap().id;
        layout.apply(&Action::SetColumnMode { group, mode: Mode::Icons }).unwrap();
        for h in &mut layout.hosts {
            h.hidden = h.id != host;
        }
        let mut h = Self::unsettled(State { layout, flyout: Some(group), ..Default::default() });
        h.app.ui.varos_picker.recent = (0..18).map(|n| format!("#{:06x}", 0xaa0000 + n * 200)).collect();
        h.screen.x = 1200.0;
        (h, group)
    }

    fn frame_at(&mut self, screen_height: f32) -> FullOutput {
        self.screen.y = screen_height;
        self.frame(vec![])
    }

    fn flyout(&self) -> Rect {
        egui::AreaState::load(&self.ctx, egui::Id::unique("panel-review-flyout")).unwrap().rect()
    }

    fn anchor(&self) -> Rect {
        self.hit("tab:color")
    }
}

#[test]
fn side_color_uses_sliders_and_its_own_height_independently_of_wheel_preferences() {
    let mut expected_height: Option<f32> = None;
    for tab in [picker::Tab::Wheel, picker::Tab::Sliders, picker::Tab::Harmony] {
        let (mut h, group) = Harness::color_flyout();
        h.app.ui.varos_picker.tab = tab;
        h.app.ui.varos_picker.drawer_open = true;
        let prefs = h.app.ui.varos_picker.clone();
        for _ in 0..6 {
            h.frame_at(1000.0);
        }
        let output = h.frame_at(1000.0);
        let flyout = h.flyout();
        let measured = crate::panels::color::content_height(&h.ctx).expect("side Color measures its own slider content");
        assert!((flyout.height() - measured - super::paint::TABS - super::paint::FOOT).abs() < 1.5);
        if let Some(height) = expected_height {
            assert!((flyout.height() - height).abs() < 1.5);
        }
        expected_height = Some(flyout.height());
        assert_eq!(h.app.ui.varos_picker, prefs, "side Color cannot alter popup wheel preferences");
        for label in ["R", "G", "B"] {
            let (rect, clip) = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(t) if t.galley.text() == label && flyout.contains(t.visual_bounding_rect().center()) => {
                        Some((t.visual_bounding_rect(), shape.clip_rect))
                    }
                    _ => None,
                })
                .expect("RGB channel is visible in the side panel");
            assert!(
                flyout.contains_rect(rect) && clip.contains_rect(rect),
                "the complete slider panel fits without scrolling: label={label}, rect={rect:?}, clip={clip:?}, flyout={flyout:?}, measured={measured}"
            );
        }
        assert!(
            !output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text()=="Document")),
            "the wheel drawer does not appear in side Color"
        );
        assert_eq!(h.app.ui.varos_panel_review.as_ref().unwrap().flyout, Some(group));
        assert!(h.anchor().is_positive());
    }
}

#[test]
fn opening_a_paint_well_keeps_the_wheel_popup_while_the_side_panel_stays_sliders() {
    let (mut h, _) = Harness::color_flyout();
    h.app.ui.varos_picker.tab = picker::Tab::Wheel;
    h.app.ui.varos_picker.drawer_open = true;
    for _ in 0..6 {
        h.frame_at(1000.0);
    }
    let sidebar = h.flyout();
    h.app.ui.toolbar = true;
    picker::open_for(&mut h.app, false, Rect::from_min_size(egui::pos2(30.0, 200.0), vec2(24.0, 24.0)));
    for _ in 0..5 {
        h.frame_at(1000.0);
    }
    let output = h.frame_at(1000.0);
    assert!(h.app.ui.varos_picker_anchor.is_some());
    assert_eq!(h.app.ui.varos_picker.tab, picker::Tab::Wheel);
    assert_eq!(h.flyout(), sidebar);
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Mesh(mesh) if mesh.vertices.len()==722 && mesh.indices.len()==2160)),
        "the paint-well popup still paints the hue wheel"
    );
    assert!(
        output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Text(t) if t.galley.text()=="Document")),
        "the popup retains its Recent/Document drawer"
    );
}

/// G1: a gradient stop's popover keeps its own wheel editor; the side Color panel stays the
/// compact sliders at its own height and never becomes (or shares state with) that wheel.
#[test]
fn a_gradient_stop_popover_leaves_the_side_color_panel_on_its_sliders() {
    let (mut h, _) = Harness::color_flyout();
    h.app.ui.varos_picker.tab = picker::Tab::Wheel;
    let id = h.app.run("shape.rectangle", json!({"x":10,"y":10,"width":50,"height":50})).unwrap()["id"].clone();
    h.app.run("select.set", json!({"ids":[id]})).unwrap();
    h.app.run("paint.setFill", json!({"gradient": {}})).unwrap();
    h.app.run("gradient.selectStop", json!({"index": 0})).unwrap();
    for _ in 0..6 {
        h.frame_at(1000.0);
    }
    let sidebar = h.flyout();
    let prefs = h.app.ui.varos_picker.clone();
    h.app.ui.dialog = Some(crate::state::Dialog::new("gradientStop", json!({"index": 0, "screen": [300.0, 600.0], "tab": "color"})));
    for _ in 0..5 {
        h.frame_at(1000.0);
    }
    let output = h.frame_at(1000.0);
    assert_eq!(h.app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("gradientStop"), "the stop popover stays open");
    let flyout = h.flyout();
    assert_eq!(flyout, sidebar, "the side panel keeps its slider height");
    for label in ["R", "G", "B"] {
        assert!(
            output.shapes.iter().any(
                |shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == label && flyout.contains(t.visual_bounding_rect().center()))
            ),
            "side Color still shows its {label} slider"
        );
    }
    let wheel = |shape: &egui::epaint::ClippedShape| matches!(&shape.shape, egui::Shape::Mesh(mesh) if mesh.vertices.len() == 722 && mesh.indices.len() == 2160);
    assert!(
        !output.shapes.iter().filter(|shape| wheel(shape)).any(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => mesh.vertices.iter().any(|v| flyout.contains(v.pos)),
            _ => false,
        }),
        "no hue wheel is painted inside the side panel"
    );
    assert!(output.shapes.iter().any(wheel), "the stop popover paints its own wheel");
    assert_eq!(h.app.ui.varos_picker, prefs, "the stop editor leaves the toolbar wheel's preferences alone");
}
