//! Varos: the Middle Eastern controls' icons (Paragraph › Direction and Kashida, Character ›
//! Harakat), original glyphs from the owner's panel mockup (`docs/varos/mockups/panels.html`).
//!
//! They are not upstream catalogue names, so they live in their own table rather than among the
//! catalogue overrides ([`super::icons`]); the conventions are the same: `currentColor` becomes
//! white, the neutral mask a tint colours.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use egui::{Color32, Rect, Ui};

/// Paragraph direction: Auto (from the first letter).
pub(crate) const DIR_AUTO: &str = "varos-me-dir-auto";
/// Paragraph direction: right to left (pilcrow, arrow to the left).
pub(crate) const DIR_RTL: &str = "varos-me-dir-rtl";
/// Paragraph direction: left to right (pilcrow, arrow to the right).
pub(crate) const DIR_LTR: &str = "varos-me-dir-ltr";
/// Kashida Off: a short tatweel.
pub(crate) const KASHIDA_OFF: &str = "varos-me-kashida-off";
/// Kashida Auto: a longer tatweel.
pub(crate) const KASHIDA_AUTO: &str = "varos-me-kashida-auto";
/// Kashida Force: the longest, heavier tatweel.
pub(crate) const KASHIDA_FORCE: &str = "varos-me-kashida-force";
/// Kashida strength: a stretched line between two arrows.
pub(crate) const KASHIDA_STRENGTH: &str = "varos-me-kashida-strength";
/// Harakat: a dotted letter with a fatha.
pub(crate) const HARAKAT: &str = "varos-me-harakat";
/// Hide the harakat: an eye, struck through.
pub(crate) const HARAKAT_HIDE: &str = "varos-me-harakat-hide";
/// Harakat as their letters: a letter and its mark on one line.
pub(crate) const HARAKAT_AS_LETTERS: &str = "varos-me-harakat-as-letters";

const FILES: &[(&str, &[u8])] = &[
    (DIR_AUTO, include_bytes!("../../assets/varos/icons/varos-me-dir-auto.svg")),
    (DIR_RTL, include_bytes!("../../assets/varos/icons/varos-me-dir-rtl.svg")),
    (DIR_LTR, include_bytes!("../../assets/varos/icons/varos-me-dir-ltr.svg")),
    (KASHIDA_OFF, include_bytes!("../../assets/varos/icons/varos-me-kashida-off.svg")),
    (KASHIDA_AUTO, include_bytes!("../../assets/varos/icons/varos-me-kashida-auto.svg")),
    (KASHIDA_FORCE, include_bytes!("../../assets/varos/icons/varos-me-kashida-force.svg")),
    (KASHIDA_STRENGTH, include_bytes!("../../assets/varos/icons/varos-me-kashida-strength.svg")),
    (HARAKAT, include_bytes!("../../assets/varos/icons/varos-me-harakat.svg")),
    (HARAKAT_HIDE, include_bytes!("../../assets/varos/icons/varos-me-harakat-hide.svg")),
    (HARAKAT_AS_LETTERS, include_bytes!("../../assets/varos/icons/varos-me-harakat-as-letters.svg")),
];

/// The image of icon `name` (white, for tinting); `None` for a name not in the table.
pub(crate) fn source(name: &str) -> Option<egui::ImageSource<'static>> {
    static TABLE: OnceLock<HashMap<&'static str, Arc<[u8]>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        FILES.iter().map(|(name, bytes)| (*name, Arc::from(String::from_utf8_lossy(bytes).replace("currentColor", "white").into_bytes()))).collect()
    });
    let bytes = table.get(name)?.clone();
    Some(egui::ImageSource::Bytes { uri: format!("bytes://varos/me-icon/{name}.svg").into(), bytes: egui::load::Bytes::Shared(bytes) })
}

/// Paint icon `name` into `rect` tinted `tint` (nothing for an unknown name).
pub(crate) fn paint(ui: &Ui, name: &str, rect: Rect, tint: Color32) {
    if let Some(src) = source(name) {
        egui::Image::new(src).tint(tint).fit_to_exact_size(rect.size()).paint_at(ui, rect);
    }
}

#[cfg(test)]
mod tests {
    /// Every icon rasterizes to a non-empty white mask at its drawn size.
    #[test]
    fn every_icon_rasterizes_white() {
        assert_eq!(super::FILES.len(), 10);
        let mut seen = std::collections::HashSet::new();
        for (name, _) in super::FILES {
            assert!(seen.insert(name), "duplicate {name}");
            let Some(egui::ImageSource::Bytes { bytes, .. }) = super::source(name) else { panic!("{name}") };
            assert!(!String::from_utf8_lossy(&bytes).contains("currentColor"), "{name}");
            let image = egui_extras::image::load_svg_bytes(bytes.as_ref(), &Default::default()).unwrap();
            let want = if name.contains("kashida-") && !name.ends_with("strength") { [24, 16] } else { [16, 16] };
            assert_eq!(image.size, want, "{name}");
            assert!(image.pixels.iter().any(|p| p.a() > 0), "{name}: empty glyph");
            assert!(
                image.pixels.iter().filter(|p| p.a() > 0).all(|p| p.r() == p.a() && p.g() == p.a() && p.b() == p.a()),
                "{name}: must tint from white"
            );
        }
        assert!(super::source("no-such-icon").is_none());
    }
}
