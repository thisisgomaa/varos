//! Transient previews retain authored effects separately and publish one command on OK.
use crate::{
    effects::{Action, Effect},
    EditCommand, Editor,
};
#[derive(Clone)]
pub struct Preview {
    pub ids: Vec<u32>,
    originals: Vec<Vec<Effect>>,
    pub revision: u64,
}
pub fn preview(ed: &mut Editor, ids: &[u32], effects: &[Effect]) {
    if ed.effects_preview.as_ref().is_some_and(|p| p.ids != ids || p.revision != ed.rev) {
        cancel(ed);
    }
    if ed.effects_preview.is_none() {
        ed.effects_preview = Some(Preview {
            ids: ids.to_vec(),
            originals: ids.iter().filter_map(|id| ed.doc.pidx(*id).map(|i| ed.doc.paths[i].effects.clone())).collect(),
            revision: ed.rev,
        });
    }
    for id in ids {
        if let Some(i) = ed.doc.pidx(*id) {
            ed.doc.paths[i].effects = effects.to_vec();
        }
    }
}
pub fn cancel(ed: &mut Editor) {
    if let Some(p) = ed.effects_preview.take() {
        if p.revision != ed.rev {
            return;
        }
        for (id, effects) in p.ids.iter().zip(p.originals) {
            if let Some(i) = ed.doc.pidx(*id) {
                ed.doc.paths[i].effects = effects;
            }
        }
    }
}

pub fn appended(ed: &Editor, ids: &[u32], effect: &Effect) -> Vec<(u32, Vec<Effect>)> {
    ids.iter()
        .filter_map(|id| {
            ed.doc.pidx(*id).map(|i| {
                let mut effects = ed
                    .effects_preview
                    .as_ref()
                    .filter(|p| p.revision == ed.rev && p.ids == ids)
                    .and_then(|p| p.ids.iter().position(|pid| pid == id).map(|i| p.originals[i].clone()))
                    .unwrap_or_else(|| ed.doc.paths[i].effects.clone());
                effects.push(effect.clone());
                (*id, effects)
            })
        })
        .collect()
}
pub fn preview_append(ed: &mut Editor, ids: &[u32], effect: &Effect) {
    let paths = appended(ed, ids, effect);
    if ed.effects_preview.as_ref().is_some_and(|p| p.ids != ids || p.revision != ed.rev) {
        cancel(ed);
    }
    if ed.effects_preview.is_none() {
        ed.effects_preview = Some(Preview {
            ids: ids.to_vec(),
            originals: ids.iter().filter_map(|id| ed.doc.pidx(*id).map(|i| ed.doc.paths[i].effects.clone())).collect(),
            revision: ed.rev,
        });
    }
    for (id, effects) in paths {
        if let Some(i) = ed.doc.pidx(id) {
            ed.doc.paths[i].effects = effects;
        }
    }
}
pub fn finish(ed: &mut Editor, accept: bool) {
    let paths = ed
        .effects_preview
        .as_ref()
        .map(|p| {
            p.ids.iter().filter_map(|id| ed.doc.pidx(*id).map(|i| (*id, ed.doc.paths[i].effects.clone()))).collect()
        })
        .unwrap_or_default();
    cancel(ed);
    if accept {
        ed.execute_ui(EditCommand::LiveEffects(Action::SetPerPath { paths }));
    }
}
