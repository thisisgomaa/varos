//! Lane B provisional Gradient tab: type/actions, stop bar, Stops list and radial placement fields.
use super::*;
use varos_app::shell::kit::field::{self as kf, TextField};
use varos_core::{
    colour_commands::ColourCommand as C,
    gradient::{Gradient, GradientKind, Spread, Stop},
    model::Paint,
};
#[derive(Default)]
pub(super) struct State {
    pub paint: Option<Gradient>,
    pub stop: usize,
    pub dragging: Option<usize>,
    pub editing: bool,
}
fn target(m: &ColorPanel) -> Option<PaintTarget> {
    if let MTarget::Paint(t) = m.target {
        Some(t)
    } else {
        None
    }
}
fn send(ops: &mut Vec<Op>, target: PaintTarget, g: &Gradient, live: bool) {
    ops.push(Op::Colour(if live {
        C::Live { target, paint: Paint::Gradient(g.clone()) }
    } else {
        C::Paint { target, paint: Paint::Gradient(g.clone()) }
    }));
}
pub(super) fn seed(m: &mut ColorPanel, ed: &Editor) {
    if m.gradient.dragging.is_some() || m.gradient.editing {
        return;
    }
    let Some(target) = target(m) else { return };
    let ids = ed.selected_pids();
    let next = ed.doc.paths.iter().find(|p| ids.contains(&p.id)).and_then(|p| {
        match p
            .appearance()
            .paint(match target {
                PaintTarget::Fill => varos_core::appearance::BaseSlot::Fill,
                PaintTarget::Stroke => varos_core::appearance::BaseSlot::Stroke,
            })
            .resolved(&ed.doc)
        {
            Paint::Gradient(g) => Some(g),
            _ => None,
        }
    });
    m.gradient.paint = next;
}
fn button(ui: &mut egui::Ui, icon: Icon, tip: &str) -> bool {
    kit::icon_button(ui, ui.id().with(tip), icon, tip, kit::IconState::Action).activated
}
fn number(
    ui: &mut egui::Ui,
    key: &str,
    label: &str,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    _ops: &mut Vec<Op>,
) -> Option<(f32, bool)> {
    let edit = kf::number_field(
        ui,
        kf::NumberField {
            id: ui.id().with(key),
            width: t::GRADIENT_NUMBER_W,
            label: kf::Label::Letter(label),
            tip: key,
            value,
            decimals: 2,
            speed: 0.01,
            range,
            disabled: false,
        },
    );
    if let Some(v) = edit.live {
        return Some((v, true));
    }
    edit.commit.map(|v| (v, false))
}
pub(super) fn show(ui: &mut egui::Ui, m: &mut ColorPanel, ops: &mut Vec<Op>) {
    let Some(target) = target(m) else {
        kit::text(ui, "Select artwork to apply a gradient", t::body(), t::MUTED);
        return;
    };
    let mut g = m.gradient.paint.clone().unwrap_or_default();
    let mut changed = false;
    ui.horizontal(|ui| {
        if let Some(i) = kit::text_dropdown(
            ui,
            ui.id().with("gradient-type"),
            if g.kind == GradientKind::Linear { "Linear" } else { "Radial" },
            &["Linear", "Radial"],
            t::PICKER_MODE_W,
            "Gradient type",
        ) {
            g.kind = if i == 0 { GradientKind::Linear } else { GradientKind::Radial };
            changed = true;
        }
        if button(ui, Icon::FlipH, "Flip gradient") {
            g.reverse();
            changed = true;
        }
        if button(ui, Icon::Landscape, "Rotate gradient 90 degrees") {
            let [a, b, c, d, e, f] = g.placement;
            g.placement = [-b, a, -d, c, e, f];
            changed = true;
        }
    });
    if let Some(i) = kit::text_dropdown(
        ui,
        ui.id().with("gradient-spread"),
        match g.spread {
            Spread::Pad => "Pad",
            Spread::Reflect => "Reflect",
            Spread::Repeat => "Repeat",
        },
        &["Pad", "Reflect", "Repeat"],
        t::PICKER_MODE_W,
        "Gradient spread",
    ) {
        g.spread = [Spread::Pad, Spread::Reflect, Spread::Repeat][i];
        changed = true;
    }
    let (bar, resp) =
        ui.allocate_exact_size(egui::vec2(t::GRADIENT_BAR_W, t::GRADIENT_BAR_H), egui::Sense::click_and_drag());
    #[cfg(test)]
    super::super::fields::tests::probe("gradient bar", bar);
    checker(&ui.painter_at(bar), bar, t::PICKER_CHECKER);
    for i in 0..128 {
        let x = bar.left() + bar.width() * i as f32 / 128.;
        let r =
            egui::Rect::from_min_max(egui::pos2(x, bar.top()), egui::pos2(x + bar.width() / 128. + 1., bar.bottom()));
        ui.painter().rect_filled(r, t::r_ctrl(), rgba_c32a(g.sample_pad(i as f32 / 127.)));
    }
    if resp.drag_started() {
        if let Some(p) = resp.interact_pointer_pos() {
            let offset = ((p.x - bar.left()) / bar.width()).clamp(0., 1.);
            let i = g
                .stops
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1.offset - offset).abs().total_cmp(&(b.1.offset - offset).abs()))
                .map_or(0, |s| s.0);
            m.gradient.stop = i;
            m.gradient.dragging = Some(i);
            ops.push(Op::Colour(C::Begin));
        }
    }
    if resp.dragged() {
        if let (Some(i), Some(p)) = (m.gradient.dragging, resp.interact_pointer_pos()) {
            let lo = i.checked_sub(1).and_then(|j| g.stops.get(j)).map_or(0., |s| s.offset);
            let hi = g.stops.get(i + 1).map_or(1., |s| s.offset);
            g.stops[i].offset = ((p.x - bar.left()) / bar.width()).clamp(lo, hi);
            send(ops, target, &g, true);
        }
    }
    if resp.double_clicked() && g.stops.len() < 256 {
        if let Some(p) = resp.interact_pointer_pos() {
            let offset = ((p.x - bar.left()) / bar.width()).clamp(0., 1.);
            let colour = g.sample_pad(offset);
            g.stops.push(Stop::new(offset, colour));
            g.stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            changed = true;
        }
    }
    for (i, s) in g.stops.iter().enumerate() {
        let p = egui::pos2(bar.left() + bar.width() * s.offset, bar.bottom());
        ui.painter().circle_filled(p, t::GRADIENT_STOP_R, if i == m.gradient.stop { t::ACCENT } else { t::TEXT });
    }
    if resp.drag_stopped() {
        m.gradient.dragging = None;
        ops.push(Op::Colour(C::Commit));
    }
    ui.horizontal(|ui| {
        kit::text(ui, "Stops", t::body(), t::TEXT);
        if button(ui, Icon::Plus, "Add stop") && g.stops.len() < 256 {
            let offset = 0.5;
            g.stops.push(Stop::new(offset, g.sample_pad(offset)));
            g.stops.sort_by(|a, b| a.offset.total_cmp(&b.offset));
            changed = true;
        }
        if button(ui, Icon::Trash, "Delete selected stop") && g.stops.len() > 2 {
            g.stops.remove(m.gradient.stop.min(g.stops.len() - 1));
            changed = true;
        }
    });
    m.gradient.stop = m.gradient.stop.min(g.stops.len() - 1);
    let selected = m.gradient.stop;
    egui::ScrollArea::vertical().max_height(t::GRADIENT_LIST_H).show(ui, |ui| {
        for (i, s) in g.stops.iter().enumerate() {
            ui.horizontal(|ui| {
                let label = format!("{} · {}%", hex_of(s.colour), (s.offset * 100.).round());
                if kit::action(ui, kit::Control::new(ui.id().with(("select-stop", i)), &label), i == m.gradient.stop)
                    .activated
                    && !kf::blocked(ui.ctx())
                {
                    m.gradient.stop = i;
                }
            });
        }
    });
    let stop = g.stops[selected].clone();
    let lo = selected.checked_sub(1).and_then(|j| g.stops.get(j)).map_or(0., |s| s.offset);
    let hi = g.stops.get(selected + 1).map_or(1., |s| s.offset);
    for (key, label, v, range) in [
        ("Stop offset", "%", stop.offset * 100., lo * 100. ..=hi * 100.),
        ("Stop opacity", "A", stop.opacity * 100., 0. ..=100.),
        ("Stop midpoint", "M", stop.midpoint * 100., 1. ..=99.),
    ] {
        if let Some((v, live)) = number(ui, key, label, v, range, ops) {
            match key {
                "Stop offset" => g.stops[selected].offset = v / 100.,
                "Stop opacity" => g.stops[selected].opacity = v / 100.,
                _ => g.stops[selected].midpoint = v / 100.,
            }
            send(ops, target, &g, live);
            m.gradient.editing = live;
            if !live {
                ops.push(Op::Colour(C::Commit));
            }
        }
    }
    let shown = hex_of(stop.colour);
    let (r, _) = ui.allocate_exact_size(egui::vec2(t::GRADIENT_BAR_W, t::FIELD_H), egui::Sense::hover());
    #[cfg(test)]
    super::super::fields::tests::probe("gradient colour", r);
    let edit = kf::text_field(
        ui,
        TextField {
            id: ui.id().with("gradient-stop-colour"),
            rect: r,
            value: &shown,
            font: t::mono(),
            framed: true,
            open: false,
            hint: "Hex colour",
        },
        |s| parse_hex(s).ok_or("Type a hex colour"),
    );
    if let Some(c) = edit.commit {
        g.stops[selected].colour = c;
        changed = true;
    }
    if g.kind == GradientKind::Radial {
        for (i, label) in [(0, "Fx"), (1, "Fy")] {
            if let Some((v, live)) = number(ui, label, label, g.focal[i], -0.98..=0.98, ops) {
                g.focal[i] = v;
                let n = g.focal[0].hypot(g.focal[1]);
                if n >= 0.99 {
                    g.focal = g.focal.map(|v| v * 0.98 / n);
                }
                send(ops, target, &g, live);
                m.gradient.editing = live;
                if !live {
                    ops.push(Op::Colour(C::Commit));
                }
            }
        }
    }
    if m.gradient.editing && !ui.input(|i| i.pointer.primary_down()) {
        ops.push(Op::Colour(C::Commit));
        m.gradient.editing = false;
    }
    if m.gradient.paint.is_none() {
        changed = true;
    }
    if changed {
        send(ops, target, &g, false);
    }
    m.gradient.paint = Some(g);
}
