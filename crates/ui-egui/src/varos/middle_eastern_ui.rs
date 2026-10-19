//! Varos: the Middle Eastern groups of the Paragraph and Character panels, after the owner's panel
//! mockup (`docs/varos/mockups/panels.html`, Paragraph variants A and C, Character Digits and
//! Harakat). Their Direction, Digits and Harakat rows show with Preferences › Type › Show Indic
//! Options, which the fork turns on once ([`default_on`]); the kashida rows ([`super::kashida_ui`])
//! show for right-to-left paragraphs.
//!
//! * Paragraph, right under the alignment buttons: a folding **Middle Eastern** head, then
//!   **Direction**, three icon buttons (Auto, right to left, left to right; `text.setFormat
//!   {direction}`). Auto lit draws a dashed ring on the side it resolved to. It replaces upstream's
//!   two direction buttons (one home for the setting). Kashida and Strength follow.
//! * Character: the same head, then **Digits**, a dropdown whose button shows the digits
//!   themselves (`text.setFormat {digits}`; the text keeps the digits typed), and two icon
//!   toggles, **Hide** the harakat and harakat **As letters** (their letters' fill). Below them the
//!   **Harakat** row: a colour well (any colour, in a popover of upstream's colour field, hue
//!   slider and hex field) and four preset swatches. All through `text.styleHarakat`, which changes
//!   only paint: letters keep their joining, marks their place.
//!
//! The groups' folded state is kept with the UI state (`ui.json`, [`Folded`]). Escape while the
//! harakat colour popover is open only closes it, and drops a colour being dragged
//! ([`escape_popover`], called before the shortcuts).
//!
//! State: icon choices and toggles that match the selection light up (a dark well, the icon
//! bright, no azure); the preset matching the harakat colour carries the azure ring. A mixed
//! selection lights nothing: the values it has are tinted, numbers read "–". Every control has a
//! tool tip, and screen readers get its name. Labels are English sources translated through the
//! upstream catalogs (`i18n/*.tsv`, "Varos: Middle Eastern rows").

use egui::{Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{CharStyle, Digits, NodeKind, ParaDirection, ParaStyle, TextObject};

use super::me_icons;
use crate::VectorcraftApp;
use crate::panels::color::{hex_digits, parse_hex};
use crate::theme::Tokens;
use crate::widgets::{self, Live};

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
pub(crate) fn targeted(app: &mut VectorcraftApp, mut p: Value) -> Value {
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

/// Each text object the panels act on, with the byte range they act on in it (`None`: all of it):
/// the Type tool's selection while it edits text, else every selected text object (in groups too).
pub(crate) fn for_each_text(app: &VectorcraftApp, mut f: impl FnMut(&TextObject, Option<(usize, usize)>)) {
    let Some(st) = app.session.active() else { return };
    if let Some((id, a, b)) = crate::panels::character::text_editing(app) {
        if let Some(NodeKind::Text(t)) = st.doc.node(id).map(|n| &n.kind) {
            f(t, Some((a, b)));
        }
        return;
    }
    for node in st.selection.objects.iter().filter_map(|id| st.doc.node(*id)) {
        node.walk(&mut |n| {
            if let NodeKind::Text(t) = &n.kind {
                f(t, None);
            }
        });
    }
}

// ---------- shared widgets ----------

/// One control as drawn (read by the tests; the panels ignore it).
#[derive(Clone, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Drawn {
    pub key: &'static str,
    pub rect: Rect,
    /// Its tool tip (already translated).
    pub tip: String,
    /// It shows the selection's value: lit (a dark well), or the azure ring of a preset.
    pub lit: bool,
    /// Mixed selection: a value some of it has (tinted, not lit).
    pub tinted: bool,
    /// Auto's resolved side: a dashed ring.
    pub ghost: bool,
}

impl Default for Drawn {
    fn default() -> Self {
        Self { key: "", rect: Rect::NOTHING, tip: String::new(), lit: false, tinted: false, ghost: false }
    }
}

/// The controls a group drew (read by the tests; the panels ignore it).
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Rows {
    pub controls: Vec<Drawn>,
}

/// Height of a row; of its controls; corner radius of a control.
const ROW: f32 = 24.0;
const FIELD: f32 = 20.0;
const RADIUS: u8 = 3;

/// The Middle Eastern groups folded, kept with the UI state (`ui.json`) as the other panel choices
/// are ([`crate::state::UiState::varos_me_folded`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Folded {
    /// The Paragraph panel's group.
    pub paragraph: bool,
    /// The Character panel's group.
    pub character: bool,
}

/// The folding "Middle Eastern" head of a panel's group: 10 pt caps, a hairline and a chevron;
/// a click flips `folded`. → open.
pub(crate) fn group_head(ui: &mut Ui, folded: &mut bool, out: &mut Vec<Drawn>) -> bool {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::click());
    let name = tl!("Middle Eastern");
    let tip = tl!("Show or hide the Middle Eastern options");
    if resp.clicked() {
        *folded = !*folded;
    }
    let folded = *folded;
    let colour = if resp.hovered() { t.text } else { t.text_dim };
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &name.to_uppercase(),
        0.0,
        egui::TextFormat { font_id: FontId::proportional(10.0), color: colour, extra_letter_spacing: 0.6, ..Default::default() },
    );
    let galley = ui.painter().layout_job(job);
    let p = ui.painter();
    let text_at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    let text_end = rect.left() + galley.size().x;
    p.galley(text_at, galley, colour);
    // The chevron: down while open, right while folded.
    let c = pos2(rect.right() - 5.0, rect.center().y);
    let pts = if folded {
        [c + vec2(-1.5, -3.0), c + vec2(1.5, 0.0), c + vec2(-1.5, 3.0)]
    } else {
        [c + vec2(-3.0, -1.5), c + vec2(0.0, 1.5), c + vec2(3.0, -1.5)]
    };
    p.line_segment([pts[0], pts[1]], Stroke::new(1.0, colour));
    p.line_segment([pts[1], pts[2]], Stroke::new(1.0, colour));
    if text_end + 6.0 < c.x - 10.0 {
        p.hline(text_end + 6.0..=c.x - 10.0, rect.center().y, Stroke::new(1.0, t.border));
    }
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, !folded, name));
    resp.on_hover_text(tip);
    out.push(Drawn { key: "group", rect, tip: tip.into(), lit: !folded, ..Default::default() });
    ui.add_space(2.0);
    !folded
}

/// One choice of a [`segments`] group: an icon `icon_size` large, and `label` after it when not
/// empty.
pub(crate) struct Choice<'a> {
    pub key: &'static str,
    pub icon: &'static str,
    pub icon_size: Vec2,
    pub label: &'a str,
    pub tip: String,
    pub lit: bool,
    pub tinted: bool,
    pub ghost: bool,
}

/// A segmented group `width` wide: a surface with a hairline, the choices sharing it, 1 pt apart.
/// The lit one is a dark well with a bright icon (no azure); a mixed selection's values are tinted;
/// Auto's resolved side carries a dashed ring. → the index clicked.
pub(crate) fn segments(ui: &mut Ui, choices: &[Choice<'_>], width: f32, out: &mut Vec<Drawn>) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
    ui.painter().rect(rect, RADIUS, t.button, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let inner = rect.shrink(1.0);
    let n = choices.len() as f32;
    let w = (inner.width() - (n - 1.0)) / n;
    let mut clicked = None;
    for (i, c) in choices.iter().enumerate() {
        let r = Rect::from_min_size(pos2(inner.left() + i as f32 * (w + 1.0), inner.top()), vec2(w, inner.height()));
        let resp = ui.interact(r, ui.id().with(("varos-me-segment", c.key)), Sense::click());
        let enabled = ui.is_enabled();
        let fill = if c.lit {
            t.input
        } else if c.tinted || (enabled && resp.hovered()) {
            t.hover
        } else {
            Color32::TRANSPARENT
        };
        let p = ui.painter();
        p.rect_filled(r, 2, fill);
        if c.ghost {
            dashed_rect(p, r.shrink(3.0), Stroke::new(1.0, t.text_disabled));
        }
        let ink = if !enabled {
            t.text_disabled
        } else if c.lit || c.tinted || resp.hovered() {
            t.text
        } else {
            t.text_dim
        };
        let galley = (!c.label.is_empty()).then(|| p.layout_no_wrap(c.label.to_string(), FontId::proportional(11.0), ink));
        let gap = if galley.is_some() { 5.0 } else { 0.0 };
        let total = c.icon_size.x + gap + galley.as_ref().map_or(0.0, |g| g.size().x);
        let left = r.center().x - total / 2.0;
        me_icons::paint(ui, c.icon, Rect::from_min_size(pos2(left, r.center().y - c.icon_size.y / 2.0), c.icon_size), ink);
        if let Some(g) = galley {
            let at = pos2(left + c.icon_size.x + gap, r.center().y - g.size().y / 2.0);
            ui.painter().galley(at, g, ink);
        }
        let name = if c.label.is_empty() { c.tip.clone() } else { c.label.to_string() };
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, c.lit, &name));
        if resp.on_hover_text(&c.tip).on_disabled_hover_text(&c.tip).clicked() {
            clicked = Some(i);
        }
        out.push(Drawn { key: c.key, rect: r, tip: c.tip.clone(), lit: c.lit, tinted: c.tinted, ghost: c.ghost });
    }
    clicked
}

/// A dashed outline of `r`.
fn dashed_rect(p: &egui::Painter, r: Rect, stroke: Stroke) {
    let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    for pair in corners.windows(2) {
        p.extend(egui::Shape::dashed_line(&[pair[0], pair[1]], stroke, 1.5, 1.5));
    }
}

/// A light-up toggle 22 tall: lit, a dark well with the icon bright (no azure). Icon-only (24
/// wide) when `label` is empty, else the icon and `label` after it, at most `max_width` wide (a
/// longer label ends in "…"). → clicked.
#[allow(clippy::too_many_arguments)]
pub(crate) fn toggle(
    ui: &mut Ui,
    key: &'static str,
    icon: &'static str,
    label: &str,
    tip: &str,
    lit: bool,
    max_width: f32,
    out: &mut Vec<Drawn>,
) -> bool {
    let t = Tokens::get(ui.ctx());
    const PAD: f32 = 5.0;
    const GAP: f32 = 4.0;
    let galley = (!label.is_empty()).then(|| {
        let room = (max_width - 2.0 * PAD - 16.0 - GAP).max(12.0);
        // Uncoloured (placeholder) text: it takes the toggle's ink when painted.
        let mut job =
            egui::text::LayoutJob::single_section(label.to_string(), egui::TextFormat::simple(FontId::proportional(11.0), Color32::PLACEHOLDER));
        job.wrap = egui::text::TextWrapping { max_width: room, max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
        ui.painter().layout_job(job)
    });
    let width = galley.as_ref().map_or(24.0, |g| 2.0 * PAD + 16.0 + GAP + g.size().x);
    let (r, resp) = ui.allocate_exact_size(vec2(width, 22.0), Sense::click());
    let enabled = ui.is_enabled();
    let p = ui.painter();
    if lit {
        p.rect(r, RADIUS, t.input, Stroke::new(1.0, t.border), StrokeKind::Inside);
    } else if enabled && resp.hovered() {
        p.rect_filled(r, RADIUS, t.hover);
    }
    let ink = if !enabled {
        t.text_disabled
    } else if lit || resp.hovered() {
        t.text
    } else {
        t.text_dim
    };
    match galley {
        None => me_icons::paint(ui, icon, Rect::from_center_size(r.center(), Vec2::splat(16.0)), ink),
        Some(g) => {
            me_icons::paint(ui, icon, Rect::from_min_size(pos2(r.left() + PAD, r.center().y - 8.0), Vec2::splat(16.0)), ink);
            ui.painter().galley(pos2(r.left() + PAD + 16.0 + GAP, r.center().y - g.size().y / 2.0), g, ink);
        }
    }
    let name = if label.is_empty() { tip } else { label };
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, lit, name));
    out.push(Drawn { key, rect: r, tip: tip.into(), lit, ..Default::default() });
    resp.on_hover_text(tip).on_disabled_hover_text(tip).clicked()
}

/// A row's leading icon (not a button), with its tool tip.
pub(crate) fn row_icon(ui: &mut Ui, key: &'static str, icon: &'static str, tip: &str, out: &mut Vec<Drawn>) {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(20.0, 22.0), Sense::hover());
    let ink = if ui.is_enabled() { t.text_dim } else { t.text_disabled };
    me_icons::paint(ui, icon, Rect::from_center_size(r.center(), Vec2::splat(16.0)), ink);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), tip));
    resp.on_hover_text(tip).on_disabled_hover_text(tip);
    out.push(Drawn { key, rect: r, tip: tip.into(), ..Default::default() });
}

/// A slider rail `width` wide over 0..=`max` at `value` (`None`, a mixed selection: no thumb): a
/// hairline, filled muted up to a light thumb. A value past `max` sits at the end. → its rect,
/// and a value clicked or dragged to with its phase.
pub(crate) fn rail(ui: &mut Ui, id: &str, name: &str, tip: &str, value: Option<f64>, max: f64, width: f32) -> (Rect, Option<(f64, Live)>) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW), Sense::hover());
    let resp = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
    let enabled = ui.is_enabled();
    let bar = rect.shrink2(vec2(5.0, 0.0));
    let y = rect.center().y;
    let p = ui.painter();
    p.hline(bar.x_range(), y, Stroke::new(2.0, t.border));
    if let Some(v) = value {
        let x = bar.left() + (v / max).clamp(0.0, 1.0) as f32 * bar.width();
        p.hline(bar.left()..=x, y, Stroke::new(2.0, if enabled { t.text_dim } else { t.border }));
        let thumb = if !enabled {
            t.text_disabled
        } else if resp.hovered() || resp.dragged() {
            Color32::WHITE
        } else {
            t.text
        };
        p.circle_filled(pos2(x, y), 5.0, thumb);
    }
    resp.widget_info(|| match value {
        Some(v) => egui::WidgetInfo::slider(enabled, v, name),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Slider, enabled, name),
    });
    let resp = resp.on_hover_text(tip).on_disabled_hover_text(tip);
    let hit = widgets::pointer_phase(&resp).map(|(at, phase)| ((((at.x - bar.left()) / bar.width()).clamp(0.0, 1.0) as f64 * max).round(), phase));
    (rect, hit)
}

/// A number field `width` wide, with the tool tip `tip` (also its screen-reader name) and the 3 pt
/// control corners, that reads "–" for a mixed selection (`None`). → its rect, and the number
/// committed.
pub(crate) fn number_field(ui: &mut Ui, id: &str, tip: &str, value: Option<f64>, width: f32) -> (Rect, Option<f64>) {
    let t = Tokens::get(ui.ctx());
    let mut focus_id = ui.id();
    let r = ui.scope(|ui| {
        focus_id = ui.id().with(id);
        control_corners(ui, |ui| widgets::mixed_field(ui, id, value, "", 0, width))
    });
    let rect = r.response.rect;
    // Screen readers name the text editor by its tool tip.
    ui.ctx().accesskit_node_builder(focus_id, |node| node.set_label(tip));
    // Its tool tip: a hover-only area on top of the text field, which takes no clicks.
    ui.interact(rect, ui.id().with((id, "tip")), Sense::hover()).on_hover_text(tip).on_disabled_hover_text(tip);
    if value.is_none() && !ui.memory(|m| m.has_focus(focus_id)) {
        let ink = if ui.is_enabled() { t.text } else { t.text_disabled };
        ui.painter().text(rect.right_center() - vec2(6.0, 0.0), egui::Align2::RIGHT_CENTER, "–", FontId::proportional(11.5), ink);
    }
    (rect, r.inner)
}

// ---------- Paragraph ----------

/// The Paragraph panel's Middle Eastern group for `para` (the first paragraph acted on, running
/// right to left when `rtl`): the head, Direction (with the Middle Eastern options), Kashida and
/// Strength (for right-to-left paragraphs). Nothing when none of them applies.
pub(crate) fn paragraph_group(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle, rtl: bool) -> Rows {
    let mut rows = Rows::default();
    let kashida = super::kashida_ui::shown(app, rtl);
    if !app.session.prefs.show_indic_options && !kashida {
        return rows;
    }
    let mut folded = app.ui.varos_me_folded.paragraph;
    let open = group_head(ui, &mut folded, &mut rows.controls);
    app.ui.varos_me_folded.paragraph = folded;
    if !open {
        return rows;
    }
    rows.controls.extend(paragraph_rows(app, ui, para, rtl).controls);
    if kashida {
        rows.controls.extend(super::kashida_ui::controls(app, ui, para).controls);
    }
    rows
}

/// The Direction row for `para` (the first paragraph acted on, resolving right to left when
/// `rtl`), with the Middle Eastern options: Auto, right to left, left to right.
pub(crate) fn paragraph_rows(app: &mut VectorcraftApp, ui: &mut Ui, para: &ParaStyle, rtl: bool) -> Rows {
    let mut rows = Rows::default();
    if !app.session.prefs.show_indic_options {
        return rows;
    }
    let all = super::kashida_ui::targeted(app);
    let first = all.first().map_or(para.direction, |p| p.direction);
    let mixed = all.iter().any(|p| p.direction != first);
    let has = |d: Option<ParaDirection>| all.iter().any(|p| p.direction == d);
    // Where Auto resolves: the sides of the paragraphs left on Auto.
    let auto_sides = |want_rtl: bool| {
        if all.is_empty() { rtl == want_rtl } else { all.iter().any(|p| p.direction.is_none() && p.rtl == want_rtl) }
    };
    let auto_lit = !mixed && first.is_none();
    let auto_tip = match (auto_lit, auto_sides(true), auto_sides(false)) {
        (true, true, false) => tl!("Auto direction: from the paragraph's first letter, now right to left"),
        (true, false, true) => tl!("Auto direction: from the paragraph's first letter, now left to right"),
        _ => tl!("Auto: from the paragraph's first letter"),
    };
    let size = Vec2::splat(16.0);
    let choice = |key, icon, tip: &str, dir: Option<ParaDirection>, ghost: bool| Choice {
        key,
        icon,
        icon_size: size,
        label: "",
        tip: tip.into(),
        lit: !mixed && first == dir,
        tinted: mixed && has(dir),
        ghost,
    };
    let choices = [
        choice("dir-auto", me_icons::DIR_AUTO, auto_tip, None, false),
        choice(
            "dir-rtl",
            me_icons::DIR_RTL,
            tl!("Right-to-Left Paragraph Direction"),
            Some(ParaDirection::RightToLeft),
            auto_lit && auto_sides(true),
        ),
        choice(
            "dir-ltr",
            me_icons::DIR_LTR,
            tl!("Left-to-Right Paragraph Direction"),
            Some(ParaDirection::LeftToRight),
            auto_lit && auto_sides(false),
        ),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if let Some(i) = segments(ui, &choices, 3.0 * 28.0 + 4.0, &mut rows.controls) {
            let key = ["auto", "rightToLeft", "leftToRight"][i];
            // Every paragraph the panel acts on takes it, whatever the first one has.
            run(app, "text.setFormat", json!({ "direction": key }));
        }
    });
    rows
}

// ---------- Character ----------

/// The harakat colours offered as presets: traditional red, green and gold, and black.
pub(crate) const HARAKAT_COLOURS: [(&str, [u8; 3]); 4] =
    [("Red", [0xC0, 0x39, 0x2B]), ("Green", [0x1E, 0x84, 0x49]), ("Gold", [0xB7, 0x95, 0x0B]), ("Black", [0x14, 0x13, 0x13])];

/// The harakat of what the Character panel acts on.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HarakatState {
    pub count: usize,
    /// The fill they all have (`None`: they differ, or there are none).
    pub fill: Option<Paint>,
    /// Every one has no fill.
    pub hidden: bool,
    /// Every one has its letter's fill.
    pub as_letters: bool,
}

/// The harakat state of what the Character panel acts on. One pass over each text's runs: a
/// harakat's letter is the last character before it that isn't one (as `text.styleHarakat`'s
/// "inherit" reads it), else the first run's.
pub(crate) fn harakat_state(app: &VectorcraftApp) -> HarakatState {
    let mut s = HarakatState { hidden: true, as_letters: true, ..Default::default() };
    let mut first: Option<Paint> = None;
    let mut same = true;
    for_each_text(app, |t, range| {
        let (a, b) = range.map_or((0, usize::MAX), |(a, b)| (a.min(b), a.max(b)));
        if a == b {
            return;
        }
        let mut letter = t.runs.first().map(|r| &r.style.fill);
        let mut at = 0;
        for run in &t.runs {
            let fill = &run.style.fill;
            for (i, c) in run.text.char_indices() {
                if !vectorcraft_text::harakat::is_harakat(c) {
                    letter = Some(fill);
                } else if (a..b).contains(&(at + i)) {
                    s.count += 1;
                    s.hidden &= *fill == Paint::None;
                    s.as_letters &= letter == Some(fill);
                    match &first {
                        None => first = Some(fill.clone()),
                        Some(f) => same &= f == fill,
                    }
                }
            }
            at += run.text.len();
        }
    });
    if s.count == 0 {
        return HarakatState::default();
    }
    s.fill = first.filter(|_| same);
    s
}

/// Panel state: the harakat colour picked last (hex digits), which Hide and As letters return to.
const LAST_COLOUR: &str = "varos-harakat-colour";

/// Give the harakat `fill` (`#hex`, "none" or "inherit").
fn style_harakat(app: &mut VectorcraftApp, fill: &str) {
    run(app, "text.styleHarakat", json!({ "fill": fill }));
}

/// Give the harakat colour `c`, previewing while `phase` is a drag (one undo step when released);
/// remember it.
fn colour_harakat(app: &mut VectorcraftApp, ctx: &egui::Context, c: &Color, phase: Live) {
    let hex = hex_digits(c);
    if phase == Live::Released {
        crate::panels::set_pstate(ctx, LAST_COLOUR, hex.clone());
    }
    let p = targeted(app, json!({ "fill": format!("#{hex}") }));
    crate::panels::live_run(app, LIVE_LABEL, "text.styleHarakat", p, phase);
}

/// The digit choices: the setting, its name, the digits that show it and its `digits` value.
fn digit_choices() -> [(Digits, &'static str, &'static str, &'static str); 4] {
    [
        (Digits::Default, tl!("Default"), "Aa", "default"),
        (Digits::Arabic, tl!("Arabic"), "456", "arabic"),
        (Digits::Hindi, tl!("Hindi"), "٤٥٦", "hindi"),
        (Digits::Farsi, tl!("Farsi"), "۴۵۶", "farsi"),
    ]
}

/// The Character panel's Middle Eastern group for `s` (the first character acted on): the head,
/// Digits with Hide and As letters, and the Harakat row.
pub(crate) fn character_rows(app: &mut VectorcraftApp, ui: &mut Ui, s: &CharStyle) -> Rows {
    let mut rows = Rows::default();
    if !app.session.prefs.show_indic_options {
        return rows;
    }
    let mut folded = app.ui.varos_me_folded.character;
    let open = group_head(ui, &mut folded, &mut rows.controls);
    app.ui.varos_me_folded.character = folded;
    if !open {
        return rows;
    }
    let t = Tokens::get(ui.ctx());
    let h = harakat_state(app);
    let last: String = crate::panels::pstate(ui.ctx(), LAST_COLOUR);
    let last = parse_hex(&last);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let digits = crate::panels::character::shared(app, |c| c.digits);
        if let Some(key) = digits_dropdown(ui, digits, &mut rows.controls) {
            run(app, "text.setFormat", json!({ "digits": key }));
        }
        // Hide (icon-only: the crossed eye is universal) and As letters (icon and label: its glyph
        // is new), together at the right end.
        let as_letters = tl!("As letters");
        let room = (ui.available_width() - 24.0 - 2.0 - 4.0).max(0.0);
        let label_w = {
            let g = ui.painter().layout_no_wrap(as_letters.to_string(), FontId::proportional(11.0), Color32::PLACEHOLDER);
            (2.0 * 5.0 + 16.0 + 4.0 + g.size().x).min(room)
        };
        ui.add_space((ui.available_width() - 24.0 - 2.0 - label_w).max(0.0));
        if toggle(ui, "hide", me_icons::HARAKAT_HIDE, "", tl!("Hide the harakat: they stay in the text"), h.hidden, 24.0, &mut rows.controls) {
            // Shown again: the colour picked last, else their letters' colour.
            let fill = if h.hidden { last.map_or("inherit".into(), |c| format!("#{}", hex_digits(&c))) } else { "none".into() };
            style_harakat(app, &fill);
        }
        if toggle(
            ui,
            "as-letters",
            me_icons::HARAKAT_AS_LETTERS,
            as_letters,
            tl!("Give the harakat their letters' colour again"),
            h.as_letters,
            room,
            &mut rows.controls,
        ) {
            // Off: the colour picked last, else the first preset.
            let fill = if h.as_letters {
                let [r, g, b] = HARAKAT_COLOURS[0].1;
                last.map_or(format!("#{r:02X}{g:02X}{b:02X}"), |c| format!("#{}", hex_digits(&c)))
            } else {
                "inherit".into()
            };
            style_harakat(app, &fill);
        }
    });
    // Hidden harakat have nothing to colour: the row greys out.
    ui.add_enabled_ui(!h.hidden, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            row_icon(ui, "harakat", me_icons::HARAKAT, tl!("Colour the harakat (Arabic vowel marks) apart from their letters"), &mut rows.controls);
            let shared = match &h.fill {
                Some(Paint::Solid { color, .. }) => Some(*color),
                _ => None,
            };
            let mixed = h.count > 0 && h.fill.is_none();
            let letters = match &s.fill {
                Paint::Solid { color, .. } => Some(*color),
                _ => None,
            };
            let shown = shared.or(last).or(letters).unwrap_or(Color::BLACK);
            colour_well(app, ui, &shown, mixed, &mut rows.controls);
            let hex = if mixed { "–".to_string() } else { hex_digits(&shown) };
            let ink = if ui.is_enabled() { t.text_dim } else { t.text_disabled };
            ui.label(egui::RichText::new(hex).font(FontId::monospace(11.0)).color(ink));
            let presets_w = 4.0 * 14.0 + 3.0 * 4.0 + 2.0;
            ui.add_space((ui.available_width() - presets_w).max(0.0));
            let current = shared.filter(|_| !h.hidden).map(|c| hex_digits(&c));
            if let Some(c) = presets(ui, current.as_deref(), "", &mut rows.controls) {
                colour_harakat(app, ui.ctx(), &c, Live::Released);
            }
        });
    });
    rows
}

/// The Digits dropdown showing `current` (`None`: mixed, "–"): its button shows the digits
/// themselves and the name, its list leads each choice with its digits. → the `digits` value chosen.
fn digits_dropdown(ui: &mut Ui, current: Option<Digits>, out: &mut Vec<Drawn>) -> Option<&'static str> {
    let t = Tokens::get(ui.ctx());
    let choices = digit_choices();
    let now = current.and_then(|d| choices.iter().find(|c| c.0 == d));
    let (rect, resp) = ui.allocate_exact_size(vec2(112.0, FIELD), Sense::click());
    let popup = resp.id.with("varos-digits-list");
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let p = ui.painter();
    p.rect(
        rect,
        RADIUS,
        if resp.hovered() { t.hover } else { t.button },
        Stroke::new(1.0, if open { t.accent } else { t.border }),
        StrokeKind::Inside,
    );
    let (glyphs, name) = now.map_or(("–", ""), |c| (c.2, c.1));
    let mut x = rect.left() + 6.0;
    x += paint_digits(p, pos2(x, rect.center().y), glyphs, t.text) + 6.0;
    let n = p.layout_no_wrap(name.to_string(), FontId::proportional(11.0), t.text_dim);
    p.galley(pos2(x, rect.center().y - n.size().y / 2.0), n, t.text_dim);
    chevron(p, pos2(rect.right() - 8.0, rect.center().y), t.text_dim);
    let tip = match now {
        Some(c) => format!("{}: {}", tl!("Digits"), c.1),
        None => format!("{}: {}", tl!("Digits"), tl!("Mixed: the selected text differs")),
    };
    resp.widget_info(|| egui::WidgetInfo {
        current_text_value: Some(name.to_string()),
        ..egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, tl!("Digits"))
    });
    out.push(Drawn { key: "digits", rect, tip: tip.clone(), lit: now.is_some(), ..Default::default() });
    let resp = resp.on_hover_text(tip);
    egui::Popup::menu(&resp)
        .id(popup)
        .width(170.0)
        .show(|ui| {
            let mut chosen = None;
            for (d, name, glyphs, key) in choices {
                let (r, row) = ui.allocate_exact_size(vec2(170.0, ROW), Sense::click());
                let p = ui.painter();
                if row.hovered() {
                    p.rect_filled(r, RADIUS, t.hover);
                }
                if current == Some(d) {
                    p.text(pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, "✓", FontId::proportional(11.0), t.text);
                }
                let ink = if d == Digits::Default { t.text_dim } else { t.text };
                paint_digits(p, pos2(r.left() + 24.0, r.center().y), glyphs, ink);
                p.text(pos2(r.left() + 60.0, r.center().y), egui::Align2::LEFT_CENTER, name, FontId::proportional(11.0), t.text);
                row.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, current == Some(d), name));
                out.push(Drawn { key: row_key(key), rect: r, tip: name.into(), lit: current == Some(d), ..Default::default() });
                if row.clicked() {
                    chosen = Some(key);
                }
            }
            chosen
        })
        .and_then(|r| r.inner)
}

/// Paint `digits` left to right from `left` (its vertical centre), 12.5 pt. → their width. Arabic
/// digits run left to right in Arabic too, but egui shapes a run of Arabic-Indic digits as
/// right-to-left Arabic and would show ٤٥٦ as ٦٥٤, so each non-ASCII digit is laid out alone.
fn paint_digits(p: &egui::Painter, left: Pos2, digits: &str, colour: Color32) -> f32 {
    let font = FontId::proportional(12.5);
    let parts: Vec<String> = if digits.is_ascii() { vec![digits.to_string()] } else { digits.chars().map(String::from).collect() };
    let mut x = left.x;
    for part in parts {
        let g = p.layout_no_wrap(part, font.clone(), colour);
        let w = g.size().x;
        p.galley(pos2(x, left.y - g.size().y / 2.0), g, colour);
        x += w;
    }
    x - left.x
}

/// The drawn record's key of the Digits list row for `digits` value `key`.
fn row_key(key: &str) -> &'static str {
    match key {
        "arabic" => "digits-arabic",
        "hindi" => "digits-hindi",
        "farsi" => "digits-farsi",
        _ => "digits-default",
    }
}

/// A small chevron pointing down at `c`.
fn chevron(p: &egui::Painter, c: Pos2, colour: Color32) {
    p.line_segment([c + vec2(-3.0, -1.5), c + vec2(0.0, 1.5)], Stroke::new(1.0, colour));
    p.line_segment([c + vec2(0.0, 1.5), c + vec2(3.0, -1.5)], Stroke::new(1.0, colour));
}

/// The four preset swatches, 14 pt, the one whose hex is `current` ringed in azure. `salt` keeps
/// the popover's apart from the row's. → the colour clicked.
fn presets(ui: &mut Ui, current: Option<&str>, salt: &str, out: &mut Vec<Drawn>) -> Option<Color> {
    let t = Tokens::get(ui.ctx());
    let mut picked = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (name, [r, g, b]) in HARAKAT_COLOURS {
            let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
            let resp = ui.interact(rect, ui.id().with(("varos-harakat-preset", salt, name)), Sense::click());
            let hex = format!("{r:02X}{g:02X}{b:02X}");
            let on = current == Some(hex.as_str());
            let p = ui.painter();
            p.rect(rect, RADIUS, Color32::from_rgb(r, g, b), Stroke::new(1.0, Color32::from_white_alpha(31)), StrokeKind::Inside);
            if on {
                p.rect_stroke(rect.expand(1.0), RADIUS + 1, Stroke::new(1.0, t.accent), StrokeKind::Outside);
            } else if resp.hovered() && ui.is_enabled() {
                p.rect_stroke(rect.expand(1.0), RADIUS + 1, Stroke::new(1.0, t.text_dim), StrokeKind::Outside);
            }
            let tip = match name {
                "Red" => tl!("Red"),
                "Green" => tl!("Green"),
                "Gold" => tl!("Gold"),
                _ => tl!("Black"),
            };
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, tip));
            let key = match name {
                "Red" => "preset-red",
                "Green" => "preset-green",
                "Gold" => "preset-gold",
                _ => "preset-black",
            };
            if salt.is_empty() {
                out.push(Drawn { key, rect, tip: tip.into(), lit: on, ..Default::default() });
            }
            if resp.on_hover_text(tip).on_disabled_hover_text(tip).clicked() {
                picked = Some(Color::rgb8(r, g, b));
            }
        }
    });
    picked
}

/// Egui memory of the popover's colour while it is edited: its hex and HSB (the hue survives a
/// grey).
const POPOVER_HSB: &str = "varos-harakat-hsb";

/// Egui memory of the well's popover: its open-state id (as [`widgets::popover`] keeps it), for
/// [`escape_popover`], which runs before the panels are drawn.
const WELL_POPOVER: &str = "varos-harakat-popover";

/// Egui memory: a colour is being dragged in the popover, previewed on the art (a live interaction
/// that is one undo step when released).
const LIVE_DRAG: &str = "varos-harakat-live";

/// The live interaction a harakat colour drag previews in ([`crate::panels::live_run`]).
const LIVE_LABEL: &str = "Style Harakat";

/// Drop a harakat colour being dragged: the harakat get back the colour they had, and nothing is
/// added to the undo history.
fn cancel_drag(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let live = ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new(LIVE_DRAG))).unwrap_or(false);
    let ours = app.session.active().and_then(|d| d.interaction.as_ref()).is_some_and(|i| i.label == LIVE_LABEL);
    if live && ours {
        app.session.cancel_interaction().ok();
        app.sync_views();
    }
    ctx.data_mut(|d| d.remove::<(String, [f32; 3])>(egui::Id::new(POPOVER_HSB)));
}

/// Escape while the harakat colour popover is open is the popover's alone: it closes the popover
/// and drops a colour being dragged in it (egui would end that drag as if released), and no
/// shortcut, tool or canvas sees it (the Type tool keeps typing). Called by the shortcuts before
/// they read the keys, and before the panels are drawn; the focus is left where it is. → it took
/// the Escape.
pub(crate) fn escape_popover(app: &mut VectorcraftApp, ctx: &egui::Context) -> bool {
    let Some(popover) = ctx.data(|d| d.get_temp::<egui::Id>(egui::Id::new(WELL_POPOVER))) else { return false };
    let open = ctx.data(|d| d.get_temp::<u64>(popover)).is_some_and(|f| f + 1 >= ctx.cumulative_frame_nr());
    if !open || !ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
        return false;
    }
    ctx.data_mut(|d| d.remove::<u64>(popover));
    cancel_drag(app, ctx);
    true
}

/// Draw `add` with the boxes it paints at upstream's 5 pt field corners given the Varos 3 pt
/// control corners instead (owner rule), leaving upstream's widget code as it is.
fn control_corners<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let layer = ui.layer_id();
    let start = ui.ctx().graphics_mut(|g| g.entry(layer).next_idx());
    let r = add(ui);
    ui.ctx().graphics_mut(|g| {
        let list = g.entry(layer);
        for i in start.0..list.next_idx().0 {
            list.mutate_shape(egui::layers::ShapeIdx(i), |c| {
                if let egui::Shape::Rect(rect) = &mut c.shape
                    && rect.corner_radius == egui::CornerRadius::same(5)
                {
                    rect.corner_radius = egui::CornerRadius::same(RADIUS);
                }
            });
        }
    });
    r
}

/// The harakat colour well showing `shown` (`mixed`: the harakat differ): a click opens a popover
/// with upstream's colour field and hue slider, a hex field and the presets; a drag previews on the
/// art and is one undo step when released.
fn colour_well(app: &mut VectorcraftApp, ui: &mut Ui, shown: &Color, mixed: bool, out: &mut Vec<Drawn>) {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(34.0, FIELD), Sense::click());
    let open_id = resp.id.with("popover");
    let open = ui.ctx().data(|d| d.get_temp::<u64>(open_id)).is_some_and(|f| f + 1 >= ui.ctx().cumulative_frame_nr());
    let p = ui.painter();
    let border = if open { t.accent } else { t.border };
    p.rect(rect, RADIUS, if resp.hovered() && ui.is_enabled() { t.hover } else { t.button }, Stroke::new(1.0, border), StrokeKind::Inside);
    let chip = Rect::from_min_size(pos2(rect.left() + 3.0, rect.center().y - 7.0), Vec2::splat(14.0));
    if mixed {
        p.rect(chip, RADIUS, t.hover, Stroke::new(1.0, Color32::from_white_alpha(31)), StrokeKind::Inside);
        p.line_segment([chip.left_bottom(), chip.right_top()], Stroke::new(1.0, t.text_dim));
    } else {
        p.rect(chip, RADIUS, crate::panels::c32(shown), Stroke::new(1.0, Color32::from_white_alpha(31)), StrokeKind::Inside);
    }
    chevron(p, pos2(rect.right() - 7.0, rect.center().y), t.text_dim);
    let tip = tl!("Harakat colour: click for any colour");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tip));
    out.push(Drawn { key: "well", rect, tip: tip.into(), ..Default::default() });
    let resp = resp.on_hover_text(tip).on_disabled_hover_text(tip);
    let toggle = resp.clicked();
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(WELL_POPOVER), open_id));
    widgets::popover(&resp, toggle, |ui| {
        ui.set_width(196.0);
        // The colour being edited: kept while it is the one shown, so a grey keeps its hue.
        let hex = hex_digits(shown);
        let kept = ui.ctx().data(|d| d.get_temp::<(String, [f32; 3])>(egui::Id::new(POPOVER_HSB)));
        let [h, s, b] = match kept {
            Some((k, hsb)) if k == hex && !mixed => hsb,
            _ => shown.to_hsb(),
        };
        let mut pick: Option<([f32; 3], Live)> = None;
        let at = |x: f32, y: f32| crate::panels::c32(&Color::from_hsb(h, x, 1.0 - y));
        let field_at = ui.next_widget_position();
        let field_id = ui.id().with("varos-harakat-field");
        if let (Some((x, y)), phase) = widgets::color_field(ui, "varos-harakat-field", vec2(196.0, 96.0), (s, 1.0 - b), &at) {
            pick = Some(([h, x, 1.0 - y], phase));
        }
        // Upstream's field has no screen-reader node of its own: a slider with no value, named.
        ui.ctx().accesskit_node_builder(field_id, |node| {
            node.set_role(egui::accesskit::Role::Slider);
            node.set_label(tl!("Saturation and brightness"));
        });
        out.push(Drawn { key: "harakat-field", rect: Rect::from_min_size(field_at, vec2(196.0, 96.0)), ..Default::default() });
        ui.add_space(4.0);
        let hue = |z: f32| crate::panels::c32(&Color::from_hsb(z * 360.0, 1.0, 1.0));
        if let (Some(z), phase) = widgets::color_slider_named(ui, "varos-harakat-hue", tl!("Hue"), h / 360.0, 196.0, &hue) {
            pick = Some(([z * 360.0, s, b], phase));
        }
        ui.add_space(2.0);
        let mut typed = None;
        let hex_row = ui.horizontal(|ui| {
            let hex_id = ui.id().with("varos-harakat-hex");
            if let Some(text) = control_corners(ui, |ui| widgets::hex_field(ui, "varos-harakat-hex", if mixed { "" } else { &hex })) {
                typed = parse_hex(&text);
            }
            ui.ctx().accesskit_node_builder(hex_id, |node| node.set_label(tl!("Hex colour")));
        });
        out.push(Drawn { key: "harakat-hex", rect: hex_row.response.rect, ..Default::default() });
        ui.add_space(4.0);
        let current = (!mixed).then_some(hex.as_str());
        let preset = ui.horizontal(|ui| presets(ui, current, "popover", &mut vec![])).inner;
        // Egui ends a drag on Escape as if it were released: that Escape drops the colour dragged
        // instead (when the shortcuts did not take it first, [`escape_popover`]).
        let live = ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new(LIVE_DRAG))).unwrap_or(false);
        if live && ui.input(|i| i.key_pressed(Key::Escape)) {
            cancel_drag(app, ui.ctx());
            return;
        }
        if let Some((hsb, phase)) = pick {
            let c = Color::from_hsb(hsb[0], hsb[1], hsb[2]);
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new(POPOVER_HSB), (hex_digits(&c), hsb));
                d.insert_temp(egui::Id::new(LIVE_DRAG), phase == Live::Dragging);
            });
            colour_harakat(app, ui.ctx(), &c, phase);
        } else if let Some(c) = typed.or(preset) {
            colour_harakat(app, ui.ctx(), &c, Live::Released);
        }
    });
}

#[cfg(test)]
#[path = "tests_middle_eastern_ui.rs"]
mod tests;
