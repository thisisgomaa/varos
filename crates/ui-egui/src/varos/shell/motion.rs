//! Only flyout transitions and completed drops animate. Layout itself never interpolates.
use egui::{Context, Id, Pos2, Rect};

#[derive(Clone)]
struct Flyout {
    panel: String,
    open: bool,
    since: f64,
    alpha: f32,
    slide: f32,
    frame: u64,
}

/// Retain the closing body for 60ms. A new body paints on the first frame, even at time zero.
pub(crate) fn flyout(ctx: &Context, id: Id, requested: Option<&str>) -> Option<(String, f32, f32)> {
    let now = ctx.input(|i| i.time);
    let frame = ctx.cumulative_frame_nr();
    // Hidden hosts (Home, Shell OFF, hidden tools) must not resurrect a closing flyout.
    let old = ctx.data(|d| d.get_temp::<Flyout>(id)).filter(|s| s.frame.saturating_add(1) >= frame);
    let open = requested.is_some();
    let changed = old.as_ref().is_none_or(|s| s.open != open);
    let mut state = match (old, requested) {
        (Some(s), _) => s,
        (None, Some(panel)) => Flyout { panel: panel.into(), open, since: now, alpha: 0.0, slide: 6.0, frame },
        (None, None) => return None,
    };
    state.frame = frame;
    if changed {
        state.since = now;
        state.open = open;
        ctx.animate_value_with_time(id.with("alpha"), state.alpha, 0.0);
    }
    if let Some(panel) = requested {
        state.panel = panel.into();
    }
    let duration = if ctx.global_style().animation_time == 0.0 {
        0.0
    } else if open {
        0.09
    } else {
        0.06
    };
    let target = if open { 1.0 } else { 0.0 };
    let alpha = ctx.animate_value_with_time(id.with("alpha"), target, duration);
    let progress = if now - state.since >= f64::from(duration) { target } else { alpha };
    let alpha = if open { progress.max(0.05) } else { progress };
    state.alpha = alpha;
    if open {
        state.slide = 6.0 * (1.0 - progress);
    }
    ctx.data_mut(|d| d.insert_temp(id, state.clone()));
    if alpha == 0.0 && !open {
        return None;
    }
    Some((state.panel, alpha, state.slide))
}

#[derive(Clone)]
struct Settle {
    from: Pos2,
    since: f64,
}

pub(crate) fn dropped(ctx: &Context, key: &str, from: Pos2) {
    let id = Id::unique("shell-settle").with(key);
    let since = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(id, Settle { from, since }));
    ctx.animate_value_with_time(id, 0.0, 0.0);
}

/// Ease-out over 80ms. Exact target at completion, bounded to the central region throughout.
pub(crate) fn settle(ctx: &Context, key: &str, target: Rect, region: Rect) -> Rect {
    let id = Id::unique("shell-settle").with(key);
    let Some(s) = ctx.data(|d| d.get_temp::<Settle>(id)) else { return target };
    if ctx.global_style().animation_time == 0.0 || ctx.input(|i| i.time) - s.since >= 0.08 {
        ctx.data_mut(|d| d.remove::<Settle>(id));
        return target;
    }
    let progress = ctx.animate_value_with_time(id, 1.0, 0.08);
    let remaining = (1.0 - progress).powi(3);
    super::floats::clamp_into(target.translate((s.from - target.min) * remaining), region)
}

/// Tear-off transfers the body to another host immediately.
pub(crate) fn cancel_flyout(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.remove::<Flyout>(id));
    ctx.animate_value_with_time(id.with("alpha"), 0.0, 0.0);
}
