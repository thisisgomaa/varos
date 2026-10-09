//! Lane H: detached typography dependencies and collision-safe paste remapping.
use crate::{
    model::Document,
    typography::{binding_path, Binding, Typography},
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
pub(super) fn story_ids(doc: &Document, ids: &[u32]) -> Vec<u32> {
    let mut selected: BTreeSet<_> = ids.iter().copied().collect();
    loop {
        let before = selected.len();
        for (&id, f) in &doc.typography.frames {
            if let Some(next) = f.next {
                if selected.contains(&id) || selected.contains(&next) {
                    selected.extend([id, next]);
                }
            }
        }
        if selected.len() == before {
            return selected.into_iter().collect();
        }
    }
}
pub(super) fn capture(doc: &Document, ids: &[u32]) -> Typography {
    let mut out = Typography::default();
    for id in ids {
        if let Some(f) = doc.typography.frames.get(id) {
            out.frames.insert(*id, f.clone());
        }
    }
    let mut chars: Vec<_> = out.frames.values().flat_map(|f| f.characters.iter().map(|a| a.name.clone())).collect();
    while let Some(name) = chars.pop() {
        if out.characters.contains_key(&name) {
            continue;
        }
        if let Some(style) = doc.typography.characters.get(&name) {
            chars.extend(style.parent.clone());
            out.characters.insert(name, style.clone());
        }
    }
    let mut paras: Vec<_> = out.frames.values().filter_map(|f| f.paragraph.clone()).collect();
    while let Some(name) = paras.pop() {
        if out.paragraphs.contains_key(&name) {
            continue;
        }
        if let Some(style) = doc.typography.paragraphs.get(&name) {
            paras.extend(style.parent.clone());
            out.paragraphs.insert(name, style.clone());
        }
    }
    out
}
fn names<T>(source: &BTreeMap<String, T>, target: &BTreeMap<String, T>) -> BTreeMap<String, String> {
    let mut taken: BTreeSet<_> = target.keys().chain(source.keys()).cloned().collect();
    source
        .keys()
        .map(|name| {
            let mut result = name.clone();
            if target.contains_key(name) {
                let mut n = 1;
                loop {
                    let suffix = format!(" ({n})");
                    let mut stem = name.clone();
                    while stem.len() + suffix.len() > 128 {
                        stem.pop();
                    }
                    result = format!("{stem}{suffix}");
                    if taken.insert(result.clone()) {
                        break;
                    }
                    n += 1;
                }
            }
            (name.clone(), result)
        })
        .collect()
}
pub(super) fn paste(source: &Typography, doc: &mut Document, ids: &HashMap<u32, u32>, offset: [f32; 2]) {
    let chars = names(&source.characters, &doc.typography.characters);
    let paras = names(&source.paragraphs, &doc.typography.paragraphs);
    for (name, definition) in &source.characters {
        let mut d = definition.clone();
        d.parent = d.parent.and_then(|p| chars.get(&p).cloned());
        doc.typography.characters.insert(chars[name].clone(), d);
    }
    for (name, definition) in &source.paragraphs {
        let mut d = definition.clone();
        d.parent = d.parent.and_then(|p| paras.get(&p).cloned());
        doc.typography.paragraphs.insert(paras[name].clone(), d);
    }
    for (id, frame) in &source.frames {
        let Some(new) = ids.get(id) else { continue };
        let mut f = frame.clone();
        f.next = f.next.and_then(|id| ids.get(&id).copied());
        f.binding = f.binding.and_then(|b| {
            let path = *ids.get(&binding_path(b))?;
            Some(match b {
                Binding::Area { inset, .. } => Binding::Area { path, inset },
                Binding::Path { start, end, offset, flip, effect, .. } => {
                    Binding::Path { path, start, end, offset, flip, effect }
                }
            })
        });
        if let Some(p) = &mut f.binding_origin {
            p[0] += offset[0];
            p[1] += offset[1];
        }
        for a in &mut f.characters {
            if let Some(n) = chars.get(&a.name) {
                a.name.clone_from(n);
            }
        }
        f.paragraph = f.paragraph.and_then(|p| paras.get(&p).cloned());
        doc.typography.frames.insert(*new, f);
    }
}
