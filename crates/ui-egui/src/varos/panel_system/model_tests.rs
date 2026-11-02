//! Ownership and transaction contracts of the two-panel native slice.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn assert_owners(layout: &Layout) {
    layout.validate().unwrap();
    let mut owned: Vec<&str> =
        layout.hosts.iter().flat_map(|host| groups(&host.root)).flat_map(|group| group.panels.iter().map(String::as_str)).collect();
    owned.sort_unstable();
    assert_eq!(owned, ["layers", "swatches"], "every admitted panel has exactly one owner, including hidden hosts");
}

fn merge_panels() -> Layout {
    let mut layout = Layout::new_slice();
    let group = layout.group_of("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: 1 } }).unwrap();
    assert_owners(&layout);
    layout
}

fn stack_panels(edge: Edge) -> Layout {
    let mut layout = Layout::new_slice();
    let source = layout.owner("swatches").unwrap().id;
    let host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Host(source), target: Target::Attach { host, edge } }).unwrap();
    assert_owners(&layout);
    layout
}

fn rejected_without_mutation(layout: &mut Layout, action: Action) {
    let before = layout.clone();
    assert!(layout.apply(&action).is_err(), "expected a rejected action: {action:?}");
    assert_eq!(*layout, before, "rejection must preserve ownership, geometry and the identity counter");
    assert_owners(layout);
}

#[test]
fn the_slice_starts_with_two_real_panel_ids_and_independent_hosts() {
    let layout = Layout::new_slice();
    assert_owners(&layout);
    assert_eq!(layout.hosts.len(), 2);
    assert_ne!(layout.owner("layers").unwrap().id, layout.owner("swatches").unwrap().id);
    assert_ne!(layout.group_of("layers").unwrap().id, layout.group_of("swatches").unwrap().id);
}

#[test]
fn merging_and_reordering_tabs_preserves_the_host_and_one_group() {
    let mut layout = merge_panels();
    let host = layout.owner("layers").unwrap().id;
    let group = layout.group_of("layers").unwrap().id;
    let next_id = layout.next_id;
    assert_eq!(layout.hosts.len(), 1);
    assert_eq!(layout.group(group).unwrap().panels, ["layers", "swatches"]);
    assert_eq!(layout.group(group).unwrap().active, "swatches");

    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: 0 } }).unwrap();
    assert_eq!(layout.group(group).unwrap().panels, ["swatches", "layers"]);
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: 2 } }).unwrap();
    assert_eq!(layout.group(group).unwrap().panels, ["layers", "swatches"]);
    assert_eq!(layout.owner("swatches").unwrap().id, host);
    assert_eq!(layout.next_id, next_id, "reordering does not replace stable identities");
    assert_owners(&layout);
}

#[test]
fn detaching_a_tab_keeps_the_other_tab_and_the_contents_expanded_size() {
    let mut layout = merge_panels();
    let source_host = layout.owner("layers").unwrap().clone();
    let source_group = layout.group_of("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Float { position: [550.0, 210.0] } }).unwrap();

    let floating = layout.owner("swatches").unwrap();
    assert_eq!(layout.hosts.len(), 2);
    assert_ne!(floating.id, source_host.id);
    assert_eq!(floating.position, [550.0, 210.0]);
    assert_eq!(floating.expanded, source_host.expanded);
    assert_eq!(layout.group(source_group).unwrap().panels, ["layers"]);
    assert_eq!(layout.group(source_group).unwrap().active, "layers");
    assert_owners(&layout);
}

#[test]
fn moving_a_single_panel_host_does_not_allocate_replacement_identities() {
    for source_kind in 0..3 {
        let mut layout = Layout::new_slice();
        let host = layout.owner("layers").unwrap().clone();
        let group = layout.group_of("layers").unwrap().id;
        let next_id = layout.next_id;
        let source = match source_kind {
            0 => Source::Host(host.id),
            1 => Source::Group(group),
            _ => Source::Panel("layers".into()),
        };
        layout.apply(&Action::Move { source, target: Target::Float { position: [180.0, 250.0] } }).unwrap();
        assert_eq!(layout.owner("layers").unwrap().id, host.id);
        assert_eq!(layout.group_of("layers").unwrap().id, group);
        assert_eq!(layout.owner("layers").unwrap().position, [180.0, 250.0]);
        assert_eq!(layout.next_id, next_id);
        assert_owners(&layout);
    }
}

#[test]
fn attaching_hosts_preserves_group_boundaries_for_each_edge() {
    for (edge, axis, expected) in [
        (Edge::Bottom, Axis::Vertical, ["layers", "swatches"]),
        (Edge::Top, Axis::Vertical, ["swatches", "layers"]),
        (Edge::Right, Axis::Horizontal, ["layers", "swatches"]),
        (Edge::Left, Axis::Horizontal, ["swatches", "layers"]),
    ] {
        let initial = Layout::new_slice();
        let layer_group = initial.group_of("layers").unwrap().id;
        let swatch_group = initial.group_of("swatches").unwrap().id;
        let layout = stack_panels(edge);
        assert_eq!(layout.hosts.len(), 1);
        let host = layout.owner("layers").unwrap();
        let Node::Split { axis: actual_axis, .. } = &host.root else { panic!("an attached host keeps a split") };
        assert_eq!(*actual_axis, axis);
        let ordered: Vec<&str> = groups(&host.root).into_iter().flat_map(|group| group.panels.iter().map(String::as_str)).collect();
        assert_eq!(ordered, expected);
        assert_eq!(layout.group_of("layers").unwrap().id, layer_group);
        assert_eq!(layout.group_of("swatches").unwrap().id, swatch_group);
        assert_eq!(groups(&host.root).len(), 2);
        assert_owners(&layout);
    }
}

#[test]
fn splitting_a_merged_tab_makes_separate_groups_and_keeps_the_remaining_group() {
    let mut layout = merge_panels();
    let original_group = layout.group_of("layers").unwrap().id;
    layout
        .apply(&Action::Move { source: Source::Panel("swatches".into()), target: Target::Split { group: original_group, edge: Edge::Bottom } })
        .unwrap();
    assert_eq!(layout.hosts.len(), 1);
    assert_eq!(layout.group(original_group).unwrap().panels, ["layers"]);
    assert_ne!(layout.group_of("swatches").unwrap().id, original_group);
    assert_eq!(groups(&layout.owner("layers").unwrap().root).len(), 2);
    assert_owners(&layout);
}

#[test]
fn a_nested_group_can_detach_and_leave_a_normalized_single_group() {
    let mut layout = stack_panels(Edge::Bottom);
    let group = layout.group_of("swatches").unwrap().id;
    let original_host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Group(group), target: Target::Float { position: [430.0, 180.0] } }).unwrap();
    assert_eq!(layout.hosts.len(), 2);
    assert_eq!(layout.group_of("swatches").unwrap().id, group);
    assert!(matches!(&layout.host(original_host).unwrap().root, Node::Group(_)), "empty branch and redundant split are removed");
    assert_owners(&layout);
}

#[test]
fn reattaching_a_group_inside_its_host_reorders_without_self_nesting() {
    let mut layout = stack_panels(Edge::Bottom);
    let host = layout.owner("swatches").unwrap().id;
    let group = layout.group_of("swatches").unwrap().id;
    layout.apply(&Action::Move { source: Source::Group(group), target: Target::Attach { host, edge: Edge::Top } }).unwrap();
    let order: Vec<&str> = groups(&layout.host(host).unwrap().root).into_iter().flat_map(|g| g.panels.iter().map(String::as_str)).collect();
    assert_eq!(order, ["swatches", "layers"]);
    assert_eq!(groups(&layout.host(host).unwrap().root).len(), 2);
    assert_eq!(layout.group_of("swatches").unwrap().id, group);
    assert_owners(&layout);
}

#[test]
fn modes_and_compact_edge_resizes_preserve_the_tree_and_expanded_dimensions() {
    let mut layout = stack_panels(Edge::Bottom);
    let host = layout.owner("layers").unwrap().clone();
    for mode in [Mode::Icons, Mode::Names, Mode::Full] {
        layout.apply(&Action::SetMode { host: host.id, mode }).unwrap();
        let actual = layout.host(host.id).unwrap();
        assert_eq!(actual.mode, mode);
        assert_eq!(actual.root, host.root);
        assert_eq!(actual.position, host.position);
        assert_eq!(actual.expanded, host.expanded);
    }
    layout.apply(&Action::SetMode { host: host.id, mode: Mode::Icons }).unwrap();
    for (width, mode) in [(180.0, Mode::Names), (44.0, Mode::Icons)] {
        layout.apply(&Action::Resize { host: host.id, position: [110.0, 80.0], width, height: 100.0 }).unwrap();
        let actual = layout.host(host.id).unwrap();
        assert_eq!(actual.mode, mode);
        assert_eq!(actual.compact_width, width);
        assert_eq!(actual.expanded, host.expanded);
        assert_eq!(actual.root, host.root);
    }
    assert_owners(&layout);
}

#[test]
fn hiding_then_showing_a_panel_restores_its_entire_host_at_the_same_place() {
    let mut layout = merge_panels();
    let host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::Move { source: Source::Host(host), target: Target::Float { position: [330.0, 190.0] } }).unwrap();
    layout.apply(&Action::SetMode { host, mode: Mode::Names }).unwrap();
    layout.apply(&Action::Activate { panel: "layers".into() }).unwrap();
    let before = layout.host(host).unwrap().clone();
    layout.apply(&Action::Hide { host }).unwrap();
    assert!(layout.host(host).unwrap().hidden);
    assert_owners(&layout);
    layout.apply(&Action::Show { panel: "swatches".into() }).unwrap();
    let restored = layout.host(host).unwrap();
    assert!(!restored.hidden);
    assert_eq!(restored.id, before.id);
    assert_eq!(restored.position, before.position);
    assert_eq!(restored.expanded, before.expanded);
    assert_eq!(restored.mode, before.mode);
    assert_eq!(layout.group_of("swatches").unwrap().panels, ["layers", "swatches"]);
    assert_eq!(layout.group_of("swatches").unwrap().active, "swatches");
    assert_owners(&layout);
}

#[test]
fn round_trip_keeps_hidden_stacks_presentation_and_geometry() {
    let mut layout = stack_panels(Edge::Right);
    let host = layout.owner("layers").unwrap().id;
    layout.apply(&Action::SetMode { host, mode: Mode::Names }).unwrap();
    layout.apply(&Action::Resize { host, position: [210.0, 90.0], width: 208.0, height: 400.0 }).unwrap();
    layout.apply(&Action::Hide { host }).unwrap();
    let encoded = serde_json::to_vec(&layout).unwrap();
    let restored: Layout = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored, layout);
    assert_owners(&restored);
    let action = Action::Move { source: Source::Panel("layers".into()), target: Target::Float { position: [100.0, 200.0] } };
    assert_eq!(serde_json::from_value::<Action>(serde_json::to_value(&action).unwrap()).unwrap(), action);
}

#[test]
fn rejected_moves_do_not_consume_ids_or_remove_panels() {
    let mut layout = merge_panels();
    let host = layout.owner("layers").unwrap().id;
    let group = layout.group_of("layers").unwrap().id;
    for action in [
        Action::Move { source: Source::Panel("color".into()), target: Target::Float { position: [10.0, 20.0] } },
        Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group: 999, index: 0 } },
        Action::Move { source: Source::Panel("swatches".into()), target: Target::Merge { group, index: usize::MAX } },
        Action::Move { source: Source::Host(host), target: Target::Attach { host, edge: Edge::Bottom } },
        Action::Move { source: Source::Group(group), target: Target::Merge { group, index: 0 } },
        Action::Move { source: Source::Panel("swatches".into()), target: Target::Float { position: [f32::NAN, 0.0] } },
        Action::Move { source: Source::Panel("swatches".into()), target: Target::Float { position: [2_000_000.0, 0.0] } },
        Action::SetMode { host: 999, mode: Mode::Icons },
        Action::Hide { host: 999 },
        Action::Show { panel: "color".into() },
    ] {
        rejected_without_mutation(&mut layout, action);
    }
}

#[test]
fn rejected_geometry_and_exhausted_identity_allocation_are_atomic() {
    let mut layout = merge_panels();
    let host = layout.owner("layers").unwrap().id;
    for (position, width, height) in
        [([0.0, 0.0], 0.0, 100.0), ([0.0, 0.0], 100.0, f32::INFINITY), ([f32::NEG_INFINITY, 0.0], 100.0, 100.0), ([2_000_000.0, 0.0], 100.0, 100.0)]
    {
        rejected_without_mutation(&mut layout, Action::Resize { host, position, width, height });
    }
    layout.next_id = u64::MAX;
    rejected_without_mutation(
        &mut layout,
        Action::Move { source: Source::Panel("swatches".into()), target: Target::Float { position: [300.0, 200.0] } },
    );
}

#[test]
fn validation_rejects_unknown_duplicate_missing_and_inactive_panel_references() {
    let original = merge_panels();
    let id = original.group_of("layers").unwrap().id;
    for invalid_panels in [vec!["layers".into()], vec!["layers".into(), "layers".into()], vec!["layers".into(), "color".into()]] {
        let mut layout = original.clone();
        group_mut(&mut layout.hosts.first_mut().unwrap().root, id).unwrap().panels = invalid_panels;
        assert!(layout.validate().is_err());
    }
    let mut layout = original.clone();
    group_mut(&mut layout.hosts.first_mut().unwrap().root, id).unwrap().active = "color".into();
    assert!(layout.validate().is_err());
    let mut layout = original;
    layout.version += 1;
    assert!(layout.validate().is_err());
}
