//! Provisional Links home, using the existing shell kit. Owner design review pending.
use crate::{
    app_command::{AppCommand, SessionId},
    image_workflows::Action,
};
use varos_app::shell::{
    kit::{
        self,
        field::{self, Label, NumberField},
    },
    tokens as t,
};
use varos_core::{
    images::{ImageAffine, ImageObject},
    trace::TraceMode,
    Editor,
};
fn button(ui: &mut egui::Ui, label: &str) -> bool {
    kit::action(ui, kit::Control::new(ui.id().with(("w2-image", label)), label), false).activated
}
fn number(ui: &mut egui::Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) {
    let e = field::number_field(
        ui,
        NumberField {
            id: ui.id().with(label),
            width: t::SLICE4A_TOOLS_FIELD_W,
            label: Label::Letter(label),
            tip: label,
            value: *v,
            decimals: 2,
            speed: 1.,
            range,
            disabled: false,
        },
    );
    if let Some(value) = e.live.or(e.commit).or(e.pending) {
        *v = value;
    }
}
#[derive(Clone)]
struct Controls {
    bounds: [f32; 4],
    opacity: f32,
    angle: f32,
    ppi: f32,
    white: bool,
    preset: usize,
}
fn defaults(i: &ImageObject) -> Controls {
    Controls {
        bounds: [
            i.xform.e,
            i.xform.f,
            i.xform.a.hypot(i.xform.b) * i.px_w as f32,
            i.xform.c.hypot(i.xform.d) * i.px_h as f32,
        ],
        opacity: i.opacity,
        angle: i.xform.b.atan2(i.xform.a).to_degrees(),
        ppi: 300.,
        white: false,
        preset: 0,
    }
}
pub fn panel(ui: &mut egui::Ui, ed: &Editor, sid: Option<SessionId>, commands: &mut Vec<AppCommand>) -> bool {
    let Some(sid) = sid else { return true };
    if button(ui, "Place image…") {
        commands.push(AppCommand::PlaceDialog(sid));
    }
    ui.label("Embed is the default. Missing originals retain accepted pixels.");
    if ed.doc.images.is_empty() {
        ui.label("Place or drop an image to manage its source here.");
    }
    for i in &ed.doc.images {
        ui.push_id((sid.0, i.id), |ui| {
            let name = i.link.as_ref().map(|l| l.absolute.as_str()).unwrap_or("Embedded bitmap");
            ui.label(name);
            let status_key = ui.id().with("link-status");
            if button(ui, "Check source status") {
                let status = varos_core::images::links::status(i, &ed.blobs);
                ui.ctx().data_mut(|d| d.insert_temp(status_key, format!("{status:?}")));
            }
            if let Some(status) = ui.ctx().data(|d| d.get_temp::<String>(status_key)) {
                ui.label(status);
            }

            let ppi = i.effective_ppi();
            ui.label(format!(
                "{:?} · {} × {} px · {:.0}/{:.0} source ppi · {:.0}/{:.0} effective ppi",
                i.placement, i.px_w, i.px_h, i.ppi[0], i.ppi[1], ppi[0], ppi[1]
            ));
            let state = ui.id().with("controls");
            let mut c = ui.ctx().data(|d| d.get_temp::<Controls>(state)).unwrap_or_else(|| defaults(i));
            let mut emit = |a| commands.push(AppCommand::ImageWorkflow(sid, a));
            for (label, a) in [
                ("Go To", Action::GoTo(i.id)),
                ("Update", Action::Update(i.id)),
                ("Relink…", Action::Relink(i.id)),
                ("Embed", Action::Embed(i.id)),
                ("Unembed…", Action::Unembed(i.id)),
            ] {
                if button(ui, label) {
                    emit(a);
                }
            }
            for (n, label) in ["X", "Y", "W", "H"].into_iter().enumerate() {
                number(
                    ui,
                    label,
                    &mut c.bounds[n],
                    if n < 2 { -1_000_000.0..=1_000_000.0 } else { 0.01..=1_000_000.0 },
                );
            }
            number(ui, "Angle", &mut c.angle, -360.0..=360.0);
            number(ui, "Opacity", &mut c.opacity, 0.0..=1.0);
            if button(ui, "Apply transform") {
                emit(Action::Transform(
                    i.id,
                    ImageAffine {
                        a: c.angle.to_radians().cos() * c.bounds[2] / i.px_w as f32,
                        b: c.angle.to_radians().sin() * c.bounds[2] / i.px_w as f32,
                        c: -c.angle.to_radians().sin() * c.bounds[3] / i.px_h as f32,
                        d: c.angle.to_radians().cos() * c.bounds[3] / i.px_h as f32,
                        e: c.bounds[0],
                        f: c.bounds[1],
                    },
                    c.opacity,
                ));
            }
            if button(ui, "Crop to bounds") {
                emit(Action::Crop(i.id, c.bounds));
            }
            number(ui, "PPI", &mut c.ppi, 1.0..=2400.0);
            if button(ui, if c.white { "Background: white" } else { "Background: transparent" }) {
                c.white = !c.white;
            }
            if button(ui, "Rasterize…") {
                emit(Action::Rasterize(i.id, c.ppi, c.white.then_some([1.; 4])));
            }
            let names = ["Black and white", "Grayscale", "6 colors", "16 colors"];
            if let Some(p) = kit::text_dropdown(
                ui,
                ui.id().with("trace"),
                names[c.preset],
                &names,
                t::SLICE4A_TOOLS_FIELD_W,
                "Image Trace preset",
            ) {
                c.preset = p;
            }
            if button(ui, "Image Trace → editable paths") {
                emit(Action::Trace(
                    i.id,
                    match c.preset {
                        0 => TraceMode::BlackWhite,
                        1 => TraceMode::Grayscale,
                        2 => TraceMode::Color { colors: 6 },
                        _ => TraceMode::Color { colors: 16 },
                    },
                ));
            }
            ui.ctx().data_mut(|d| d.insert_temp(state, c));
        });
    }
    let key = ui.id().with("effects-ppi");
    let mut ppi = ui.ctx().data(|d| d.get_temp::<f32>(key)).unwrap_or(ed.doc.raster_effects_ppi);
    number(ui, "Raster effects PPI", &mut ppi, 1.0..=2400.0);
    ui.ctx().data_mut(|d| d.insert_temp(key, ppi));
    if button(ui, "Apply raster effects PPI") {
        commands.push(AppCommand::ImageWorkflow(sid, Action::EffectsPpi(ppi)));
    }
    if button(ui, "Package…") {
        commands.push(AppCommand::ImageWorkflow(sid, Action::Package));
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind {
    Place,
    Crop,
    Trace,
    Rasterize,
}
#[derive(Clone)]
struct Sheet {
    sid: SessionId,
    kind: SheetKind,
    link: bool,
    controls: Option<Controls>,
    id: Option<u32>,
    sized: bool,
    drag_start: Option<[f32; 2]>,
}
pub fn open(ctx: &egui::Context, sid: SessionId, kind: SheetKind) {
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("w2-image-sheet"),
            Sheet { sid, kind, link: false, controls: None, id: None, sized: false, drag_start: None },
        )
    });
    ctx.request_repaint();
}
pub fn draw(
    ctx: &egui::Context,
    ed: &Editor,
    sid: Option<SessionId>,
    hole: egui::Rect,
    commands: &mut Vec<AppCommand>,
    view: varos_core::geom::View,
    ppp: f32,
) {
    let key = egui::Id::new("w2-image-sheet");
    let Some(mut sheet) = ctx.data(|d| d.get_temp::<Sheet>(key)) else { return };
    if Some(sheet.sid) != sid {
        ctx.data_mut(|d| d.remove::<Sheet>(key));
        return;
    }
    if sheet.id.is_none() {
        sheet.id = ed.doc.images.iter().find(|i| ed.objsel.contains(&i.id)).map(|i| i.id);
        if sheet.kind == SheetKind::Rasterize && sheet.id.is_none() {
            sheet.id = ed
                .selected_image_groups()
                .next()
                .or_else(|| ed.objsel.iter().copied().find(|id| ed.doc.pidx(*id).is_some()));
        }
    }
    if sheet.kind == SheetKind::Rasterize {
        if let Some(group) = ed.selected_image_groups().next() {
            sheet.id = Some(group);
        }
    }
    if sheet.controls.is_none() {
        sheet.controls = sheet.id.and_then(|id| ed.doc.images.iter().find(|i| i.id == id)).map(defaults);
    }
    if sheet.kind == SheetKind::Rasterize && sheet.id.is_some() && sheet.controls.is_none() {
        sheet.controls = Some(Controls {
            bounds: [0., 0., 72., 72.],
            opacity: 1.,
            angle: 0.,
            ppi: ed.doc.raster_effects_ppi,
            white: false,
            preset: 0,
        });
    }
    if sheet.kind == SheetKind::Place && sheet.controls.is_none() {
        sheet.controls =
            Some(Controls { bounds: [0., 0., 72., 72.], opacity: 1., angle: 0., ppi: 300., white: false, preset: 0 });
    }
    if matches!(sheet.kind, SheetKind::Place | SheetKind::Crop) {
        egui::Area::new(key.with("canvas")).order(egui::Order::Middle).fixed_pos(hole.min).show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(hole.size(), egui::Sense::click_and_drag());
            if let Some(pos) = response.interact_pointer_pos() {
                let world = view.s2w([pos.x * ppp, pos.y * ppp]);
                if response.drag_started() {
                    sheet.drag_start = Some(world);
                }
                if response.clicked() {
                    if let Some(c) = &mut sheet.controls {
                        c.bounds[0] = world[0];
                        c.bounds[1] = world[1];
                    }
                }
                if response.dragged() {
                    if let (Some(start), Some(c)) = (sheet.drag_start, sheet.controls.as_mut()) {
                        c.bounds = [
                            start[0].min(world[0]),
                            start[1].min(world[1]),
                            (world[0] - start[0]).abs().max(0.01),
                            (world[1] - start[1]).abs().max(0.01),
                        ];
                        sheet.sized = true;
                    }
                }
            }
            if response.drag_stopped() {
                sheet.drag_start = None;
            }
            if let Some(c) = &sheet.controls {
                let [x, y, w, h] = c.bounds;
                let a = view.w2s([x, y]);
                let b = view.w2s([x + w, y + h]);
                let outline =
                    egui::Rect::from_two_pos(egui::pos2(a[0] / ppp, a[1] / ppp), egui::pos2(b[0] / ppp, b[1] / ppp))
                        .intersect(rect);
                ui.painter().rect_stroke(
                    outline,
                    t::r_ctrl(),
                    egui::Stroke::new(t::KIT_STROKE, t::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        });
    }
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    egui::Area::new(key.with("area")).order(egui::Order::Foreground).fixed_pos(hole.center()).show(ctx, |ui| {
        egui::Frame {
            fill: t::PANEL,
            stroke: egui::Stroke::new(t::KIT_STROKE, t::LINE),
            corner_radius: egui::CornerRadius::same(t::RBOX),
            inner_margin: egui::Margin::same(t::SLICE4A_TOOLS_MARGIN),
            ..Default::default()
        }
        .show(ui, |ui| {
            ui.set_width(t::SLICE4A_TOOLS_HOME_W);
            if matches!(sheet.kind, SheetKind::Place | SheetKind::Crop) {
                ui.label("Click the canvas for position; drag to set bounds.");
            }
            ui.label(match sheet.kind {
                SheetKind::Place => "Place image",
                SheetKind::Crop => "Crop image",
                SheetKind::Trace => "Image Trace",
                SheetKind::Rasterize => "Rasterize",
            });
            if sheet.kind == SheetKind::Place {
                if button(ui, if sheet.link { "Placement: Link" } else { "Placement: Embed" }) {
                    sheet.link = !sheet.link;
                }
                if sheet.controls.is_none() {
                    sheet.controls = Some(Controls {
                        bounds: [0., 0., 72., 72.],
                        opacity: 1.,
                        angle: 0.,
                        ppi: 300.,
                        white: false,
                        preset: 0,
                    });
                }
                if button(ui, if sheet.sized { "Size: specified bounds" } else { "Size: natural physical size" }) {
                    sheet.sized = !sheet.sized;
                }
                if let Some(c) = &mut sheet.controls {
                    for (n, label) in ["X", "Y", "W", "H"].into_iter().enumerate() {
                        if n < 2 || sheet.sized {
                            number(
                                ui,
                                label,
                                &mut c.bounds[n],
                                if n < 2 { -1_000_000.0..=1_000_000.0 } else { 0.01..=1_000_000.0 },
                            );
                        }
                    }
                }
                if button(ui, "Choose image…") {
                    let c = sheet.controls.as_ref();
                    let bounds = c.map(|c| c.bounds);
                    commands.push(AppCommand::ChooseImage(
                        sheet.sid,
                        crate::image_jobs::Options {
                            ppi: None,
                            xform: None,
                            mode: if sheet.link { varos_core::images::PlacementMode::Link } else { Default::default() },
                            at: bounds.map(|b| [b[0], b[1]]).unwrap_or([0.; 2]),
                            bounds: if sheet.sized { bounds } else { None },
                        },
                    ));
                    close = true;
                }
            } else if let (Some(id), Some(c)) = (sheet.id, sheet.controls.as_mut()) {
                match sheet.kind {
                    SheetKind::Crop => {
                        for (n, label) in ["X", "Y", "W", "H"].into_iter().enumerate() {
                            number(
                                ui,
                                label,
                                &mut c.bounds[n],
                                if n < 2 { -1_000_000.0..=1_000_000.0 } else { 0.01..=1_000_000.0 },
                            );
                        }
                        if button(ui, "Apply crop") {
                            commands.push(AppCommand::ImageWorkflow(sheet.sid, Action::Crop(id, c.bounds)));
                            close = true;
                        }
                    }
                    SheetKind::Trace => {
                        let names = ["Black and white", "Grayscale", "6 colors", "16 colors"];
                        if let Some(p) = kit::text_dropdown(
                            ui,
                            ui.id().with("preset"),
                            names[c.preset],
                            &names,
                            t::SLICE4A_TOOLS_FIELD_W,
                            "Trace preset",
                        ) {
                            c.preset = p;
                        }
                        ui.label("Expands accepted pixels to editable paths; one undo.");
                        if button(ui, "Trace and expand") {
                            commands.push(AppCommand::ImageWorkflow(
                                sheet.sid,
                                Action::Trace(
                                    id,
                                    match c.preset {
                                        0 => TraceMode::BlackWhite,
                                        1 => TraceMode::Grayscale,
                                        2 => TraceMode::Color { colors: 6 },
                                        _ => TraceMode::Color { colors: 16 },
                                    },
                                ),
                            ));
                            close = true;
                        }
                    }
                    SheetKind::Rasterize => {
                        number(ui, "PPI", &mut c.ppi, 1.0..=2400.0);
                        if button(ui, if c.white { "Background: white" } else { "Background: transparent" }) {
                            c.white = !c.white;
                        }
                        if button(ui, "Rasterize") {
                            commands.push(AppCommand::ImageWorkflow(
                                sheet.sid,
                                Action::Rasterize(id, c.ppi, c.white.then_some([1.; 4])),
                            ));
                            close = true;
                        }
                    }
                    _ => {}
                }
            } else {
                ui.label("Select an image first.");
            }
            if button(ui, "Cancel") {
                close = true;
            }
        });
    });
    ctx.data_mut(|d| {
        if close {
            d.remove::<Sheet>(key);
        } else {
            d.insert_temp(key, sheet);
        }
    });
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Sheet>(egui::Id::new("w2-image-sheet")).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_sheet_escape_and_tab_switch_publish_nothing() {
        let ctx = egui::Context::default();
        let ed = Editor::new();
        let before = ed.doc.clone();
        let sid = SessionId(1);
        let hole = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900., 700.));
        for kind in [SheetKind::Place, SheetKind::Crop, SheetKind::Trace, SheetKind::Rasterize] {
            open(&ctx, sid, kind);
            let mut commands = vec![];
            let mut input = egui::RawInput { screen_rect: Some(hole), ..Default::default() };
            input.events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
            let _ = ctx.run_ui(input, |_| {
                draw(&ctx, &ed, Some(sid), hole, &mut commands, varos_core::geom::View::identity(), 1.)
            });
            assert!(commands.is_empty());
            assert!(!is_open(&ctx));
            assert_eq!(ed.doc, before);
            open(&ctx, sid, kind);
            let _ = ctx.run_ui(Default::default(), |_| {
                draw(&ctx, &ed, Some(SessionId(2)), hole, &mut commands, varos_core::geom::View::identity(), 1.)
            });
            assert!(!is_open(&ctx));
            assert!(commands.is_empty());
        }
    }
}
