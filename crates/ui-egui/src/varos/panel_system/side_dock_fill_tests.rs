//! Side list panels fill their dock; their real controls remain attached to that body.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::test_support::*;

fn list_group() -> Harness {
    let mut layout = Layout::new_workspace();
    layout.apply(&Action::Move { source: Source::Panel("layers".into()), target: Target::Float { position: [100.0, 100.0] } }).unwrap();
    let host = layout.owner("layers").unwrap().id;
    let group = layout.group_of("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Panel("properties".into()), target: Target::Merge { group, index: 1 } }).unwrap();
    layout.apply(&Action::Resize { host, position: [100.0, 100.0], width: 320.0, height: 260.0 }).unwrap();
    layout.apply(&Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } }).unwrap();
    for other in &mut layout.hosts {
        if other.id != host && !model::groups(&other.root).iter().any(|g| g.panels.iter().any(|p| p == "tools" || p == "control")) {
            other.hidden = true;
        }
    }
    let mut h = Harness::unsettled(State { layout, ..Default::default() });
    h.settle();
    h.measure_workspace();
    h
}

#[test]
fn docked_layers_and_properties_fill_the_side_after_tab_switch_and_window_resize() {
    let mut h = list_group();
    let saved = h.layout().host(h.host_id("layers")).unwrap().expanded;
    for height in [1000.0, 760.0, 1200.0] {
        h.screen.y = height;
        for panel in ["layers", "properties", "layers"] {
            h.action(Action::Activate { panel: panel.into() });
            h.settle();
            h.measure_workspace();
            let host = h.host_rect(panel);
            near(host.bottom(), h.workspace.bottom());
            let group = h.group_id(panel);
            let body = h.hit(&format!("group-body:{group}"));
            let footer = h.hit(&format!("group-footer:{group}"));
            near(body.bottom(), footer.top());
            near(footer.bottom(), host.bottom());
            assert_eq!(h.layout().host(h.host_id(panel)).unwrap().expanded, saved, "filling a dock must preserve remembered floating extents");
        }
    }
}

#[test]
fn layers_actual_bottom_bar_stays_next_to_the_group_footer() {
    let mut h = list_group();
    h.action(Action::Activate { panel: "layers".into() });
    for height in [760.0, 1100.0] {
        h.screen.y = height;
        let out = h.settle();
        let body = h.hit(&format!("group-body:{}", h.group_id("layers")));
        // The count is painted by the real bottom_bar next to its right-to-left buttons.
        let count = texts(&out).into_iter().find(|(text, _, _)| text == "1 Layer").expect("actual Layers bottom-bar count").1;
        // bottom_bar allocates a 30pt row and paints the count at its center.
        let inset = body.bottom() - (count.center().y + 15.0);
        assert!((0.0..=12.5).contains(&inset), "toolbar must retain only its normal content inset: {inset}, body={body:?}, count={count:?}");
    }
}
