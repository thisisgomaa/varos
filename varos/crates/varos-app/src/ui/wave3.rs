//! Integration w3: per-frame glue for the wave-3 lanes (Appearance, live effects + Width, live
//! objects), moved out of `ui.rs` so its line ratchet holds (≤ 843). Behaviour is unchanged.
use super::{appearance, effects, live, Op};
use crate::app_command::SessionId;
use varos_core::{geom::View, Editor};

/// Before the frame borrows the shell: an Appearance "fx"/row request opens Properties.
pub(super) fn open_requests(ctx: &egui::Context, shell: &mut varos_app::shell::ShellState) {
    if ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("appearance-open"))).unwrap_or(false) {
        shell.show_panel(varos_app::shell::PanelId::Properties);
    }
}

/// Appearance edits staged by the Properties section become this frame's ops.
pub(super) fn settle(ctx: &egui::Context, ops: &mut Vec<Op>) {
    appearance::settle(ctx, ops);
}

/// The Effect dialogs and the live-object options sheet.
pub(super) fn sheets(ctx: &egui::Context, ed: &mut Editor, doc_active: Option<SessionId>, ops: &mut Vec<Op>) {
    effects::sheet(ctx, ed, doc_active);
    live::sheet(ctx, ed, doc_active, ops);
}

/// Canvas overlays: Width tool points.
pub(super) fn canvas(ctx: &egui::Context, ed: &Editor, view: &View, ppp: f32, hole: egui::Rect) {
    effects::width_points(ctx, ed, view, ppp, hole);
}
