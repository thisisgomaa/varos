//! Overflow navigation; activation reveals the current document, manual browsing preserves it.
use egui::{Id, Ui};

#[derive(Clone, Default)]
struct Scroll {
    first: usize,
    active: Option<usize>,
    room: f32,
    document: Option<u64>,
    widths: Vec<f32>,
}

pub(crate) fn first(ui: &mut Ui, widths: &[f32], active: Option<usize>, document: Option<u64>, room: f32, step: i32) -> usize {
    let id = Id::unique("varos-document-tab-scroll");
    let mut state = ui.data_mut(|d| d.get_temp::<Scroll>(id)).unwrap_or_default();
    if active != state.active || document != state.document || room != state.room || widths != state.widths {
        state.first = reveal(widths, active, room);
    }
    state.first = state.first.saturating_add_signed(step as isize).min(widths.len().saturating_sub(1));
    state.active = active;
    state.document = document;
    state.widths = widths.to_vec();
    state.room = room;
    let first = state.first;
    ui.data_mut(|d| d.insert_temp(id, state));
    first
}

fn reveal(widths: &[f32], active: Option<usize>, room: f32) -> usize {
    let Some(active) = active.filter(|i| *i < widths.len()) else { return 0 };
    let mut first = active;
    let mut used = widths.get(active).copied().unwrap_or_default().min(room);
    while first > 0 {
        let w = widths.get(first - 1).copied().unwrap_or_default();
        if used + w > room {
            break;
        }
        used += w;
        first -= 1;
    }
    first
}

#[cfg(test)]
mod tests {
    #[test]
    fn activation_and_resize_reveal_active_and_arrows_reach_every_tab() {
        let ctx = egui::Context::default();
        let widths = vec![180.; 40];
        let mut result = 0;
        let mut frame = |active, room, step| {
            let _ = ctx.run_ui(Default::default(), |ui| {
                result = super::first(ui, &widths, active, active.map(|i| i as u64), room, step);
            });
            result
        };
        assert_eq!(frame(Some(39), 400., 0), 38);
        assert_eq!(frame(Some(39), 150., 0), 39);
        for i in (0..39).rev() {
            assert_eq!(frame(Some(39), 150., -1), i);
        }
        for i in 1..40 {
            assert_eq!(frame(Some(39), 150., 1), i);
        }
        assert_eq!(frame(Some(0), 150., 0), 0);
    }
}

#[cfg(test)]
mod strip_tests {
    #[test]
    fn every_activated_document_is_painted_in_a_narrow_strip() {
        let mut app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        for i in 0..20 {
            app.run("file.new", serde_json::json!({"width":100,"height":100,"name":format!("overflow-document-{i:02}.svg")})).unwrap();
        }
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        for i in (0..20).rev() {
            assert!(app.session.set_active(i));
            let output = ctx.run_ui(
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(480., 100.))), ..Default::default() },
                |ui| crate::chrome::doc_tabs(&mut app, ui),
            );
            let title = format!("overflow-document-{i:02}.svg");
            assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains(title.as_str()) && s.clip_rect.intersects(t.visual_bounding_rect()))), "active {title} was unreachable");
        }
    }
}
