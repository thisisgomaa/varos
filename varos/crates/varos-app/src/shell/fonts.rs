//! Bundled, unmodified IBM Plex faces and OFL Noto shortcut-symbol fallbacks. No system-font discovery or runtime font-file I/O.
//! Provenance, exact hashes and OFL notices live in assets/fonts. Keep egui's bundled fallbacks:
//! Plex's character coverage does not include every shortcut/symbol used by the editor.
use egui::{FontData, FontDefinitions, FontFamily};

const SANS: &str = "IBM Plex Sans";
const MONO: &str = "IBM Plex Mono";
const SYMBOLS_ONE: &str = "Noto Sans Symbols";
const SYMBOLS: &str = "Noto Sans Symbols 2";
const ARABIC: &str = "IBM Plex Sans Arabic";

pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        (SYMBOLS_ONE, include_bytes!("../../assets/fonts/NotoSansSymbols-Regular.ttf").as_slice()),
        (SYMBOLS, include_bytes!("../../assets/fonts/NotoSansSymbols2-Regular.ttf").as_slice()),
        (SANS, include_bytes!("../../assets/fonts/IBMPlexSans-Regular.ttf").as_slice()),
        (MONO, include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf").as_slice()),
        (ARABIC, include_bytes!("../../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice()),
    ] {
        fonts.font_data.insert(name.into(), FontData::from_static(bytes).into());
    }
    for (family, primary) in [(FontFamily::Proportional, SANS), (FontFamily::Monospace, MONO)] {
        let fallbacks = fonts.families.entry(family).or_default();
        fallbacks.splice(0..0, [primary.into(), SYMBOLS.into(), SYMBOLS_ONE.into()]);
    }
    // Register Arabic for the diagnostic only. egui 0.35 currently breaks RTL cluster/cursor
    // correspondence; do not promote this family into editable UI until that seam is repaired.
    fonts
        .families
        .insert(FontFamily::Name(ARABIC.into()), vec![ARABIC.into(), SANS.into(), SYMBOLS.into(), SYMBOLS_ONE.into()]);
    fonts
}

/// Install once when creating a context. set_fonts takes effect on the following frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}
