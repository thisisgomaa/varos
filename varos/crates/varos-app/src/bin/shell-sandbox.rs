//! The box-system SANDBOX (BOX_SYSTEM_PLAN Wave 1 / Stage 2–3): an eframe window hosting the shell's
//! box tree with DUMMY panels — the playground where the container model is proven before the real app
//! ever moves in. eframe is used ONLY here (the shell modules are context-agnostic); it keeps the
//! sandbox's window boot trivial and version-aligned with egui 0.35.
//!
//! Run:  cargo run -p varos-app --bin shell-sandbox
//! (console stays attached on purpose — a dev sandbox should print panics/logs to the terminal.)
use varos_app::shell::{tokens, ShellState};

struct Sandbox {
    shell: ShellState,
}

impl eframe::App for Sandbox {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Windows/eframe: a minimized (or zero-sized) window drives pixels_per_point → 0, which panics
        // deep in epaint text layout ("Bad px_scale_factor: 0"). Skip the frame while degenerate;
        // rendering resumes cleanly once the window is restored.
        if ui.ctx().pixels_per_point() < 0.1
            || ui.ctx().input(|i| i.viewport().minimized == Some(true))
            || ui.available_size().min_elem() < 1.0
        {
            return;
        }
        tokens::apply(ui.ctx()); // warm-dark visuals + INSTANT (animation_time = 0); idempotent
                                 // the whole surface IS the void: seam-coloured, with an outer seam matching the inner ones.
        egui::Frame::default()
            .fill(tokens::SEAM)
            .inner_margin(egui::Margin::same(tokens::SEAM_GAP as i8))
            .show(ui, |ui| self.shell.ui(ui));
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Varos — Shell Sandbox")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([560.0, 380.0]), // small enough to test the tiny-window edge cases
        ..Default::default()
    };
    eframe::run_native(
        "Varos — Shell Sandbox",
        options,
        Box::new(|cc| {
            tokens::apply(&cc.egui_ctx);
            Ok(Box::new(Sandbox { shell: ShellState::standard() }))
        }),
    )
}
