//! Registry ownership, dock transitions and multi-group geometry contracts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn apply(layout: &mut Layout, action: Action) {
    layout.apply(&action).unwrap();
    layout.validate().unwrap();
}

fn owned(layout: &Layout) -> BTreeSet<String> {
    layout.validate().unwrap();
    let mut seen = BTreeSet::new();
    for group in layout.hosts.iter().flat_map(|host| groups(&host.root)) {
        for panel in &group.panels {
            assert!(super::super::registry::contains(panel), "unknown owner: {panel}");
            assert!(seen.insert(panel.clone()), "duplicate owner: {panel}");
        }
    }
    seen
}

fn close_size(actual: [f32; 2], expected: [f32; 2]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 0.002, "dimension {actual} != {expected}");
    }
}

fn group_dimensions(layout: &Layout, panel: &str) -> [f32; 2] {
    layout.expanded_group_size(layout.group_of(panel).unwrap().id).unwrap()
}

fn resize(layout: &mut Layout, panel: &str, size: [f32; 2]) {
    let host = layout.owner(panel).unwrap().id;
    apply(layout, Action::Resize { host, position: [80.0, 100.0], width: size[0], height: size[1] });
}

fn round_trip(layout: &Layout) {
    let json = serde_json::to_vec(layout).unwrap();
    let restored: Layout = serde_json::from_slice(&json).unwrap();
    assert_eq!(&restored, layout);
    assert_eq!(owned(&restored), owned(layout));
}

#[test]
fn fitted_control_content_changes_only_measurement_and_survives_resize_and_float_but_not_serialization() {
    for panel in ["control", "control@2"] {
        let mut layout = Layout::new_workspace();
        layout.ensure_panel(panel).unwrap();
        let host = layout.owner(panel).unwrap().id;
        let before = layout.clone();
        apply(&mut layout, Action::FitControlContent { host, height: 34.0 });
        let mut expected = before;
        expected.host_mut(host).unwrap().dock_content_height = Some(34.0);
        assert_eq!(layout, expected, "measurement does not alter floating size, ownership or presentation");
        assert_eq!(layout.host(host).unwrap().dock_content_height, Some(34.0));
        resize(&mut layout, panel, [900.0, 140.0]);
        assert_eq!(layout.host(host).unwrap().dock_content_height, Some(34.0));
        apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Float { position: [320.0, 260.0] } });
        assert_eq!(layout.host(host).unwrap().dock_content_height, Some(34.0));
        close_size(layout.host(host).unwrap().expanded, [900.0, 140.0]);
        apply(&mut layout, Action::FitControlContent { host, height: 62.0 });
        assert_eq!(layout.host(host).unwrap().dock_content_height, Some(62.0));
        close_size(layout.host(host).unwrap().expanded, [900.0, 140.0]);
        round_trip(&layout);
        let serialized = serde_json::to_value(&layout).unwrap();
        assert!(!serialized.to_string().contains("dock_content_height"));
        let restored: Layout = serde_json::from_value(serialized).unwrap();
        assert_eq!(restored.host(host).unwrap().dock_content_height, None);
        assert_eq!(restored, layout, "restoring a workspace ignores obsolete presentation measurements");
    }
}

#[test]
fn workspace_equality_ignores_content_fit_but_compares_every_persisted_host_field() {
    let layout = Layout::new_workspace();
    let base = layout.owner("control").unwrap().clone();
    let mut first = base.clone();
    first.dock_content_height = Some(34.0);
    let mut second = base.clone();
    second.dock_content_height = Some(62.0);
    assert_eq!(first, second);
    assert_eq!(base, first);
    let mut measured = layout.clone();
    measured.host_mut(base.id).unwrap().dock_content_height = Some(62.0);
    assert_eq!(layout, measured, "context changes must not dirty the workspace");

    let mut variants = Vec::new();
    let mut changed = base.clone();
    changed.id += 1;
    variants.push(changed);
    let mut changed = base.clone();
    changed.position[0] += 1.0;
    variants.push(changed);
    let mut changed = base.clone();
    changed.expanded[1] += 1.0;
    variants.push(changed);
    let mut changed = base.clone();
    changed.compact_width += 1.0;
    variants.push(changed);
    let mut changed = base.clone();
    changed.mode = Mode::Names;
    variants.push(changed);
    let mut changed = base.clone();
    changed.hidden = !changed.hidden;
    variants.push(changed);
    let mut changed = base.clone();
    changed.dock = None;
    variants.push(changed);
    let mut changed = base.clone();
    if let Node::Group(group) = &mut changed.root {
        group.id += 1;
    }
    variants.push(changed);
    for changed in variants {
        assert_ne!(changed, base, "persisted host state remains part of equality");
    }
}

#[test]
fn fitted_control_content_rejects_invalid_measurements_and_non_control_hosts_atomically() {
    let mut layout = Layout::new_workspace();
    let control = layout.owner("control").unwrap().id;
    for height in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -1.0, 1_000_001.0] {
        rejected(&mut layout, Action::FitControlContent { host: control, height });
        let mut invalid = layout.clone();
        invalid.host_mut(control).unwrap().dock_content_height = Some(height);
        assert!(invalid.validate().is_err());
    }
    for panel in ["tools", "layers", "swatches", "properties"] {
        let host = layout.owner(panel).unwrap().id;
        rejected(&mut layout, Action::FitControlContent { host, height: 34.0 });
        let mut invalid = layout.clone();
        invalid.host_mut(host).unwrap().dock_content_height = Some(34.0);
        assert!(invalid.validate().is_err(), "saved non-Control hosts cannot carry this measurement");
    }
    rejected(&mut layout, Action::FitControlContent { host: u64::MAX, height: 34.0 });
    apply(&mut layout, Action::FitControlContent { host: control, height: 1_000_000.0 });
}

#[test]
fn layouts_saved_before_fitted_control_content_default_to_no_measurement() {
    let layout = Layout::new_workspace();
    assert!(layout.hosts.iter().all(|host| host.dock_content_height.is_none()));
    let mut encoded = serde_json::to_value(&layout).unwrap();
    for host in encoded.get_mut("hosts").unwrap().as_array_mut().unwrap() {
        host.as_object_mut().unwrap().remove("dock_content_height");
    }
    let restored: Layout = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored, layout);
    restored.validate().unwrap();
}

fn rejected(layout: &mut Layout, action: Action) {
    let before = layout.clone();
    assert!(layout.apply(&action).is_err(), "expected placement rejection: {action:?}");
    assert_eq!(*layout, before, "a rejected action must preserve identities, ownership and geometry");
    layout.validate().unwrap();
}

#[test]
fn workspace_registers_each_real_builtin_once_and_preserves_the_accepted_pair() {
    let layout = Layout::new_workspace();
    let expected: BTreeSet<String> = super::super::registry::BUILTINS.iter().map(|spec| spec.id.to_string()).collect();
    assert_eq!(owned(&layout), expected);
    let visible: BTreeSet<&str> = layout
        .hosts
        .iter()
        .filter(|host| !host.hidden)
        .flat_map(|host| groups(&host.root))
        .flat_map(|group| group.panels.iter().map(String::as_str))
        .collect();
    assert_eq!(visible, BTreeSet::from(["layers", "swatches", "tools", "control"]));
    assert_eq!(layout.owner("tools").unwrap().dock, Some(Edge::Left));
    assert_eq!(layout.owner("control").unwrap().dock, Some(Edge::Top));
    for panel in PANELS {
        assert_eq!(layout.owner(panel).unwrap().dock, None);
        close_size(layout.owner(panel).unwrap().expanded, [300.0, 380.0]);
    }
    round_trip(&layout);
}

#[test]
fn tools_and_control_dock_float_hide_and_restore_without_losing_geometry() {
    for panel in ["tools", "control"] {
        let mut layout = Layout::new_workspace();
        let original = layout.owner(panel).unwrap().clone();
        let identities = owned(&layout);
        let mut remembered = original.expanded;
        let edges = if panel == "tools" { [Edge::Left, Edge::Right] } else { [Edge::Top, Edge::Bottom] };
        for edge in edges {
            apply(&mut layout, Action::Move { source: Source::Host(original.id), target: Target::Float { position: [180.0, 220.0] } });
            assert_eq!(layout.owner(panel).unwrap().dock, None);
            assert_eq!(layout.owner(panel).unwrap().id, original.id);
            close_size(layout.owner(panel).unwrap().expanded, remembered);
            apply(&mut layout, Action::Move { source: Source::Host(original.id), target: Target::Dock { edge } });
            let docked = layout.owner(panel).unwrap();
            assert_eq!(docked.dock, Some(edge));
            assert_eq!(docked.id, original.id);
            assert_eq!(docked.root, original.root);
            remembered = docked.expanded;
            if panel == "tools" {
                assert!(remembered[0] <= remembered[1], "Tools remains vertical");
            } else {
                assert!(remembered[0] >= remembered[1], "Control remains horizontal");
            }
            if panel == "tools" {
                rejected(&mut layout, Action::SetMode { host: original.id, mode: Mode::Icons });
                apply(&mut layout, Action::SetToolsWidth { width: 44.0 });
                remembered[0] = 44.0;
                assert_eq!(layout.owner(panel).unwrap().mode, Mode::Full);
            } else {
                for mode in [Mode::Icons, Mode::Names] {
                    rejected(&mut layout, Action::SetMode { host: original.id, mode });
                }
                assert_eq!(layout.owner(panel).unwrap().mode, Mode::Full);
            }
            close_size(layout.owner(panel).unwrap().expanded, remembered);
            apply(&mut layout, Action::Hide { host: original.id });
            assert!(layout.owner(panel).unwrap().hidden);
            apply(&mut layout, Action::Show { panel: panel.into() });
            let restored = layout.owner(panel).unwrap();
            assert!(!restored.hidden);
            assert_eq!(restored.dock, Some(edge));
            assert_eq!(restored.position, [180.0, 220.0]);
            assert_eq!(restored.compact_width, original.compact_width);
            close_size(restored.expanded, remembered);
            assert_eq!(restored.root, original.root);
            round_trip(&layout);
            apply(&mut layout, Action::SetMode { host: original.id, mode: Mode::Full });
        }
        assert_eq!(owned(&layout), identities);
    }
}

#[test]
fn moving_tools_and_control_between_edges_of_the_same_orientation_preserves_user_size() {
    for (panel, first, second, size) in [("tools", Edge::Left, Edge::Right, [64.0, 620.0]), ("control", Edge::Top, Edge::Bottom, [860.0, 140.0])] {
        let mut layout = Layout::new_workspace();
        let host = layout.owner(panel).unwrap().id;
        let root = layout.host(host).unwrap().root.clone();
        apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Dock { edge: first } });
        resize(&mut layout, panel, size);
        apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Dock { edge: second } });
        close_size(layout.host(host).unwrap().expanded, size);
        assert_eq!(layout.host(host).unwrap().root, root);
        apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Float { position: [150.0, 170.0] } });
        close_size(layout.host(host).unwrap().expanded, size);
        apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Dock { edge: first } });
        close_size(layout.host(host).unwrap().expanded, size);
        assert_eq!(layout.host(host).unwrap().root, root);
    }
}

#[test]
fn special_bar_capabilities_reject_wrong_docks_tab_merges_and_incompatible_splits_atomically() {
    let mut layout = Layout::new_workspace();
    let layers = layout.owner("layers").unwrap().id;
    let layers_group = layout.group_of("layers").unwrap().id;
    for panel in ["tools", "control"] {
        let host = layout.owner(panel).unwrap().id;
        let group = layout.group_of(panel).unwrap().id;
        let invalid_edges = if panel == "tools" { [Edge::Top, Edge::Bottom] } else { [Edge::Left, Edge::Right] };
        for edge in invalid_edges {
            assert!(!dock_allowed(&layout.host(host).unwrap().root, edge));
            for source in [Source::Host(host), Source::Group(group), Source::Panel(panel.into())] {
                rejected(&mut layout, Action::Move { source, target: Target::Dock { edge } });
            }
        }
        rejected(&mut layout, Action::Move { source: Source::Panel(panel.into()), target: Target::Merge { group: layers_group, index: 1 } });
        rejected(&mut layout, Action::Move { source: Source::Panel("layers".into()), target: Target::Merge { group, index: 1 } });
        let split_edges: &[Edge] = if panel == "tools" { &[Edge::Top, Edge::Bottom] } else { &[Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] };
        for edge in split_edges {
            rejected(&mut layout, Action::Move { source: Source::Panel(panel.into()), target: Target::Attach { host: layers, edge: *edge } });
            rejected(&mut layout, Action::Move { source: Source::Panel(panel.into()), target: Target::Split { group: layers_group, edge: *edge } });
            rejected(&mut layout, Action::Move { source: Source::Panel("layers".into()), target: Target::Attach { host, edge: *edge } });
        }
    }
    let tools = layout.owner("tools").unwrap().id;
    let tools_group = layout.group_of("tools").unwrap().id;
    rejected(&mut layout, Action::Move { source: Source::Panel("control".into()), target: Target::Merge { group: tools_group, index: 1 } });
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        rejected(&mut layout, Action::Move { source: Source::Panel("control".into()), target: Target::Attach { host: tools, edge } });
    }
}

#[test]
fn tools_can_join_ordinary_groups_beside_them_without_sharing_tabs_or_stacking_tools() {
    let mut layout = Layout::new_workspace();
    let expected = owned(&layout);
    let host = layout.owner("layers").unwrap().id;
    let tools_group = layout.group_of("tools").unwrap().id;
    let layers_group = layout.group_of("layers").unwrap().id;
    apply(&mut layout, Action::Move { source: Source::Group(tools_group), target: Target::Attach { host, edge: Edge::Left } });
    assert_eq!(layout.owner("tools").unwrap().id, host);
    assert_eq!(layout.group(tools_group).unwrap().panels, ["tools"]);
    assert_eq!(layout.group(layers_group).unwrap().panels, ["layers"]);
    // The ordinary sibling can still have its own stacked groups.
    apply(&mut layout, Action::Move { source: Source::Panel("swatches".into()), target: Target::Split { group: layers_group, edge: Edge::Bottom } });
    assert_eq!(groups(&layout.host(host).unwrap().root).len(), 3);
    apply(&mut layout, Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Right } });
    rejected(&mut layout, Action::Move { source: Source::Host(host), target: Target::Dock { edge: Edge::Top } });
    let remembered = group_dimensions(&layout, "tools");
    rejected(&mut layout, Action::SetMode { host, mode: Mode::Icons });
    apply(&mut layout, Action::Hide { host });
    for panel in ["layers", "tools", "swatches"] {
        assert!(layout.owner(panel).unwrap().hidden);
    }
    apply(&mut layout, Action::Show { panel: "tools".into() });
    assert!(!layout.owner("layers").unwrap().hidden);
    round_trip(&layout);
    apply(&mut layout, Action::Move { source: Source::Group(tools_group), target: Target::Float { position: [500.0, 150.0] } });
    assert_ne!(layout.owner("tools").unwrap().id, host);
    assert_eq!(layout.owner("tools").unwrap().dock, None);
    close_size(layout.owner("tools").unwrap().expanded, remembered);
    assert_eq!(layout.group_of("tools").unwrap().id, tools_group);
    assert_eq!(layout.owner("layers").unwrap().dock, Some(Edge::Right));
    assert_eq!(owned(&layout), expected);
    round_trip(&layout);
}

#[test]
fn special_bar_orientation_is_enforced_by_resize_and_saved_layout_validation() {
    let mut layout = Layout::new_workspace();
    for (panel, bad_size, bad_edge) in [("tools", [900.0, 100.0], Edge::Top), ("control", [100.0, 900.0], Edge::Left)] {
        let host = layout.owner(panel).unwrap().id;
        let resize = Action::Resize { host, position: [80.0, 100.0], width: bad_size[0], height: bad_size[1] };
        if panel == "tools" {
            apply(&mut layout, resize);
            close_size(layout.host(host).unwrap().expanded, [80.0, 110.0]);
        } else {
            rejected(&mut layout, resize);
        }
        let mut invalid = layout.clone();
        invalid.host_mut(host).unwrap().expanded = bad_size;
        assert!(invalid.validate().is_err());
        let mut invalid = layout.clone();
        invalid.host_mut(host).unwrap().dock = Some(bad_edge);
        assert!(invalid.validate().is_err());
    }
    let tools = layout.owner("tools").unwrap().id;
    let mut invalid = layout.clone();
    invalid.hosts.retain(|host| host.id != tools);
    let layers = invalid.owner("layers").unwrap().id;
    if let Node::Group(group) = &mut invalid.host_mut(layers).unwrap().root {
        group.panels.push("tools".into());
    }
    assert!(invalid.validate().is_err(), "serialized tab groups cannot bypass the same placement rules");
}

#[test]
fn resizing_a_host_or_divider_cannot_turn_its_nested_tools_group_horizontal() {
    let mut layout = Layout::new_workspace();
    let host = layout.owner("layers").unwrap().id;
    apply(&mut layout, Action::Move { source: Source::Panel("tools".into()), target: Target::Attach { host, edge: Edge::Left } });
    resize(&mut layout, "layers", [380.0, 180.0]);
    let Node::Split { id: split, .. } = layout.host(host).unwrap().root else { panic!("expected horizontal groups") };
    apply(&mut layout, Action::SplitRatio { split, ratio: 0.95 });
    assert!((layout.tools_width().unwrap() - 80.0).abs() < 0.01);
    apply(&mut layout, Action::Resize { host, position: [80.0, 100.0], width: 2000.0, height: 180.0 });
    assert!((layout.tools_width().unwrap() - 80.0).abs() < 0.01);
    let mut invalid = layout.clone();
    if let Node::Split { axis, .. } = &mut invalid.host_mut(host).unwrap().root {
        *axis = Axis::Vertical;
    }
    assert!(invalid.validate().is_err(), "serialized ancestors cannot stack Tools above another group");
}

#[test]
fn dynamic_editors_and_libraries_have_independent_stable_owners() {
    let mut layout = Layout::new_workspace();
    let dynamic = [
        "properties@2",
        "properties@3",
        "control@2",
        "library:swatches:pastels",
        "library:brushes:sample",
        "library:symbols:sample",
        "library:graphicStyles:sample",
    ];
    let mut host_ids = BTreeSet::new();
    for panel in dynamic {
        layout.ensure_panel(panel).unwrap();
        let before = layout.clone();
        layout.ensure_panel(panel).unwrap();
        assert_eq!(layout, before, "ensuring an owned panel is idempotent");
        let host = layout.owner(panel).unwrap();
        assert!(host.hidden);
        assert!(host_ids.insert(host.id));
        assert_eq!(layout.group_of(panel).unwrap().panels, [panel]);
        apply(&mut layout, Action::Show { panel: panel.into() });
    }
    assert_eq!(owned(&layout).len(), super::super::registry::BUILTINS.len() + dynamic.len());
    round_trip(&layout);
    for invalid in ["properties@1", "layers@2", "library:unknown:sample", "unknown"] {
        let before = layout.clone();
        assert!(layout.ensure_panel(invalid).is_err());
        assert_eq!(layout, before);
    }
}

#[test]
fn lazy_instance_limit_is_atomic() {
    let mut layout = Layout::new_workspace();
    let extra = MAX_PANELS - super::super::registry::BUILTINS.len();
    for index in 0..extra {
        layout.ensure_panel(&format!("properties@{}", index + 2)).unwrap();
    }
    assert_eq!(owned(&layout).len(), MAX_PANELS);
    let before = layout.clone();
    assert!(layout.ensure_panel("properties@1000").is_err());
    assert_eq!(layout, before);
}

fn nested_workspace(edge: Edge) -> (Layout, u64, [f32; 2], [f32; 2]) {
    let mut layout = Layout::new_workspace();
    let horizontal = matches!(edge, Edge::Left | Edge::Right);
    resize(&mut layout, "layers", if horizontal { [280.0, 500.0] } else { [500.0, 280.0] });
    resize(&mut layout, "swatches", if horizontal { [360.0, 500.0] } else { [500.0, 360.0] });
    resize(&mut layout, "properties", if horizontal { [320.0, 500.0] } else { [500.0, 320.0] });
    let host = layout.owner("layers").unwrap().id;
    apply(&mut layout, Action::Move { source: Source::Panel("swatches".into()), target: Target::Attach { host, edge } });
    let layers_before = group_dimensions(&layout, "layers");
    let sibling_before = group_dimensions(&layout, "swatches");
    let Node::Split { id: ancestor, .. } = layout.host(host).unwrap().root else { panic!("expected initial split") };
    let group = layout.group_of("layers").unwrap().id;
    apply(&mut layout, Action::Move { source: Source::Panel("properties".into()), target: Target::Split { group, edge } });
    (layout, ancestor, layers_before, sibling_before)
}

#[test]
fn splitting_a_nested_leaf_preserves_its_ancestor_and_unrelated_sibling_extents() {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let (layout, ancestor, layers_before, sibling_before) = nested_workspace(edge);
        let host = layout.owner("layers").unwrap();
        let Node::Split { id, .. } = &host.root else { panic!("expected retained ancestor") };
        assert_eq!(*id, ancestor);
        close_size(group_dimensions(&layout, "layers"), layers_before);
        close_size(group_dimensions(&layout, "swatches"), sibling_before);
        assert_eq!(groups(&host.root).len(), 3);
        round_trip(&layout);
    }
}

#[test]
fn detaching_a_nested_leaf_releases_only_its_space_without_inflating_the_remaining_group() {
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        let (mut layout, _, layers_before, sibling_before) = nested_workspace(edge);
        let properties_before = group_dimensions(&layout, "properties");
        apply(&mut layout, Action::Move { source: Source::Panel("properties".into()), target: Target::Float { position: [640.0, 200.0] } });
        close_size(group_dimensions(&layout, "layers"), layers_before);
        close_size(group_dimensions(&layout, "swatches"), sibling_before);
        close_size(group_dimensions(&layout, "properties"), properties_before);
        round_trip(&layout);
    }
}

#[test]
fn divider_changes_are_scoped_and_invalid_values_are_atomic() {
    let (mut layout, split, _, _) = nested_workspace(Edge::Right);
    let host = layout.owner("layers").unwrap().id;
    let expanded = layout.host(host).unwrap().expanded;
    let identities = owned(&layout);
    apply(&mut layout, Action::SplitRatio { split, ratio: 0.35 });
    assert_eq!(layout.host(host).unwrap().expanded, expanded);
    assert_eq!(owned(&layout), identities);
    round_trip(&layout);
    for ratio in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, 1.0] {
        let before = layout.clone();
        assert!(layout.apply(&Action::SplitRatio { split, ratio }).is_err());
        assert_eq!(layout, before);
    }
    let before = layout.clone();
    assert!(layout.apply(&Action::SplitRatio { split: u64::MAX, ratio: 0.5 }).is_err());
    assert_eq!(layout, before);
}

#[test]
fn control_fitted_geometry_uses_only_content_and_border_in_all_placements() {
    use super::super::geometry;
    let region = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
    for panel in ["control", "control@2"] {
        for dock in [None, Some(Edge::Top), Some(Edge::Bottom)] {
            let mut layout = Layout::new_workspace();
            layout.ensure_panel(panel).unwrap();
            let id = layout.owner(panel).unwrap().id;
            apply(&mut layout, Action::Show { panel: panel.into() });
            let target = dock.map_or(Target::Float { position: [100.0, 200.0] }, |edge| Target::Dock { edge });
            apply(&mut layout, Action::Move { source: Source::Host(id), target });
            resize(&mut layout, panel, [1000.0, 260.0]);
            apply(&mut layout, Action::FitControlContent { host: id, height: 34.0 });
            let host = layout.host(id).unwrap();
            assert!(geometry::is_control(host));
            assert_eq!(geometry::title_height(host), 0.0);
            let measured = geometry::measure(&layout, region);
            let rect = measured.iter().find(|rect| rect.id == id).unwrap();
            let group = rect.groups.first().unwrap();
            assert_eq!(rect.rect.height(), 36.0, "saved tall hosts cannot reserve a blank strip");
            assert!(group.tabs.is_empty());
            assert_eq!(group.body.top(), rect.rect.top() + 1.0);
            assert_eq!(group.body.bottom(), rect.rect.bottom() - 1.0);
            assert_eq!(group.body.left(), rect.rect.left() + 13.0, "one border plus inline grip");
            assert_eq!(group.body.height(), 34.0);
            assert_eq!(layout.host(id).unwrap().expanded, [1000.0, 260.0], "measurement preserves stored geometry");
        }
    }
}

#[test]
fn restored_compact_control_is_normalized_without_changing_ordinary_panel_modes() {
    for mode in [Mode::Icons, Mode::Names] {
        let mut layout = Layout::new_workspace();
        let id = layout.owner("control").unwrap().id;
        let layer = layout.owner("layers").unwrap().id;
        apply(&mut layout, Action::SetMode { host: layer, mode });
        let ordinary = layout.host(layer).unwrap().clone();
        layout.host_mut(id).unwrap().mode = mode;
        layout.normalize_tools().unwrap();
        assert_eq!(layout.host(id).unwrap().mode, Mode::Full);
        assert_eq!(layout.host(layer), Some(&ordinary));
        layout.validate().unwrap();
        round_trip(&layout);
    }
}

#[test]
fn independent_column_modes_and_widths_preserve_expanded_neighbours_and_ownership() {
    use super::super::geometry;
    for dock in [None, Some(Edge::Left), Some(Edge::Right)] {
        let mut layout = Layout::new_slice();
        let left = layout.owner("layers").unwrap().id;
        let right = layout.owner("swatches").unwrap().id;
        apply(&mut layout, Action::Move { source: Source::Host(right), target: Target::Attach { host: left, edge: Edge::Right } });
        if let Some(edge) = dock {
            apply(&mut layout, Action::Move { source: Source::Host(left), target: Target::Dock { edge } });
        }
        let a = layout.group_of("layers").unwrap().id;
        let b = layout.group_of("swatches").unwrap().id;
        let saved = layout.host(left).unwrap().expanded;
        let sibling = layout.expanded_group_size(b).unwrap();
        let identities = owned(&layout);
        apply(&mut layout, Action::SetColumnMode { group: a, mode: Mode::Icons });
        assert_eq!(layout.column_mode(a), Some(Mode::Icons));
        assert_eq!(layout.column_mode(b), Some(Mode::Full));
        close_size(layout.host(left).unwrap().expanded, saved);
        close_size(layout.expanded_group_size(b).unwrap(), sibling);
        let measured = geometry::measure(&layout, egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0)));
        let host = measured.iter().find(|h| h.id == left).unwrap();
        assert_eq!(host.columns.len(), 2);
        assert_eq!(host.groups.iter().find(|g| g.id == a).unwrap().mode, Mode::Icons);
        assert!((host.groups.iter().find(|g| g.id == b).unwrap().rect.width() - (sibling[0] - 2.0)).abs() < 0.01);
        apply(&mut layout, Action::ResizeColumn { group: a, width: 180.0 });
        assert_eq!(layout.column_mode(a), Some(Mode::Names));
        assert_eq!(layout.column_width(a), Some(180.0));
        close_size(layout.expanded_group_size(b).unwrap(), sibling);
        apply(&mut layout, Action::SetColumnMode { group: a, mode: Mode::Full });
        apply(&mut layout, Action::ResizeColumn { group: a, width: 420.0 });
        assert!((layout.column_width(a).unwrap() - 420.0).abs() < 0.01);
        close_size(layout.expanded_group_size(b).unwrap(), sibling);
        assert_eq!(owned(&layout), identities);
        round_trip(&layout);
    }
}

#[test]
fn vertically_stacked_groups_share_only_their_column_presentation() {
    let mut layout = Layout::new_workspace();
    let host = layout.owner("layers").unwrap().id;
    let swatches = layout.owner("swatches").unwrap().id;
    let properties = layout.owner("properties").unwrap().id;
    apply(&mut layout, Action::Move { source: Source::Host(swatches), target: Target::Attach { host, edge: Edge::Bottom } });
    apply(&mut layout, Action::Move { source: Source::Host(properties), target: Target::Attach { host, edge: Edge::Right } });
    let layers = layout.group_of("layers").unwrap().id;
    let swatches = layout.group_of("swatches").unwrap().id;
    let properties = layout.group_of("properties").unwrap().id;
    assert_eq!(layout.column_key(layers), layout.column_key(swatches));
    assert_ne!(layout.column_key(layers), layout.column_key(properties));
    apply(&mut layout, Action::SetColumnMode { group: swatches, mode: Mode::Icons });
    assert_eq!(layout.column_mode(layers), Some(Mode::Icons));
    assert_eq!(layout.column_mode(properties), Some(Mode::Full));
    apply(&mut layout, Action::ResizeColumn { group: layers, width: 160.0 });
    assert_eq!(layout.column_width(swatches), Some(160.0));
    assert_eq!(layout.column_mode(swatches), Some(Mode::Names));
    round_trip(&layout);
}

#[test]
fn compact_column_can_attach_beside_full_host_and_detach_without_losing_its_mode() {
    let mut layout = Layout::new_slice();
    let a = layout.group_of("layers").unwrap().id;
    let b = layout.group_of("swatches").unwrap().id;
    let source = layout.owner("layers").unwrap().id;
    let destination = layout.owner("swatches").unwrap().id;
    apply(&mut layout, Action::SetColumnMode { group: a, mode: Mode::Icons });
    apply(&mut layout, Action::Move { source: Source::Host(source), target: Target::Attach { host: destination, edge: Edge::Left } });
    assert_eq!(layout.column_mode(a), Some(Mode::Icons));
    assert_eq!(layout.column_mode(b), Some(Mode::Full));
    apply(&mut layout, Action::Move { source: Source::Group(a), target: Target::Float { position: [100.0, 100.0] } });
    assert_eq!(layout.column_mode(a), Some(Mode::Icons));
    assert_eq!(layout.column_mode(b), Some(Mode::Full));
    assert_ne!(layout.owner("layers").unwrap().id, layout.owner("swatches").unwrap().id);
    round_trip(&layout);
}

#[test]
fn reexpanded_columns_keep_content_minimums_and_tools_divider_limits() {
    use super::super::geometry;
    let region = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1800.0, 1200.0));
    for neighbor in ["swatches", "tools"] {
        for dock in [None, Some(Edge::Left), Some(Edge::Right)] {
            let mut layout = Layout::new_workspace();
            let host = layout.owner("layers").unwrap().id;
            let incoming = layout.owner(neighbor).unwrap().id;
            apply(&mut layout, Action::Move { source: Source::Host(incoming), target: Target::Attach { host, edge: Edge::Right } });
            let target = dock.map_or(Target::Float { position: [200.0, 100.0] }, |edge| Target::Dock { edge });
            apply(&mut layout, Action::Move { source: Source::Host(host), target });
            let group = layout.group_of("layers").unwrap().id;
            let before = geometry::measure(&layout, region).into_iter().find(|h| h.id == host).unwrap();
            apply(&mut layout, Action::SetColumnMode { group, mode: Mode::Icons });
            apply(&mut layout, Action::SetColumnMode { group, mode: Mode::Full });
            let after = geometry::measure(&layout, region).into_iter().find(|h| h.id == host).unwrap();
            assert_eq!(before.dividers.len(), after.dividers.len());
            for (a, b) in before.dividers.iter().zip(&after.dividers) {
                assert_eq!(a.id, b.id);
                assert!((a.min_ratio - b.min_ratio).abs() < 0.0001, "re-expansion preserves content-based minimum");
                assert!((a.max_ratio - b.max_ratio).abs() < 0.0001, "re-expansion preserves content and Tools maximum");
                assert!((a.rect.center().x - b.rect.center().x).abs() < 0.01);
            }
            for (a, b) in before.groups.iter().zip(&after.groups) {
                close_size([a.rect.width(), a.rect.height()], [b.rect.width(), b.rect.height()]);
            }
            let divider = after.dividers.first().unwrap();
            apply(&mut layout, Action::SplitRatio { split: divider.id, ratio: divider.min_ratio });
            let measured = geometry::measure(&layout, region).into_iter().find(|h| h.id == host).unwrap();
            let layers = measured.groups.iter().find(|g| g.id == group).unwrap();
            assert!(layers.rect.width() + HOST_CHROME[0] + 0.01 >= super::super::registry::get("layers").unwrap().min_size[0]);
            if neighbor == "tools" {
                let tools = layout.group_of("tools").unwrap().id;
                let tools = measured.groups.iter().find(|g| g.id == tools).unwrap();
                assert!(tools.rect.width() + HOST_CHROME[0] <= super::super::metrics::TOOLS_MAX_WIDTH + 0.01);
            }
            round_trip(&layout);
        }
    }
}
