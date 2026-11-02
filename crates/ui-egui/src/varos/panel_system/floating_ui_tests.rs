//! Floating collapse uses the arrow, preserves its visible origin, and remains distinct from X.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::test_support::*;

#[test]
fn floating_arrow_uses_icons_even_after_names_and_preserves_a_clamped_visible_origin() {
    let mut h = Harness::slice();
    let host = h.host_id("layers");
    let group = h.group_id("layers");
    h.action(Action::SetColumnMode { group, mode: Mode::Icons });
    h.action(Action::ResizeColumn { group, width: 160.0 });
    assert_eq!(h.mode("layers"), Mode::Names);
    h.action(Action::SetColumnMode { group, mode: Mode::Full });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [1550.0, 900.0] } });
    let full = h.host_rect("layers");
    assert!(full.left() < 1550.0 && full.top() < 900.0, "the saved position is outside the visible full-size extent");
    let expanded = h.layout().host(host).unwrap().expanded;
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons, "the floating arrow always goes to icons, never the previously remembered name list");
    near(h.host_rect("layers").width(), 44.0);
    near(h.host_rect("layers").left(), full.left());
    near(h.host_rect("layers").top(), full.top());
    assert_eq!(h.layout().host(host).unwrap().dock, None);
    assert!(!h.layout().host(host).unwrap().hidden);
    assert_eq!(h.layout().host(host).unwrap().expanded, expanded);
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Full);
    near(h.host_rect("layers").left(), full.left());
    near(h.host_rect("layers").top(), full.top());
    near(h.host_rect("layers").width(), full.width());
    near(h.host_rect("layers").height(), full.height());
    h.assert_owners();
}

#[test]
fn floating_full_resize_cannot_collapse_but_compact_resize_and_icon_peek_still_work() {
    let mut h = Harness::slice();
    let host = h.host_id("layers");
    let group = h.group_id("layers");
    h.action(Action::Hide { host: h.host_id("swatches") });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [400.0, 200.0] } });
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from - vec2(250.0, 0.0));
    assert_eq!(h.mode("layers"), Mode::Full, "a full panel edge resize must stop at its minimum instead of changing presentation");
    assert!(h.host_rect("layers").width() >= 239.0);
    assert!(h.state().flyout.is_none());
    let full = h.host_rect("layers");
    let expanded = h.layout().host(host).unwrap().expanded;
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons);
    let compact = h.host_rect("layers");
    near(compact.left(), full.left());
    near(compact.top(), full.top());
    let icon = h.hit("tab:layers");
    h.click(icon.center());
    assert_eq!(h.state().flyout, Some(group));
    assert_eq!(h.mode("layers"), Mode::Icons);
    h.click(h.hit(&format!("flyout-close:{group}")).center());
    assert!(h.state().flyout.is_none());
    assert_eq!(h.host_rect("layers"), compact, "dismissing the peek keeps the icon at its floating anchor");
    let from = h.hit(&format!("resize-right:{host}")).center();
    h.drag(from, from + vec2(116.0, 0.0));
    assert_eq!(h.mode("layers"), Mode::Names, "edge resizing remains available inside the already-compact state");
    near(h.host_rect("layers").width(), 160.0);
    assert_eq!(h.layout().host(host).unwrap().expanded, expanded);
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Full);
    near(h.host_rect("layers").width(), full.width());
    near(h.host_rect("layers").height(), full.height());
    near(h.host_rect("layers").left(), full.left());
    near(h.host_rect("layers").top(), full.top());
    h.assert_owners();
}

#[test]
fn floating_x_hides_the_whole_mixed_block_and_window_restores_its_exact_modes_and_position() {
    let mut h = Harness::slice();
    let host = h.host_id("layers");
    h.action(Action::Move { source: Source::Panel("swatches".into()), target: Target::Attach { host, edge: Edge::Right } });
    h.action(Action::Move { source: Source::Host(host), target: Target::Float { position: [400.0, 180.0] } });
    h.toggle("layers");
    assert_eq!(h.mode("layers"), Mode::Icons);
    assert_eq!(h.mode("swatches"), Mode::Full);
    let before = h.layout().host(host).unwrap().clone();
    let rect = h.host_rect("layers");
    h.click(h.hit(&format!("close:{host}")).center());
    assert!(h.layout().owner("layers").unwrap().hidden);
    assert!(h.layout().owner("swatches").unwrap().hidden);
    assert_eq!(h.host_id("swatches"), host);
    assert!(h.state().hits.iter().all(|(name, _)| name != "tab:layers" && name != "tab:swatches"));
    h.app.run("window.panel", json!({"panel":"layers"})).unwrap();
    h.settle();
    assert_eq!(h.layout().host(host), Some(&before));
    assert_eq!(h.host_rect("layers"), rect);
    assert_eq!(h.mode("layers"), Mode::Icons);
    assert_eq!(h.mode("swatches"), Mode::Full);
    h.assert_owners();
}
