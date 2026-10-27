//! Box painting: an ordinary box (its tab row, its ≡ menu and its front panel's body) and the
//! Board (the canvas). Flat: an 8-point box on the seam, a 1-point hairline, no shadow; azure is
//! never used here (the front tab is lit, not coloured).

use egui::{Align, CornerRadius, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2};

use super::Hit;
use super::tree::PanelBox;
use crate::VectorcraftApp;
use crate::state::DockTab;
use crate::theme::Tokens;

/// The tab row's height.
pub(crate) const HEADER: f32 = 32.0;
/// A box's corner radius.
pub(crate) const RADIUS: u8 = 8;
/// A tab pill's height (a capsule: its radius is half of it).
const PILL_H: f32 = 22.0;
/// Room left and right of a tab's label inside its pill.
const PILL_PAD: f32 = 10.0;
/// Between two pills.
const PILL_GAP: f32 = 2.0;
/// Between the tab row's ends and the box's edges.
const PAD: f32 = 6.0;
/// Tab labels' size.
const LABEL: f32 = 12.0;
/// The ≡ button.
const MENU: Vec2 = vec2(20.0, 24.0);
/// Controls' corner radius (the ≡ button's hover well).
const CONTROL_RADIUS: u8 = 3;

/// What a tab row did this frame.
pub(crate) struct TabRow {
    /// The tab clicked (its index), if any.
    pub clicked: Option<usize>,
    /// Each tab's pill, in order.
    pub tabs: Vec<Rect>,
}

/// The egui id of panel `panel`'s tab (a panel is in one box at a time, so it follows the panel).
pub(crate) fn tab_id(panel: &str) -> egui::Id {
    egui::Id::unique("varos-shell-tab").with(panel)
}

/// The pills of `tabs` (labels) in `row` with tab `active` in front (semibold, so wider), and
/// each label's galley; narrower than their labels when they don't all fit.
fn pills(painter: &egui::Painter, tabs: &[(String, String)], active: usize, row: Rect, t: &Tokens) -> Vec<(Rect, std::sync::Arc<egui::Galley>)> {
    let galleys: Vec<_> = tabs
        .iter()
        .enumerate()
        .map(|(i, (_, label))| {
            let font = if i == active { crate::theme::semibold(LABEL) } else { egui::FontId::proportional(LABEL) };
            painter.layout_no_wrap(label.clone(), font, t.text)
        })
        .collect();
    let wanted: f32 = galleys.iter().map(|g| g.size().x + 2.0 * PILL_PAD + PILL_GAP).sum();
    let scale = if wanted > row.width() && wanted > 0.0 { row.width() / wanted } else { 1.0 };
    let mut x = row.left();
    galleys
        .into_iter()
        .map(|galley| {
            let w = ((galley.size().x + 2.0 * PILL_PAD) * scale).floor().max(1.0);
            let pill = Rect::from_min_size(pos2(x, row.center().y - PILL_H / 2.0), vec2(w, PILL_H));
            x = pill.right() + PILL_GAP * scale;
            (pill, galley)
        })
        .collect()
}

/// Draw a tab row in `row`: `tabs` (panel id, label) as capsule pills from the left, the front one
/// lit (a brighter fill, semibold text); narrower than their labels when they don't all fit
/// (labels are cut). A click switches at once: the clicked pill is painted in front on the very
/// frame of the click. Stage 3 drags tabs by these same pills.
pub(crate) fn tab_row(ui: &mut egui::Ui, tabs: &[(String, String)], active: usize, row: Rect, t: &Tokens) -> TabRow {
    let painter = ui.painter().clone();
    // The pills where they were drawn (with the old front tab) take the input…
    let before = pills(&painter, tabs, active, row, t);
    let responses: Vec<egui::Response> =
        tabs.iter().zip(&before).map(|((panel, _), (pill, _))| ui.interact(*pill, tab_id(panel), Sense::click())).collect();
    let clicked = responses.iter().position(|r| r.clicked()).filter(|i| *i != active);
    // … and the row is painted with the front tab that input leaves.
    let front_tab = clicked.unwrap_or(active);
    let after = if clicked.is_some() { pills(&painter, tabs, front_tab, row, t) } else { before };
    let capsule = CornerRadius::same((PILL_H / 2.0) as u8);
    for (i, ((pill, galley), response)) in after.iter().zip(&responses).enumerate() {
        let front = i == front_tab;
        let text = if front {
            painter.rect_filled(*pill, capsule, t.button);
            t.text_strong
        } else if response.hovered() {
            painter.rect_filled(*pill, capsule, t.hover);
            t.text
        } else {
            t.text_dim
        };
        let at = pos2(pill.left() + PILL_PAD.min(pill.width() / 2.0), pill.center().y - galley.size().y / 2.0);
        let clip = pill.shrink2(vec2(4.0, 0.0)).intersect(painter.clip_rect());
        painter.with_clip_rect(clip).galley(at, galley.clone(), text);
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, front, galley.text()));
    }
    TabRow { clicked, tabs: after.into_iter().map(|(pill, _)| pill).collect() }
}

/// Panel `id`'s tab label: its library's name, or its translated name.
pub(crate) fn label(app: &VectorcraftApp, id: &str) -> String {
    crate::panels::library_panel::tab_label(app, id)
        .or_else(|| crate::state::all_panels().find(|p| p.0 == id).map(|p| tl!(p.1).to_string()))
        .unwrap_or_else(|| id.to_string())
}

/// Paint over the parts of `rect` outside its rounded corners in the seam's colour, then its
/// hairline: content drawn edge to edge (the canvas, a scroll bar) gets the box's rounded corners.
fn frame(painter: &egui::Painter, rect: Rect, t: &Tokens) {
    // A stroke whose inner edge is the rounded box covers the corners' outside (6 points reach
    // past the corner of an 8-point radius) and stays inside the 12-point seam.
    const COVER: f32 = 6.0;
    let r = f32::from(RADIUS);
    painter.rect_stroke(rect.expand(COVER / 2.0), CornerRadius::same((r + COVER / 2.0) as u8), Stroke::new(COVER, t.divider), StrokeKind::Middle);
    painter.rect_stroke(rect, CornerRadius::same(RADIUS), Stroke::new(1.0, t.border), StrokeKind::Inside);
}

/// Draw the Board in `rect`: the document tabs, the canvas and the isolation bar (Home instead,
/// when it shows). `framed`: as a box (rounded, a hairline); else edge to edge.
pub(crate) fn board(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, framed: bool) {
    if !rect.is_positive() {
        return;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt("varos-shell-board").max_rect(rect).layout(egui::Layout::top_down(Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    app.canvas_area(&mut child);
    if framed {
        let t = Tokens::get(ui.ctx());
        frame(ui.painter(), rect, &t);
    }
}

/// What a box asks for after it is drawn.
pub(crate) enum Ask {
    /// Close its panel `0` (the ≡ menu's Close Panel).
    Close(String),
}

/// Draw ordinary box `b` in `rect`: tab row, ≡, the front panel's body. Its controls go to `hits`.
pub(crate) fn show(app: &mut VectorcraftApp, ui: &mut egui::Ui, b: &PanelBox, rect: Rect, hits: &mut Vec<(Hit, Rect)>) -> Option<Ask> {
    if !rect.is_positive() {
        return None;
    }
    let t = Tokens::get(ui.ctx());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, CornerRadius::same(RADIUS), t.panel);
    hits.push((Hit::Box(b.id), rect));
    let head = Rect::from_min_max(rect.min, pos2(rect.right(), (rect.top() + HEADER).min(rect.bottom())));
    let menu = Rect::from_min_size(pos2(head.right() - PAD - MENU.x, head.center().y - MENU.y / 2.0), MENU);
    let row = Rect::from_min_max(pos2(head.left() + PAD, head.top()), pos2((menu.left() - PAD).max(head.left() + PAD), head.bottom()));
    hits.push((Hit::TabRow(b.id), row));
    let tabs: Vec<(String, String)> = b.tabs.iter().map(|p| (p.clone(), label(app, p))).collect();
    let active = b.active.min(tabs.len().saturating_sub(1));
    let out = tab_row(ui, &tabs, active, row, &t);
    for ((panel, _), pill) in tabs.iter().zip(&out.tabs) {
        hits.push((Hit::Tab { box_id: b.id, panel: panel.clone() }, *pill));
    }
    // A click switches before the body is drawn: the new panel shows in the same frame.
    let active = match out.clicked {
        Some(i) => {
            app.ui.varos_shell.tree.set_active(b.id, i);
            i
        }
        None => active,
    };
    let front = tabs.get(active).map(|(p, _)| p.clone());
    let mut ask = None;
    if let Some(front) = &front {
        hits.push((Hit::Menu(b.id), menu));
        if menu_button(app, ui, menu, b.id, front, &t) {
            ask = Some(Ask::Close(front.clone()));
        }
    }
    painter.hline((rect.left() + 1.0)..=(rect.right() - 1.0), head.bottom() - 0.5, Stroke::new(1.0, t.border));
    let inner = Rect::from_min_max(pos2(rect.left() + 1.0, head.bottom()), pos2(rect.right() - 1.0, rect.bottom() - 1.0));
    if let Some(front) = &front
        && inner.is_positive()
    {
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt(("varos-shell-box", b.id)).max_rect(inner).layout(egui::Layout::top_down(Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        body(app, &mut child, front);
    }
    frame(&painter, rect, &t);
    ask
}

/// The ≡ button: the panel's own menu items (`panels::panel_menu_items`) and Close Panel; true when
/// Close Panel was chosen.
fn menu_button(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, box_id: u32, panel: &str, t: &Tokens) -> bool {
    let response = ui.interact(rect, egui::Id::unique("varos-shell-menu").with(box_id), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(CONTROL_RADIUS), t.hover);
    }
    crate::icons::paint(
        ui,
        "menu",
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        if response.hovered() { t.text_strong } else { t.text_dim },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tl!("Panel menu")));
    let response = response.on_hover_text(tl!("Panel menu"));
    let mut close = false;
    egui::Popup::menu(&response).show(|ui| {
        crate::widgets::menu_scroll(ui, |ui| {
            ui.set_min_width(220.0);
            if crate::panels::panel_menu_items(app, ui, panel) {
                ui.separator();
            }
            if crate::widgets::menu_item(ui, "Close Panel", true, false) {
                close = true;
            }
        });
    });
    close
}

/// Panel `id`'s body filling `ui`: the main column's panels size themselves (Layers fills,
/// Properties scrolls); every other panel scrolls.
fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, id: &str) {
    let width = ui.available_width();
    let height = (ui.available_height() - 20.0).max(0.0);
    if DockTab::from_id(id).is_some() {
        crate::dock::panel_body(app, ui, id, width, height);
    } else {
        crate::widgets::panel_scroll(ui, egui::ScrollArea::vertical().id_salt(("varos-shell-body", id)).auto_shrink([false, false]), |ui| {
            let width = ui.available_width();
            crate::dock::panel_body(app, ui, id, width, height)
        });
    }
}
