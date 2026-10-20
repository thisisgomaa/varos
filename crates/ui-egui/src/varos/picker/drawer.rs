// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! The swatch row: nine recent colours (16 × 16, 3 pt corners) and ▾, which opens the drawer
//! (Recent · Document: the document's colour swatches). A click on a chip sets the focused target
//! as one undo step. The recent colours and the drawer's state are kept with the picker's
//! preferences, so they come back after a restart.

use egui::{Pos2, Rect, Sense, Ui, vec2};
use serde::{Deserialize, Serialize};
use vectorcraft_color::{Color, Paint};

use super::{Action, PAD, Prefs};
use crate::VectorcraftApp;
use crate::theme::Tokens;

pub(crate) const ROW_H: f32 = 28.0;
pub(crate) const DRAWER_H: f32 = 100.0;
const SWATCH: f32 = 16.0;
const GAP: f32 = 4.0;
const SHOWN: usize = 9;
/// How many recent colours the preferences keep (the row's nine and the drawer's rest).
pub(crate) const KEEP: usize = 18;

/// The drawer's tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawerTab {
    #[default]
    Recent,
    Document,
}

/// Fold the session's recent colours (newest first) into the kept ones. Cheap when nothing is new.
pub(crate) fn sync_recent(app: &mut VectorcraftApp) {
    let session = &app.session.recent_colors;
    let kept = &app.ui.varos_picker.recent;
    if session.is_empty() || (session.len() <= kept.len() && session.iter().zip(kept).all(|(c, k)| c.to_hex() == *k)) {
        return;
    }
    let mut merged: Vec<String> = session.iter().map(Color::to_hex).collect();
    for k in kept {
        if !merged.contains(k) {
            merged.push(k.clone());
        }
    }
    merged.truncate(KEEP);
    app.ui.varos_picker.recent = merged;
}

/// The recent colours: the session's (in their own model), then the kept ones from before.
pub(crate) fn recent(app: &VectorcraftApp) -> Vec<Color> {
    let mut out: Vec<Color> = app.session.recent_colors.clone();
    for k in &app.ui.varos_picker.recent {
        if out.len() >= KEEP {
            break;
        }
        if let Some(c) = Color::from_hex(k.trim_start_matches('#'))
            && !out.iter().any(|o| o.to_hex() == c.to_hex())
        {
            out.push(c);
        }
    }
    out
}

/// The document's solid colour swatches.
fn document(app: &VectorcraftApp) -> Vec<Color> {
    let Some(st) = app.session.active() else { return vec![] };
    st.doc
        .swatches_iter()
        .filter_map(|s| match &s.paint {
            Paint::Solid { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

/// The swatch row at `origin` and, when open, the drawer below it. Returns the height drawn.
pub(super) fn show(app: &VectorcraftApp, ui: &mut Ui, origin: Pos2, prefs: &mut Prefs, acts: &mut Vec<Action>) -> f32 {
    let t = Tokens::get(ui.ctx());
    let row = Rect::from_min_size(origin, vec2(super::W, ROW_H));
    ui.painter().hline(row.x_range(), row.top(), egui::Stroke::new(1.0, t.divider));
    let recent = recent(app);
    for i in 0..SHOWN {
        let r = Rect::from_min_size(row.min + vec2(PAD + i as f32 * (SWATCH + GAP), (ROW_H - SWATCH) / 2.0), egui::Vec2::splat(SWATCH));
        let c = recent.get(i);
        let resp = ui.interact(r, ui.id().with(("varos-picker-recent", i)), Sense::click());
        super::chip(ui, r, c, resp.hovered() && c.is_some());
        if let Some(c) = c
            && resp.on_hover_text(c.to_hex().to_uppercase()).clicked()
        {
            acts.push(Action::Set(*c));
        }
    }
    let r = Rect::from_center_size(egui::pos2(row.right() - PAD - 8.0, row.center().y), egui::Vec2::splat(16.0));
    let resp = ui.interact(r, ui.id().with("varos-picker-drawer"), Sense::click());
    let ink = if prefs.drawer_open || resp.hovered() { t.text } else { t.text_dim };
    super::icons::paint(ui, "chevron-down", r, ink);
    if resp.on_hover_text(tl!("Recent Colors")).clicked() {
        prefs.drawer_open = !prefs.drawer_open;
    }
    if !prefs.drawer_open {
        return ROW_H;
    }
    let drawer = Rect::from_min_size(row.left_bottom(), vec2(super::W, DRAWER_H));
    ui.scope_builder(egui::UiBuilder::new().max_rect(drawer.shrink2(vec2(PAD, 2.0))).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
        ui.set_clip_rect(drawer);
        ui.horizontal(|ui| {
            for (tab, label) in [(DrawerTab::Recent, tl!("Recent")), (DrawerTab::Document, tl!("Document"))] {
                let on = prefs.drawer_tab == tab;
                let text = egui::RichText::new(label).size(11.5).color(if on { t.text } else { t.text_dim });
                let resp = ui.add(egui::Button::new(text).frame(false).selected(on));
                if on {
                    ui.painter().hline(resp.rect.x_range(), resp.rect.bottom(), egui::Stroke::new(1.0, t.text));
                }
                if resp.clicked() {
                    prefs.drawer_tab = tab;
                }
            }
        });
        let colours = match prefs.drawer_tab {
            DrawerTab::Recent => recent,
            DrawerTab::Document => document(app),
        };
        egui::ScrollArea::vertical().id_salt("varos-picker-drawer-scroll").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(GAP, GAP);
                for c in &colours {
                    let (r, resp) = ui.allocate_exact_size(egui::Vec2::splat(SWATCH), Sense::click());
                    super::chip(ui, r, Some(c), resp.hovered());
                    if resp.on_hover_text(c.to_hex().to_uppercase()).clicked() {
                        acts.push(Action::Set(*c));
                    }
                }
            });
        });
    });
    ROW_H + DRAWER_H
}
