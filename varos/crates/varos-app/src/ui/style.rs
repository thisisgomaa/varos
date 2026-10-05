pub(crate) fn install_fonts(ctx: &egui::Context) {
    varos_app::shell::fonts::install(ctx);
}

/// ⌘+ / ⌘− / ⌘0 belong to the CANVAS (zoom the artwork, Fit), never to the chrome. egui's built-in
/// browser-style shortcut (`Options::zoom_with_keyboard`, on by default) scaled the whole UI instead
/// (Astra F05) — switch it off, so the UI scale only follows the display.
pub(crate) fn disable_ui_keyboard_zoom(ctx: &egui::Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

pub(crate) fn install_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    // Stage 4: the shell law is the base (warm visuals + INSTANT + thin overlay scrollbars +
    // tight seam grab) — the app only adds its text ramp on top.
    varos_app::shell::tokens::apply(ctx);
    let mut s = (*ctx.style_of(egui::Theme::Dark)).clone();
    // labels are UI chrome, not documents — double-clicking the artboard name / size chip must never
    // paint a text-selection highlight over it (Ahmed 2026-07-11 "حاجة رخمة"). TextEdits keep selection.
    s.interaction.selectable_labels = false;
    s.text_styles = varos_app::shell::tokens::text_styles();
    ctx.set_style_of(egui::Theme::Dark, s.clone());
    ctx.set_style_of(egui::Theme::Light, s);
}

// ───────────────────────────── hand-rolled dropdown menus ─────────────────────────────
// egui 0.35 removed the old memory-popup API (toggle_popup/is_popup_open/popup_below_widget). We
// hand-roll the replacement on Area — same look as the 0.27 popups (our Visuals drove those) and
// our close rules: open = a temp bool at `id`; closes on Escape or a primary press outside menu+anchor.
