// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The picker's own glyphs that are not upstream catalogue names (the Sliders tab and the eight
//! harmony rules), original Varos drawings copied from the old repo's `assets/icons/varos/`. Same
//! conventions as [`crate::varos::me_icons`]: `currentColor` becomes white, the mask a tint colours.
//! The Wheel, Harmony, Gradient, swap, none and eyedropper glyphs are catalogue overrides already
//! (`palette`, `dc-color-guide`, `dc-gradient`, `dc-swap`, `dc-fill-none`, `pipette`).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use egui::{Color32, Rect, Ui};

pub(crate) const SLIDERS: &str = "varos-picker-sliders";
pub(crate) const COMPLEMENTARY: &str = "varos-harmony-complementary";
pub(crate) const ANALOGOUS: &str = "varos-harmony-analogous";
pub(crate) const SPLIT: &str = "varos-harmony-split";
pub(crate) const TRIAD: &str = "varos-harmony-triad";
pub(crate) const TETRADIC: &str = "varos-harmony-tetradic";
pub(crate) const SQUARE: &str = "varos-harmony-square";
pub(crate) const MONO: &str = "varos-harmony-mono";
pub(crate) const NONE: &str = "varos-harmony-none";

const FILES: &[(&str, &[u8])] = &[
    (SLIDERS, include_bytes!("../../../assets/varos/icons/varos-picker-sliders.svg")),
    (COMPLEMENTARY, include_bytes!("../../../assets/varos/icons/varos-harmony-complementary.svg")),
    (ANALOGOUS, include_bytes!("../../../assets/varos/icons/varos-harmony-analogous.svg")),
    (SPLIT, include_bytes!("../../../assets/varos/icons/varos-harmony-split.svg")),
    (TRIAD, include_bytes!("../../../assets/varos/icons/varos-harmony-triad.svg")),
    (TETRADIC, include_bytes!("../../../assets/varos/icons/varos-harmony-tetradic.svg")),
    (SQUARE, include_bytes!("../../../assets/varos/icons/varos-harmony-square.svg")),
    (MONO, include_bytes!("../../../assets/varos/icons/varos-harmony-mono.svg")),
    (NONE, include_bytes!("../../../assets/varos/icons/varos-harmony-none.svg")),
];

/// The image of icon `name` (white, for tinting); `None` for a name not in the table.
pub(crate) fn source(name: &str) -> Option<egui::ImageSource<'static>> {
    static TABLE: OnceLock<HashMap<&'static str, Arc<[u8]>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        FILES
            .iter()
            .map(|(name, bytes)| {
                // Upstream's mixed-set stroke weight, as the catalogue overrides use.
                let white = String::from_utf8_lossy(bytes).replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
                (*name, Arc::from(white.into_bytes()))
            })
            .collect()
    });
    let bytes = table.get(name)?.clone();
    Some(egui::ImageSource::Bytes { uri: format!("bytes://varos/picker-icon/{name}.svg").into(), bytes: egui::load::Bytes::Shared(bytes) })
}

/// Paint icon `name` into `rect` tinted `tint`: a picker glyph, else an upstream catalogue icon.
pub(crate) fn paint(ui: &Ui, name: &str, rect: Rect, tint: Color32) {
    match source(name) {
        Some(src) => egui::Image::new(src).tint(tint).fit_to_exact_size(rect.size()).paint_at(ui, rect),
        None => crate::icons::paint(ui, name, rect, tint),
    }
}

#[cfg(test)]
mod tests {
    /// Every glyph rasterizes to a non-empty white mask.
    #[test]
    fn every_picker_glyph_rasterizes_white() {
        assert_eq!(super::FILES.len(), 9);
        let mut seen = std::collections::HashSet::new();
        for (name, _) in super::FILES {
            assert!(seen.insert(name), "duplicate {name}");
            let Some(egui::ImageSource::Bytes { bytes, .. }) = super::source(name) else { panic!("{name}") };
            assert!(!String::from_utf8_lossy(&bytes).contains("currentColor"), "{name}");
            let image = egui_extras::image::load_svg_bytes(bytes.as_ref(), &Default::default()).unwrap();
            assert_eq!(image.size, [24, 24], "{name}");
            assert!(image.pixels.iter().any(|p| p.a() > 0), "{name}: empty glyph");
            assert!(
                image.pixels.iter().filter(|p| p.a() > 0).all(|p| p.r() == p.a() && p.g() == p.a() && p.b() == p.a()),
                "{name}: must tint from white"
            );
        }
        assert!(super::source("no-such-icon").is_none());
        for name in ["palette", "dc-color-guide", "dc-gradient", "dc-swap", "dc-fill-none", "pipette", "x", "chevron-down"] {
            assert!(crate::icons::exists(name), "{name}: catalogue icon the picker paints");
        }
    }
}
