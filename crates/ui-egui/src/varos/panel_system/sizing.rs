//! Intrinsic panel measurements and scoped flexible regions. No saved workspace sizes are changed.
use egui::{Id, Rect, Ui, UiBuilder, UiStackInfo};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BodySize {
    pub width: f32,
    /// Padded controls plus the smallest usable viewport, excluding tabs and footer.
    pub minimum: f32,
    pub stretches: bool,
}
pub(super) type BodySizes = BTreeMap<String, BodySize>;

fn cache_id() -> Id {
    Id::unique("varos-panel-body-sizes")
}
pub(super) fn snapshot(ctx: &egui::Context) -> BodySizes {
    ctx.data(|d| d.get_temp::<BodySizes>(cache_id())).unwrap_or_default()
}
pub(super) fn measured<'a>(sizes: &'a BodySizes, panel: &str, width: f32) -> Option<&'a BodySize> {
    sizes.get(panel).filter(|size| (size.width - width).abs() < 1.0)
}
fn remember(ctx: &egui::Context, panel: &str, size: BodySize) {
    if !size.minimum.is_finite() || size.minimum <= 0.0 || !size.width.is_finite() {
        return;
    }
    let changed = ctx.data_mut(|d| {
        let sizes = d.get_temp_mut_or_default::<BodySizes>(cache_id());
        let changed = sizes
            .get(panel)
            .is_none_or(|old| old.stretches != size.stretches || (old.width - size.width).abs() >= 1.0 || (old.minimum - size.minimum).abs() > 0.5);
        if changed {
            sizes.insert(panel.to_owned(), size);
        }
        changed
    });
    if changed {
        ctx.request_repaint();
    }
}

#[derive(Clone)]
struct Budget {
    extra: f32,
    used: Arc<AtomicU32>,
    flexible: Arc<AtomicBool>,
}
fn budget(ui: &Ui) -> Option<&Budget> {
    ui.stack().iter().find_map(|node| node.tags().get_downcast::<Budget>("varos-panel-budget"))
}

/// Only a preview or list opts in. Controls and spacing never receive surplus height.
/// Outside the workspace this leaves the original panel dimensions unchanged.
pub(crate) fn stretch(ui: &Ui, minimum: f32) -> f32 {
    let Some(budget) = budget(ui) else { return minimum };
    if budget.flexible.swap(true, Ordering::Relaxed) {
        return minimum;
    }
    budget.used.store(budget.extra.to_bits(), Ordering::Relaxed);
    minimum + budget.extra
}

/// A real list viewport, including when it is empty. Its original cap is kept in other presenters.
pub(crate) fn scroll(ui: &Ui, area: egui::ScrollArea, original: f32) -> egui::ScrollArea {
    if budget(ui).is_none() {
        return area.max_height(original);
    }
    let height = stretch(ui, 110.0);
    area.max_height(height).min_scrolled_height(height).auto_shrink([false, false])
}

pub(super) fn show(ui: &mut Ui, panel: &str, rect: Rect, sizes: &BodySizes, contents: impl FnOnce(&mut Ui)) {
    // While the width changes, use the last intrinsic height for one frame, then replace it
    // with the new width measurement. Never collapse a live preview to its minimum mid-drag.
    let old = measured(sizes, panel, rect.width()).or_else(|| sizes.get(panel));
    let extra = old.filter(|s| s.stretches).map_or(0.0, |s| (rect.height() - s.minimum).max(0.0));
    let budget = Budget { extra, used: Arc::default(), flexible: Arc::default() };
    // Measure inside the viewport wrapper, before it imposes a minimum height. Enough measuring
    // room keeps a list's minimum independent of a temporarily too-short host; the outer clip and
    // scroll area still enforce the real viewport.
    let measuring = Rect::from_min_size(rect.min, egui::vec2(rect.width(), old.map_or(1024.0, |s| s.minimum.max(1024.0)) + extra));
    let out = ui.scope_builder(
        UiBuilder::new().max_rect(measuring).ui_stack_info(UiStackInfo::default().with_tag_value("varos-panel-budget", budget.clone())),
        |ui| {
            ui.set_width(rect.width());
            contents(ui);
        },
    );
    let used = f32::from_bits(budget.used.load(Ordering::Relaxed));
    remember(
        ui.ctx(),
        panel,
        BodySize { width: rect.width(), minimum: (out.response.rect.height() - used).ceil(), stretches: budget.flexible.load(Ordering::Relaxed) },
    );
}

/// Layers/Properties/Libraries already allocate their own remaining-height viewport.
pub(super) fn own_scroll(ctx: &egui::Context, panel: &str, width: f32) {
    remember(ctx, panel, BodySize { width, minimum: 134.0, stretches: true });
}

/// Preserve a legacy list's drop rectangle while giving its contents a real scrolling viewport.
pub(crate) fn list_box(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> Rect) -> Rect {
    if budget(ui).is_none() {
        return crate::widgets::list_box(ui, contents);
    }
    crate::widgets::list_box(ui, |ui| scroll(ui, egui::ScrollArea::vertical(), 110.0).show(ui, contents).inner_rect)
}

/// Lists without a legacy scroll area keep their return value and gain a bounded viewport.
pub(crate) fn list_contents<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    if budget(ui).is_none() {
        return crate::widgets::list_box(ui, contents);
    }
    crate::widgets::list_box(ui, |ui| scroll(ui, egui::ScrollArea::vertical(), 110.0).show(ui, contents).inner)
}
