//! Real-frame sizing contracts: fixed controls, flexible previews, footer ownership and persistence.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;

fn isolated(panel: &str, height: f32) -> Harness {
    let mut layout = Layout::new_workspace();
    layout.apply(&Action::Move { source: Source::Panel(panel.into()), target: Target::Float { position: [100.0, 100.0] } }).unwrap();
    let host = layout.owner(panel).unwrap().id;
    layout.apply(&Action::Resize { host, position: [100.0, 100.0], width: 320.0, height }).unwrap();
    for h in &mut layout.hosts {
        h.hidden = h.id != host;
    }
    let mut h = Harness::unsettled(State { layout, ..Default::default() });
    h.settle();
    h
}
fn resize(h: &mut Harness, panel: &str, height: f32) {
    let host = h.host_id(panel);
    h.action(Action::Resize { host, position: [100.0, 100.0], width: 320.0, height });
    h.settle();
}
fn body(h: &Harness, panel: &str) -> Rect {
    h.hit(&format!("group-body:{}", h.group_id(panel)))
}
fn footer(h: &Harness, panel: &str) -> Rect {
    h.hit(&format!("group-footer:{}", h.group_id(panel)))
}

#[test]
fn fixed_panel_fits_content_and_rendering_does_not_rewrite_saved_extents() {
    let mut h = isolated("align", 750.0);
    let saved = serde_json::to_value(h.layout()).unwrap();
    let rect = h.host_rect("align");
    assert!(rect.height() < 500.0, "fixed controls leave no arbitrary tall frame: {rect:?}");
    assert!(!h.has_hit(&format!("resize-bottom:{}", h.host_id("align"))));
    near(body(&h, "align").bottom(), footer(&h, "align").top());
    near(footer(&h, "align").bottom(), rect.bottom());
    for _ in 0..12 {
        h.frame(vec![]);
        near(h.host_rect("align").height(), rect.height());
    }
    assert_eq!(serde_json::to_value(h.layout()).unwrap(), saved);
    resize(&mut h, "align", 100.0);
    near(h.host_rect("align").height(), rect.height());
}

#[test]
fn switching_fixed_and_flexible_tabs_restores_the_remembered_flexible_height() {
    let mut h = isolated("align", 600.0);
    let fixed = h.host_rect("align").height();
    let group = h.group_id("align");
    h.action(Action::Move { source: Source::Panel("gradient".into()), target: Target::Merge { group, index: 1 } });
    h.action(Action::Activate { panel: "gradient".into() });
    h.settle();
    near(h.host_rect("gradient").height(), 600.0);
    h.action(Action::Activate { panel: "align".into() });
    h.settle();
    near(h.host_rect("align").height(), fixed);
    h.action(Action::Activate { panel: "gradient".into() });
    h.settle();
    near(h.host_rect("gradient").height(), 600.0);
}

#[test]
fn minimum_shrink_ignores_inactive_tabs_and_obsolete_split_ratios() {
    let mut h = isolated("layers", 400.0);
    let group = h.group_id("layers");
    h.action(Action::Move { source: Source::Panel("properties".into()), target: Target::Merge { group, index: 1 } });
    h.action(Action::Activate { panel: "layers".into() });
    h.action(Action::ResizeColumn { group, width: 240.0 });
    near(h.layout().owner("layers").unwrap().expanded[0], 240.0);
    h.action(Action::Move { source: Source::Panel("history".into()), target: Target::Split { group, edge: Edge::Bottom } });
    let root = &h.layout().owner("layers").unwrap().root;
    let mut extreme = root.clone();
    if let super::model::Node::Split { ratio, .. } = &mut extreme {
        *ratio = 0.05;
    }
    near(geometry::minimum_height(root), geometry::minimum_height(&extreme));
}

#[test]
fn flexible_panels_resize_the_viewport_without_growing_controls_or_drifting() {
    for panel in ["color", "gradient", "swatches", "history", "brushes", "symbols", "navigator"] {
        let mut h = isolated(panel, 420.0);
        let first = h.settle();
        let size = super::sizing::snapshot(&h.ctx).get(panel).unwrap().clone();
        assert!(size.stretches, "{panel} has a real flexible region");
        resize(&mut h, panel, 620.0);
        let output = h.settle();
        near(h.host_rect(panel).height(), 620.0);
        near(body(&h, panel).bottom(), footer(&h, panel).top());
        let updated = super::sizing::snapshot(&h.ctx).get(panel).unwrap().clone();
        near(updated.minimum, size.minimum);
        // Every existing numeric input keeps its control height when the viewport gains 200 pt.
        let fields = |out: &FullOutput| {
            out.shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    Shape::Rect(r) if (r.rect.height() - 24.0).abs() < 0.1 => Some(r.rect.height()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(fields(&first), fields(&output), "{panel}: fixed control sizes");
        for _ in 0..10 {
            h.frame(vec![]);
            near(super::sizing::snapshot(&h.ctx).get(panel).unwrap().minimum, size.minimum);
        }
        resize(&mut h, panel, 1.0);
        near(h.host_rect(panel).height(), size.minimum + super::paint::TITLE + super::paint::TABS + super::paint::FOOT);
    }
}

#[test]
fn docked_footer_is_at_the_panel_end_and_resizes_its_flexible_region() {
    let mut h = isolated("gradient", 400.0);
    let host = h.host_id("gradient");
    h.action(Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } });
    h.settle();
    let end = footer(&h, "gradient");
    near(end.bottom(), h.host_rect("gradient").bottom());
    let handle = h.hit(&format!("resize-bottom:{host}"));
    assert!(end.contains_rect(handle));
    h.drag(handle.center(), handle.center() + vec2(0.0, 100.0));
    h.settle();
    near(h.host_rect("gradient").height(), 500.0);
}

#[test]
fn active_tab_minimum_width_is_projected_without_overwriting_the_narrow_tab() {
    let mut h = isolated("layers", 400.0);
    let group = h.group_id("layers");
    h.action(Action::Move { source: Source::Panel("properties".into()), target: Target::Merge { group, index: 1 } });
    h.action(Action::Activate { panel: "layers".into() });
    h.action(Action::ResizeColumn { group, width: 240.0 });
    near(h.host_rect("layers").width(), 240.0);
    h.action(Action::Activate { panel: "properties".into() });
    near(h.host_rect("properties").width(), 300.0);
    h.action(Action::Activate { panel: "layers".into() });
    near(h.host_rect("layers").width(), 240.0);
}

#[test]
fn empty_links_and_bottom_docks_keep_their_useful_resize_handle() {
    let mut h = isolated("links", 400.0);
    assert!(super::sizing::snapshot(&h.ctx).get("links").unwrap().stretches);
    let host = h.host_id("links");
    h.action(Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Bottom } });
    let before = h.host_rect("links");
    let from = h.hit(&format!("resize-top:{host}")).center();
    h.drag(from, from - vec2(0.0, 80.0));
    near(h.host_rect("links").height(), before.height() + 80.0);
    near(h.host_rect("links").bottom(), before.bottom());
}

#[test]
fn a_flexible_flyout_keeps_its_remembered_height() {
    let mut h = isolated("gradient", 620.0);
    let group = h.group_id("gradient");
    h.action(Action::SetColumnMode { group, mode: Mode::Icons });
    h.click(h.tab("gradient").center());
    for _ in 0..8 {
        h.frame(vec![]);
    }
    let flyout = egui::AreaState::load(&h.ctx, egui::Id::unique("panel-review-flyout")).unwrap().rect();
    near(flyout.height(), 620.0 - super::paint::TITLE);
}

#[test]
fn width_drag_keeps_the_gradient_preview_expanded_while_held() {
    let mut h = isolated("gradient", 620.0);
    let minimum = super::sizing::snapshot(&h.ctx).get("gradient").unwrap().minimum;
    let ramp = |output: &FullOutput| {
        output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                Shape::Rect(r)
                    if r.stroke.width == 1.0 && r.stroke_kind == egui::StrokeKind::Outside && r.rect.width() > 200.0 && r.rect.height() >= 18.0 =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .max_by(|a, b| a.height().total_cmp(&b.height()))
            .expect("the actual gradient ramp outline")
    };
    let before = ramp(&h.settle());
    let from = h.hit(&format!("resize-right:{}", h.host_id("gradient"))).center();
    h.hold_drag(from, from + vec2(80.0, 0.0));
    let size = super::sizing::snapshot(&h.ctx).get("gradient").unwrap().clone();
    near(size.minimum, minimum);
    near(ramp(&h.frame(vec![])).height(), before.height());
    near(h.host_rect("gradient").height(), 620.0);
    h.release(from + vec2(80.0, 0.0));
}

#[test]
fn fixed_columns_still_fit_after_collapse_and_expand() {
    let mut h = isolated("align", 700.0);
    let group = h.group_id("align");
    h.action(Action::Move { source: Source::Panel("transform".into()), target: Target::Split { group, edge: Edge::Right } });
    h.settle();
    let fitted = h.host_rect("align").height();
    assert!(fitted < 600.0);
    let column = h.group_id("transform");
    h.action(Action::SetColumnMode { group: column, mode: Mode::Icons });
    h.action(Action::SetColumnMode { group: column, mode: Mode::Full });
    h.settle();
    near(h.host_rect("align").height(), fitted);
}

#[test]
fn mixed_stacks_give_surplus_to_the_flexible_body_in_either_order() {
    for edge in [Edge::Top, Edge::Bottom] {
        let mut h = isolated("gradient", 700.0);
        let group = h.group_id("gradient");
        h.action(Action::Move { source: Source::Panel("align".into()), target: Target::Split { group, edge } });
        let host = h.host_id("gradient");
        h.action(Action::Resize { host, position: [100.0, 100.0], width: 320.0, height: 700.0 });
        h.action(Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } });
        let fixed = body(&h, "align").height();
        let flex = body(&h, "gradient").height();
        let handle = h.hit(&format!("resize-bottom:{host}")).center();
        h.drag(handle, handle + vec2(0.0, 80.0));
        near(body(&h, "align").height(), fixed);
        near(body(&h, "gradient").height(), flex + 80.0);
        let measured = geometry::measure(h.layout(), h.workspace, h.tabs());
        let host = measured.iter().find(|item| item.id == host).unwrap();
        let divider = host.dividers.iter().find(|d| d.axis == super::model::Axis::Vertical).unwrap();
        let upper = host.groups.iter().min_by(|a, b| a.rect.top().total_cmp(&b.rect.top())).unwrap();
        assert_eq!(upper.footer, divider.rect, "one strip owns the seam and its resize hit");
    }
}

#[test]
fn every_standard_panel_has_finite_stable_body_and_footer_geometry() {
    for spec in registry::BUILTINS.iter().filter(|s| s.kind == registry::PanelKind::Standard) {
        let mut h = isolated(spec.id, 480.0);
        h.settle();
        let first = h.host_rect(spec.id);
        for _ in 0..6 {
            h.frame(vec![]);
        }
        let last = h.host_rect(spec.id);
        assert!(last.is_finite() && last.is_positive(), "{}: {last:?}", spec.id);
        near(first.height(), last.height());
        near(body(&h, spec.id).bottom(), footer(&h, spec.id).top());
        near(footer(&h, spec.id).bottom(), last.bottom());
    }
}

#[test]
#[ignore = "optional software review: VAROS_PANEL_SIZING_CAPTURE_DIR"]
fn capture_panel_sizing() {
    let Ok(dir) = std::env::var("VAROS_PANEL_SIZING_CAPTURE_DIR") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (panel, height) in
        [("gradient", 320.0), ("gradient", 560.0), ("align", 560.0), ("swatches", 320.0), ("swatches", 560.0), ("color", 320.0), ("color", 560.0)]
    {
        let mut h = isolated(panel, height);
        let pixels = super::ui_tests::software_capture(&mut h.app, 900, 800);
        let rect =
            h.app.ui.varos_panel_review.as_ref().unwrap().hits.iter().find(|(name, _)| name == &format!("host:{}", h.host_id(panel))).unwrap().1;
        let image = image::RgbaImage::from_raw(900, 800, pixels).unwrap();
        let crop = image::imageops::crop_imm(&image, rect.left() as u32, rect.top() as u32, rect.width().ceil() as u32, rect.height().ceil() as u32)
            .to_image();
        crop.save(std::path::Path::new(&dir).join(format!("{panel}-{height}.png"))).unwrap();
    }
}

#[test]
fn active_tab_width_minimum_also_applies_to_reexpanded_independent_columns() {
    let mut h = isolated("layers", 400.0);
    let group = h.group_id("layers");
    h.action(Action::Move { source: Source::Panel("properties".into()), target: Target::Merge { group, index: 1 } });
    h.action(Action::Move { source: Source::Panel("align".into()), target: Target::Split { group, edge: Edge::Right } });
    h.action(Action::Activate { panel: "layers".into() });
    h.action(Action::ResizeColumn { group, width: 240.0 });
    h.action(Action::SetColumnMode { group, mode: Mode::Icons });
    h.action(Action::SetColumnMode { group, mode: Mode::Full });
    h.action(Action::Activate { panel: "properties".into() });
    assert!(body(&h, "properties").width() >= 298.0, "active Properties keeps its minimum in a projected full column");
}
