//! Expanded geometry survives attachment, compaction and detachment.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::*;

fn sized_layout() -> Layout {
    let mut layout = Layout::new_slice();
    for (panel, width, height) in [("layers", 280.0, 340.0), ("swatches", 420.0, 520.0)] {
        let host = layout.owner(panel).unwrap().id;
        layout.apply(&Action::Resize { host, position: [80.0, 80.0], width, height }).unwrap();
    }
    layout
}

fn close_size(actual: [f32; 2], expected: [f32; 2]) {
    assert!((actual[0] - expected[0]).abs() < 0.001, "width: {actual:?} != {expected:?}");
    assert!((actual[1] - expected[1]).abs() < 0.001, "height: {actual:?} != {expected:?}");
}

fn attach(layout: &mut Layout, edge: Edge) {
    let source = layout.owner("swatches").unwrap().id;
    let host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Host(source), target: Target::Attach { host, edge } }).unwrap();
}

#[test]
fn unequal_attached_groups_retain_their_actual_extents_when_detached() {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        for detach_tab in [false, true] {
            let mut layout = sized_layout();
            attach(&mut layout, edge);
            let horizontal = matches!(edge, Edge::Left | Edge::Right);
            let joined = if horizontal { [703.0, 520.0] } else { [420.0, 839.0] };
            close_size(layout.owner("layers").unwrap().expanded, joined);

            let group = layout.group_of("swatches").unwrap().id;
            let source = if detach_tab { Source::Panel("swatches".into()) } else { Source::Group(group) };
            layout.apply(&Action::Move { source, target: Target::Float { position: [500.0, 200.0] } }).unwrap();

            close_size(layout.owner("layers").unwrap().expanded, if horizontal { [280.0, 520.0] } else { [420.0, 340.0] });
            close_size(layout.owner("swatches").unwrap().expanded, [420.0, 520.0]);
            assert_eq!(layout.group_of("swatches").unwrap().id, group);
            assert_eq!(layout.hosts.len(), 2);
            layout.validate().unwrap();
        }
    }
}

#[test]
fn compact_detachment_uses_expanded_extents_not_the_compact_width_or_height() {
    for mode in [Mode::Icons, Mode::Names] {
        let mut layout = sized_layout();
        attach(&mut layout, Edge::Bottom);
        let host = layout.owner("layers").unwrap().id;
        layout.apply(&Action::SetMode { host, mode }).unwrap();
        let width = if mode == Mode::Icons { 44.0 } else { 180.0 };
        layout.apply(&Action::Resize { host, position: [80.0, 80.0], width, height: 96.0 }).unwrap();
        let group = layout.group_of("swatches").unwrap().id;
        layout.apply(&Action::Move { source: Source::Group(group), target: Target::Float { position: [500.0, 200.0] } }).unwrap();

        close_size(layout.owner("layers").unwrap().expanded, [420.0, 340.0]);
        close_size(layout.owner("swatches").unwrap().expanded, [420.0, 520.0]);
        for panel in PANELS {
            let owner = layout.owner(panel).unwrap();
            assert_eq!(owner.mode, mode);
            assert_eq!(owner.compact_width, width);
        }
    }
}

#[test]
fn reordering_nested_groups_does_not_accumulate_container_size() {
    let mut layout = sized_layout();
    attach(&mut layout, Edge::Bottom);
    let host = layout.owner("layers").unwrap().id;
    let group = layout.group_of("swatches").unwrap().id;
    let size = layout.host(host).unwrap().expanded;
    for edge in [Edge::Top, Edge::Bottom, Edge::Top] {
        layout.apply(&Action::Move { source: Source::Group(group), target: Target::Attach { host, edge } }).unwrap();
        close_size(layout.host(host).unwrap().expanded, size);
    }
}

#[test]
fn splitting_two_merged_tabs_reserves_space_for_both_bodies() {
    let mut layout = sized_layout();
    let group = layout.group_of("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: 1 } }).unwrap();
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Split { group, edge: Edge::Bottom } }).unwrap();
    close_size(layout.owner("layers").unwrap().expanded, [280.0, 659.0]);
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Float { position: [500.0, 200.0] } }).unwrap();
    for panel in PANELS {
        close_size(layout.owner(panel).unwrap().expanded, [280.0, 340.0]);
    }
}

#[test]
fn tools_width_toggle_and_resize_keep_expanded_content_and_other_hosts() {
    let mut layout = Layout::new_workspace();
    let id = layout.owner("tools").unwrap().id;
    let others: Vec<_> = layout.hosts.iter().filter(|h| h.id != id).cloned().collect();
    for width in [44.0, 80.0, 44.0, 80.0] {
        layout.apply(&Action::SetToolsWidth { width }).unwrap();
        assert_eq!(layout.tools_width(), Some(width));
        assert_eq!(layout.host(id).unwrap().mode, Mode::Full);
        assert_eq!(layout.hosts.iter().filter(|h| h.id != id).cloned().collect::<Vec<_>>(), others);
    }
    for (width, bounded) in [(2.0, 44.0), (2000.0, 80.0), (63.0, 63.0)] {
        layout.apply(&Action::Resize { host: id, position: [10.0, 20.0], width, height: 620.0 }).unwrap();
        assert_eq!(layout.tools_width(), Some(bounded));
        assert_eq!(layout.host(id).unwrap().expanded[1], 620.0);
    }
    for mode in [Mode::Icons, Mode::Names] {
        let before = layout.clone();
        assert!(layout.apply(&Action::SetMode { host: id, mode }).is_err());
        assert_eq!(layout, before);
    }
    let before = layout.clone();
    assert!(layout.apply(&Action::SetToolsWidth { width: f32::NAN }).is_err());
    assert_eq!(layout, before);
}

#[test]
fn joined_tools_toggle_retains_sibling_width_and_detaches_at_selected_width() {
    for edge in [Edge::Left, Edge::Right] {
        let mut layout = Layout::new_workspace();
        let layers = layout.owner("layers").unwrap().id;
        let group = layout.group_of("layers").unwrap().id;
        layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host: layers, edge } }).unwrap();
        let sibling = layout.expanded_group_size(group).unwrap();
        for width in [44.0, 80.0, 55.0] {
            layout.apply(&Action::SetToolsWidth { width }).unwrap();
            assert!((layout.tools_width().unwrap() - width).abs() < 0.001);
            close_size(layout.expanded_group_size(group).unwrap(), sibling);
            assert!(layout.apply(&Action::SetMode { host: layers, mode: Mode::Names }).is_err());
        }
        layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Float { position: [30.0, 40.0] } }).unwrap();
        assert!((layout.tools_width().unwrap() - 55.0).abs() < 0.001);
        close_size(layout.owner("layers").unwrap().expanded, sibling);
        assert_eq!(layout.owner("tools").unwrap().mode, Mode::Full);
    }
}

#[test]
fn old_tools_modes_and_oversized_widths_normalize_without_changing_other_hosts() {
    for mode in [Mode::Icons, Mode::Names, Mode::Full] {
        let mut layout = Layout::new_workspace();
        let id = layout.owner("tools").unwrap().id;
        layout.host_mut(id).unwrap().mode = mode;
        layout.host_mut(id).unwrap().expanded = [240.0, 620.0];
        let others: Vec<_> = layout.hosts.iter().filter(|h| h.id != id).cloned().collect();
        let encoded = serde_json::to_string(&layout).unwrap();
        let mut loaded: Layout = serde_json::from_str(&encoded).unwrap();
        loaded.normalize_tools().unwrap();
        assert_eq!(loaded.tools_width(), Some(80.0));
        assert_eq!(loaded.host(id).unwrap().mode, Mode::Full);
        assert_eq!(loaded.hosts.iter().filter(|h| h.id != id).cloned().collect::<Vec<_>>(), others);
        let normalized = loaded.clone();
        loaded.normalize_tools().unwrap();
        assert_eq!(loaded, normalized);
    }
}

#[test]
fn very_wide_sibling_does_not_force_tools_above_its_bound() {
    for edge in [Edge::Left, Edge::Right] {
        let mut layout = Layout::new_workspace();
        let layers = layout.owner("layers").unwrap().id;
        layout.apply(&Action::Resize { host: layers, position: [0.0, 0.0], width: 20_000.0, height: 800.0 }).unwrap();
        layout.apply(&Action::SetToolsWidth { width: 44.0 }).unwrap();
        layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host: layers, edge } }).unwrap();
        assert!((layout.tools_width().unwrap() - 44.0).abs() < 0.01);
        assert!((layout.expanded_group_size(layout.group_of("layers").unwrap().id).unwrap()[0] - 20_000.0).abs() < 0.01);
        let Node::Split { id, ratio, .. } = &layout.host(layers).unwrap().root else { panic!("joined Tools") };
        assert!(*ratio < 0.05 || *ratio > 0.95);
        layout.apply(&Action::SplitRatio { split: *id, ratio: *ratio }).unwrap();
        assert!((layout.tools_width().unwrap() - 44.0).abs() < 0.01);
        layout.validate().unwrap();
    }
}

#[test]
fn joined_tools_geometry_preserves_saved_leaf_width_when_host_is_enlarged_or_clipped() {
    use crate::varos::panel_system::geometry;
    use egui::{Rect, pos2, vec2};
    for edge in [Edge::Left, Edge::Right] {
        for width in [44.0, 80.0] {
            let mut layout = Layout::new_workspace();
            let host = layout.owner("layers").unwrap().id;
            let group = layout.group_of("layers").unwrap().id;
            layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host, edge } }).unwrap();
            // An ordinary sibling can be smaller than its presentation minimum. Dock
            // measurement grows the outer host, while Tools keeps its chosen width.
            let Node::Split { id: split, .. } = layout.host(host).unwrap().root else { panic!("joined Tools") };
            layout.apply(&Action::SplitRatio { split, ratio: if edge == Edge::Left { 0.9 } else { 0.1 } }).unwrap();
            layout.apply(&Action::SetToolsWidth { width }).unwrap();
            assert!(layout.expanded_group_size(group).unwrap()[0] < 240.0);
            layout.apply(&Action::Move { source: Source::Host(host), target: Target::Dock { edge } }).unwrap();
            let tools = layout.group_of("tools").unwrap().id;
            for region_width in [1600.0, 400.0] {
                let measured = geometry::measure(&layout, Rect::from_min_size(pos2(0.0, 0.0), vec2(region_width, 1000.0)));
                let measured = measured.iter().find(|h| h.id == host).unwrap();
                let leaf = measured.groups.iter().find(|g| g.id == tools).unwrap();
                let actual = leaf.rect.width() + HOST_CHROME[0];
                assert!(
                    (actual - width).abs() < 0.01,
                    "Tools width {actual}, expected {width}; edge {edge:?}, region width {region_width}, host {:?}",
                    measured.rect,
                );
                assert!(leaf.body.height() > 0.0);
            }
        }
    }
}

#[test]
fn narrow_floating_tools_has_full_body_without_clipped_tab_controls() {
    use crate::varos::panel_system::geometry;
    use egui::{Rect, pos2, vec2};
    let mut layout = Layout::new_workspace();
    layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Float { position: [30.0, 40.0] } }).unwrap();
    layout.apply(&Action::SetToolsWidth { width: 44.0 }).unwrap();
    let host = layout.owner("tools").unwrap();
    let measured = geometry::measure_host(host, Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1000.0)));
    let group = measured.groups.first().unwrap();
    assert!(group.tabs.is_empty());
    assert_eq!(group.body, group.rect);
    assert_eq!(host.mode, Mode::Full);
}

#[test]
fn tools_normalization_rejects_malformed_saved_geometry_atomically() {
    for width in [f32::NAN, f32::INFINITY, -20.0] {
        let mut layout = Layout::new_workspace();
        let id = layout.owner("tools").unwrap().id;
        layout.host_mut(id).unwrap().expanded[0] = width;
        layout.host_mut(id).unwrap().mode = Mode::Icons;
        assert!(layout.normalize_tools().is_err());
        assert_eq!(layout.host(id).unwrap().mode, Mode::Icons);
        assert_eq!(layout.host(id).unwrap().expanded[0].to_bits(), width.to_bits());
    }
    let mut layout = Layout::new_workspace();
    let host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host, edge: Edge::Left } }).unwrap();
    for invalid in [f32::NAN, f32::INFINITY, -0.1, 0.0, 1.0] {
        let mut saved = layout.clone();
        if let Node::Split { ratio, .. } = &mut saved.host_mut(host).unwrap().root {
            *ratio = invalid;
        }
        assert!(saved.normalize_tools().is_err());
    }
}

#[test]
fn tools_geometry_divider_limits_are_ordered_in_tiny_viewports() {
    use crate::varos::panel_system::geometry;
    use egui::{Rect, pos2, vec2};
    for edge in [Edge::Left, Edge::Right] {
        let mut layout = Layout::new_workspace();
        let host = layout.owner("layers").unwrap().id;
        layout.apply(&Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host, edge } }).unwrap();
        for width in [10.0, 45.0, 100.0] {
            let measured = geometry::measure_host(layout.host(host).unwrap(), Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 100.0)));
            for divider in measured.dividers {
                assert!(divider.min_ratio.is_finite() && divider.max_ratio.is_finite());
                assert!(divider.min_ratio <= divider.max_ratio);
                assert!(0.5_f32.clamp(divider.min_ratio, divider.max_ratio).is_finite());
            }
        }
    }
}
