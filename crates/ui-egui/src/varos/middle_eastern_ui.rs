//! Varos: the Middle Eastern rows of the Paragraph and Character panels (provisional UI, awaiting
//! the owner's review), shown with Preferences › Type › Show Indic Options, which the fork turns
//! on once ([`default_on`]).
//!
//! * Paragraph panel, under the alignment buttons: **Direction: Auto · RTL · LTR**. Auto names
//!   what the paragraph resolved to ("Auto (RTL)"). It replaces upstream's two direction buttons
//!   (one home for the setting). `text.setFormat {direction}`.
//! * Character panel: **Digits** (Default, Arabic 0–9, Hindi ٠–٩, Farsi ۰–۹; the text keeps the
//!   digits typed, `text.setFormat {digits}`; the panel menu still mirrors it) and **Harakat**:
//!   four colours, Hide (no fill) and As Letters (their letters' fill again), through
//!   `text.styleHarakat`, which changes only paint: letters keep their joining, marks their place.
//!
//! Design: Varos tokens. Azure marks only the active segment; 3 px corners at the segmented
//! control's ends and on the swatches; 1 px borders; no shadows. Labels are English sources
//! translated through the upstream catalogs (`i18n/*.tsv`, "Varos: Middle Eastern rows").

use egui::{Color32, CornerRadius, Rect, Sense, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{CharStyle, ParaDirection, ParaStyle};

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// Turn the Middle Eastern (Indic) options on once per saved UI profile, the fork's default for an
/// Arabic-first tool: a fresh profile and an upgraded one alike. Afterwards the user's own choice
/// in Preferences stands (remembered in [`crate::state::UiState::varos_middle_eastern`]).
pub(crate) fn default_on(app: &mut VectorcraftApp, prefs: &mut vectorcraft_engine::Prefs) {
    if !app.ui.varos_middle_eastern {
        prefs.show_indic_options = true;
        app.ui.varos_middle_eastern = true;
    }
}

/// `p` aimed at what the panels act on: the Type tool's selection (ending its typing session),
/// else the selected type.
fn targeted(app: &mut VectorcraftApp, mut p: Value) -> Value {
    if let Some((id, a, b)) = crate::panels::character::text_editing(app) {
        crate::panels::character::end_typing(app);
        p["ids"] = json!([id.0]);
        p["start"] = json!(a);
        p["end"] = json!(b);
    }
    p
}

fn run(app: &mut VectorcraftApp, cmd: &str, p: Value) {
    let p = targeted(app, p);
    if let Err(e) = app.run(cmd, p) {
        app.ui.status = e;
    }
}

/// One segment of a segmented control, `w` wide, at `index` of `count`; `true` when clicked.
fn segment(ui: &mut Ui, label: &str, tip: &str, on: bool, index: usize, count: usize, w: f32) -> (bool, Rect) {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
    let r = 3;
    let radius = match index {
        0 => CornerRadius { nw: r, sw: r, ne: 0, se: 0 },
        i if i + 1 == count => CornerRadius { nw: 0, sw: 0, ne: r, se: r },
        _ => CornerRadius::ZERO,
    };
    let fill = if on {
        t.accent
    } else if resp.hovered() {
        t.hover
    } else {
        t.button
    };
    let p = ui.painter();
    p.rect(rect, radius, fill, egui::Stroke::new(1.0, if on { t.accent } else { t.button_border }), egui::StrokeKind::Inside);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.5), if on { Color32::WHITE } else { t.text });
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, label));
    (resp.on_hover_text(tip).clicked(), rect)
}

/// A small text button sized to its label (3 px corners, a 1 px border, no shadow).
fn text_button(ui: &mut Ui, text: &str, tip: &str) -> (Rect, bool) {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(11.5), t.text);
    let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 12.0, 20.0), Sense::click());
    let fill = if resp.hovered() { t.hover } else { t.button };
    ui.painter().rect(rect, 3, fill, egui::Stroke::new(1.0, t.button_border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() * 0.5, galley, t.text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    (rect, resp.on_hover_text(tip).clicked())
}

/// A row label, 58 points wide, as the kashida rows have.
fn label(ui: &mut Ui, text: &str, tip: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_sized(vec2(58.0, 22.0), egui::Label::new(egui::RichText::new(text).size(11.5).color(t.text_dim))).on_hover_text(tip);
}

/// Where the rows were drawn (read by the tests; the panels ignore it).
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Rows {
    /// Direction: Auto, RTL, LTR.
    pub direction: Vec<Rect>,
    /// Harakat: the four colours, Hide, As Letters.
    pub harakat: Vec<Rect>,
}

/// The Paragraph panel's Middle Eastern row for `para` (the first paragraph acted on, running
/// right to left when `rtl`): Direction.
pub(crate) fn paragraph_rows(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle, rtl: bool) -> Rows {
    let mut rows = Rows::default();
    if !app.session.prefs.show_indic_options {
        return rows;
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        label(ui, tl!("Direction"), tl!("Paragraph direction"));
        let resolved = if rtl { tl!("RTL") } else { tl!("LTR") };
        let auto = format!("{} ({resolved})", tl!("Auto"));
        let w = ((ui.available_width() - 4.0) / 3.0).clamp(40.0, 84.0);
        let choices = [
            (None, if para.direction.is_none() { auto.as_str() } else { tl!("Auto") }, tl!("Auto: from the paragraph's first letter"), "auto"),
            (Some(ParaDirection::RightToLeft), tl!("RTL"), tl!("Right-to-Left Paragraph Direction"), "rightToLeft"),
            (Some(ParaDirection::LeftToRight), tl!("LTR"), tl!("Left-to-Right Paragraph Direction"), "leftToRight"),
        ];
        for (i, (dir, text, tip, key)) in choices.into_iter().enumerate() {
            let (clicked, rect) = segment(ui, text, tip, para.direction == dir, i, 3, w);
            rows.direction.push(rect);
            if clicked {
                run(app, "text.setFormat", json!({ "direction": key }));
            }
        }
    });
    rows
}

/// The harakat colours offered: traditional red, green and gold, and black.
const HARAKAT_COLOURS: [(&str, [u8; 3]); 4] =
    [("Red", [0xC0, 0x39, 0x2B]), ("Green", [0x1E, 0x84, 0x49]), ("Gold", [0xB7, 0x95, 0x0B]), ("Black", [0x14, 0x13, 0x13])];

/// The Character panel's Middle Eastern rows for `s` (the first character acted on): Digits and
/// Harakat.
pub(crate) fn character_rows(app: &mut VectorcraftApp, ui: &mut Ui, s: &CharStyle) -> Rows {
    let mut rows = Rows::default();
    if !app.session.prefs.show_indic_options {
        return rows;
    }
    let t = Tokens::get(ui.ctx());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        label(ui, tl!("Digits"), tl!("Digits"));
        use vectorcraft_doc::Digits;
        let options = [
            (tl!("Default"), Digits::Default, "default"),
            (tl!("Arabic"), Digits::Arabic, "arabic"),
            (tl!("Hindi"), Digits::Hindi, "hindi"),
            (tl!("Farsi"), Digits::Farsi, "farsi"),
        ];
        let names = options.map(|o| o.0);
        let current = options.iter().find(|o| o.1 == s.digits).map_or(names[0], |o| o.0);
        // Already translated: shown as they are.
        if let Some(i) = crate::widgets::dropdown_names(ui, "varos-digits", current, &names, (ui.available_width() - 4.0).max(80.0))
            && let Some((_, _, key)) = options.get(i)
        {
            run(app, "text.setFormat", json!({ "digits": key }));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        label(ui, tl!("Harakat"), tl!("Colour the harakat (Arabic vowel marks) apart from their letters"));
        for (name, [r, g, b]) in HARAKAT_COLOURS {
            let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            rows.harakat.push(rect);
            let border = if resp.hovered() { t.text } else { t.button_border };
            ui.painter().rect(rect, 3, Color32::from_rgb(r, g, b), egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
            let tip = match name {
                "Red" => tl!("Red"),
                "Green" => tl!("Green"),
                "Gold" => tl!("Gold"),
                _ => tl!("Black"),
            };
            if resp.on_hover_text(tip).clicked() {
                run(app, "text.styleHarakat", json!({ "fill": format!("#{r:02X}{g:02X}{b:02X}") }));
            }
        }
        for (text, tip, fill) in [
            (tl!("Hide"), tl!("Hide the harakat: they stay in the text"), "none"),
            (tl!("As Letters"), tl!("Give the harakat their letters' colour again"), "inherit"),
        ] {
            let (rect, clicked) = text_button(ui, text, tip);
            rows.harakat.push(rect);
            if clicked {
                run(app, "text.styleHarakat", json!({ "fill": fill }));
            }
        }
    });
    rows
}

#[cfg(test)]
#[path = "tests_middle_eastern_ui.rs"]
mod tests;
