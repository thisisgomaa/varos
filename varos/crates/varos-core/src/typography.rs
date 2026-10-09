//! Lane H: pure authored typography, live path bindings and undoable style/flow edits.
use crate::{
    editor::Editor,
    model::Document,
    text::{ParaStyle, Run, TextBox, TextStyle},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typography {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub frames: BTreeMap<u32, Frame>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub characters: BTreeMap<String, CharacterStyle>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paragraphs: BTreeMap<String, ParagraphStyle>,
}
impl Typography {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterStyle {
    pub parent: Option<String>,
    pub style: Option<TextStyle>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParagraphStyle {
    pub parent: Option<String>,
    pub style: Option<ParaStyle>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub start: usize,
    pub end: usize,
    pub name: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    #[serde(default)]
    pub binding: Option<Binding>,
    #[serde(default)]
    pub next: Option<u32>,
    #[serde(default)]
    pub characters: Vec<Assignment>,
    #[serde(default)]
    pub paragraph: Option<String>,
    /// Explicit overrides only. Shaper's Arabic-required features remain enabled.
    #[serde(default)]
    pub features: BTreeMap<String, u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Binding {
    Area { path: u32, inset: f32 },
    Path { path: u32, start: f32, end: f32, offset: f32, flip: bool, effect: PathEffect },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PathEffect {
    Rainbow,
    Skew,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Bind { text: u32, binding: Option<Binding> },
    Thread { from: u32, to: Option<u32> },
    DefineCharacter { name: String, definition: CharacterStyle },
    DefineParagraph { name: String, definition: ParagraphStyle },
    ApplyCharacter { text: u32, start: usize, end: usize, name: String },
    ApplyParagraph { text: u32, name: String },
    Features { text: u32, features: BTreeMap<String, u32> },
}
fn named(name: &str) -> Result<(), String> {
    if name.trim() != name || name.is_empty() || name.len() > 128 {
        return Err("invalid style name".into());
    }
    Ok(())
}
impl Typography {
    pub fn character(&self, name: &str) -> Result<TextStyle, String> {
        let mut name = name;
        let mut seen = BTreeSet::new();
        let mut result = None;
        loop {
            if !seen.insert(name) {
                return Err("character style cycle".into());
            }
            let s = self.characters.get(name).ok_or("unknown character style")?;
            if result.is_none() {
                result = s.style.clone();
            }
            match s.parent.as_deref() {
                Some(p) => name = p,
                None => break,
            }
        }
        result.ok_or_else(|| "character style has no definition".into())
    }
    pub fn paragraph(&self, name: &str) -> Result<ParaStyle, String> {
        let mut name = name;
        let mut seen = BTreeSet::new();
        let mut result = None;
        loop {
            if !seen.insert(name) {
                return Err("paragraph style cycle".into());
            }
            let s = self.paragraphs.get(name).ok_or("unknown paragraph style")?;
            if result.is_none() {
                result = s.style.clone();
            }
            match s.parent.as_deref() {
                Some(p) => name = p,
                None => break,
            }
        }
        result.ok_or_else(|| "paragraph style has no definition".into())
    }
    pub fn resolved(&self, text: &TextBox) -> Result<TextBox, String> {
        let mut out = text.clone();
        if let Some(frame) = self.frames.get(&text.id) {
            if let Some(name) = &frame.paragraph {
                out.para = self.paragraph(name)?;
            }
            for a in &frame.characters {
                apply_range(&mut out, a.start, a.end, &self.character(&a.name)?)?;
            }
        }
        out.validate()?;
        Ok(out)
    }
}
pub fn apply_range(text: &mut TextBox, start: usize, end: usize, style: &TextStyle) -> Result<(), String> {
    let source = text.source();
    if start >= end || source.get(start..end).is_none() {
        return Err("invalid style byte range".into());
    }
    let mut pos = 0;
    let mut runs = Vec::new();
    for r in &text.runs {
        let tail = pos + r.text.len();
        for (a, b, replace) in
            [(pos, tail.min(start), false), (pos.max(start), tail.min(end), true), (pos.max(end), tail, false)]
        {
            if a < b {
                runs.push(Run {
                    text: source[a..b].into(),
                    style: if replace { style.clone() } else { r.style.clone() },
                });
            }
        }
        pos = tail;
    }
    text.runs = runs;
    text.validate()
}
pub fn validate(doc: &Document) -> Result<(), String> {
    let t = &doc.typography;
    if t.frames.len() > 4096 || t.characters.len() > 1024 || t.paragraphs.len() > 1024 {
        return Err("typography budget exceeded".into());
    }
    for name in t.characters.keys() {
        named(name)?;
        let s = t.character(name)?;
        // Validate unused definitions as rigorously as applied ones.
        let sample = TextBox {
            id: 0,
            frame: [0., 0.],
            box_kind: crate::text::TextBoxKind::Point,
            runs: vec![Run { text: "".into(), style: s }],
            para: ParaStyle::default(),
        };
        sample.validate()?;
    }
    for name in t.paragraphs.keys() {
        named(name)?;
        let p = t.paragraph(name)?;
        if !p.line_height.is_finite() || !(1.3..=20.).contains(&p.line_height) {
            return Err("invalid paragraph style".into());
        }
    }
    let mut incoming = BTreeSet::new();
    for (&id, f) in &t.frames {
        let text = doc.text_boxes.iter().find(|x| x.id == id).ok_or("dangling text frame")?;
        if f.characters.len() > 4096 || f.features.len() > 64 {
            return Err("frame style budget exceeded".into());
        }
        for (tag, value) in &f.features {
            if tag.len() != 4 || !tag.bytes().all(|b| b.is_ascii_alphanumeric()) || *value > 65535 {
                return Err("invalid OpenType feature".into());
            }
            if crate::text::contains_arabic(&text.source())
                && ["rlig", "ccmp", "locl", "curs", "mark", "mkmk"].contains(&tag.as_str())
                && *value == 0
            {
                return Err("required Arabic shaping feature cannot be disabled".into());
            }
        }
        if let Some(b) = f.binding {
            let path = match b {
                Binding::Area { path, .. } | Binding::Path { path, .. } => path,
            };
            let p = doc.paths.iter().find(|p| p.id == path).ok_or("dangling text path binding")?;
            match b {
                Binding::Area { inset, .. } if !p.closed || !inset.is_finite() || !(0. ..=1e6).contains(&inset) => {
                    return Err("area text requires a closed path and finite inset".into())
                }
                Binding::Path { start, end, offset, .. }
                    if !start.is_finite()
                        || !end.is_finite()
                        || !offset.is_finite()
                        || start < 0.
                        || end < 0.
                        || start > 1e6
                        || end > 1e6
                        || offset.abs() > 1e6 =>
                {
                    return Err("invalid path text brackets".into())
                }
                _ => {}
            }
        }
        if let Some(next) = f.next {
            if next == id || !incoming.insert(next) || !doc.text_boxes.iter().any(|x| x.id == next) {
                return Err("invalid text thread target".into());
            }
            if matches!(f.binding, Some(Binding::Path { .. })) {
                return Err("path text cannot be threaded".into());
            }
            let target = doc.text_boxes.iter().find(|x| x.id == next).ok_or("missing thread target")?;
            if !target.source().is_empty() {
                return Err("thread target must have no independent source".into());
            }
        }
        t.resolved(text)?;
        let mut seen = BTreeSet::new();
        let mut at = Some(id);
        while let Some(i) = at {
            if !seen.insert(i) {
                return Err("text thread cycle".into());
            }
            at = t.frames.get(&i).and_then(|f| f.next);
        }
    }
    Ok(())
}
fn change(doc: &mut Document, action: &Action) -> Result<(), String> {
    match action {
        Action::Bind { text, binding } => doc.typography.frames.entry(*text).or_default().binding = *binding,
        Action::Thread { from, to } => doc.typography.frames.entry(*from).or_default().next = *to,
        Action::DefineCharacter { name, definition } => {
            named(name)?;
            doc.typography.characters.insert(name.clone(), definition.clone());
        }
        Action::DefineParagraph { name, definition } => {
            named(name)?;
            doc.typography.paragraphs.insert(name.clone(), definition.clone());
        }
        Action::ApplyCharacter { text, start, end, name } => {
            let f = doc.typography.frames.entry(*text).or_default();
            let mut retained = Vec::new();
            for a in &f.characters {
                if a.end <= *start || a.start >= *end {
                    retained.push(a.clone());
                    continue;
                }
                if a.start < *start {
                    retained.push(Assignment { start: a.start, end: *start, name: a.name.clone() });
                }
                if a.end > *end {
                    retained.push(Assignment { start: *end, end: a.end, name: a.name.clone() });
                }
            }
            f.characters = retained;
            f.characters.push(Assignment { start: *start, end: *end, name: name.clone() });
        }
        Action::ApplyParagraph { text, name } => {
            doc.typography.frames.entry(*text).or_default().paragraph = Some(name.clone())
        }
        Action::Features { text, features } => {
            doc.typography.frames.entry(*text).or_default().features = features.clone()
        }
    }
    validate(doc)
}
pub fn check(ed: &Editor, action: &Action) -> Result<(), String> {
    let ids: Vec<u32> = match action {
        Action::Bind { text, .. }
        | Action::ApplyCharacter { text, .. }
        | Action::ApplyParagraph { text, .. }
        | Action::Features { text, .. } => vec![*text],
        Action::Thread { from, to } => std::iter::once(*from).chain(*to).collect(),
        _ => ed.doc.text_boxes.iter().map(|t| t.id).collect(),
    };
    for id in ids {
        let text = ed.doc.text_boxes.iter().find(|t| t.id == id).ok_or("missing text")?;
        crate::text::check_change(ed, text, Some(id), None)?;
    }
    change(&mut ed.doc.clone(), action)
}
pub fn execute(ed: &mut Editor, action: Action) -> Result<(), String> {
    check(ed, &action)?;
    let mut doc = ed.doc.clone();
    change(&mut doc, &action)?;
    if doc != ed.doc {
        ed.begin();
        ed.doc = doc;
        ed.dirty = true;
        ed.commit();
    }
    Ok(())
}

/// Called only after an intentional editor deletion, never by the format loader.
pub fn after_delete(doc: &mut Document) {
    let ids: BTreeSet<_> = doc.text_boxes.iter().map(|t| t.id).collect();
    let paths: BTreeSet<_> = doc.paths.iter().map(|p| p.id).collect();
    doc.typography.frames.retain(|id, _| ids.contains(id));
    for frame in doc.typography.frames.values_mut() {
        if frame.next.is_some_and(|id| !ids.contains(&id)) {
            frame.next = None;
        }
        let path = frame.binding.map(|b| match b {
            Binding::Area { path, .. } | Binding::Path { path, .. } => path,
        });
        if path.is_some_and(|id| !paths.contains(&id)) {
            frame.binding = None;
        }
    }
}

pub fn story_root(doc: &Document, mut id: u32) -> Result<u32, String> {
    let mut seen = BTreeSet::new();
    while let Some((&parent, _)) = doc.typography.frames.iter().find(|(_, f)| f.next == Some(id)) {
        if !seen.insert(id) {
            return Err("text thread cycle".into());
        }
        id = parent;
    }
    Ok(id)
}

/// Preserve named spans through the single replacement represented by a text edit.
/// Prefix/suffix boundaries are Unicode scalar boundaries; no authored bytes are rewritten.
pub(crate) fn remap_characters(frame: &mut Frame, old: &str, new: &str) {
    if old == new {
        return;
    }
    let prefix = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).map(|(c, _)| c.len_utf8()).sum::<usize>();
    let suffix = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum::<usize>();
    let old_end = old.len() - suffix;
    let new_end = new.len() - suffix;
    for span in &mut frame.characters {
        let start = span.start;
        if span.start > prefix || (span.start == prefix && prefix > 0 && old_end == prefix) {
            span.start = if span.start >= old_end { new_end + span.start - old_end } else { prefix };
        }
        if span.end > prefix || (span.end == prefix && start < prefix) {
            span.end = if span.end >= old_end { new_end + span.end - old_end } else { new_end };
        }
    }
    frame.characters.retain(|span| span.start < span.end);
}
