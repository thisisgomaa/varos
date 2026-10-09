//! Lane G: persisted, engine-independent text data and checked edit operations.
use crate::{
    editor::Editor,
    model::{Document, GroupRole, Node, NodeKind, Xform},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontRef {
    pub family: String,
    pub weight: u32,
    /// SHA-256 of the exact face bytes. Never a host-specific filesystem path.
    pub hash: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    pub font: FontRef,
    pub size: f32,
    pub letter_spacing: f32,
    pub fill: [f32; 4],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub text: String,
    pub style: TextStyle,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum TextBoxKind {
    Point,
    Area([f32; 4]),
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Auto,
    Ltr,
    Rtl,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Alignment {
    Left,
    Centre,
    Right,
    Justify,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Kashida {
    Off,
    Minimal,
    Balanced,
    Display,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParaStyle {
    pub align: Alignment,
    pub kashida: Kashida,
    pub direction: Direction,
    /// Minimum baseline distance as a multiple of em; ink may expand it.
    pub line_height: f32,
}
impl Default for ParaStyle {
    fn default() -> Self {
        Self { align: Alignment::Right, kashida: Kashida::Off, direction: Direction::Auto, line_height: 1.6 }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextBox {
    pub id: u32,
    pub box_kind: TextBoxKind,
    /// Baseline anchor for point text; area origin is carried in box_kind.
    pub frame: [f32; 2],
    pub runs: Vec<Run>,
    pub para: ParaStyle,
}
impl TextBox {
    pub fn source(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.runs.is_empty()
            || self.runs.len() > 4096
            || self.runs.iter().map(|r| r.text.len()).sum::<usize>() > 1_048_576
        {
            return Err("text resource limit exceeded".into());
        }
        if self.frame.iter().any(|n| !n.is_finite())
            || !self.para.line_height.is_finite()
            || !(1.3..=20.).contains(&self.para.line_height)
        {
            return Err("invalid text frame or line height".into());
        }
        if let TextBoxKind::Area(r) = self.box_kind {
            if r.iter().any(|n| !n.is_finite()) || r[2] <= 0. || r[3] <= 0. {
                return Err("invalid text area".into());
            }
        }
        for run in &self.runs {
            let s = &run.style;
            if s.font.hash.len() != 64
                || !s.font.hash.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
                || s.font.family.is_empty()
                || s.font.family.len() > 256
                || !(1..=1000).contains(&s.font.weight)
                || !s.size.is_finite()
                || !(0.1..=4096.).contains(&s.size)
                || !s.letter_spacing.is_finite()
                || s.letter_spacing.abs() > s.size
                || s.fill.iter().any(|n| !n.is_finite() || !(0. ..=1.).contains(n))
            {
                return Err("invalid text style".into());
            }
            if contains_arabic(&run.text) && s.letter_spacing != 0. {
                return Err("Arabic letter spacing must be zero".into());
            }
        }
        Ok(())
    }
}
pub fn contains_arabic(text: &str) -> bool {
    text.chars().any(|c| matches!(c as u32, 0x600..=0x8ff | 0xfb50..=0xfdff | 0xfe70..=0xfeff))
}

pub fn validate_document(doc: &Document) -> Result<(), String> {
    crate::typography::validate(doc)?;
    if doc.text_boxes.len() > 4096 {
        return Err("text box limit exceeded".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut bytes = 0usize;
    let mut ownership = std::collections::HashMap::<u32, usize>::new();
    for node in &doc.nodes {
        if let NodeKind::Text(id) = node.kind {
            *ownership.entry(id).or_default() += 1;
        }
    }
    for t in &doc.text_boxes {
        t.validate()?;
        bytes += t.runs.iter().map(|r| r.text.len()).sum::<usize>();
        if bytes > 8 * 1024 * 1024 || t.id == 0 || !ids.insert(t.id) {
            return Err("duplicate text id or text budget exceeded".into());
        }
        if ownership.get(&t.id) != Some(&1) {
            return Err("text must have exactly one leaf".into());
        }
    }
    for n in &doc.nodes {
        if let NodeKind::Text(id) = n.kind {
            if !ids.contains(&id) || !n.children.is_empty() {
                return Err("invalid text leaf".into());
            }
        }
    }
    Ok(())
}
pub fn check_parent(ed: &Editor, parent: Option<u32>) -> Result<(), String> {
    let mut id = Some(parent.unwrap_or(ed.doc.active_layer));
    let n = ed.doc.node(id.ok_or("missing parent")?).ok_or("missing text parent")?;
    if !matches!(n.kind, NodeKind::Layer | NodeKind::Group) {
        return Err("text parent must be a container".into());
    }
    while let Some(i) = id {
        let n = ed.doc.node(i).ok_or("missing ancestor")?;
        if n.locked || n.hidden {
            return Err("text parent is locked or hidden".into());
        }
        id = n.parent;
    }
    Ok(())
}
pub fn add(ed: &mut Editor, mut text: TextBox, parent: Option<u32>) -> Result<u32, String> {
    check_change(ed, &text, None, parent)?;
    if ed.doc.text_boxes.len() >= 4096 || ed.doc.ids > u32::MAX - 2 {
        return Err("text allocation limit exceeded".into());
    }
    ed.begin();
    let id = ed.doc.nid();
    text.id = id;
    let node = ed.doc.nid();
    let parent = parent.unwrap_or(ed.doc.active_layer);
    ed.doc.nodes.push(Node {
        id: node,
        kind: NodeKind::Text(id),
        name: "Text".into(),
        parent: Some(parent),
        children: vec![],
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
    });
    if let Some(n) = ed.doc.node_mut(parent) {
        n.children.insert(0, node);
    }
    ed.doc.text_boxes.push(text);
    ed.dirty = true;
    ed.commit();
    Ok(id)
}
pub fn set(ed: &mut Editor, id: u32, mut text: TextBox) -> Result<(), String> {
    check_change(ed, &text, Some(id), None)?;
    let n = ed.doc.nodes.iter().find(|n| n.kind == NodeKind::Text(id)).ok_or("missing text node")?;
    if n.locked || n.hidden {
        return Err("text is locked or hidden".into());
    }
    check_parent(ed, n.parent)?;
    let index = ed.doc.text_boxes.iter().position(|t| t.id == id).ok_or("missing text")?;
    text.id = id;
    if ed.doc.text_boxes[index] == text {
        return Ok(());
    }
    ed.begin();
    if ed.doc.text_boxes[index].source() != text.source() {
        if let Some(frame) = ed.doc.typography.frames.get_mut(&id) {
            crate::typography::remap_characters(frame, &ed.doc.text_boxes[index].source(), &text.source());
        }
    }
    ed.doc.text_boxes[index] = text;
    ed.dirty = true;
    ed.commit();
    Ok(())
}

/// Preflight allocating/replacement edits without touching history or authored state.
pub fn check_change(ed: &Editor, text: &TextBox, replacing: Option<u32>, parent: Option<u32>) -> Result<(), String> {
    text.validate()?;
    if let Some(id) = replacing {
        let n = ed.doc.nodes.iter().find(|n| n.kind == NodeKind::Text(id)).ok_or("missing text")?;
        if n.hidden || n.locked {
            return Err("text is hidden or locked".into());
        }
        check_parent(ed, n.parent)?;
        if !ed.doc.text_boxes.iter().any(|t| t.id == id) {
            return Err("missing text storage".into());
        }
    } else {
        check_parent(ed, parent)?;
        if ed.doc.text_boxes.len() >= 4096
            || ed.doc.nodes.len() >= crate::format::Limits::DEFAULT.max_nodes
            || ed.allocation_floor() > u32::MAX - 2
        {
            return Err("text allocation limit exceeded".into());
        }
    }
    let bytes = ed
        .doc
        .text_boxes
        .iter()
        .filter(|t| Some(t.id) != replacing)
        .flat_map(|t| &t.runs)
        .map(|r| r.text.len())
        .sum::<usize>()
        + text.runs.iter().map(|r| r.text.len()).sum::<usize>();
    if bytes > 8 * 1024 * 1024 {
        return Err("text document budget exceeded".into());
    }
    if let Some(id) = replacing {
        let mut doc = ed.doc.clone();
        if let Some(old) = doc.text_boxes.iter_mut().find(|t| t.id == id) {
            if old.source() != text.source() {
                if let Some(f) = doc.typography.frames.get_mut(&id) {
                    crate::typography::remap_characters(f, &old.source(), &text.source());
                }
            }
            *old = text.clone();
            old.id = id;
        }
        crate::typography::validate(&doc)?;
    }
    Ok(())
}

pub fn node_id(doc: &Document, id: u32) -> Option<u32> {
    doc.nodes.iter().find(|n| n.kind == NodeKind::Text(id)).map(|n| n.id)
}
pub fn subtree_has_text(doc: &Document, root: u32) -> bool {
    doc.node(root)
        .is_some_and(|n| matches!(n.kind, NodeKind::Text(_)) || n.children.iter().any(|c| subtree_has_text(doc, *c)))
}
pub fn selected_ids(ed: &Editor) -> Vec<u32> {
    ed.doc.text_boxes.iter().filter(|t| ed.objsel.contains(&t.id) && editable(ed, t.id).is_ok()).map(|t| t.id).collect()
}
pub fn editable(ed: &Editor, id: u32) -> Result<(), String> {
    let n = node_id(&ed.doc, id).and_then(|n| ed.doc.node(n)).ok_or("missing text")?;
    if n.hidden || n.locked || !ed.in_isolation(id) {
        return Err("text is hidden, locked or outside isolation".into());
    }
    check_parent(ed, n.parent)
}
pub fn translate(text: &mut TextBox, delta: [f32; 2]) {
    text.frame[0] += delta[0];
    text.frame[1] += delta[1];
    if let TextBoxKind::Area(r) = &mut text.box_kind {
        r[0] += delta[0];
        r[1] += delta[1];
    }
}
pub fn translation_only(s: crate::select_transform::Transform) -> bool {
    s.scale == [1., 1.] && s.angle == 0. && s.reflect.is_none() && s.shear == 0. && !s.copy && !s.random
}
pub fn translate_selected(ed: &mut Editor, spec: crate::select_transform::Transform) -> bool {
    if !translation_only(spec) {
        return false;
    }
    let ids = selected_ids(ed);
    for id in &ids {
        let mut at = node_id(&ed.doc, *id).and_then(|n| ed.doc.node(n));
        let mut xf = at.map(|n| n.xform).unwrap_or_default();
        while let Some(n) = at {
            if n.kind == NodeKind::Group {
                xf = n.xform;
            }
            at = n.parent.and_then(|p| ed.doc.node(p));
        }
        let delta = crate::geom::rotate_about(spec.movement, [0., 0.], -xf.rot);
        if let Some(t) = ed.doc.text_boxes.iter_mut().find(|t| t.id == *id) {
            translate(t, delta);
        }
    }
    !ids.is_empty() && spec.movement != [0., 0.]
}
pub fn remove(doc: &mut Document, ids: &[u32]) {
    let nodes: Vec<_> = ids.iter().filter_map(|id| node_id(doc, *id)).collect();
    doc.text_boxes.retain(|t| !ids.contains(&t.id));
    doc.nodes.retain(|n| !nodes.contains(&n.id));
    for n in &mut doc.nodes {
        n.children.retain(|c| !nodes.contains(c));
    }
}

/// A mixed translation keeps every live unit transform, avoiding baking a shared text/path group.
pub fn translate_objects(ed: &mut Editor, spec: crate::select_transform::Transform) -> bool {
    let changed = translate_selected(ed, spec);
    for id in ed.structural_object_paths() {
        let xf = ed.doc.unit_xform(id);
        let delta = crate::geom::rotate_about(spec.movement, [0., 0.], -xf.rot);
        let moved = |p: [f32; 2]| [p[0] + delta[0], p[1] + delta[1]];
        if let Some(pi) = ed.doc.pidx(id) {
            let path = &mut ed.doc.paths[pi];
            for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
                a.p = moved(a.p);
                a.hin = a.hin.map(moved);
                a.hout = a.hout.map(moved);
            }
        }
    }
    // integration w2: a mixed text + image selection moves its (editable) images too
    let images: Vec<u32> = ed
        .doc
        .images
        .iter()
        .map(|i| i.id)
        .filter(|id| ed.objsel.contains(id) && !ed.doc.eff_hidden(*id) && !ed.doc.eff_locked(*id))
        .collect();
    for id in images {
        let xf = ed.doc.unit_xform(id);
        let delta = crate::geom::rotate_about(spec.movement, [0., 0.], -xf.rot);
        if let Some(image) = ed.doc.images.iter_mut().find(|i| i.id == id) {
            image.xform.e += delta[0];
            image.xform.f += delta[1];
        }
    }
    changed
}
