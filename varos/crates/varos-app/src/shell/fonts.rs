//! Bundled static Inter/JetBrains Mono faces and OFL Noto shortcut-symbol fallbacks.
//! No system-font discovery or runtime font-file I/O; install these definitions once at startup.
use egui::{FontData, FontDefinitions, FontFamily};

pub const UI_400: &str = "ui-400";
pub const UI_500: &str = "ui-500";
pub const UI_600: &str = "ui-600";
pub const MONO_400: &str = "mono-400";
const INTER_REGULAR: &str = "Inter Regular";
const INTER_MEDIUM: &str = "Inter Medium";
const INTER_SEMIBOLD: &str = "Inter SemiBold";
const JETBRAINS_MONO: &str = "JetBrains Mono Regular";
const SYMBOLS_ONE: &str = "Noto Sans Symbols";
const SYMBOLS: &str = "Noto Sans Symbols 2";
const ARABIC: &str = "IBM Plex Sans Arabic";

pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let builtin_proportional = fonts.families[&FontFamily::Proportional].clone();
    for (name, bytes) in [
        (SYMBOLS_ONE, include_bytes!("../../assets/fonts/NotoSansSymbols-Regular.ttf").as_slice()),
        (SYMBOLS, include_bytes!("../../assets/fonts/NotoSansSymbols2-Regular.ttf").as_slice()),
        (INTER_REGULAR, include_bytes!("../../assets/fonts/Inter-Regular.ttf").as_slice()),
        (INTER_MEDIUM, include_bytes!("../../assets/fonts/Inter-Medium.ttf").as_slice()),
        (INTER_SEMIBOLD, include_bytes!("../../assets/fonts/Inter-SemiBold.ttf").as_slice()),
        (JETBRAINS_MONO, include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf").as_slice()),
        (ARABIC, include_bytes!("../../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice()),
    ] {
        fonts.font_data.insert(name.into(), FontData::from_static(bytes).into());
    }
    for (family, primary) in [(FontFamily::Proportional, INTER_REGULAR), (FontFamily::Monospace, JETBRAINS_MONO)] {
        let fallbacks = fonts.families.entry(family).or_default();
        fallbacks.splice(0..0, [primary.into(), SYMBOLS.into(), SYMBOLS_ONE.into()]);
    }
    for (family, primary) in [
        (FontFamily::Name(UI_400.into()), INTER_REGULAR),
        (FontFamily::Name(UI_500.into()), INTER_MEDIUM),
        (FontFamily::Name(UI_600.into()), INTER_SEMIBOLD),
        (FontFamily::Name(MONO_400.into()), JETBRAINS_MONO),
    ] {
        let mut fallbacks = vec![primary.into(), SYMBOLS.into(), SYMBOLS_ONE.into()];
        fallbacks.extend(builtin_proportional.iter().cloned());
        fonts.families.insert(family, fallbacks);
    }
    // Register Arabic for the diagnostic only. egui 0.35 currently breaks RTL cluster/cursor
    // correspondence; do not promote this family into editable UI until that seam is repaired.
    fonts.families.insert(
        FontFamily::Name(ARABIC.into()),
        vec![ARABIC.into(), INTER_REGULAR.into(), SYMBOLS.into(), SYMBOLS_ONE.into()],
    );
    fonts
}

/// Install once when creating a context. set_fonts takes effect on the following frame.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}
