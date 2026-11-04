//! Side Color layout and flyout sizing; the toolbar's wheel remains an independent popover.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;
use crate::varos::picker;

impl Harness {
    /// Color alone in an icon column; wheel recents deliberately do not populate side recents.
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

/// The owner's reported arrangement, rather than the wheel-popover capture: three real tabs,
/// Color active, empty session recents, in a floating panel at its normal and minimum widths.
fn color_group(width: f32) -> Harness {
    let mut h = Harness::workspace();
    h.app
        .run(
            "ui.panelReview.action",
            json!({"action": Action::Move {
                source: Source::Panel("color".into()), target: Target::Float { position: [100.0, 100.0] }
            }}),
        )
        .unwrap();
    let group = h.group_id("color");
    let host = h.host_id("color");
    h.app
        .run(
            "ui.panelReview.action",
            json!({"actions": [
                Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: 0 } },
                Action::Move { source: Source::Panel("gradient".into()), target: Target::Merge { group, index: 2 } },
                Action::Activate { panel: "color".into() },
                Action::Resize { host, position: [100.0, 100.0], width, height: 360.0 },
            ]}),
        )
        .unwrap();
    for panel_host in &mut h.app.ui.varos_panel_review.as_mut().unwrap().layout.hosts {
        panel_host.hidden = panel_host.id != host;
    }
    h.app.session.recent_colors.clear();
    h.settle();
    h
}

fn spectrum_rect(output: &FullOutput) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            Shape::Mesh(mesh) if mesh.vertices.len() == 73 * 13 => Some(mesh.calc_bounds()),
            _ => None,
        })
        .expect("the actual side Color spectrum was painted")
}

#[test]
fn grouped_side_color_insets_all_modes_and_does_not_reserve_empty_recents() {
    for width in [256.0, 300.0, 360.0] {
        let mut h = color_group(width);
        for (mode, channels) in [("rgb", 3), ("hsb", 3), ("cmyk", 4), ("grayscale", 1), ("websafe", 3)] {
            h.app.run("ui.colorPanel", json!({"mode":mode})).unwrap();
            // Changing a colour model may itself record a colour; this matrix is the empty state.
            h.app.session.recent_colors.clear();
            let output = h.settle();
            let group = h.group_rect("color");
            let spectrum = spectrum_rect(&output);
            assert!(spectrum.left() >= group.left() + 7.5 && spectrum.right() <= group.right() - 7.5, "spectrum has side insets: {mode} {width}");
            assert!(group.contains_rect(spectrum), "the whole spectrum fits: {mode} {width}");
            assert!(!texts(&output).iter().any(|(text, _, _)| text == "Recent Colors"), "an empty recent strip must consume no space");
            let mut fields: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Rect(r) if (r.rect.width() - 48.0).abs() < 0.1 && (r.rect.height() - 24.0).abs() < 0.1 => Some((r.rect, shape.clip_rect)),
                    _ => None,
                })
                .collect();
            fields.dedup_by(|a, b| a.0 == b.0);
            assert_eq!(fields.len(), channels, "the actual channel value boxes are drawn: {mode}");
            for (field, clip) in &fields {
                assert!(
                    group.shrink2(vec2(7.5, 0.0)).contains_rect(*field) && clip.contains_rect(*field),
                    "complete field fits, not just its text: {field:?}"
                );
            }
            assert!(fields.first().unwrap().0.top() <= h.tab("color").bottom() + 10.0, "channels start just below the tabs");
            if mode == "rgb" {
                assert!(spectrum.top() - fields.last().unwrap().0.bottom() <= 58.0, "no oversized warning-column gap below RGB");
            }
        }
        h.app.run("ui.colorPanel", json!({"options":false})).unwrap();
        let output = h.settle();
        assert!(h.group_rect("color").shrink2(vec2(7.5, 0.0)).contains_rect(spectrum_rect(&output)), "Hide Options keeps its inset too");
    }
}

#[test]
fn spectrum_stays_still_while_dragging_then_makes_room_for_recents_on_release() {
    let mut h = color_group(300.0);
    let spectrum = spectrum_rect(&h.settle());
    let at = spectrum.center();
    h.frame(vec![Event::PointerMoved(at), button(at, true)]);
    let to = at + vec2(35.0, -8.0);
    h.frame(vec![Event::PointerMoved(to)]);
    let held = h.frame(vec![]);
    assert_eq!(spectrum_rect(&held), spectrum, "adding recents during a drag cannot shift its input surface");
    h.frame(vec![button(to, false)]);
    assert!(!h.app.session.recent_colors.is_empty(), "the gesture really applied a colour");
    let output = h.settle();
    let settled = spectrum_rect(&output);
    near(settled.top(), spectrum.top());
    assert!(settled.height() >= 46.0);
    let recent = texts(&output).into_iter().find(|(text, _, _)| text == "Recent Colors").unwrap().1;
    assert!(recent.top() > settled.bottom(), "recents are secondary, below the ramp");
    for color in ["#ffffff", "#fefefe", "#ded3ff", "#ff0000"] {
        h.app.run("paint.setFill", json!({"color":color})).unwrap();
        assert_eq!(spectrum_rect(&h.settle()), settled, "warnings cannot shift the ramp: {color}");
    }
}

#[test]
#[ignore = "optional software visual capture; set VAROS_SIDE_COLOR_CAPTURE_DIR"]
fn capture_grouped_side_color_layout() {
    let Ok(dir) = std::env::var("VAROS_SIDE_COLOR_CAPTURE_DIR") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (name, width, mode, recent, options) in [
        ("rgb-empty", 300.0, "rgb", false, true),
        ("rgb-recent", 300.0, "rgb", true, true),
        ("cmyk-narrow", 256.0, "cmyk", true, true),
        ("hsb", 300.0, "hsb", false, true),
        ("grayscale", 256.0, "grayscale", false, true),
        ("hidden-options", 300.0, "rgb", false, false),
    ] {
        let mut h = color_group(width);
        h.app.run("ui.colorPanel", json!({"mode":mode,"options":options})).unwrap();
        h.app.session.recent_colors.clear();
        if recent {
            h.app.session.recent_colors =
                ["#0c8ce9", "#ff6b57", "#f7c948", "#42b883", "#b58af9"].iter().filter_map(|hex| vectorcraft_color::Color::from_hex(hex)).collect();
        }
        h.settle();
        let host = h.host_rect("color");
        let pixels = super::ui_tests::software_capture(&mut h.app, 900, 700);
        let image = image::RgbaImage::from_raw(900, 700, pixels).unwrap();
        let crop = image::imageops::crop_imm(&image, host.left() as u32, host.top() as u32, host.width().ceil() as u32, host.height().ceil() as u32)
            .to_image();
        crop.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
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
        let measured = crate::varos::color_panel::content_height(&h.ctx).expect("side Color measures its own slider content");
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
