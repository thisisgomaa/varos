//! Lane C: provisional controls from the existing kit; owner design review pending.
use super::*;
use varos_app::shell::{kit::field as kf, tokens as t};
use varos_core::colour_management::{Cmyk, Colour, ColourMode, ManagedColour};
use varos_core::colour_management_commands::Command;
pub(crate) fn mode(ui: &mut egui::Ui, value: &mut ColourMode) -> bool {
    if let Some(i) = kit::text_dropdown(
        ui,
        ui.id().with("colour-mode"),
        if *value == ColourMode::Rgb { "RGB" } else { "CMYK" },
        &["RGB", "CMYK"],
        t::DOC_UNITS_W,
        "Document colour mode",
    ) {
        *value = if i == 0 { ColourMode::Rgb } else { ColourMode::Cmyk };
        return true;
    }
    false
}
pub(crate) fn setup(ui: &mut egui::Ui, ed: &Editor, ops: &mut Vec<Op>) {
    let mut value = ed.doc.colour_mode;
    if mode(ui, &mut value) {
        ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Mode { mode: value })));
    }
    kit::notice(ui, ed.doc.output_profile.as_ref().map_or("Unprofiled CMYK approximation", |p| p.name.as_str()));
}
#[derive(Clone)]
pub(crate) struct State {
    source: Option<Source>,
    pub(crate) original: Option<ManagedColour>,
    pub(crate) values: [f32; 4],
    pub(crate) spot: bool,
    pub(crate) name: String,
    pub(crate) tint: f32,
    pub(crate) alpha: f32,
}
#[derive(Clone, PartialEq)]
struct Source {
    target: PaintTarget,
    paints: Vec<(u32, varos_core::model::Paint)>,
}
pub(crate) fn source_paint(ed: &Editor, target: PaintTarget) -> varos_core::model::Paint {
    let ids = ed.selected_pids();
    ed.doc.paths.iter().find(|p| ids.contains(&p.id)).map_or_else(
        || ed.current_paint(target).resolved(&ed.doc),
        |p| if target == PaintTarget::Fill { p.fill.resolved(&ed.doc) } else { p.stroke.resolved(&ed.doc) },
    )
}
impl State {
    pub(crate) fn from_paint(paint: &varos_core::model::Paint) -> Self {
        let rgba = paint.representative().unwrap_or([0., 0., 0., 1.]);
        let mut state = Self {
            source: None,
            original: if let varos_core::model::Paint::Managed(m) = paint { Some(m.clone()) } else { None },
            values: Cmyk::from_rgb(rgba).channels().map(|v| v * 100.),
            spot: false,
            name: "Spot 1".into(),
            tint: 100.,
            alpha: rgba[3],
        };
        if let varos_core::model::Paint::Managed(m) = paint {
            let channels = match &m.colour {
                Colour::Cmyk { c, m, y, k } => Some([*c, *m, *y, *k]),
                Colour::Spot { name, tint, alt } => {
                    state.spot = true;
                    state.name = name.clone();
                    state.tint = tint * 100.;
                    Some(alt.channels())
                }
                _ => None,
            };
            if let Some(channels) = channels {
                state.values = channels.map(|v| v * 100.);
            }
        }
        state
    }
    fn sync(&mut self, ed: &Editor) {
        let ids = ed.selected_pids();
        let source = Source {
            target: ed.paint,
            paints: ed
                .doc
                .paths
                .iter()
                .filter(|p| ids.contains(&p.id))
                .map(|p| {
                    (
                        p.id,
                        if ed.paint == PaintTarget::Fill {
                            p.fill.resolved(&ed.doc)
                        } else {
                            p.stroke.resolved(&ed.doc)
                        },
                    )
                })
                .collect(),
        };
        let source = if source.paints.is_empty() {
            Source { paints: vec![(0, ed.current_paint(ed.paint).resolved(&ed.doc))], ..source }
        } else {
            source
        };
        if self.source.as_ref() != Some(&source) {
            *self = Self::from_paint(&source_paint(ed, ed.paint));
            self.source = Some(source);
        }
    }
}
/// Keep unchanged source components bit-for-bit: percent display scaling is not lossless in f32.
pub(crate) fn source_colour(
    values: [f32; 4],
    spot: bool,
    name: &str,
    tint: f32,
    alpha: f32,
    original: Option<&ManagedColour>,
) -> ManagedColour {
    let channels = original.and_then(|m| match &m.colour {
        Colour::Cmyk { c, m, y, k } => Some([*c, *m, *y, *k]),
        Colour::Spot { alt, .. } => Some(alt.channels()),
        _ => None,
    });
    let [c, m, y, k] =
        std::array::from_fn(|i| channels.filter(|v| values[i] == v[i] * 100.).map_or(values[i] / 100., |v| v[i]));
    let tint = original
        .and_then(|m| match &m.colour {
            Colour::Spot { tint: t, .. } if tint == t * 100. => Some(*t),
            _ => None,
        })
        .unwrap_or(tint / 100.);
    ManagedColour {
        colour: if spot {
            Colour::Spot { name: name.into(), tint, alt: Cmyk { c, m, y, k } }
        } else {
            Colour::Cmyk { c, m, y, k }
        },
        alpha,
    }
}
fn number(ui: &mut egui::Ui, key: egui::Id, name: &str, value: &mut f32) {
    let e = kf::number_field(
        ui,
        kf::NumberField {
            id: key.with(name),
            width: t::GRADIENT_NUMBER_W,
            label: kf::Label::Letter(name),
            tip: name,
            value: *value,
            decimals: 1,
            speed: 0.5,
            range: 0.0..=100.0,
            disabled: false,
        },
    );
    if let Some(v) = e.commit.or(e.live) {
        *value = v;
    }
}
pub(crate) fn panel(ui: &mut egui::Ui, ed: &Editor, ops: &mut Vec<Op>) {
    for (label, enabled, command) in [
        ("Proof Colours", ed.colour_preview.proof, Command::Proof { enabled: !ed.colour_preview.proof }),
        (
            "Overprint Preview (approx.)",
            ed.colour_preview.overprint,
            Command::Overprint { enabled: !ed.colour_preview.overprint },
        ),
    ] {
        let mut control = kit::Control::new(ui.id().with(label), label);
        control.selected = enabled;
        if kit::action(ui, control, false).activated {
            ops.push(Op::DocumentSetup(EditCommand::ColourManagement(command)));
        }
    }
    if ed.colour_preview.overprint {
        kit::notice(ui, "All vector paints multiply; ink separations are not simulated.");
    }
    kit::text(ui, "CMYK / Spot", t::body(), t::TEXT);
    let key = doc_id(ui, "managed-colour");
    let mut state =
        ui.ctx().data(|d| d.get_temp::<State>(key)).unwrap_or_else(|| State::from_paint(&source_paint(ed, ed.paint)));
    state.sync(ed);
    // Scope field buffers to the current selection, target and external document revision.
    let mut ids: Vec<_> = ed.selected_pids().into_iter().collect();
    ids.sort_unstable();
    let field_key = key.with((ids, ed.paint == PaintTarget::Fill, ed.rev));
    if let Some(i) = kit::text_dropdown(
        ui,
        field_key.with("kind"),
        if state.spot { "Spot" } else { "CMYK" },
        &["CMYK", "Spot"],
        t::DOC_UNITS_W,
        "Colour model",
    ) {
        state.spot = i == 1;
    }
    ui.horizontal(|ui| {
        for (name, v) in ["C", "M", "Y", "K"].into_iter().zip(&mut state.values) {
            number(ui, field_key, name, v);
        }
    });
    if state.spot {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t::GRADIENT_BAR_W, t::FIELD_H), egui::Sense::hover());
        if let Some(name) = kf::text_field(
            ui,
            kf::TextField {
                id: field_key.with("name"),
                rect,
                value: &state.name,
                font: t::body(),
                framed: true,
                open: false,
                hint: "Spot name",
            },
            |s| if !s.trim().is_empty() && s.len() <= 256 { Ok(s.to_owned()) } else { Err("Enter a spot name") },
        )
        .commit
        {
            state.name = name;
        }
        number(ui, field_key, "Tint", &mut state.tint);
    }
    if kit::action(ui, kit::Control::new(key.with("apply"), "Apply colour"), false).activated {
        ops.push(Op::DocumentSetup(EditCommand::ColourManagement(Command::Paint {
            target: ed.paint,
            colour: source_colour(
                state.values,
                state.spot,
                &state.name,
                state.tint,
                state.alpha,
                state.original.as_ref(),
            ),
        })));
    }
    kit::text(ui, "Unprofiled screen approximation", t::micro(), t::MUTED);
    ui.ctx().data_mut(|d| d.insert_temp(key, state));
}

#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::model::{Paint, Path};
    fn paint(c: f32, alpha: f32) -> Paint {
        Paint::Managed(ManagedColour {
            colour: Colour::Spot { name: "Brand".into(), tint: 0.65, alt: Cmyk { c, m: 0.3, y: 0.4, k: 0.1 } },
            alpha,
        })
    }
    #[test]
    fn panel_preserves_drafts_until_source_selection_or_target_changes() {
        let mut ed = Editor::new();
        for (id, c, a) in [(1, 0.2, 0.7), (2, 0.8, 0.4)] {
            let mut p = Path::new(id, vec![], true, None, Some([0.1; 4]), 1.);
            p.fill = paint(c, a);
            ed.doc.paths.push(p);
        }
        ed.doc.sync_tree();
        ed.objsel.insert(1);
        let mut state = State::from_paint(&source_paint(&ed, ed.paint));
        state.sync(&ed);
        assert_eq!(state.values, [0.2, 0.3, 0.4, 0.1].map(|v| v * 100.));
        assert_eq!(state.alpha, 0.7);
        assert_eq!(state.name, "Brand");
        assert_eq!(state.tint, 65.);
        state.values[0] = 55.;
        for _ in 0..4 {
            state.sync(&ed);
            assert_eq!(state.values[0], 55.);
        }
        ed.objsel.clear();
        ed.objsel.insert(2);
        state.sync(&ed);
        assert_eq!(state.values, [0.8, 0.3, 0.4, 0.1].map(|v| v * 100.));
        assert_eq!(state.alpha, 0.4);
        ed.doc.paths[1].fill = paint(0.6, 0.2);
        state.sync(&ed);
        assert_eq!(state.values[0], 0.6 * 100.);
        assert_eq!(state.alpha, 0.2);
        ed.paint = PaintTarget::Stroke;
        state.sync(&ed);
        assert!(!state.spot);
        assert_eq!(state.alpha, 0.1);
    }
    #[test]
    fn document_scoped_panel_ids_keep_drafts_separate() {
        let ctx = egui::Context::default();
        let mut keys = Vec::new();
        for session in [1, 2] {
            super::super::controls::set_doc_salt(&ctx, Some(crate::app_command::SessionId(session)));
            let _ = ctx.run_ui(Default::default(), |ui| {
                keys.push(doc_id(ui, "managed-colour"));
            });
        }
        assert_ne!(keys[0], keys[1]);
    }
}
