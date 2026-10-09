use super::*;
use varos_app::shell::kit::field::{self as kf, TextField};

pub(super) fn route(e: kf::Edit<Rgba>, m: &mut ColorPanel, ops: &mut Vec<Op>) {
    if e.editing || (e.commit.is_some() && m.gesture_active()) {
        m.finish(ops);
        m.eyedropping = false;
    }
    let target = m.target;
    if let Some(c) = e.live {
        ops.push(Op::PickerSet(target, c));
    }
    if let Some(c) = e.commit {
        ops.push(Op::Field(Box::new(Op::PickerSet(target, c))));
    }
    if let Some(c) = e.pending {
        ops.push(Op::FieldPending(e.id, Box::new(Op::PickerSet(target, c))));
    }
}
pub(crate) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, ops: &mut Vec<Op>, with_hex: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t::PICKER_PAD;
        ui.add_space(t::PICKER_PAD);
        if with_hex {
            hex(ui, m, ops);
        }
        if !with_hex {
            kit::text(ui, "A", t::mono(), t::MUTED);
        }
        alpha(
            ui,
            m,
            ops,
            if m.mini() {
                t::PICKER_MINI_ALPHA_W
            } else if with_hex {
                t::PICKER_ALPHA_W
            } else {
                t::PICKER_SLIDER_W + t::PICKER_ALPHA_KNOB * 2.0
            },
        );
    });
}
pub(super) fn hex(ui: &mut egui::Ui, m: &mut ColorPanel, ops: &mut Vec<Op>) {
    let shown = if m.mixed { String::new() } else { hex_of(m.color()) };
    let (r, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_HEX_W, t::FIELD_H), egui::Sense::hover());
    #[cfg(test)]
    super::super::fields::tests::probe("picker hex", r);
    let alpha = m.hsva[3];
    let parse = |s: &str| {
        parse_hex(s)
            .map(|mut c| {
                if s.trim().trim_start_matches('#').len() != 8 {
                    c[3] = alpha;
                }
                c
            })
            .ok_or("Type a hex colour")
    };
    route(
        kf::text_field(
            ui,
            TextField {
                id: doc_id(ui, "picker-v3-hex"),
                rect: r,
                value: &shown,
                font: t::mono(),
                framed: true,
                open: false,
                hint: if m.mixed { "Mixed" } else { "#" },
            },
            parse,
        ),
        m,
        ops,
    );
}
pub(super) fn alpha(ui: &mut egui::Ui, m: &mut ColorPanel, ops: &mut Vec<Op>, width: f32) {
    let (r, response) = ui.allocate_exact_size(
        egui::vec2(width, t::FIELD_H),
        if kf::blocked(ui.ctx()) { egui::Sense::hover() } else { egui::Sense::click_and_drag() },
    );
    #[cfg(test)]
    super::super::fields::tests::probe("picker alpha slider", r);
    let track = egui::Rect::from_center_size(
        r.center(),
        egui::vec2(r.width() - t::PICKER_ALPHA_KNOB * 2.0, t::PICKER_ALPHA_TRACK),
    );
    checker(&ui.painter_at(track), track, t::PICKER_CHECKER);
    let mut mesh = egui::Mesh::default();
    let c = m.color();
    mesh.colored_vertex(track.left_top(), Color32::TRANSPARENT);
    mesh.colored_vertex(track.left_bottom(), Color32::TRANSPARENT);
    mesh.colored_vertex(track.right_top(), rgba_c32a([c[0], c[1], c[2], 1.0]));
    mesh.colored_vertex(track.right_bottom(), rgba_c32a([c[0], c[1], c[2], 1.0]));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    ui.painter().add(egui::Shape::mesh(mesh));
    if response.is_pointer_button_down_on() || response.clicked() {
        m.start(Gesture::Alpha, ops);
        if let Some(p) = response.interact_pointer_pos() {
            m.hsva[3] = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            m.change_requested = true;
        }
    }
    let knob = egui::pos2(track.left() + m.hsva[3] * track.width(), track.center().y);
    ui.painter().circle(knob, t::PICKER_ALPHA_KNOB, t::PICKER_WHITE, Stroke::new(t::KIT_STROKE, t::PICKER_BLACK));
    response.on_hover_text("Alpha");
    if m.mini() {
        if kit::icon_button_sized(
            ui,
            ui.id().with("mini-eye"),
            Icon::Pipette,
            "Eyedropper (I)",
            kit::IconState::Toggle(m.eyedropping),
            egui::vec2(t::PICKER_TAB_W, t::PICKER_TAB_H),
            t::PICKER_GLYPH,
        )
        .activated
            && !kf::blocked(ui.ctx())
        {
            if m.eyedropping {
                m.finish(ops);
            } else {
                m.arm();
            }
        }
        return;
    }
    let shown = format!("{:.0}%", m.hsva[3] * 100.0);
    let (r, _) = ui.allocate_exact_size(egui::vec2(t::PICKER_PERCENT_W, t::FIELD_H), egui::Sense::hover());
    #[cfg(test)]
    super::super::fields::tests::probe("picker alpha", r);
    let c = m.color();
    let parse = |s: &str| {
        s.trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| [c[0], c[1], c[2], v.clamp(0.0, 100.0) / 100.0])
            .ok_or("Type an alpha percentage")
    };
    route(
        kf::text_field(
            ui,
            TextField {
                id: doc_id(ui, "picker-v3-alpha"),
                rect: r,
                value: &shown,
                font: t::mono(),
                framed: true,
                open: false,
                hint: "%",
            },
            parse,
        ),
        m,
        ops,
    );
}
