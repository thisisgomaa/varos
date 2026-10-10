//! Lane B provisional Swatches home and Recolor sheet; existing kit, deferred commands only.
use super::*;
use varos_app::shell::{kit::field as kf, tokens as t};
use varos_core::{colour_commands::ColourCommand as C, model::Paint, palette_io::PaletteFormat, swatches::Swatch};
#[derive(Clone, Default)]
struct State {
    selected: Option<u32>,
    path: String,
    group: String,
    recolor: bool,
    palette: Vec<Rgba>,
    count: f32,
}
fn action(ui: &mut egui::Ui, label: &str) -> bool {
    let response = kit::action(ui, kit::Control::new(ui.id().with(label), label), false);
    #[cfg(test)]
    super::fields::tests::probe(label, response.response.rect);
    !kf::blocked(ui.ctx()) && response.activated
}
fn text(ui: &mut egui::Ui, key: &str, value: &str) -> Option<String> {
    let (r, _) = ui.allocate_exact_size(egui::vec2(t::GRADIENT_BAR_W, t::FIELD_H), egui::Sense::hover());
    kf::text_field(
        ui,
        kf::TextField { id: ui.id().with(key), rect: r, value, font: t::mono(), framed: true, open: false, hint: key },
        |s| if s.len() <= 4096 { Ok(s.to_owned()) } else { Err("Text too long") },
    )
    .commit
}
fn current(ed: &Editor) -> Paint {
    let ids = ed.selected_pids();
    ed.doc
        .paths
        .iter()
        .find(|p| ids.contains(&p.id))
        .map(|p| if ed.paint == PaintTarget::Fill { p.appearance().fill() } else { p.appearance().stroke() })
        .map_or(ed.current_paint(ed.paint), |p| p.resolved(&ed.doc))
}
fn reduced(ed: &Editor, count: usize) -> Vec<Rgba> {
    varos_core::recolor::reduced(&varos_core::recolor::selected_colours(ed), count)
}
pub(crate) fn panel(ui: &mut egui::Ui, ed: &Editor, ops: &mut Vec<Op>) {
    // ---- w3-cmyk ----
    super::colour_management::panel(ui, ed, ops);
    let key = egui::Id::new("lane-b-swatches").with(&ed.doc.name);
    let mut state = ui.ctx().data(|d| d.get_temp::<State>(key)).unwrap_or_default();
    ui.horizontal(|ui| {
        if action(ui, "Add current paint") {
            let id = ed.doc.swatches.iter().map(|s| s.id).max().unwrap_or(0).saturating_add(1);
            ops.push(Op::Colour(C::UpsertSwatch {
                swatch: Swatch {
                    id,
                    name: format!("Swatch {id}"),
                    paint: current(ed),
                    global: false,
                    group: state.group.clone(),
                },
            }));
            state.selected = Some(id);
        }
        if action(ui, "Basics library") {
            ops.push(Op::Colour(C::ImportSwatches { swatches: varos_core::palette_io::library() }));
        }
    });
    let mut groups = vec!["All".to_owned()];
    for s in &ed.doc.swatches {
        if !s.group.is_empty() && !groups.contains(&s.group) {
            groups.push(s.group.clone());
        }
    }
    let refs = groups.iter().map(String::as_str).collect::<Vec<_>>();
    if let Some(i) = kit::text_dropdown(
        ui,
        ui.id().with("swatch-groups"),
        if state.group.is_empty() { "All" } else { &state.group },
        &refs,
        t::GRADIENT_BAR_W,
        "Colour group",
    ) {
        state.group = if i == 0 { String::new() } else { groups[i].clone() };
    }
    if ed.doc.swatches.is_empty() {
        kit::text(ui, "Add artwork colours or import a library", t::body(), t::MUTED);
    }
    egui::ScrollArea::vertical().max_height(t::GRADIENT_LIST_H * 2.).show(ui, |ui| {
        for s in ed.doc.swatches.iter().filter(|s| state.group.is_empty() || s.group == state.group) {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::Vec2::splat(t::PICKER_HARMONY_SWATCH), egui::Sense::hover());
                if let Some(c) = s.paint.sample([50., 0.], &ed.doc) {
                    ui.painter().rect_filled(r, t::r_ctrl(), rgba_c32a(c));
                }
                let label = format!("{}{}", s.name, if s.global { " · Global" } else { "" });
                if action(ui, &label) {
                    state.selected = Some(s.id);
                    ops.push(Op::Colour(C::Paint {
                        target: ed.paint,
                        paint: if s.global { Paint::SwatchRef { id: s.id } } else { s.paint.clone() },
                    }));
                }
            });
        }
    });
    if let Some(s) = state.selected.and_then(|id| ed.doc.swatches.iter().find(|s| s.id == id)) {
        let mut next = s.clone();
        let mut changed = false;
        if let Some(name) = text(ui, "Swatch name", &s.name) {
            next.name = name;
            changed = true;
        }
        if let Some(group) = text(ui, "Group", &s.group) {
            next.group = group;
            changed = true;
        }
        ui.horizontal(|ui| {
            if action(ui, if s.global { "Global: On" } else { "Global: Off" }) {
                next.global = !s.global;
                changed = true;
            }
            if action(ui, "Update from artwork") {
                next.paint = current(ed);
                changed = true;
            }
            if action(ui, "Delete swatch") {
                ops.push(Op::Colour(C::DeleteSwatch { id: s.id }));
            }
        });
        if changed {
            ops.push(Op::Colour(C::UpsertSwatch { swatch: next }));
        }
    }
    kit::text(ui, "Libraries · GPL / ASE / JSON", t::body(), t::TEXT);
    if let Some(path) = text(ui, "Palette file path", &state.path) {
        state.path = path;
    }
    ui.horizontal(|ui| {
        if action(ui, "Import") {
            ops.push(Op::PaletteFile { path: state.path.clone(), export: false });
        }
        if action(ui, "Export") {
            ops.push(Op::PaletteFile { path: state.path.clone(), export: true });
        }
    });
    if action(ui, "Recolor Artwork") {
        state.recolor = true;
        state.count = 6.;
        state.palette = reduced(ed, 6);
    }
    if state.recolor {
        recolor_sheet(ui, ed, &mut state, ops);
    }
    if let Some(error) = &ed.colour_error {
        kit::text(ui, error, t::body(), t::MUTED);
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, state));
}
fn recolor_sheet(ui: &mut egui::Ui, ed: &Editor, state: &mut State, ops: &mut Vec<Op>) {
    kit::text(ui, "Recolor Artwork", t::body(), t::TEXT);
    kit::text(ui, "Preview palette · Lab reduction, ΔE2000 matching", t::micro(), t::MUTED);
    let edit = kf::number_field(
        ui,
        kf::NumberField {
            id: ui.id().with("recolor-count"),
            width: t::GRADIENT_NUMBER_W,
            label: kf::Label::Letter("N"),
            tip: "Number of colours",
            value: state.count,
            decimals: 0,
            speed: 1.,
            range: 1. ..=16.,
            disabled: false,
        },
    );
    if let Some(n) = edit.commit.or(edit.live) {
        state.count = n;
        state.palette = reduced(ed, n as usize);
    }
    if action(ui, "Use swatch group") {
        state.palette = ed
            .doc
            .swatches
            .iter()
            .filter(|s| state.group.is_empty() || s.group == state.group)
            .filter_map(|s| s.paint.solid())
            .collect();
    }
    for (i, c) in state.palette.iter_mut().enumerate() {
        if let Some(value) = text(ui, &format!("Replacement {}", i + 1), &hex_of(*c)) {
            if let Some(next) = parse_hex(&value) {
                *c = next;
            }
        }
    }
    ui.horizontal(|ui| {
        if action(ui, "Apply recolor") && !state.palette.is_empty() {
            ops.push(Op::Colour(C::Recolor { palette: state.palette.clone() }));
            state.recolor = false;
        }
        if action(ui, "Close recolor") {
            state.recolor = false;
        }
    });
}
pub(crate) fn file(ed: &mut Editor, path: String, export: bool) {
    let format = match std::path::Path::new(&path)
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gpl") => PaletteFormat::Gpl,
        Some("ase") => PaletteFormat::Ase,
        Some("json") => PaletteFormat::Native,
        _ => {
            ed.colour_error = Some("Use a .gpl, .ase or .json path".into());
            return;
        }
    };
    let result = (|| -> Result<(), String> {
        if export {
            let bytes = varos_core::palette_io::encode(&ed.doc.swatches, format)?;
            let mut f =
                std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut f, &bytes).map_err(|e| e.to_string())
        } else {
            use std::io::Read;
            let f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let mut bytes = vec![];
            f.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            let swatches = varos_core::palette_io::decode(&bytes, format)?;
            ed.try_execute(EditCommand::Colour(C::ImportSwatches { swatches }))
        }
    })();
    ed.colour_error = result.err();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swatches_home_is_idle_and_library_action_routes_checked_command() {
        let ctx = egui::Context::default();
        varos_app::shell::fonts::install(&ctx);
        t::apply(&ctx);
        let mut ed = Editor::new();
        let original = ed.doc.clone();
        let mut ops = vec![];
        let frame = |events: Vec<egui::Event>, ops: &mut Vec<Op>, ed: &Editor| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900., 900.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    panel(ui, ed, ops);
                },
            )
        };
        let _ = frame(vec![], &mut ops, &ed);
        assert!(ops.is_empty());
        assert_eq!(ed.doc, original);
        let at = super::super::fields::tests::probed_rect("Basics library", 0).center();
        for pressed in [true, false] {
            let _ = frame(
                vec![
                    egui::Event::PointerMoved(at),
                    egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
                &mut ops,
                &ed,
            );
        }
        for op in ops {
            if let Op::Colour(c) = op {
                ed.try_execute(EditCommand::Colour(c)).unwrap();
            }
        }
        assert_eq!(ed.doc.swatches.len(), 5);
        ed.undo();
        assert_eq!(ed.doc, original);
    }
}
