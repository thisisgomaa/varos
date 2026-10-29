//! Box painting: an ordinary box (its tab row, its ≡ menu and its front panel's body) and the
//! Board (the canvas). Flat: an 8-point box on the seam, a 1-point hairline, no shadow; azure is
//! never used here (the front tab is lit, not coloured). A tab pill is also where a tab is dragged
//! from ([`super::drag`]) and double-clicked to collapse its box to its tab row; a double-click on
//! the tab row's empty space maximizes the box (and restores the layout).

use egui::{Align, CornerRadius, Rect, Sense, Stroke, Vec2, pos2, vec2};

use super::Hit;
use super::drag::{self, Item};
use super::tree::PanelBox;
use crate::VectorcraftApp;
use crate::state::DockTab;
use crate::theme::Tokens;

/// The tab row's height.
pub(crate) const HEADER: f32 = super::tree::ROW;
pub(crate) const FLOAT_HEADER: f32 = 32.0;
/// A box's corner radius.
pub(crate) const RADIUS: u8 = 10;
/// A tab pill's height (a capsule: its radius is half of it).
pub(crate) const PILL_H: f32 = 24.0;
/// Room left and right of a tab's label inside its pill.
const PILL_PAD: f32 = 10.0;
/// Between two pills.
const PILL_GAP: f32 = 2.0;
/// Between the tab row's ends and the box's edges.
const PAD: f32 = 6.0;
/// Tab labels' size.
const LABEL: f32 = 11.0;
/// The ≡ button.
const MENU: Vec2 = vec2(20.0, 24.0);
/// Controls' corner radius (the ≡ button's hover well).
const CONTROL_RADIUS: u8 = 3;

/// What a tab row did this frame.
pub(crate) struct TabRow {
    /// The tab clicked (its index), if any.
    pub clicked: Option<usize>,
    /// The tab double-clicked (its index), if any.
    pub double_clicked: Option<usize>,
    /// The tab whose drag began this frame (its index), if any.
    pub drag_started: Option<usize>,
    /// Each tab's pill, in order.
    pub tabs: Vec<Rect>,
}

/// The egui id of panel `panel`'s tab (a panel is in one box at a time, so it follows the panel).
pub(crate) fn tab_id(panel: &str) -> egui::Id {
    egui::Id::unique("varos-shell-tab").with(panel)
}

fn options_key(panel: &str) -> Option<&'static str> {
    match panel {
        "character" => Some("ch-hide-options"),
        "paragraph" => Some("pa-hide-options"),
        _ => None,
    }
}

fn ellipsize(painter: &egui::Painter, text: &str, font: egui::FontId, color: egui::Color32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let full = painter.layout_no_wrap(text.to_string(), font.clone(), color);
    if full.size().x <= max_w + 0.5 {
        return full;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut best = painter.layout_no_wrap("…".to_string(), font.clone(), color);
    let mut lo = 0usize;
    let mut hi = chars.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let mut s: String = chars.iter().take(mid).collect();
        s.push('…');
        let galley = painter.layout_no_wrap(s, font.clone(), color);
        if galley.size().x <= max_w + 0.5 {
            best = galley;
            lo = mid;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    best
}

fn option_width(panel: &str) -> f32 {
    if options_key(panel).is_some() { 18.0 } else { 0.0 }
}

/// The pills of `tabs` (labels) in `row` with tab `active` in front (semibold, so wider), and
/// each label's galley; narrower than their labels when they don't all fit.
fn pills(painter: &egui::Painter, tabs: &[(String, String)], active: usize, row: Rect, t: &Tokens) -> Vec<(Rect, std::sync::Arc<egui::Galley>)> {
    let mut galleys: Vec<_> = tabs
        .iter()
        .enumerate()
        .map(|(i, (_, label))| {
            let font = if i == active { crate::theme::semibold(LABEL) } else { egui::FontId::proportional(LABEL) };
            painter.layout_no_wrap(label.clone(), font, t.text)
        })
        .collect();
    let mut widths: Vec<f32> = galleys.iter().zip(tabs).map(|(g, (id, _))| g.size().x + 2.0 * PILL_PAD + option_width(id)).collect();
    let natural = widths.iter().sum::<f32>() + PILL_GAP * widths.len().saturating_sub(1) as f32;
    let overflow = natural > row.width();
    // One 28-point overflow control, then as many whole pills as fit. A pill is never narrowed,
    // so a label is never cut mid-word — unless the active pill itself does not fit beside `»`,
    // in which case that one pill is ellipsized and the rest stay in the menu.
    let room = if overflow { (row.width() - 28.0).max(0.0) } else { row.width() };
    let mut visible = vec![false; widths.len()];
    let mut used = 0.0;
    let consider = |i: usize, widths: &[f32], used: &mut f32, visible: &mut [bool]| {
        let need = widths[i] + if *used > 0.0 { PILL_GAP } else { 0.0 };
        if *used + need <= room + 0.5 {
            *used += need;
            visible[i] = true;
        }
    };
    if active < widths.len() {
        consider(active, &widths, &mut used, &mut visible);
    }
    if overflow && active < widths.len() && !visible[active] && room > 8.0 {
        let text_w = (room - 2.0 * PILL_PAD - option_width(&tabs[active].0)).max(4.0);
        let font = crate::theme::semibold(LABEL);
        galleys[active] = ellipsize(painter, &tabs[active].1, font, t.text, text_w);
        widths[active] = (galleys[active].size().x + 2.0 * PILL_PAD + option_width(&tabs[active].0)).min(room);
        visible[active] = true;
    }
    for i in 0..widths.len() {
        if i != active {
            consider(i, &widths, &mut used, &mut visible);
        }
    }
    let mut x = row.left();
    galleys
        .into_iter()
        .zip(widths)
        .zip(visible)
        .map(|((galley, width), show)| {
            if !show {
                return (Rect::NOTHING, galley);
            }
            let pill = Rect::from_min_size(pos2(x, row.center().y - PILL_H / 2.0), vec2(width, PILL_H));
            x = pill.right() + PILL_GAP;
            (pill, galley)
        })
        .collect()
}

/// Draw a tab row in `row`: `tabs` (panel id, label) as capsule pills from the left, the front one
/// lit (a brighter fill, semibold text). Tabs that do not fit move to the overflow menu;
/// an oversized active label is ellipsized. A click switches at once: the clicked pill is painted in front on the very
/// frame of the click. Tabs are dragged and double-clicked by these same pills.
pub(crate) fn tab_row(ui: &mut egui::Ui, tabs: &[(String, String)], active: usize, row: Rect, t: &Tokens) -> TabRow {
    let painter = ui.painter().clone();
    // The pills where they were drawn (with the old front tab) take the input…
    let before = pills(&painter, tabs, active, row, t);
    let responses: Vec<egui::Response> = tabs
        .iter()
        .zip(&before)
        .map(|((panel, _), (pill, _))| {
            ui.interact(
                if pill.is_positive() {
                    Rect::from_min_max(pill.min, pill.max - vec2(option_width(panel), 0.0))
                } else {
                    Rect::from_min_size(row.min, Vec2::ZERO)
                },
                tab_id(panel),
                Sense::click_and_drag(),
            )
        })
        .collect();
    let mut clicked = responses.iter().position(|r| r.clicked()).filter(|i| *i != active);
    let hidden = before.iter().filter(|(r, _)| !r.is_positive()).count();
    let truncated = before.iter().any(|(_, galley)| galley.text().contains('…'));
    // `»` also stays when the only overflow is the active pill, which is drawn truncated.
    if hidden > 0 || truncated {
        let more = Rect::from_min_size(pos2(row.right() - 28.0, row.center().y - 14.0), vec2(28.0, 28.0));
        let response = ui.interact(more, ui.scope_id().with("tab-overflow"), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("More tabs")));
        crate::icons::paint(ui, "shell-chevrons-right", Rect::from_center_size(more.center(), vec2(16.0, 16.0)), t.text_dim);
        // A lone truncated active pill still shows `»`, with no "1 more panels" tip.
        let response = if hidden >= 1 {
            let tip = crate::i18n::trn(crate::i18n::current(), hidden as u64, "{n} more panel", "{n} more panels");
            response.on_hover_text(tip)
        } else {
            response
        };
        egui::Popup::menu(&response).show(|ui| {
            for (i, (_, label)) in tabs.iter().enumerate() {
                if before.get(i).is_some_and(|(pill, _)| pill.is_positive()) {
                    continue;
                }
                if ui.selectable_label(i == active, label).clicked() {
                    clicked = Some(i);
                    ui.close();
                }
            }
        });
    }
    let double_clicked = responses.iter().position(egui::Response::double_clicked);
    let drag_started = responses.iter().position(egui::Response::drag_started);
    // … and the row is painted with the front tab that input leaves.
    let front_tab = clicked.unwrap_or(active);
    let after = if clicked.is_some() { pills(&painter, tabs, front_tab, row, t) } else { before };
    let capsule = CornerRadius::same(6);
    for (i, ((pill, galley), response)) in after.iter().zip(&responses).enumerate() {
        if !pill.is_positive() {
            continue;
        }
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
        let panel = &tabs[i].0;
        let clip = Rect::from_min_max(pill.min, pill.max - vec2(option_width(panel), 0.0)).shrink2(vec2(4.0, 0.0)).intersect(painter.clip_rect());
        if let Some(key) = options_key(panel) {
            let rect = Rect::from_center_size(pos2(pill.right() - 9.0, pill.center().y), vec2(16.0, 16.0));
            let response = ui.interact(rect, tab_id(panel).with("options"), Sense::click());
            let hidden: bool = crate::panels::pstate(ui.ctx(), key);
            crate::icons::paint(ui, "shell-chevrons-right", rect, text);
            if response.on_hover_text(if hidden { tl!("Show Options") } else { tl!("Hide Options") }).clicked() {
                crate::panels::set_pstate(ui.ctx(), key, !hidden);
            }
        }
        painter.with_clip_rect(clip).galley(at, galley.clone(), text);
        response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, front, galley.text()));
    }
    TabRow { clicked, double_clicked, drag_started, tabs: after.into_iter().map(|(pill, _)| pill).collect() }
}

/// Panel `id`'s tab label: its library's name, or its translated name; another Properties editor
/// is numbered ("Properties 2").
pub(crate) fn label(app: &VectorcraftApp, id: &str) -> String {
    if id == "control" {
        return tl!("Control").to_string();
    }
    if super::persist::is_extra_instance(id) {
        let n = id.split_once('@').map_or("", |(_, n)| n);
        return format!("{} {n}", label(app, super::persist::instance_kind(id)));
    }
    crate::panels::library_panel::tab_label(app, id)
        .or_else(|| crate::state::all_panels().find(|p| p.0 == id).map(|p| tl!(p.1).to_string()))
        .unwrap_or_else(|| id.to_string())
}

/// Paint over the parts of `rect` outside its rounded corners in the seam's colour, then its
/// hairline: content drawn edge to edge (the canvas, a scroll bar) gets the box's rounded corners.
pub(crate) fn frame(_painter: &egui::Painter, _rect: Rect, _t: &Tokens) { /* docked surfaces are flat */
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
    /// Collapse box `0` to its tab row, or show it whole again (a double-click on a tab).
    Collapse(u32),
    /// Maximize box `0` (fill the central region), or restore the layout (a double-click on the
    /// tab row's empty space, or the ≡ menu).
    Maximize(u32),
    /// Another Properties editor beside Properties editor `0` (the ≡ menu's Duplicate Panel).
    Duplicate(String),
    /// Collapse box `0`'s tabs into the Board's strip as icons.
    ToIcon(u32),
}

/// Draw ordinary box `b` in `rect`: tab row, ≡, the front panel's body (none while it is collapsed).
/// Its controls go to `hits`. `floating`: a floating panel's box ([`super::floats`]), which draws
/// its own frame and publishes its own whole rect.
pub(crate) fn show(
    app: &mut VectorcraftApp,
    ui: &mut egui::Ui,
    b: &PanelBox,
    rect: Rect,
    floating: bool,
    hits: &mut Vec<(Hit, Rect)>,
) -> Option<Ask> {
    if !rect.is_positive() {
        return None;
    }
    let t = Tokens::get(ui.ctx());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, if floating { CornerRadius::same(RADIUS) } else { CornerRadius::ZERO }, t.panel);
    if !floating {
        hits.push((Hit::Box(b.id), rect));
    }
    let head = Rect::from_min_max(rect.min, pos2(rect.right(), (rect.top() + if floating { FLOAT_HEADER } else { HEADER }).min(rect.bottom())));
    let menu = Rect::from_min_size(pos2(head.right() - PAD - MENU.x, head.center().y - MENU.y / 2.0), MENU);
    let row = Rect::from_min_max(pos2(head.left() + PAD, head.top()), pos2((menu.left() - PAD).max(head.left() + PAD), head.bottom()));
    hits.push((Hit::TabRow(b.id), row));
    // The row's empty space (under the pills, which take their own input) maximizes the box.
    let row_response = ui.interact(row, egui::Id::unique("varos-shell-row").with(b.id), Sense::click());
    let tabs: Vec<(String, String)> = b.tabs.iter().map(|p| (p.clone(), label(app, p))).collect();
    let active = b.active.min(tabs.len().saturating_sub(1));
    let out = tab_row(ui, &tabs, active, row, &t);
    for ((panel, _), pill) in tabs.iter().zip(&out.tabs) {
        if pill.is_positive() {
            hits.push((Hit::Tab { box_id: b.id, panel: panel.clone() }, *pill));
        }
    }
    if let Some(i) = out.drag_started
        && let (Some((panel, _)), Some(pill)) = (tabs.get(i), out.tabs.get(i))
    {
        drag::begin(app, ui.ctx(), Item::Tab(panel.clone()), *pill);
    }
    // A click switches before the body is drawn: the new panel shows in the same frame.
    let active = match out.clicked {
        Some(i) => {
            app.ui.varos_shell.set_active(b.id, i);
            i
        }
        None => active,
    };
    let front = tabs.get(active).map(|(p, _)| p.clone());
    let mut ask = out.double_clicked.map(|_| Ask::Collapse(b.id));
    if !floating
        && row_response.double_clicked()
        && out.tabs.iter().all(|pill| !row_response.interact_pointer_pos().is_some_and(|p| pill.contains(p)))
    {
        ask = Some(Ask::Maximize(b.id));
    }
    if let Some(front) = &front {
        hits.push((Hit::Menu(b.id), menu));
        let extras = MenuExtras { duplicate: true, maximized: (!floating).then(|| app.ui.varos_shell.maximized_box() == Some(b.id)), to_icon: true };
        match panel_menu(app, ui, menu, b.id, front, &t, extras) {
            Some(MenuPick::Close) => ask = Some(Ask::Close(front.clone())),
            Some(MenuPick::Maximize) => ask = Some(Ask::Maximize(b.id)),
            Some(MenuPick::Duplicate) => ask = Some(Ask::Duplicate(front.clone())),
            Some(MenuPick::ToIcon) => ask = Some(Ask::ToIcon(b.id)),
            None => {}
        }
    }
    painter.hline((rect.left() + 1.0)..=(rect.right() - 1.0), head.bottom() - 0.5, Stroke::new(1.0, t.border));
    let inner = Rect::from_min_max(pos2(rect.left() + 1.0, head.bottom()), pos2(rect.right() - 1.0, rect.bottom() - 1.0));
    // A collapsed box is laid out as its tab row alone (one with nothing to take its room shows
    // whole), so the rect says whether the body shows.
    if let Some(front) = &front
        && rect.height() > HEADER + 0.5
        && inner.is_positive()
    {
        let mut child =
            ui.new_child(egui::UiBuilder::new().id_salt(("varos-shell-box", b.id)).max_rect(inner).layout(egui::Layout::top_down(Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        body(app, &mut child, front);
    }
    if !floating {
        frame(&painter, rect, &t);
    }
    ask
}

/// What the ≡ menu was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuPick {
    Close,
    Maximize,
    Duplicate,
    /// Every tab of the box goes back to the Board's strip as icons.
    ToIcon,
}

/// The Board's ≡ (at the top of its strip): Maximize Board, or Restore Layout while it is
/// maximized. True when chosen.
pub(crate) fn board_menu(ui: &mut egui::Ui, rect: Rect, maximized: bool, t: &Tokens) -> bool {
    let response = ui.interact(rect, egui::Id::unique("varos-shell-board-menu"), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect.shrink(3.0), CornerRadius::same(CONTROL_RADIUS), t.hover);
    }
    crate::icons::paint(
        ui,
        "ellipsis",
        Rect::from_center_size(rect.center(), vec2(14.0, 14.0)),
        if response.hovered() { t.text_strong } else { t.text_dim },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tl!("Board menu")));
    let response = response.on_hover_text(tl!("Board menu"));
    let mut chosen = false;
    egui::Popup::menu(&response).show(|ui| {
        ui.set_min_width(180.0);
        if crate::widgets::menu_item(ui, if maximized { "Restore Layout" } else { "Maximize Board" }, true, false) {
            chosen = true;
        }
    });
    chosen
}

/// What a ≡ menu offers besides the panel's own items and Close Panel.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MenuExtras {
    /// Duplicate Panel, for a Properties editor (a box's or a floating panel's; not a flyout's,
    /// which has no place to put the copy).
    pub duplicate: bool,
    /// Maximize Box, or Restore Layout while it is maximized (a box of the tree).
    pub maximized: Option<bool>,
    /// Collapse to icon: the box's tabs return to the Board's strip.
    pub to_icon: bool,
}

/// A flyout's ≡ button: the panel's own menu items (`panels::panel_menu_items`) and Close Panel;
/// true when Close Panel was chosen.
pub(crate) fn menu_button(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, box_id: u32, panel: &str, t: &Tokens) -> bool {
    panel_menu(app, ui, rect, box_id, panel, t, MenuExtras::default()) == Some(MenuPick::Close)
}

/// The ≡ button with `extras`.
fn panel_menu(app: &mut VectorcraftApp, ui: &mut egui::Ui, rect: Rect, box_id: u32, panel: &str, t: &Tokens, extras: MenuExtras) -> Option<MenuPick> {
    let response = ui.interact(rect, egui::Id::unique("varos-shell-menu").with(box_id), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(CONTROL_RADIUS), t.hover);
    }
    crate::icons::paint(
        ui,
        "ellipsis",
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        if response.hovered() { t.text_strong } else { t.text_dim },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tl!("Panel menu")));
    let response = response.on_hover_text(tl!("Panel menu"));
    let mut pick = None;
    egui::Popup::menu(&response).show(|ui| {
        crate::widgets::menu_scroll(ui, |ui| {
            ui.set_min_width(220.0);
            if crate::panels::panel_menu_items(app, ui, super::persist::instance_kind(panel)) {
                ui.separator();
            }
            if extras.duplicate
                && (super::persist::is_properties(panel) || super::persist::instance_kind(panel) == "control")
                && crate::widgets::menu_item(ui, "Duplicate Panel", true, false)
            {
                pick = Some(MenuPick::Duplicate);
            }
            if let Some(max) = extras.maximized
                && crate::widgets::menu_item(ui, if max { "Restore Layout" } else { "Maximize Box" }, true, false)
            {
                pick = Some(MenuPick::Maximize);
            }
            if extras.to_icon && crate::widgets::menu_item(ui, "Collapse to icon", true, false) {
                pick = Some(MenuPick::ToIcon);
            }
            if crate::widgets::menu_item(ui, "Close Panel", true, false) {
                pick = Some(MenuPick::Close);
            }
        });
    });
    pick
}

/// Panel `id`'s body filling `ui`: the main column's panels size themselves (Layers fills,
/// Properties scrolls); every other panel scrolls.
fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, id: &str) {
    let width = ui.available_width();
    let height = (ui.available_height() - 20.0).max(0.0);
    if DockTab::from_id(id).is_some() || super::persist::is_properties(id) {
        panel_body(app, ui, id, width, height);
    } else {
        crate::widgets::panel_scroll(ui, egui::ScrollArea::vertical().id_salt(("varos-shell-body", id)).auto_shrink([false, false]), |ui| {
            let width = ui.available_width();
            panel_body(app, ui, id, width, height)
        });
    }
}

/// Panel `id`'s contents `width` wide (`dock::panel_body`, the stable host API), but a Properties
/// editor is drawn as one: its sections (collapsed, in its order), its pin
/// (`panels::properties::show_editor`), with its own view.
pub(crate) fn panel_body(app: &mut VectorcraftApp, ui: &mut egui::Ui, id: &str, width: f32, tall: f32) {
    ui.scope_builder(egui::UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag("shell-panel")), |ui| {
        panel_contents(app, ui, id, width, tall);
    });
}

fn panel_contents(app: &mut VectorcraftApp, ui: &mut egui::Ui, id: &str, width: f32, tall: f32) {
    ui.spacing_mut().scroll = egui::style::ScrollStyle { bar_width: 4.0, floating_width: 2.0, ..egui::style::ScrollStyle::floating() };
    let style = ui.style_mut();
    style.text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(12.0));
    style.text_styles.insert(egui::TextStyle::Button, egui::FontId::new(11.0, egui::FontFamily::Name("ui-medium".into())));
    style.text_styles.insert(egui::TextStyle::Heading, crate::theme::semibold(14.0));
    if id == "color" {
        crate::panels::show_icon_panel(app, ui, id);
        return;
    }
    if super::persist::instance_kind(id) == "control" {
        crate::chrome::control_panel(app, ui);
        return;
    }
    if !super::persist::is_properties(id) {
        crate::dock::panel_body_frameless(app, ui, id, width, tall);
        return;
    }
    let _profile = crate::varos::profile::scope(super::persist::PROPERTIES);
    // Hosts reserve the legacy 20-point vertical margin; this editor has 32.
    let tall = (tall - 12.0).max(0.0);
    let stored = app.ui.varos_shell.views.get(id).cloned();
    let mut view = stored.clone().unwrap_or_default();
    egui::Frame::NONE.inner_margin(egui::Margin::same(16)).show(ui, |ui| {
        ui.set_width((width - 32.0).max(0.0));
        // As the Properties panel in the dock: at least `tall` when the sections are taller.
        crate::widgets::panel_scroll(
            ui,
            egui::ScrollArea::vertical().id_salt(("props-editor", id)).max_height(tall).min_scrolled_height(tall).auto_shrink([false, true]),
            |ui| crate::panels::properties::show_editor(app, ui, id, &mut view),
        );
    });
    // Kept only when it differs from what is kept (a view that was never changed stays unsaved),
    // and only for an editor that still has an owner.
    if stored.as_ref().unwrap_or(&Default::default()) != &view && app.ui.varos_shell.owns(id) {
        app.ui.varos_shell.views.insert(id.to_string(), view);
    }
}
