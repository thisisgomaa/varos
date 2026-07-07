//! Panel registry — the named panels the box tree can host, plus their content.
//! For the SANDBOX the bodies are DUMMY (fake rows) — the ids / titles / min-sizes are real and
//! survive into Stage 4, where only the bodies get replaced by the live panels. (BOX_SYSTEM_PLAN §4.2)
use super::tokens as T;
use egui::{vec2, Align2, FontId, RichText, Sense, StrokeKind, Vec2};

/// Every panel the shell knows. `Board` is special (its body is the canvas + the two floating hands,
/// drawn by `boxtree`); the rest are dockable dummy panels for the sandbox.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum PanelId {
    Board,
    Align,
    Pathfinder,
    Properties,
    Layers,
    Swatches,
    History,
    Assets,
}

impl PanelId {
    /// The panels the Window / box ⌄ menu can dock. Swatches/History/Assets are NOT here yet — they are
    /// unbuilt (sandbox dummies), so they must not appear as choices until they have real bodies (Ahmed
    /// 07-08). The enum variants stay, ready to re-list the moment each is built for real.
    pub const DOCKABLE: [PanelId; 4] = [PanelId::Align, PanelId::Pathfinder, PanelId::Properties, PanelId::Layers];

    pub fn title(self) -> &'static str {
        match self {
            PanelId::Board => "Board",
            PanelId::Align => "Align",
            PanelId::Pathfinder => "Pathfinder",
            PanelId::Properties => "Properties",
            PanelId::Layers => "Layers",
            PanelId::Swatches => "Swatches",
            PanelId::History => "History",
            PanelId::Assets => "Assets",
        }
    }

    pub fn is_board(self) -> bool {
        matches!(self, PanelId::Board)
    }

    /// Panels that manage their OWN vertical scrolling (a pinned footer / an internal list) — the
    /// box must not wrap them in a second ScrollArea (nested scrolls fight over the wheel).
    pub fn self_scrolling(self) -> bool {
        matches!(self, PanelId::Board | PanelId::Layers)
    }

    /// Minimum content size hint (points). Used by the tree for min-size clamps (BOX_SYSTEM_PLAN §3.5).
    pub fn min_size(self) -> Vec2 {
        if self.is_board() {
            vec2(240.0, 180.0)
        } else {
            vec2(190.0, 120.0)
        }
    }
}

/// Render a panel's body into the given Ui (the box has already painted its own background + header).
pub fn render_panel(id: PanelId, ui: &mut egui::Ui) {
    match id {
        PanelId::Board => {} // drawn by boxtree (bg + dot grid + hands)
        PanelId::Align => align_panel(ui),
        PanelId::Pathfinder => pathfinder_panel(ui),
        PanelId::Properties => properties_panel(ui),
        PanelId::Layers => layers_panel(ui),
        PanelId::Swatches => swatches_panel(ui),
        PanelId::History => history_panel(ui),
        PanelId::Assets => assets_panel(ui),
    }
}

// ───────────────────────── tiny hand-painted fake widgets (no egui default widgets) ─────────────────────────

fn micro(ui: &mut egui::Ui, s: &str) {
    ui.add_space(3.0);
    ui.label(RichText::new(s).color(T::FAINT).size(9.5).strong());
    ui.add_space(4.0);
}

/// A SURFACE inset field with an optional letter label + a mono value (the mockup `.fld`, h25).
fn fake_field(ui: &mut egui::Ui, letter: &str, value: &str, w: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, 25.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, T::r_ctrl(), T::SURFACE, T::hairline(), StrokeKind::Middle);
    let mut x = rect.left() + 7.0;
    if !letter.is_empty() {
        p.text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, letter, FontId::proportional(9.5), T::FAINT);
        x += 13.0;
    }
    p.text(egui::pos2(x, rect.center().y), Align2::LEFT_CENTER, value, FontId::monospace(11.0), T::TEXT);
}

/// A 26×24 hand-painted icon button (glyph placeholder — real Lucide icons arrive in Stage 4).
fn icon_btn(ui: &mut egui::Ui, glyph: &str, on: bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 24.0), Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(rect, T::r_ctrl(), T::HOVER);
    } else if resp.hovered() {
        p.rect_filled(rect, T::r_ctrl(), T::VOID_HOVER);
    }
    let col = if on || resp.hovered() { T::TEXT } else { T::MUTED };
    p.text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::proportional(13.0), col);
}

fn swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, r2(), color, egui::Stroke::new(1.0, T::LINE2), StrokeKind::Middle);
}
fn r2() -> egui::CornerRadius {
    egui::CornerRadius::same(2)
}

// ───────────────────────── the dummy panels ─────────────────────────

fn align_panel(ui: &mut egui::Ui) {
    micro(ui, "ALIGN OBJECTS");
    ui.horizontal(|ui| {
        for g in ["⇤", "⇔", "⇥", "⤒", "⇕", "⤓"] {
            icon_btn(ui, g, false);
            ui.add_space(2.0);
        }
    });
    ui.add_space(8.0);
    micro(ui, "DISTRIBUTE");
    ui.horizontal(|ui| {
        for g in ["≡", "≣", "⋮", "⋯"] {
            icon_btn(ui, g, false);
            ui.add_space(2.0);
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Align to").color(T::MUTED).size(11.5));
        fake_field(ui, "", "Artboard        ⌄", 130.0);
    });
}

fn pathfinder_panel(ui: &mut egui::Ui) {
    micro(ui, "SHAPE MODES");
    ui.horizontal(|ui| {
        for g in ["◕", "◑", "◔", "◒"] {
            icon_btn(ui, g, false);
            ui.add_space(3.0);
        }
    });
    ui.add_space(8.0);
    micro(ui, "PATHFINDERS");
    ui.horizontal(|ui| {
        for g in ["⬒", "⬓", "⬔", "⬕"] {
            icon_btn(ui, g, false);
            ui.add_space(3.0);
        }
    });
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        icon_btn(ui, "⊙", true);
        ui.label(RichText::new("Shape Builder").color(T::MUTED).size(11.5));
    });
}

fn properties_panel(ui: &mut egui::Ui) {
    // collapsible section homes (Stage 3 "collapse states" — instant, animation_time = 0)
    section(ui, "transform", "TRANSFORM", |ui| {
        ui.horizontal(|ui| {
            fake_field(ui, "X", "266", 84.0);
            ui.add_space(6.0);
            fake_field(ui, "Y", "118", 84.0);
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            fake_field(ui, "W", "126", 84.0);
            ui.add_space(6.0);
            fake_field(ui, "H", "64", 84.0);
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            fake_field(ui, "∠", "0°", 84.0);
            ui.add_space(6.0);
            icon_btn(ui, "⤢", false);
            icon_btn(ui, "⤡", false);
            icon_btn(ui, "⟳", false);
        });
    });
    section(ui, "appearance", "APPEARANCE", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Fill").color(T::MUTED).size(11.5));
            swatch(ui, T::AMBER);
            fake_field(ui, "", "F0B429", 92.0);
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Stroke").color(T::MUTED).size(11.5));
            swatch(ui, T::NONE_RED);
            fake_field(ui, "", "None", 92.0);
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Opacity").color(T::MUTED).size(11.5));
            fake_field(ui, "", "85 %", 72.0);
        });
    });
    section(ui, "shape", "SHAPE", |ui| {
        ui.horizontal(|ui| {
            for g in ["◐", "◑", "◒", "◓"] {
                icon_btn(ui, g, false);
                ui.add_space(3.0);
            }
            icon_btn(ui, "⊙", true);
        });
    });
}

/// A collapsible section home: a clickable header (chevron + micro-label) that toggles its body.
/// Open-state persists in egui memory; toggling is INSTANT (no animation). Used by Properties.
fn section(ui: &mut egui::Ui, key: &str, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let id = egui::Id::new(("shell.sec", key));
    let mut open = ui.ctx().data_mut(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    ui.add_space(3.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click());
    if resp.clicked() {
        open = !open;
        ui.ctx().data_mut(|d| d.insert_temp(id, open));
    }
    let p = ui.painter();
    // hand-painted disclosure triangle — the "⌄"/"›" glyphs were tofu (☐) in the default font (Ahmed 07-05).
    let (cx, cy) = (rect.left() + 4.0, rect.center().y);
    let tri = if open {
        vec![egui::pos2(cx - 4.0, cy - 2.0), egui::pos2(cx + 4.0, cy - 2.0), egui::pos2(cx, cy + 3.0)]
    } else {
        vec![egui::pos2(cx - 2.0, cy - 4.0), egui::pos2(cx - 2.0, cy + 4.0), egui::pos2(cx + 3.0, cy)]
    };
    p.add(egui::Shape::convex_polygon(tri, if resp.hovered() { T::MUTED } else { T::FAINT }, egui::Stroke::NONE));
    p.text(
        egui::pos2(rect.left() + 15.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(9.5),
        if resp.hovered() { T::MUTED } else { T::FAINT },
    );
    if open {
        ui.add_space(4.0);
        body(ui);
    }
    ui.add_space(4.0);
}

fn layers_panel(ui: &mut egui::Ui) {
    for (name, depth) in [("Logo mark", 0), ("Circle", 1), ("Star", 1), ("Text · VAROS", 0), ("Background", 0)] {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, T::r_ctrl(), T::VOID_HOVER);
        }
        let x0 = rect.left() + 8.0 + depth as f32 * 14.0;
        p.text(egui::pos2(x0, rect.center().y), Align2::LEFT_CENTER, "◉", FontId::proportional(12.0), T::MUTED); // eye
        p.rect(
            egui::Rect::from_min_size(egui::pos2(x0 + 18.0, rect.center().y - 8.0), vec2(16.0, 16.0)),
            r2(),
            T::SURFACE,
            T::hairline(),
            StrokeKind::Middle,
        ); // thumb
        p.text(egui::pos2(x0 + 42.0, rect.center().y), Align2::LEFT_CENTER, name, FontId::proportional(12.0), T::TEXT);
    }
}

fn swatches_panel(ui: &mut egui::Ui) {
    micro(ui, "SWATCHES");
    let cols = [T::AMBER, T::NAVY, T::ACCENT, T::GUIDE, T::TEXT, T::NONE_RED, T::MUTED, T::LINE2];
    ui.horizontal_wrapped(|ui| {
        for c in cols {
            swatch(ui, c);
            ui.add_space(4.0);
        }
    });
}

fn history_panel(ui: &mut egui::Ui) {
    micro(ui, "HISTORY");
    for (i, step) in ["Open document", "Draw ellipse", "Fill amber", "Add text", "Move ×3"].iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        let on = i == 3;
        let p = ui.painter();
        if on {
            p.rect_filled(rect, T::r_ctrl(), T::SURFACE);
        } else if resp.hovered() {
            p.rect_filled(rect, T::r_ctrl(), T::VOID_HOVER);
        }
        p.text(
            egui::pos2(rect.left() + 10.0, rect.center().y),
            Align2::LEFT_CENTER,
            *step,
            FontId::proportional(11.5),
            if on { T::TEXT } else { T::MUTED },
        );
    }
}

fn assets_panel(ui: &mut egui::Ui) {
    micro(ui, "ASSETS");
    ui.horizontal_wrapped(|ui| {
        for _ in 0..6 {
            let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
            ui.painter().rect(rect, T::r_ctrl(), T::SURFACE, T::hairline(), StrokeKind::Middle);
            ui.add_space(6.0);
        }
    });
}
