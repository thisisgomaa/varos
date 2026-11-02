//! Stable panel identities backed by existing native bodies.
//! Window ownership, geometry and paint share this catalogue; it does not create panel content.

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
}

const fn standard(id: &'static str, label: &'static str, icon: &'static str) -> PanelSpec<'static> {
    PanelSpec { id, label, icon: PanelIcon::Named(icon), default_size: [260.0, 360.0], min_size: [240.0, 180.0], kind: PanelKind::Standard }
}

const fn sized(spec: PanelSpec<'static>, default_size: [f32; 2], min_size: [f32; 2]) -> PanelSpec<'static> {
    PanelSpec { default_size, min_size, ..spec }
}

/// All fixed identities that have real bodies: DockTab, ICON_PANELS, tools and control.
/// Library instances and additional Properties/Control instances are recognized by [`get`].
pub(super) const BUILTINS: &[PanelSpec<'static>] = &[
    sized(standard("properties", "Properties", "dc-options"), [300.0, 420.0], [300.0, 180.0]),
    sized(standard("layers", "Layers", "layers"), [300.0, 380.0], [240.0, 180.0]),
    sized(standard("libraries", "Libraries", "library"), [300.0, 420.0], [240.0, 180.0]),
    standard("pages", "Pages", "file"),
    sized(standard("color", "Color", "palette"), [280.0, 480.0], [256.0, 180.0]),
    standard("colorGuide", "Color Guide", "dc-color-guide"),
    PanelSpec { icon: PanelIcon::SwatchesGrid, ..sized(standard("swatches", "Swatches", "swatch-book"), [300.0, 380.0], [240.0, 180.0]) },
    standard("brushes", "Brushes", "paintbrush"),
    standard("symbols", "Symbols", "spray-can"),
    standard("patternOptions", "Pattern Options", "grid-3x3"),
    standard("stroke", "Stroke", "dc-stroke"),
    standard("gradient", "Gradient", "dc-gradient"),
    standard("transparency", "Transparency", "dc-transparency"),
    standard("appearance", "Appearance", "dc-appearance"),
    standard("graphicStyles", "Graphic Styles", "dc-graphic-styles"),
    standard("artboards", "Artboards", "dc-artboards"),
    standard("transform", "Transform", "dc-transform-panel"),
    standard("align", "Align", "dc-align"),
    standard("pathfinder", "Pathfinder", "dc-pathfinder"),
    standard("blend", "Blend", "dc-blend"),
    sized(standard("character", "Character", "type"), [300.0, 360.0], [300.0, 180.0]),
    sized(standard("paragraph", "Paragraph", "pilcrow"), [300.0, 360.0], [300.0, 180.0]),
    standard("glyphs", "Glyphs", "text-cursor-input"),
    standard("charStyles", "Character Styles", "type"),
    standard("openType", "OpenType", "dc-touch-type"),
    standard("paraStyles", "Paragraph Styles", "pilcrow"),
    standard("tabs", "Tabs", "align-horizontal-justify-start"),
    standard("history", "History", "history"),
    standard("actions", "Actions", "dc-actions"),
    standard("info", "Info", "info"),
    standard("docInfo", "Document Info", "dc-list-view"),
    standard("navigator", "Navigator", "map"),
    standard("separations", "Separations Preview", "printer"),
    standard("imageTrace", "Image Trace", "image"),
    standard("magicWand", "Magic Wand", "wand-sparkles"),
    standard("flattenerPreview", "Flattener Preview", "eye"),
    standard("attributes", "Attributes", "settings"),
    standard("svgInteractivity", "SVG Interactivity", "spline-pointer"),
    standard("colorThemes", "Color Themes", "sun"),
    standard("links", "Links", "link"),
    standard("assetExport", "Asset Export", "share-2"),
    standard("cssProperties", "CSS Properties", "globe"),
    standard("variables", "Variables", "chart-column"),
    PanelSpec {
        id: "tools",
        label: "Tools",
        icon: PanelIcon::Named("pen-tool"),
        default_size: [super::metrics::TOOLS_MAX_WIDTH, 760.0],
        min_size: [super::metrics::TOOLS_MIN_WIDTH, 110.0],
        kind: PanelKind::Tools,
    },
    PanelSpec {
        id: "control",
        label: "Control",
        icon: PanelIcon::Named("shell-sliders-horizontal"),
        default_size: [1000.0, 110.0],
        min_size: [300.0, 96.0],
        kind: PanelKind::Control,
    },
];

/// A definition keeps the exact requested identity for instance ownership.
pub(super) fn get(id: &str) -> Option<PanelSpec<'_>> {
    if let Some(spec) = BUILTINS.iter().find(|spec| spec.id == id) {
        return Some(*spec);
    }
    let instance_kind = super::super::shell::persist::instance_kind(id);
    if instance_kind != id {
        return get(instance_kind).map(|spec| PanelSpec { id, ..spec });
    }
    let (library, _) = crate::panels::library_panel::parse(id)?;
    let (base, label) = match library.kind {
        "swatches" => ("swatches", "Swatch Library"),
        "brushes" => ("brushes", "Brush Library"),
        "symbols" => ("symbols", "Symbol Library"),
        "graphicStyles" => ("graphicStyles", "Graphic Style Library"),
        _ => return None,
    };
    get(base).map(|spec| PanelSpec { id, label, ..spec })
}

pub(super) fn contains(id: &str) -> bool {
    get(id).is_some()
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
        for spec in BUILTINS {
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
