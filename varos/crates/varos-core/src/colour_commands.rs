//! Lane B: one checked command boundary for picker, annotator, swatches and recolor.
use crate::{
    editor::{Editor, PaintTarget},
    model::Paint,
    swatches::{self, Swatch},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ColourCommand {
    Tool,
    Paint { target: PaintTarget, paint: Paint },
    Begin,
    Live { target: PaintTarget, paint: Paint },
    Commit,
    Cancel,
    UpsertSwatch { swatch: Swatch },
    DeleteSwatch { id: u32 },
    ImportSwatches { swatches: Vec<Swatch> },
    ImportPalette { format: crate::palette_io::PaletteFormat, data: Vec<u8> },
    Reduce { count: usize },
    Recolor { palette: Vec<crate::geom::Rgba> },
}
pub fn check(ed: &Editor, c: &ColourCommand) -> Result<(), String> {
    if matches!(
        c,
        ColourCommand::Paint { .. }
            | ColourCommand::Live { .. }
            | ColourCommand::Recolor { .. }
            | ColourCommand::Reduce { .. }
    ) && ed.selected_pids().iter().any(|id| ed.doc.eff_hidden(*id) || ed.doc.eff_locked(*id))
    {
        return Err("colour target is hidden or locked".into());
    }
    match c {
        ColourCommand::ImportPalette { format, data } => {
            let entries = crate::palette_io::decode(data, *format)?;
            check(ed, &ColourCommand::ImportSwatches { swatches: entries })
        }
        ColourCommand::Reduce { count } if !(1..=256).contains(count) => Err("recolor count must be in 1..256".into()),
        // ---- w3-cmyk ----
        ColourCommand::Paint { target, paint } | ColourCommand::Live { target, paint } => {
            swatches::validate_paint(paint, &ed.doc)?;
            let selected = ed.selected_pids();
            let mut prospective = ed.doc.clone();
            for p in &mut prospective.paths {
                if selected.contains(&p.id) {
                    match target {
                        PaintTarget::Fill => p.fill = paint.clone(),
                        PaintTarget::Stroke => p.stroke = paint.clone(),
                    }
                }
            }
            crate::colour_management::validate_document(&prospective)
        }
        // ---- end w3-cmyk ----
        ColourCommand::UpsertSwatch { swatch } => {
            let mut d = ed.doc.clone();
            d.swatches.retain(|s| s.id != swatch.id);
            d.swatches.push(swatch.clone());
            swatches::validate_document(&d)
        }
        ColourCommand::ImportSwatches { swatches: entries } => {
            let mut d = ed.doc.clone();
            let mut id = d.swatches.iter().map(|s| s.id).max().unwrap_or(0);
            for s in entries {
                id = id.checked_add(1).ok_or("swatch id exhaustion")?;
                let mut s = s.clone();
                s.id = id;
                d.swatches.push(s);
            }
            swatches::validate_document(&d)
        }
        ColourCommand::DeleteSwatch { id } if !ed.doc.swatches.iter().any(|s| s.id == *id) => {
            Err("unknown swatch".into())
        }
        ColourCommand::Recolor { palette }
            if palette.is_empty()
                || palette.len() > 256
                || palette.iter().any(|c| c.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))) =>
        {
            Err("invalid recolor palette".into())
        }
        _ => Ok(()),
    }
}
fn paint_selected(ed: &mut Editor, target: PaintTarget, paint: Paint) {
    let ids = ed.selected_pids();
    if ids.is_empty() {
        ed.set_current_paint(target, paint.clone());
    }
    for p in &mut ed.doc.paths {
        if ids.contains(&p.id) {
            match target {
                PaintTarget::Fill => p.fill = paint.clone(),
                PaintTarget::Stroke => p.stroke = paint.clone(),
            }
        }
    }
}
pub fn apply(ed: &mut Editor, c: ColourCommand) {
    if check(ed, &c).is_err() {
        return;
    }
    match c {
        ColourCommand::ImportPalette { format, data } => {
            if let Ok(swatches) = crate::palette_io::decode(&data, format) {
                apply(ed, ColourCommand::ImportSwatches { swatches });
            }
        }
        ColourCommand::Reduce { count } => {
            let colours = crate::recolor::selected_colours(ed);
            let palette = crate::recolor::reduced(&colours, count);
            if !palette.is_empty() {
                apply(ed, ColourCommand::Recolor { palette });
            }
        }
        ColourCommand::Tool => ed.set_tool(crate::editor::ToolKind::Gradient),
        ColourCommand::Begin => ed.begin(),
        ColourCommand::Cancel => {
            ed.gradient_tool.drag = None;
            ed.picker_cancel();
        }
        ColourCommand::Commit => ed.finish_document_setup(),
        ColourCommand::Live { target, paint } => {
            if !ed.transaction_open() {
                ed.begin();
            }
            paint_selected(ed, target, paint);
        }
        ColourCommand::Paint { target, paint } => {
            if !ed.transaction_open() {
                ed.begin();
            }
            paint_selected(ed, target, paint);
            ed.finish_document_setup();
        }
        _ => {
            ed.begin();
            match c {
                ColourCommand::Paint { target, paint } => paint_selected(ed, target, paint),
                ColourCommand::UpsertSwatch { swatch } => {
                    if let Some(s) = ed.doc.swatches.iter_mut().find(|s| s.id == swatch.id) {
                        *s = swatch;
                    } else {
                        ed.doc.swatches.push(swatch);
                    }
                }
                ColourCommand::DeleteSwatch { id } => {
                    if let Some(s) = ed.doc.swatches.iter().find(|s| s.id == id) {
                        let paint = s.paint.clone();
                        ed.materialize_current_swatch(id, &paint);
                        for p in &mut ed.doc.paths {
                            for slot in [&mut p.fill, &mut p.stroke] {
                                if *slot == (Paint::SwatchRef { id }) {
                                    *slot = paint.clone();
                                }
                            }
                        }
                    }
                    ed.doc.swatches.retain(|s| s.id != id);
                }
                ColourCommand::ImportSwatches { swatches } => {
                    let mut id = ed.doc.swatches.iter().map(|s| s.id).max().unwrap_or(0);
                    for mut s in swatches {
                        id += 1;
                        s.id = id;
                        ed.doc.swatches.push(s);
                    }
                }
                ColourCommand::Recolor { palette } => {
                    let ids = ed.selected_pids();
                    let doc = ed.doc.clone();
                    for p in &mut ed.doc.paths {
                        if ids.contains(&p.id) {
                            for paint in [&mut p.fill, &mut p.stroke] {
                                *paint = crate::recolor::map_paint(&paint.resolved(&doc), &palette);
                            }
                        }
                    }
                }
                _ => {}
            }
            ed.finish_document_setup();
        }
    }
}
