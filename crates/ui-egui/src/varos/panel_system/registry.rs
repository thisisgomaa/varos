//! Stable panel identities backed by existing native bodies.
//! Window ownership, geometry and paint share this catalogue; it does not create panel content.
//! Identity comes from the existing panel catalogue in `state.rs`; this adds only Varos policy.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelKind {
    Standard,
    Tools,
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PanelIcon {
    /// An existing original or licensed icon from the application's icon catalogue.
    Named(&'static str),
    /// The original four-cell glyph approved with the Layers/Swatches slice.
    SwatchesGrid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PanelSpec<'a> {
    pub id: &'a str,
    pub label: &'static str,
    pub icon: PanelIcon,
    /// Full host dimensions, including the shared chrome, in logical points.
    pub default_size: [f32; 2],
    pub min_size: [f32; 2],
    pub kind: PanelKind,
    /// Every layout keeps an owner for it; a saved layout without one is rejected.
    pub required: bool,
    /// The body manages its own scrolling (lists, the Properties editor); others get a scroll area.
    pub scrolls_itself: bool,
}

/// Varos sizes for the panels whose bodies need more than the standard 260 x 360 (min 240 x 180).
const SIZES: &[(&str, [f32; 2], [f32; 2])] = &[
    ("properties", [300.0, 420.0], [300.0, 180.0]),
    ("layers", [300.0, 380.0], [240.0, 180.0]),
    ("libraries", [300.0, 420.0], [240.0, 180.0]),
    ("color", [280.0, 480.0], [256.0, 180.0]),
    ("swatches", [300.0, 380.0], [240.0, 180.0]),
    ("character", [300.0, 360.0], [300.0, 180.0]),
    ("paragraph", [300.0, 360.0], [300.0, 180.0]),
];

/// The pair the workspace was approved with; every layout keeps them.
const REQUIRED: [&str; 2] = ["layers", "swatches"];

/// Bodies that scroll themselves rather than inside the host's scroll area.
const SCROLLS_ITSELF: [&str; 3] = ["layers", "properties", "libraries"];

const TOOLS: PanelSpec<'static> = PanelSpec {
    id: "tools",
    label: "Tools",
    icon: PanelIcon::Named("pen-tool"),
    default_size: [super::metrics::TOOLS_MAX_WIDTH, 760.0],
    min_size: [super::metrics::TOOLS_MIN_WIDTH, 110.0],
    kind: PanelKind::Tools,
    required: false,
    scrolls_itself: false,
};

const CONTROL: PanelSpec<'static> = PanelSpec {
    id: "control",
    label: "Control",
    icon: PanelIcon::Named("shell-sliders-horizontal"),
    default_size: [1000.0, 110.0],
    min_size: [300.0, 96.0],
    kind: PanelKind::Control,
    required: false,
    scrolls_itself: false,
};

/// All fixed identities that have real bodies: the existing panel catalogue (the dock tabs, then
/// the icon panels, with their ids, English labels and icons, `state::all_panels`), then Tools and
/// Control. Only Varos sizes, kinds, capabilities and the Swatches glyph are added here.
/// Library instances and additional Properties/Control instances are recognized by [`get`].
pub(super) static BUILTINS: std::sync::LazyLock<Vec<PanelSpec<'static>>> = std::sync::LazyLock::new(|| {
    crate::state::all_panels()
        .map(|(id, label)| {
            let (default_size, min_size) = SIZES.iter().find(|s| s.0 == id).map_or(([260.0, 360.0], [240.0, 180.0]), |s| (s.1, s.2));
            let icon = match crate::state::rail_panel(id) {
                _ if id == "swatches" => PanelIcon::SwatchesGrid,
                Some((_, _, icon)) => PanelIcon::Named(icon),
                None => PanelIcon::Named(""),
            };
            PanelSpec {
                id,
                label,
                icon,
                default_size,
                min_size,
                kind: PanelKind::Standard,
                required: REQUIRED.contains(&id),
                scrolls_itself: SCROLLS_ITSELF.contains(&id),
            }
        })
        .chain([TOOLS, CONTROL])
        .collect()
});

/// A definition keeps the exact requested identity for instance ownership.
pub(super) fn get(id: &str) -> Option<PanelSpec<'_>> {
    if let Some(spec) = BUILTINS.iter().find(|spec| spec.id == id) {
        return Some(*spec);
    }
    let instance_kind = super::super::shell::persist::instance_kind(id);
    if instance_kind != id {
        return get(instance_kind).map(|spec| PanelSpec { id, required: false, ..spec });
    }
    let (library, _) = crate::panels::library_panel::parse(id)?;
    let (base, label) = match library.kind {
        "swatches" => ("swatches", "Swatch Library"),
        "brushes" => ("brushes", "Brush Library"),
        "symbols" => ("symbols", "Symbol Library"),
        "graphicStyles" => ("graphicStyles", "Graphic Style Library"),
        _ => return None,
    };
    get(base).map(|spec| PanelSpec { id, label, required: false, ..spec })
}

pub(super) fn contains(id: &str) -> bool {
    get(id).is_some()
}

/// The fixed identities every layout must own.
pub(super) fn required() -> impl Iterator<Item = &'static str> {
    BUILTINS.iter().filter(|spec| spec.required).map(|spec| spec.id)
}

pub(super) fn label(id: &str) -> &'static str {
    get(id).map_or("", |spec| spec.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_existing_fixed_panel_has_a_real_registry_identity() {
        for (id, label) in crate::state::all_panels() {
            assert_eq!(get(id).map(|spec| (spec.id, spec.label)), Some((id, label)));
        }
        assert_eq!(get("tools").map(|spec| spec.kind), Some(PanelKind::Tools));
        assert_eq!(get("control").map(|spec| spec.kind), Some(PanelKind::Control));
        let mut ids = std::collections::BTreeSet::new();
        for spec in BUILTINS.iter() {
            assert!(ids.insert(spec.id));
            if let PanelIcon::Named(icon) = spec.icon {
                assert!(crate::icons::exists(icon), "missing original/licensed icon: {icon}");
            }
            assert!(spec.default_size.iter().zip(spec.min_size).all(|(default, min)| default.is_finite() && min > 0.0 && *default >= min));
        }
    }

    #[test]
    fn dynamic_instances_keep_their_identity_and_reject_unknown_bodies() {
        for id in [
            "properties@2",
            "control@3",
            "library:swatches:pastels",
            "library:brushes:sample",
            "library:symbols:sample",
            "library:graphicStyles:sample",
        ] {
            assert_eq!(get(id).map(|spec| spec.id), Some(id));
        }
        for id in ["properties@1", "control@02", "layers@2", "library:unknown:sample", "library:swatches:", "board", "unknown"] {
            assert!(!contains(id), "unsupported body: {id}");
        }
    }
}
