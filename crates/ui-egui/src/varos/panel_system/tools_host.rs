//! The Tools contents as the Varos presenters show them. Upstream's tool body
//! (`toolbar::body_with_layout`, `toolbar::bottom_controls`) asks here, through marked hooks and
//! the explicit [`Hosting`] it is given, what it draws itself: upstream's header and bottom
//! controls only in its own docks, how the strip scrolls, the Fill/Stroke wells, the paint chips
//! and the Edit Toolbar menu of the box shell and of a panel-workspace host. A panel-workspace
//! host also owns all moving, resizing and hiding, and places the tool flyout beside it.
use super::{Hosting, metrics};
use crate::theme::Tokens;
use crate::{VectorcraftApp, icons, widgets};
use egui::{Color32, CornerRadius, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_color::Paint;

/// Where the hosted Tools are this frame, for the flyout placed beside them: the workspace region
/// popovers stay inside, the first tool of the slot whose flyout is open (whichever presenter
/// opened it: [`note_slot`]), and that slot's hosted button (refreshed every frame, so the open
/// flyout follows a moved host). One typed record in egui's temporary memory, written and read
/// only here.
#[derive(Clone, Debug, Default)]
struct Placement {
    region: Option<Rect>,
    flyout_tool: Option<String>,
    anchor: Option<Rect>,
}

fn placement(ctx: &egui::Context) -> Placement {
    ctx.data(|d| d.get_temp::<Placement>(egui::Id::NULL)).unwrap_or_default()
}

fn update_placement(ctx: &egui::Context, change: impl FnOnce(&mut Placement)) {
    ctx.data_mut(|d| change(d.get_temp_mut_or_default::<Placement>(egui::Id::NULL)));
}

/// Real tool contents for a generic panel host. Its host owns all moving, resizing and hiding.
/// Tools stay vertical in one or two columns; a short host scrolls the entire strip.
pub(crate) fn content(app: &mut VectorcraftApp, ui: &mut Ui) {
    let bounds = ui.max_rect();
    let t = Tokens::get(ui.ctx());
    // The tool body reports the slot whose flyout it opens, and that slot's button, through
    // `note_slot`; a closed flyout has no open slot.
    let open = app.ui.flyout.is_some();
    update_placement(ui.ctx(), |placement| {
        if !open {
            placement.flyout_tool = None;
        }
        placement.anchor = None;
    });
    widgets::panel_scroll(
        ui,
        egui::ScrollArea::vertical().id_salt("hosted-tools-scroll").max_height(bounds.height()).auto_shrink([false, false]),
        |ui| {
            let cols = if ui.available_width() >= 2.0 * metrics::TOOL_BUTTON { 2 } else { 1 };
            crate::toolbar::body_with_layout(app, ui, cols, None, Hosting::Workspace);
            ui.vertical_centered(|ui| crate::toolbar::bottom_controls(app, ui, &t, cols, Hosting::Workspace));
        },
    );
    // Body sublayouts must not replace the outer host used by the drawer and flyout anchors.
    // Varos bridge: upstream's All Tools drawer reads the toolbar's bounds from its own keyed
    // state (`edit_toolbar::toolbar_id`), so the host writes them there too.
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| {
        d.insert_temp(crate::edit_toolbar::toolbar_id(), bounds);
        d.insert_temp(panel_host_id(), (pass, bounds));
    });
}

fn panel_host_id() -> egui::Id {
    egui::Id::unique("panel-system-tools-host")
}

fn current_panel_host(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|d| d.get_temp::<(u64, egui::Rect)>(panel_host_id())).filter(|(pass, _)| *pass == ctx.cumulative_pass_nr()).map(|(_, rect)| rect)
}

/// Called once after the generic hosts. A hidden Tools tab has no live popup anchor.
pub(crate) fn popovers(app: &mut VectorcraftApp, ctx: &egui::Context, region: egui::Rect) {
    update_placement(ctx, |placement| placement.region = Some(region));
    if current_panel_host(ctx).is_none() {
        app.ui.flyout = None;
        app.ui.tools_drawer = false;
    }
    crate::toolbar::shell_popovers(app, ctx, region);
}

/// Where the tool flyout `id` opens while the panel workspace hosts Tools: beside the live host,
/// facing the workspace, level with the open slot's button; `None` leaves it to the other
/// presenters. Varos bridge: upstream's flyout keeps its own tool list and opening anchor in its
/// keyed state (`flyout-tools`, `flyout-anchor`), which the classic shell still uses; it passes
/// that anchor (used only until the hosted body has reported the open slot's button) and the
/// height of that list here.
pub(crate) fn flyout_at(app: &VectorcraftApp, ctx: &egui::Context, id: egui::Id, anchor: egui::Rect, default_height: f32) -> Option<egui::Pos2> {
    let host = current_panel_host(ctx).filter(|_| super::enabled(app))?;
    let placement = placement(ctx);
    let region = placement.region.unwrap_or_else(|| ctx.content_rect());
    let size = ctx.memory(|m| m.area_rect(id)).map_or(vec2(240.0, default_height), |r| r.size());
    Some(flyout_position(host, placement.anchor.unwrap_or(anchor), size, region))
}

/// Whether the tool body draws upstream's own header (title bar and grip) and bottom controls:
/// only in upstream's docks and bars. The box shell (`shell`) and a panel-workspace host draw
/// their own.
pub(crate) fn own_chrome(shell: bool, hosting: Hosting) -> bool {
    !shell && hosting == Hosting::Own
}

/// Scroll the tool strip `contents` as its presenter does: a panel-workspace host scrolls the
/// whole body itself, so the strip draws unscrolled; the box shell's narrow strip hides its
/// scroll bar, keeping the right-edge flyout triangles clear (wheel and trackpad scrolling still
/// reach every tool in a short window); upstream's docks use its strip scroll.
pub(crate) fn scroll<R>(ui: &mut Ui, shell: bool, hosting: Hosting, contents: impl FnOnce(&mut Ui) -> R) -> R {
    if hosting == Hosting::Workspace {
        contents(ui)
    } else if shell {
        egui::ScrollArea::vertical()
            .id_salt("toolbar")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .auto_shrink([false, true])
            .show(ui, contents)
            .inner
    } else {
        widgets::strip_scroll(ui, "toolbar", contents)
    }
}

/// The box shell's Edit Toolbar menu on the strip's `edit` button: the strip's column count (a
/// panel-workspace host derives its columns from its width instead) and Edit Toolbar. False
/// leaves the button to upstream (a click opens the All Tools drawer).
pub(crate) fn edit_menu(app: &mut VectorcraftApp, edit: &egui::Response, hosting: Hosting) -> bool {
    if !crate::varos::shell::enabled(app) {
        return false;
    }
    egui::Popup::menu(edit).show(|ui| {
        if hosting != Hosting::Workspace {
            for (n, label) in [(1, tl!("1 column")), (2, tl!("2 columns")), (3, tl!("3 columns"))] {
                if ui.selectable_label(app.ui.varos_shell.tools_columns == n, label).clicked() {
                    app.ui.varos_shell.tools_columns = n;
                    ui.close();
                }
            }
        }
        if ui.button(tl!("Edit Toolbar")).clicked() {
            app.run("window.editToolbar", json!({})).ok();
            crate::edit_toolbar::opened_from_popup(ui.ctx());
            ui.close();
        }
    });
    true
}

/// The Fill/Stroke wells of the box shell and of a panel-workspace host; false leaves upstream's
/// proxy to the tool body.
pub(crate) fn wells(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, cols: usize, hosting: Hosting) -> bool {
    let look = if hosting == Hosting::Workspace {
        WellLook::Squares { size: (ui.available_width() - 2.0 * metrics::TOOL_PAINT_INSET).clamp(1.0, metrics::TOOL_PAINT_PROXY) }
    } else if crate::varos::shell::enabled(app) {
        WellLook::Circles { size: if cols <= 1 { 28.0 } else { 40.0 } }
    } else {
        return false;
    };
    paint_wells(app, ui, t, look);
    true
}

/// The Color, Gradient and None chips of the box shell and of a panel-workspace host; false
/// leaves upstream's chips to the tool body.
pub(crate) fn chips(app: &mut VectorcraftApp, ui: &mut Ui, hosting: Hosting) -> bool {
    let layout = if hosting == Hosting::Workspace {
        // Three small adjacent squares in one row at either Tools width.
        let gap = metrics::TOOL_PAINT_GAP;
        let hit = ((ui.available_width() - 2.0 * gap) / 3.0).floor().clamp(1.0, metrics::TOOL_PAINT_HIT);
        ChipLayout { hit, chip: hit.min(metrics::TOOL_PAINT_CHIP), gap, stack: false }
    } else if crate::varos::shell::enabled(app) {
        // The strip's tile size: one row when three fit, else a column.
        ChipLayout { hit: 28.0, chip: 28.0, gap: 4.0, stack: true }
    } else {
        return false;
    };
    paint_chips(app, ui, layout);
    true
}

/// The tool body's hook for each tool button, after the button handled its press (`opening`: the
/// slot whose flyout this frame's presses open, if any). In every presenter, a button that opens
/// its slot's flyout makes that slot the open one: the tool body is the only place a flyout opens,
/// so the record names the open flyout across presenter hand-offs (a flyout opened in the box
/// shell or the classic dock and still open when the workspace presents again anchors to its own
/// hosted button). Only a hosted button anchors the flyout; the other presenters place it
/// themselves and never read the record.
pub(crate) fn note_slot(ctx: &egui::Context, hosting: Hosting, slot: &[&str], button: Rect, opening: Option<&(Vec<&str>, Rect)>) {
    let opened = opening.is_some_and(|(open, _)| open.as_slice() == slot);
    if !opened && hosting != Hosting::Workspace {
        return;
    }
    update_placement(ctx, |placement| {
        if opened {
            placement.flyout_tool = slot.first().map(|tool| (*tool).to_string());
        }
        if hosting == Hosting::Workspace && placement.flyout_tool.as_deref().is_some_and(|tool| slot.first() == Some(&tool)) {
            placement.anchor = Some(button);
        }
    });
}

/// How a Fill/Stroke pair looks, `size` points square.
#[derive(Clone, Copy, Debug, PartialEq)]
enum WellLook {
    /// The box shell's: the picker's overlapping circles (28 pt in one column, 40 pt in two or
    /// three).
    Circles { size: f32 },
    /// A panel-workspace host's: two overlapping squares sized to the host's width.
    Squares { size: f32 },
}

/// Where a pair's parts are: each well (Fill at the upper left, Stroke at the lower right,
/// overlapping), the swap arrow and the default-colours squares.
struct WellParts {
    fill: Rect,
    stroke: Rect,
    swap: Rect,
    defaults: Rect,
}

impl WellLook {
    fn size(self) -> f32 {
        match self {
            Self::Circles { size } | Self::Squares { size } => size,
        }
    }

    fn parts(self, rect: Rect) -> WellParts {
        let size = self.size();
        let (side, swap, defaults) = match self {
            // Each circle is 62% of the cluster, so each well's own outer corner stays on it.
            Self::Circles { .. } => (size * 0.62, vec2(size * 0.40, size * 0.34), vec2(size * 0.46, size * 0.30)),
            Self::Squares { .. } => (size * 2.0 / 3.0, vec2(size * 0.30, size * 0.30), vec2(size * 0.30, size * 0.30)),
        };
        WellParts {
            fill: Rect::from_min_size(rect.min, vec2(side, side)),
            stroke: Rect::from_min_size(rect.max - vec2(side, side), vec2(side, side)),
            swap: Rect::from_min_size(pos2(rect.right() - swap.x, rect.top()), swap),
            defaults: Rect::from_min_size(pos2(rect.left(), rect.bottom() - defaults.y), defaults),
        }
    }
}

/// The one Fill/Stroke pair of the Varos Tools: `look` paints it; every look shares the
/// interaction. The focused well is painted in front and owns the overlap; each well's outer
/// corner still selects its own paint; a click on a well makes it active and opens the picker
/// for it; the swap arrow and the default-colours squares run their commands.
fn paint_wells(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, look: WellLook) {
    let size = look.size();
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let parts = look.parts(rect);
    let open = app.ui.varos_picker_anchor.is_some();
    let outline = |active: bool| Stroke::new(1.0, if open && active { t.accent } else { t.button_border });
    let (fill_paint, stroke_paint) = crate::panels::current_paints(app);
    let painter = ui.painter().clone();
    match look {
        WellLook::Circles { .. } => {
            let draw = |r: Rect, paint: &Paint, active: bool| {
                let (center, radius) = (r.center(), r.width() * 0.5);
                painter.circle_filled(center, radius, t.input);
                if let Some(color) = paint.color() {
                    painter.circle_filled(center, radius - 1.0, crate::panels::c32(&color));
                }
                painter.circle_stroke(center, radius, outline(active));
            };
            if app.session.fill_active {
                draw(parts.stroke, &stroke_paint, false);
                draw(parts.fill, &fill_paint, true);
            } else {
                draw(parts.fill, &fill_paint, false);
                draw(parts.stroke, &stroke_paint, true);
            }
            let square = size * 0.16;
            for (i, color) in [Color32::WHITE, Color32::BLACK].into_iter().enumerate() {
                let r = Rect::from_min_size(parts.defaults.min + vec2(i as f32 * square, size * 0.06), vec2(square, square));
                painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(1.0, t.button_border), egui::StrokeKind::Inside);
                painter.rect_filled(r.shrink(1.0), CornerRadius::ZERO, color);
            }
        }
        WellLook::Squares { .. } => {
            let mixed = crate::panels::mixed_paints(app, ui.ctx());
            let side = parts.fill.width();
            let draw = |r: Rect, paint: &Paint, mixed: bool, stroke: bool, active: bool| {
                painter.rect_filled(r, CornerRadius::ZERO, t.input_border);
                let chip = r.shrink(1.0);
                let hole = widgets::proxy_chip(ui, chip, paint, mixed, stroke.then_some(side * 0.25));
                painter.rect_stroke(r, CornerRadius::ZERO, outline(active), egui::StrokeKind::Inside);
                if stroke {
                    painter.rect_stroke(chip, CornerRadius::ZERO, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
                    painter.rect_stroke(hole, CornerRadius::ZERO, Stroke::new(1.0, t.text_dim), egui::StrokeKind::Outside);
                }
            };
            if app.session.fill_active {
                draw(parts.stroke, &stroke_paint, mixed.1, true, false);
                draw(parts.fill, &fill_paint, mixed.0, false, true);
            } else {
                draw(parts.fill, &fill_paint, mixed.0, false, false);
                draw(parts.stroke, &stroke_paint, mixed.1, true, true);
            }
            let square = size * 0.16;
            let fill = Rect::from_min_size(parts.defaults.min + vec2(1.0, 1.0), vec2(square, square));
            let stroke = fill.translate(vec2(square * 0.55, square * 0.55));
            painter.rect_filled(stroke, CornerRadius::ZERO, Color32::BLACK);
            painter.rect_stroke(stroke, CornerRadius::ZERO, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
            painter.rect_filled(stroke.shrink(square * 0.30), CornerRadius::ZERO, t.panel);
            painter.rect_filled(fill, CornerRadius::ZERO, Color32::WHITE);
            painter.rect_stroke(fill, CornerRadius::ZERO, Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
        }
    }
    icons::paint(ui, "dc-swap", parts.swap.shrink(size * 0.06), t.text);
    for stroke in [app.session.fill_active, !app.session.fill_active] {
        crate::varos::picker::register_well(ui.ctx(), if stroke { parts.stroke } else { parts.fill }, stroke);
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        pick(app, &parts, p);
    }
    ui.add_space(6.0);
}

/// A click at `p` on a Fill/Stroke pair.
fn pick(app: &mut VectorcraftApp, parts: &WellParts, p: egui::Pos2) {
    if parts.swap.contains(p) {
        app.run("paint.swap", json!({})).ok();
    } else if parts.defaults.contains(p) {
        app.run("paint.default", json!({})).ok();
    } else {
        // The well in front owns the overlap; a well's outer corner stays its own.
        let front_fill = app.session.fill_active;
        let in_fill = parts.fill.contains(p);
        let in_stroke = parts.stroke.contains(p);
        let on_fill = in_fill && (front_fill || !in_stroke);
        let on_stroke = in_stroke && (!front_fill || !in_fill);
        if on_fill || on_stroke {
            let stroke = on_stroke;
            if stroke == app.session.fill_active {
                app.run("paint.toggleActive", json!({})).ok();
            }
            crate::varos::picker::open_for(app, stroke, if stroke { parts.stroke } else { parts.fill });
        }
    }
}

/// How the Color, Gradient and None chips sit: each one's hit square (`hit`), the chip painted
/// centred in it (`chip`), the gap between them, and whether they stack in a column when a row
/// of three doesn't fit (`stack`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct ChipLayout {
    hit: f32,
    chip: f32,
    gap: f32,
    stack: bool,
}

/// The one Color, Gradient and None chip loop of the Varos Tools: each chip runs its command.
fn paint_chips(app: &mut VectorcraftApp, ui: &mut Ui, layout: ChipLayout) {
    let ChipLayout { hit, chip, gap, stack } = layout;
    let items = [("Color (,)", "paint.lastColor"), ("Gradient (.)", "paint.lastGradient"), ("None (/)", "paint.none")];
    let mut clicked = None;
    {
        let place = |ui: &mut Ui, clicked: &mut Option<&str>| {
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            for (tip, cmd) in items {
                let (r, resp) = ui.allocate_exact_size(vec2(hit, hit), Sense::click());
                let paint_rect = Rect::from_center_size(r.center(), egui::Vec2::splat(chip));
                match cmd {
                    "paint.lastColor" => widgets::paint_chip(ui, paint_rect, &Paint::solid(app.session.last_solid)),
                    "paint.lastGradient" => widgets::gradient_chip(ui, paint_rect, &app.session.last_gradient),
                    _ => widgets::paint_chip(ui, paint_rect, &Paint::None),
                }
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), tl!(tip)));
                if resp.on_hover_text(tl!(tip)).clicked() {
                    *clicked = Some(cmd);
                }
            }
        };
        let row = 3.0 * hit + 2.0 * gap;
        if !stack || ui.available_width() >= row {
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
                place(ui, &mut clicked);
            });
        } else {
            ui.vertical_centered(|ui| place(ui, &mut clicked));
        }
    }
    if let Some(cmd) = clicked {
        app.run(cmd, json!({})).ok();
    }
    ui.add_space(6.0);
}

/// Prefer the side facing the workspace; keep the popup beside its live host after moves.
fn flyout_position(host: egui::Rect, anchor: egui::Rect, size: egui::Vec2, region: egui::Rect) -> egui::Pos2 {
    let gap = 6.0;
    let right = host.right() + gap;
    let preferred = pos2(if right + size.x <= region.right() { right } else { host.left() - size.x - gap }, anchor.top());
    pos2(
        preferred.x.clamp(region.left(), (region.right() - size.x).max(region.left())),
        preferred.y.clamp(region.top(), (region.bottom() - size.y).max(region.top())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;
    use vectorcraft_engine::Session;

    #[test]
    fn hosted_paint_chips_are_small_adjacent_squares_at_both_toolbar_widths() {
        for (width, size) in [(44.0, 13.0), (80.0, metrics::TOOL_PAINT_HIT)] {
            let mut app = VectorcraftApp::new(Session::new(), Default::default());
            app.ui.varos_shell.enabled = true;
            app.ui.varos_panel_review = Some(Default::default());
            let ctx = egui::Context::default();
            let rect = egui::Rect::from_min_size(pos2(20.0, 20.0), vec2(width, 100.0));
            for _ in 0..3 {
                let mut out = ctx.run_ui(Default::default(), |ui| {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)), |ui| {
                        ui.set_width(width);
                        assert!(chips(&mut app, ui, Hosting::Workspace));
                    });
                });
                out.textures_delta.clear();
            }
            let mut chips = crate::toolbar::tests::widget_rects(&ctx, egui::Vec2::splat(size));
            chips.sort_by(|a, b| a.left().total_cmp(&b.left()));
            assert_eq!(chips.len(), 3);
            assert!(chips.iter().all(|chip| rect.contains_rect(*chip)));
            for pair in chips.windows(2) {
                assert_eq!(pair[0].top(), pair[1].top(), "the paint choices never stack");
                assert!((pair[1].left() - pair[0].right() - metrics::TOOL_PAINT_GAP).abs() < 0.1);
            }
        }
    }

    /// The box shell's circles and a host's squares differ in looks only: the same click on the
    /// same part does the same thing.
    #[test]
    fn both_well_looks_share_one_interaction() {
        for look in [WellLook::Circles { size: 40.0 }, WellLook::Squares { size: 60.0 }] {
            let mut app = VectorcraftApp::new(Session::new(), Default::default());
            app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
            let parts = look.parts(Rect::from_min_size(pos2(10.0, 10.0), egui::Vec2::splat(look.size())));
            assert!(app.session.fill_active);
            pick(&mut app, &parts, parts.fill.intersect(parts.stroke).center());
            assert!(app.session.fill_active, "{look:?}: the well in front owns the overlap");
            assert_eq!(app.ui.varos_picker_anchor, Some(parts.fill), "{look:?}: the picker opens for Fill");
            pick(&mut app, &parts, parts.stroke.max - vec2(1.0, 1.0));
            assert!(!app.session.fill_active, "{look:?}: Stroke's outer corner selects Stroke");
            assert_eq!(app.ui.varos_picker_anchor, Some(parts.stroke));
            let (fill, stroke) = crate::panels::current_paints(&app);
            pick(&mut app, &parts, parts.swap.center());
            assert_eq!(crate::panels::current_paints(&app), (stroke, fill), "{look:?}: the arrow swaps the paints");
        }
    }

    #[test]
    fn the_open_flyout_anchors_to_its_slot_button_only_while_hosted() {
        let ctx = egui::Context::default();
        update_placement(&ctx, |placement| placement.flyout_tool = Some("pen".into()));
        let button = Rect::from_min_size(pos2(10.0, 300.0), vec2(28.0, 28.0));
        note_slot(&ctx, Hosting::Own, &["pen"], button, None);
        assert_eq!(placement(&ctx).anchor, None, "only a hosted body reports its buttons");
        note_slot(&ctx, Hosting::Workspace, &["selection"], button, None);
        assert_eq!(placement(&ctx).anchor, None, "another slot is not the anchor");
        note_slot(&ctx, Hosting::Workspace, &["pen", "curvature"], button, None);
        assert_eq!(placement(&ctx).anchor, Some(button));
        // A button that opens its slot's flyout makes that slot the open one.
        let other = Rect::from_min_size(pos2(10.0, 340.0), vec2(28.0, 28.0));
        note_slot(&ctx, Hosting::Workspace, &["rectangle", "ellipse"], other, Some(&(vec!["rectangle", "ellipse"], other)));
        assert_eq!(placement(&ctx).flyout_tool.as_deref(), Some("rectangle"));
        assert_eq!(placement(&ctx).anchor, Some(other));
        // A flyout opened in another presenter is the open one too, but only a hosted button
        // anchors it.
        note_slot(&ctx, Hosting::Own, &["pen"], button, Some(&(vec!["pen"], button)));
        assert_eq!(placement(&ctx).flyout_tool.as_deref(), Some("pen"), "every presenter's tool body opens the one record");
        assert_eq!(placement(&ctx).anchor, Some(other), "an unhosted button never anchors");
    }

    #[test]
    fn hosted_tools_flyout_opens_inward_at_the_right_and_bottom_edges() {
        let region = egui::Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0));
        let size = vec2(240.0, 300.0);
        let right_host = egui::Rect::from_min_size(pos2(920.0, 100.0), vec2(80.0, 600.0));
        let anchor = egui::Rect::from_min_size(pos2(940.0, 650.0), vec2(28.0, 28.0));
        let popup = egui::Rect::from_min_size(flyout_position(right_host, anchor, size, region), size);
        assert!(region.contains_rect(popup));
        assert!(popup.right() < right_host.left());
        let bottom_host = egui::Rect::from_min_size(pos2(0.0, 700.0), vec2(80.0, 100.0));
        let anchor = egui::Rect::from_min_size(pos2(20.0, 750.0), vec2(28.0, 28.0));
        let popup = egui::Rect::from_min_size(flyout_position(bottom_host, anchor, size, region), size);
        assert!(region.contains_rect(popup));
        assert!(popup.left() > bottom_host.right());
        assert!(popup.top() < anchor.top(), "a low tool's flyout stays inside the workspace");
    }
}
