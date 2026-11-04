//! Presentation only: these entries have no command identity or action callback.
use super::theme_components::FOUNDATION as F;
use crate::theme::Tokens;
use egui::{Align2, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

#[derive(Clone, Copy)]
pub(crate) struct Item {
    pub label: &'static str,
    pub tooltip: &'static str,
    pub icon: &'static str,
}
impl Item {
    pub fn hover(self) -> String {
        format!("{} — Soon\n{}", self.label.trim_end_matches('…'), self.tooltip)
    }
}

// Copy supplied by the approved roadmap table; the four draft descriptions retain
// the approved mockup legend wording.
pub(crate) const ITEMS: &[Item] = &[
    Item {
        label: "Chat with Codex / Claude Code",
        tooltip: "Type what you want; your own Codex or Claude Code edits this board.",
        icon: "message-circle",
    },
    Item { label: "Chat with your own API key", tooltip: "For people without a CLI: a chat model you pay for runs Varos commands.", icon: "lock" },
    Item { label: "Project memory", tooltip: "The AI remembers this project's decisions, saved inside the project.", icon: "library" },
    Item { label: "Recipes & quick scripts", tooltip: "Ready jobs (number 50 artboards, batch export) that cost almost no tokens.", icon: "menu" },
    Item { label: "Agents on your server", tooltip: "An agent on your VPS works on this same project while you are away.", icon: "monitor" },
    Item {
        label: "Build an extension with AI",
        tooltip: "A catalogue that teaches your AI how to write and install a Varos extension.",
        icon: "shapes",
    },
    Item { label: "Generate images", tooltip: "Use any image model with your own key; no subscription.", icon: "wand-sparkles" },
    Item { label: "Several versions", tooltip: "Get a few options at once and keep the one you like.", icon: "copy" },
    Item { label: "Styles", tooltip: "Pick a style and apply it to new images.", icon: "palette" },
    Item { label: "Find references", tooltip: "The AI searches for references and brings them onto your board.", icon: "search" },
    Item { label: "Edit with a prompt", tooltip: "Select an image or shape, describe the change; the result is placed beside it.", icon: "pencil" },
    Item { label: "Expand image", tooltip: "Extend an image beyond its edges.", icon: "maximize-2" },
    Item { label: "Remove object", tooltip: "Paint over something and the AI removes it.", icon: "eraser" },
    Item { label: "Remove background", tooltip: "Cut the subject out of its background.", icon: "scissors" },
    Item { label: "Upscale", tooltip: "Make a small image larger and sharper with your own image model.", icon: "scaling" },
    Item { label: "Adjustments", tooltip: "Light and colour for photos in one short panel, Camera Raw style.", icon: "settings" },
    Item { label: "Auto merge", tooltip: "Merge several photos into one: a panorama or one clean shot.", icon: "layers" },
    Item { label: "History per AI", tooltip: "See what each AI did and go back to before any run.", icon: "history" },
    Item { label: "Undo this run", tooltip: "Undo everything one AI run did in one click.", icon: "undo-2" },
    Item { label: "Image providers & keys", tooltip: "Add your own key; stored in a private file on this Mac, never in your files.", icon: "lock" },
    Item { label: "Share", tooltip: "Open this project in the browser or send a link to a teammate.", icon: "" },
    Item { label: "Dieline", tooltip: "Cut, crease, bleed and safe lines that the printer and the cutter understand.", icon: "" },
    Item { label: "Make Dieline…", tooltip: "Cut, crease, bleed and safe lines that the printer and the cutter understand.", icon: "" },
    Item { label: "Extension Library…", tooltip: "Free extensions from the community; cutter exporters come first.", icon: "" },
    Item { label: "3D Box Preview", tooltip: "Fold your dieline into a box and turn it around before you print.", icon: "" },
    Item { label: "Comments", tooltip: "Leave notes on the board for teammates and agents.", icon: "" },
    Item { label: "Preflight", tooltip: "Check low-res images, missing fonts, RGB in CMYK and ink limits before printing.", icon: "" },
    Item { label: "Print rules", tooltip: "Bleed, safe area, inks and minimum ppi saved in the file, so agents follow them too.", icon: "" },
    Item { label: "Choose press condition", tooltip: "Pick FOGRA / GRACoL and get guided to install the right profile.", icon: "" },
    Item { label: "Shared with me", tooltip: "Projects your team or your agents shared with you.", icon: "home-users" },
    Item { label: "Join with a code", tooltip: "Open a project someone shared with you.", icon: "" },
    Item { label: "Box templates", tooltip: "Start from a box style and type its size.", icon: "" },
    Item { label: "Brand guide template", tooltip: "A multi-page identity guide with colour and type pages made for you.", icon: "" },
    Item {
        label: "Copy placed images into the project folder",
        tooltip: "Every image you place is copied into this project's folder and stays linked.",
        icon: "",
    },
];

pub(crate) fn get(label: &str) -> Option<Item> {
    ITEMS.iter().find(|item| item.label == label).copied()
}
pub(crate) fn hover(label: &str) -> String {
    get(label).map_or_else(|| format!("{label} — Soon"), Item::hover)
}
pub(crate) fn tag(ui: &Ui, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_stroke(rect, F.control_radius, Stroke::new(F.soon_stroke, t.button_border), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Soon", FontId::proportional(F.badge_font), t.text_disabled);
}

/// A disabled response (including accessibility/focus), painted outside egui's disabled
/// opacity scope so the approved tertiary ink is not dimmed a second time.
pub(crate) fn row(ui: &mut Ui, label: &str, icon: bool, fill_width: bool) -> Response {
    row_style(ui, label, icon, fill_width, F.panel_body_font, F.soon_row_height, false, false)
}
pub(crate) fn menu_item(ui: &mut Ui, label: &str) -> Response {
    row_style(ui, label, false, false, F.legacy_control_font, F.control_height, false, label == "Dieline")
}
pub(crate) fn button(ui: &mut Ui, label: &str) -> Response {
    row_style(ui, label, false, false, F.control_font, F.control_height, true, false)
}
pub(crate) fn chat_input(ui: &mut Ui, width: f32) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(t.input).stroke(Stroke::new(F.soon_stroke, t.input_border)).corner_radius(F.control_radius).show(ui, |ui| {
        ui.set_width(width.max(0.0));
        row_style(ui, "Chat with Codex / Claude Code", false, true, F.control_font, F.soon_row_height, false, false);
    });
}
#[allow(clippy::too_many_arguments)]
fn row_style(ui: &mut Ui, label: &str, icon: bool, fill_width: bool, font: f32, height: f32, filled: bool, chevron: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let icon = get(label).map_or("", |item| if icon { item.icon } else { "" });
    let font = FontId::proportional(font);
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, t.text_disabled);
    let icon_space = if icon.is_empty() { 0.0 } else { F.soon_icon + F.soon_gap };
    let arrow_space = if chevron { F.soon_icon + F.soon_gap } else { 0.0 };
    let width = galley.size().x + icon_space + F.soon_tag_size.x + F.soon_gap * 3.0 + arrow_space;
    let width = if fill_width { width.max(ui.available_width()) } else { width };
    let r = ui.add_enabled_ui(false, |ui| ui.allocate_exact_size(vec2(width, height), Sense::click()).1).inner;
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, label));
    if filled || r.contains_pointer() {
        ui.painter().rect_filled(r.rect, F.control_radius, if r.contains_pointer() { t.hover } else { t.button });
    }
    let x = r.rect.left() + F.soon_gap;
    if !icon.is_empty() {
        crate::icons::paint(
            ui,
            icon,
            Rect::from_center_size(pos2(x + F.soon_icon * 0.5, r.rect.center().y), vec2(F.soon_icon, F.soon_icon)),
            t.text_disabled,
        );
    }
    ui.painter().galley(pos2(x + icon_space, r.rect.center().y - galley.size().y * 0.5), galley, t.text_disabled);
    tag(ui, Rect::from_center_size(pos2(r.rect.right() - arrow_space - F.soon_gap - F.soon_tag_size.x * 0.5, r.rect.center().y), F.soon_tag_size));
    if chevron {
        crate::icons::paint(
            ui,
            "chevron-right",
            Rect::from_center_size(pos2(r.rect.right() - F.soon_gap - F.soon_icon * 0.5, r.rect.center().y), vec2(F.soon_icon, F.soon_icon)),
            t.text_disabled,
        );
        r
    } else {
        r.on_disabled_hover_text(hover(label))
    }
}

/// Only decorations are added; existing items, their relative order and targets are retained.
pub(crate) fn menus(mut tree: Vec<(&'static str, Vec<crate::menus::Item>)>) -> Vec<(&'static str, Vec<crate::menus::Item>)> {
    use crate::menus::Item as M;
    fn before(items: &mut Vec<M>, anchor: &str, item: M) {
        if let Some(i) = items.iter().position(|item| matches!(item, M::Cmd(l, ..) | M::Sub(l, _) if *l == anchor)) {
            items.insert(i, item);
        }
    }
    for (title, items) in &mut tree {
        match *title {
            "Object" => {
                before(items, "Flatten Transparency…", M::Sub("Dieline", vec![M::Todo("Make Dieline…", "")]));
                if let Some(M::Sub(_, children)) = items.iter_mut().find(|item| matches!(item, M::Sub("Plug-ins", _))) {
                    children.extend([M::Sep, M::Todo("Extension Library…", "")]);
                }
            }
            "Window" => {
                before(items, "Actions", M::Todo("3D Box Preview", ""));
                before(items, "Align", M::Cmd("AI", "window.panel", serde_json::json!({"panel":"ai", "toggle":true})));
                before(items, "CSS Properties", M::Todo("Comments", ""));
                before(items, "Properties", M::Todo("Preflight", ""));
            }
            _ => {}
        }
    }
    tree
}

pub(crate) fn settings(ui: &mut Ui) {
    crate::widgets::section_header(ui, "Agents on this Mac");
    ui.label("Codex, Claude Code and other agents you run on this Mac can open, edit and save your boards through Bridge. No sign-in, no pairing; every change is one undo step.");
    ui.add_space(F.tab_padding);
    crate::widgets::section_header(ui, "Images");
    row(ui, "Image providers & keys", true, true);
    ui.label(egui::RichText::new(get("Image providers & keys").map_or("", |item| item.tooltip)).color(Tokens::get(ui.ctx()).text_disabled));
}

pub(crate) fn submenu(ui: &mut Ui, children: &[crate::menus::Item]) {
    let r = menu_item(ui, "Dieline");
    // Automatic disabled tooltips are suppressed by an open popup in the parent's layer.
    // This presentation-only tooltip must remain available while its child is visible.
    if r.contains_pointer() {
        egui::Tooltip::for_widget(&r).show(|ui| {
            ui.label(hover("Dieline"));
        });
    }
    let id = r.id.with("soon-submenu-bounds");
    let previous = ui.ctx().data(|data| data.get_temp::<Rect>(id));
    let open =
        r.contains_pointer() || previous.is_some_and(|rect| ui.ctx().pointer_hover_pos().is_some_and(|pos| rect.expand(F.soon_gap).contains(pos)));
    let popup = egui::Popup::from_response(&r)
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(egui::containers::menu::menu_style)
        .align(egui::RectAlign::RIGHT_START)
        .open(open)
        .show(|ui| {
            crate::widgets::menu_scroll(ui, |ui| {
                ui.set_min_width(F.soon_submenu_min_width);
                for item in children {
                    if let crate::menus::Item::Todo(label, _) = item {
                        menu_item(ui, label);
                    }
                }
            });
        });
    ui.ctx().data_mut(|data| {
        if let Some(popup) = popup {
            data.insert_temp(id, popup.response.rect);
        } else {
            data.remove::<Rect>(id);
        }
    });
}

pub(crate) fn press_condition(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Press:").color(Tokens::get(ui.ctx()).text_disabled));
        row(ui, "Choose press condition", false, false);
    });
}

pub(crate) fn document_tabs(ui: &mut Ui, tabs: &[&str], current: usize) -> Option<usize> {
    ui.horizontal(|ui| {
        let changed = crate::widgets::tab_bar(ui, tabs, current);
        row(ui, "Print rules", false, false);
        changed
    })
    .inner
}

/// Uses panel-tab surface roles, including the neutral active well from frame A.
pub(crate) fn panel_tabs(ui: &mut Ui, tabs: &[&str], current: usize) -> Option<usize> {
    let mut changed = None;
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in tabs.iter().enumerate() {
            let width = ui.painter().layout_no_wrap((*label).into(), F.tab_font(), t.text).size().x + F.tab_padding * 2.0;
            let (rect, r) = ui.allocate_exact_size(vec2(width, F.tab_height), Sense::click());
            if i == current || r.hovered() {
                ui.painter().rect_filled(rect, F.control_radius, t.hover);
            }
            F.tab_label(ui.painter(), rect, label, &t, i == current, r.hovered());
            r.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, i == current, *label));
            if r.clicked() {
                changed = Some(i);
            }
        }
    });
    changed
}

pub(crate) fn template_card(ui: &mut Ui, label: &str, width: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let height = width * 0.75;
    let r = ui.add_enabled_ui(false, |ui| ui.allocate_exact_size(vec2(width, height + F.soon_row_height * 2.0), Sense::click()).1).inner;
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, false, label));
    let thumb = Rect::from_min_size(r.rect.min, vec2(width, height));
    ui.painter().rect_filled(thumb, F.card_radius, t.panel);
    ui.painter().rect_stroke(thumb, F.card_radius, Stroke::new(F.soon_stroke, t.hover), StrokeKind::Inside);
    // Original geometric diagrams from frame D, never a simulated template preview.
    let origin = thumb.center() - vec2(60.0, 40.0);
    let stroke = Stroke::new(F.soon_stroke, t.button_border);
    let p = ui.painter();
    let outline = |x, y, w, h| {
        p.rect_stroke(Rect::from_min_size(origin + vec2(x, y), vec2(w, h)), F.tab_radius, stroke, StrokeKind::Inside);
    };
    if label == "Box templates" {
        for (x, y, w, h) in [
            (10.0, 25.0, 25.0, 30.0),
            (35.0, 25.0, 30.0, 30.0),
            (65.0, 25.0, 25.0, 30.0),
            (90.0, 25.0, 22.0, 30.0),
            (35.0, 5.0, 30.0, 20.0),
            (35.0, 55.0, 30.0, 20.0),
        ] {
            outline(x, y, w, h);
        }
    } else {
        outline(14.0, 10.0, 42.0, 60.0);
        outline(64.0, 10.0, 42.0, 60.0);
        for (x, y, w) in [(20.0, 20.0, 20.0), (20.0, 26.0, 28.0), (70.0, 20.0, 14.0), (20.0, 56.0, 30.0)] {
            p.line_segment([origin + vec2(x, y), origin + vec2(x + w, y)], stroke);
        }
        for x in [76.0, 90.0] {
            p.circle_stroke(origin + vec2(x, 42.0), F.soon_gap, stroke);
        }
    }
    let label_font = FontId::proportional(F.control_font);
    let label_galley = p.layout_no_wrap(label.into(), label_font, t.text_disabled);
    let y = thumb.bottom() + F.soon_gap;
    let tag_x = (r.rect.left() + label_galley.size().x + F.soon_gap).min(r.rect.right() - F.soon_tag_size.x);
    p.galley(pos2(r.rect.left(), y), label_galley, t.text_disabled);
    tag(ui, Rect::from_min_size(pos2(tag_x, y), F.soon_tag_size));
    let detail = get(label).map_or("", |item| item.tooltip);
    let detail = p.layout(detail.into(), FontId::proportional(F.control_font), t.text_disabled, width);
    p.galley(pos2(r.rect.left(), y + F.soon_tag_size.y + F.soon_gap), detail, t.text_disabled);
    r.on_disabled_hover_text(hover(label))
}

#[cfg(test)]
#[path = "tests_soon.rs"]
mod tests;

/// Native system menus retain system typography; they cannot use an egui tag painter.
pub(crate) fn native_row_label(row: &crate::menus::Row<'_>) -> String {
    if row.command.is_none() && get(row.source).is_some() { format!("{} — Soon", row.label) } else { row.label.to_string() }
}
pub(crate) fn native_submenu_label(label: &str, children: &[crate::menus::Item]) -> String {
    if label == "Dieline" && children.iter().all(|item| matches!(item, crate::menus::Item::Todo("Make Dieline…", ""))) {
        format!("{label} — Soon")
    } else {
        label.to_string()
    }
}
pub(crate) fn native_tooltip(node: &crate::native_menu::Node) -> Option<String> {
    use crate::native_menu::Node;
    match node {
        Node::Item(item) if item.command.is_none() => get(item.source).map(Item::hover),
        Node::Submenu { label, children }
            if label == "Dieline — Soon" && children.iter().all(|child| matches!(child,Node::Item(item) if item.command.is_none())) =>
        {
            Some(hover("Dieline"))
        }
        _ => None,
    }
}
