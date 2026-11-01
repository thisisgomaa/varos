//! The shell's divider and strip-edge resize gesture. Escape, pointer loss and focus loss
//! cancel before shortcuts run; a later release must not commit the cancelled preview.

use egui::{Context, Id, Key};

use crate::VectorcraftApp;

#[derive(Clone)]
struct Gesture {
    widget: Id,
    /// Dividers preview their ratio live. Strip edges only commit on release.
    ratio: Option<(Vec<u8>, f32)>,
}

fn key() -> Id {
    Id::unique("varos-shell-resize-gesture")
}

pub(super) fn begin(ctx: &Context, widget: Id, ratio: Option<(Vec<u8>, f32)>) {
    // Preserve the original ratio when a discarded pass revisits drag_started.
    if active(ctx, widget) {
        return;
    }
    // A discarded UI pass can revisit the drag-start event after cancellation.
    let cancelled = ctx.data(|d| d.get_temp::<(Id, u64)>(key().with("cancelled")));
    if cancelled != Some((widget, ctx.cumulative_frame_nr())) {
        ctx.data_mut(|d| d.insert_temp(key(), Gesture { widget, ratio }));
    }
}

pub(super) fn active(ctx: &Context, widget: Id) -> bool {
    ctx.data(|d| d.get_temp::<Gesture>(key())).is_some_and(|g| g.widget == widget)
}

/// Only the gesture that is still active may commit on release.
pub(super) fn finish(ctx: &Context, widget: Id) -> bool {
    if !active(ctx, widget) {
        return false;
    }
    ctx.data_mut(|d| d.remove::<Gesture>(key()));
    true
}

pub(super) fn before(app: &mut VectorcraftApp, ctx: &Context) {
    let Some(gesture) = ctx.data(|d| d.get_temp::<Gesture>(key())) else { return };
    let interrupted = ctx.input(|i| {
        !i.focused || i.events.iter().any(|e| matches!(e, egui::Event::PointerGone)) || (!i.pointer.primary_down() && !i.pointer.primary_released())
    });
    let escape = ctx.input_mut(|i| i.key_pressed(Key::Escape) && i.consume_key(i.modifiers, Key::Escape));
    if !interrupted && !escape {
        return;
    }
    if let Some((path, ratio)) = gesture.ratio {
        app.ui.varos_shell.tree.set_ratio(&path, ratio);
    }
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|d| {
        d.remove::<Gesture>(key());
        d.insert_temp(key().with("cancelled"), (gesture.widget, frame));
    });
    ctx.stop_dragging();
    ctx.request_repaint();
}
