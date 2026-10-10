//! Lane G: product seam. Core holds data; this headless adapter owns fonts, layout and outlines.
//! No discovery occurs while shaping. A host supplies an immutable FontSet snapshot.
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};
use std::collections::HashMap;
use varos_core::{
    model::{Anchor, Document, Node, NodeKind, Path},
    text::*,
};
use varos_text::{
    composer::{Alignment as A, ParagraphOptions},
    outlines::Command,
    Engine, FaceId, FontSet, Layout, Request,
};

pub fn bundled_fonts() -> Result<FontSet, String> {
    use varos_text::{FallbackPolicy, FontFace};
    let inter = FontFace::new(
        "Inter",
        400,
        include_bytes!("../../varos-text/assets/fonts/Inter-Regular.ttf").as_slice().into(),
    )?;
    let arabic = FontFace::new(
        "IBM Plex Sans Arabic",
        400,
        include_bytes!("../../varos-text/assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice().into(),
    )?;
    Ok(FontSet::new(vec![inter, arabic], FallbackPolicy { common: vec![FaceId(1), FaceId(0)], scripts: vec![] })?)
}
pub fn default_text(source: &str, frame: [f32; 2]) -> Result<TextBox, String> {
    let fonts = bundled_fonts()?;
    let font = &fonts.faces()[1];
    Ok(TextBox {
        id: 0,
        frame,
        box_kind: TextBoxKind::Point,
        runs: vec![Run {
            text: source.into(),
            style: TextStyle {
                font: FontRef {
                    family: font.family.into(),
                    weight: u32::from(font.weight),
                    hash: font_hash(font.content_hash),
                },
                size: 24.,
                letter_spacing: 0.,
                fill: [0., 0., 0., 1.],
            },
        }],
        para: ParaStyle::default(),
    })
}
#[derive(Clone, Debug)]
pub struct Composed {
    pub layout: Layout,
    pub paths: Vec<Path>,
    pub origin: [f32; 2],
    pub overset: bool,
}
struct Entry {
    source: TextBox,
    bucket: i32,
    output: Composed,
    used: u64,
}
pub struct TextLayout {
    engine: Engine,
    flow_cache: Option<flow::Cache>,
    cache: HashMap<u32, Entry>,
    shaping: HashMap<u32, (TextBox, Layout, u64)>,
    clock: u64,
    pub layouts: u64,
}
impl TextLayout {
    pub fn new(fonts: FontSet) -> Result<Self, String> {
        Ok(Self {
            engine: Engine::new(fonts)?,
            flow_cache: None,
            cache: HashMap::new(),
            shaping: HashMap::new(),
            clock: 0,
            layouts: 0,
        })
    }
    pub fn bundled() -> Result<Self, String> {
        Self::new(bundled_fonts()?)
    }
    pub fn fonts(&self) -> &FontSet {
        self.engine.font_set()
    }
    pub fn compose(&mut self, text: &TextBox, zoom: f32) -> Result<&Composed, String> {
        text.validate()?;
        if !zoom.is_finite() || zoom <= 0. {
            return Err("invalid text zoom".into());
        }
        let bucket = (zoom.log2().ceil() as i32).clamp(-8, 12);
        self.clock += 1;
        if let Some(entry) = self.cache.get_mut(&text.id) {
            entry.used = self.clock;
        }
        if self.cache.get(&text.id).is_some_and(|e| e.source == *text && e.bucket == bucket) {
            return Ok(&self.cache[&text.id].output);
        }
        let mut layout =
            if let Some((_, layout, used)) = self.shaping.get_mut(&text.id).filter(|(source, _, _)| source == text) {
                *used = self.clock;
                layout.clone()
            } else {
                let layout = self.shape(text)?;
                self.shaping.remove(&text.id);
                // Independent bounds: document-sized small layouts, capped source/glyph retention.
                while self.shaping.len() >= 4096
                    || self
                        .shaping
                        .values()
                        .map(|(t, _, _)| t.runs.iter().map(|r| r.text.len()).sum::<usize>())
                        .sum::<usize>()
                        + text.source().len()
                        > 8 * 1024 * 1024
                    || self
                        .shaping
                        .values()
                        .map(|(_, l, _)| l.lines.iter().map(|line| line.glyphs.len()).sum::<usize>())
                        .sum::<usize>()
                        + layout.lines.iter().map(|line| line.glyphs.len()).sum::<usize>()
                        > 1_000_000
                {
                    let Some(id) = self.shaping.iter().min_by_key(|(_, (_, _, used))| *used).map(|(id, _)| *id) else {
                        break;
                    };
                    self.shaping.remove(&id);
                }
                self.shaping.insert(text.id, (text.clone(), layout.clone(), self.clock));
                layout
            };
        let first = text.runs.first().ok_or("text requires a style")?;
        let origin = match text.box_kind {
            TextBoxKind::Point => [text.frame[0], text.frame[1] - layout.lines.first().map_or(0., |l| l.baseline)],
            TextBoxKind::Area(r) => [r[0], r[1]],
        };
        let overset = matches!(text.box_kind, TextBoxKind::Area(r) if layout.lines.last().is_some_and(|l| l.baseline + l.descent > r[3]));
        let mut paths = Vec::new();
        let mut vertices = 0usize;
        if layout.lines.iter().map(|l| l.glyphs.len()).sum::<usize>() > 16_384 {
            return Err("text glyph limit exceeded".into());
        }
        for glyph in layout
            .lines
            .iter()
            .filter(|line| match text.box_kind {
                TextBoxKind::Point => true,
                TextBoxKind::Area(r) => line.baseline + line.descent <= r[3],
            })
            .flat_map(|l| &l.glyphs)
        {
            let mut start = 0;
            let mut fill = first.style.fill;
            for run in &text.runs {
                if (start..start + run.text.len()).contains(&glyph.cluster.start) {
                    fill = run.style.fill;
                    break;
                }
                start += run.text.len();
            }
            let outline = varos_text::outlines::glyph_outline(self.engine.font_set(), glyph)?;
            let glyph_paths = outline_paths(&outline.commands, origin, fill, 0.1 / 2f32.powi(bucket))?;
            vertices += glyph_paths.iter().map(path_vertices).sum::<usize>();
            if vertices > 250_000 {
                return Err("text outline budget exceeded".into());
            }
            paths.extend(glyph_paths);
        }
        // Refresh ink bounds after tracking/point alignment using the same coverage we draw.
        layout.ink_bounds = paths
            .iter()
            .flat_map(|p| p.anchors.iter().chain(p.holes.iter().flatten()))
            .map(|a| [a.p[0] - origin[0], a.p[1] - origin[1]])
            .fold(None, |bounds, p| {
                Some(match bounds {
                    None => [p[0], p[1], p[0], p[1]],
                    Some([x0, y0, x1, y1]) => [x0.min(p[0]), y0.min(p[1]), x1.max(p[0]), y1.max(p[1])],
                })
            });
        let retained = self.cache.values().flat_map(|e| &e.output.paths).map(path_vertices).sum::<usize>();
        let mut retained = retained;
        while self.cache.len() >= 256 || retained + vertices > 500_000 {
            let Some(id) = self.cache.iter().min_by_key(|(_, e)| e.used).map(|(id, _)| *id) else { break };
            if let Some(old) = self.cache.remove(&id) {
                retained = retained.saturating_sub(old.output.paths.iter().map(path_vertices).sum::<usize>());
            }
        }
        self.cache.insert(
            text.id,
            Entry {
                source: text.clone(),
                bucket,
                used: self.clock,
                output: Composed { layout, paths, origin, overset },
            },
        );
        Ok(&self.cache[&text.id].output)
    }
    fn shape(&mut self, text: &TextBox) -> Result<Layout, String> {
        self.shape_features(text, &Default::default())
    }
    fn shape_features(
        &mut self,
        text: &TextBox,
        features: &std::collections::BTreeMap<String, u32>,
    ) -> Result<Layout, String> {
        let source = text.source();
        if source.len() > 65_536 {
            return Err("text layout limit exceeded: 64 KiB per frame".into());
        }
        let first = text.runs.first().ok_or("text requires a style")?;
        let resolve = |s: &TextStyle| {
            self.engine
                .font_set()
                .faces()
                .iter()
                .position(|f| {
                    f.family == s.font.family
                        && u32::from(f.weight) == s.font.weight
                        && font_hash(f.content_hash) == s.font.hash
                })
                .map(FaceId)
                .ok_or_else(|| format!("missing font snapshot: {}", s.font.family))
        };
        let width = match text.box_kind {
            TextBoxKind::Point => None,
            TextBoxKind::Area(r) => Some(r[2]),
        };
        let mut req = Request::new(&source, first.style.size, width);
        req.face = resolve(&first.style)?;
        for (tag, value) in features {
            req.features.push(varos_text::Feature {
                tag: tag.as_bytes().try_into().map_err(|_| "invalid feature tag")?,
                value: *value,
            });
        }
        req.direction = match text.para.direction {
            Direction::Auto => varos_text::Direction::Auto,
            Direction::Ltr => varos_text::Direction::Ltr,
            Direction::Rtl => varos_text::Direction::Rtl,
        };
        let mut at = 0;
        for run in &text.runs {
            let end = at + run.text.len();
            if end == at {
                continue;
            }
            req.styles.push(varos_text::Style {
                range: at..end,
                face: resolve(&run.style)?,
                size: run.style.size,
                baseline_shift: 0.,
                paint: 0,
            });
            at = end;
        }
        let options = ParagraphOptions {
            alignment: match text.para.align {
                Alignment::Left => A::Left,
                Alignment::Centre => A::Centre,
                Alignment::Right => A::Right,
                Alignment::Justify => {
                    if text.para.direction == Direction::Ltr {
                        A::JustifyLastLeft
                    } else {
                        A::JustifyLastRight
                    }
                }
            },
            kashida: match text.para.kashida {
                Kashida::Off => varos_text::kashida::Kashida::Off,
                Kashida::Minimal => varos_text::kashida::Kashida::Minimal,
                Kashida::Balanced => varos_text::kashida::Kashida::Balanced,
                Kashida::Display => varos_text::kashida::Kashida::Display,
            },
            line_height: varos_text::metrics::LineHeight::Multiple(text.para.line_height),
            ..Default::default()
        };
        let mut layout = self.engine.compose(&req, &options)?;
        self.layouts += 1;
        tracking::apply(&mut layout, text);
        if text.box_kind == TextBoxKind::Point {
            // Point text aligns each baseline around the authored anchor, without inventing a wrap width.
            for (index, line) in layout.lines.iter_mut().enumerate() {
                let shift = match text.para.align {
                    Alignment::Left => 0.,
                    Alignment::Centre => -line.width * 0.5,
                    Alignment::Right => -line.width,
                    Alignment::Justify => {
                        if line.rtl {
                            -line.width
                        } else {
                            0.
                        }
                    }
                };
                for glyph in &mut line.glyphs {
                    glyph.x += shift;
                }
                line.empty_caret_x += shift;
                for caret in layout.carets.iter_mut().filter(|c| c.line == index) {
                    caret.x += shift;
                }
            }
        }
        Ok(layout)
    }
    /// Transient render/export snapshot: replace each text leaf IN PLACE, preserving order/ancestors.
    pub fn outlined(&mut self, source: &Document, zoom: f32) -> Result<Document, String> {
        let mut doc = source.clone();
        for text in &source.text_boxes {
            let output = self.compose_document(source, text, zoom)?;
            let index = doc.nodes.iter().position(|n| n.kind == NodeKind::Text(text.id)).ok_or("missing text leaf")?;
            let extra = output.paths.iter().map(path_vertices).sum::<usize>() + output.paths.len() * 2;
            if u64::from(doc.ids) + extra as u64 > u64::from(u32::MAX) {
                return Err("text outline ids exhausted".into());
            }
            if doc.paths.iter().map(path_vertices).sum::<usize>() + extra > 1_000_000 {
                return Err("document text outline budget exceeded".into());
            }
            let parent = doc.nodes[index].id;
            doc.nodes[index].kind = NodeKind::Group;
            for mut path in output.paths.into_iter().rev() {
                path.id = doc.nid();
                for a in path.anchors.iter_mut().chain(path.holes.iter_mut().flatten()) {
                    a.id = doc.nid();
                }
                let id = doc.nid();
                let node = Node {
                    id,
                    kind: NodeKind::Path(path.id),
                    parent: Some(parent),
                    children: vec![],
                    name: String::new(),
                    hidden: false,
                    locked: false,
                    color: None,
                    clip_exempt: false,
                    xform: Default::default(),
                    role: Default::default(),
                    // ---- Lane A ----
                    look: None,
                    mask_child: None,
                };
                doc.nodes[index].children.push(id);
                doc.nodes.push(node);
                doc.paths.push(path);
            }
        }
        doc.text_boxes.clear();
        doc.typography = Default::default();
        doc.sync_tree();
        Ok(doc)
    }
}
// Flatten each glyph independently then normalize NonZero coverage into disjoint even-odd paths.
// Overlapping glyphs remain separate paint operations, preserving their original paint order.
fn outline_paths(commands: &[Command], origin: [f32; 2], fill: [f32; 4], tolerance: f32) -> Result<Vec<Path>, String> {
    let mut contours: Vec<Vec<[f64; 2]>> = vec![];
    let mut current = [0., 0.];
    for command in commands {
        match *command {
            Command::Move(p) => {
                contours.push(vec![[f64::from(p[0] + origin[0]), f64::from(p[1] + origin[1])]]);
                current = p;
            }
            Command::Line(p) => {
                if let Some(c) = contours.last_mut() {
                    c.push([f64::from(p[0] + origin[0]), f64::from(p[1] + origin[1])]);
                }
                current = p;
            }
            Command::Cubic(a, b, p) => {
                if let Some(c) = contours.last_mut() {
                    flatten(current, a, b, p, tolerance, 0, origin, c)?;
                }
                current = p;
            }
            Command::Close => {}
        }
    }
    let empty: Vec<Vec<[f64; 2]>> = Vec::new();
    Ok(contours
        .overlay(&empty, OverlayRule::Union, FillRule::NonZero)
        .into_iter()
        .filter_map(|shape| {
            let mut rings = shape.into_iter().map(|r| {
                r.into_iter()
                    .map(|p| Anchor { id: 0, p: [p[0] as f32, p[1] as f32], hin: None, hout: None, smooth: false })
                    .collect::<Vec<_>>()
            });
            let outer = rings.next()?;
            let mut path = Path::new(0, outer, true, Some(fill), None, 0.);
            path.holes = rings.collect();
            Some(path)
        })
        .collect())
}
fn path_vertices(path: &Path) -> usize {
    path.anchors.len() + path.holes.iter().map(Vec::len).sum::<usize>()
}
#[allow(clippy::too_many_arguments)]
fn flatten(
    p: [f32; 2],
    a: [f32; 2],
    b: [f32; 2],
    q: [f32; 2],
    tol: f32,
    depth: u8,
    origin: [f32; 2],
    out: &mut Vec<[f64; 2]>,
) -> Result<(), String> {
    if out.len() >= 65_536 {
        return Err("glyph outline budget exceeded".into());
    }
    let mid = |p: [f32; 2], q: [f32; 2]| [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
    let chord = varos_core::geom::dist(p, q);
    let hull = varos_core::geom::dist(p, a) + varos_core::geom::dist(a, b) + varos_core::geom::dist(b, q);
    if depth >= 16 || hull - chord <= tol {
        out.push([f64::from(q[0] + origin[0]), f64::from(q[1] + origin[1])]);
        return Ok(());
    }
    let pa = mid(p, a);
    let ab = mid(a, b);
    let bq = mid(b, q);
    let l = mid(pa, ab);
    let r = mid(ab, bq);
    let m = mid(l, r);
    flatten(p, pa, l, m, tol, depth + 1, origin, out)?;
    flatten(m, r, bq, q, tol, depth + 1, origin, out)
}
pub fn outline_document(doc: &Document) -> Result<Document, String> {
    if doc.text_boxes.is_empty() {
        return Ok(doc.clone());
    }
    TextLayout::new(font_export::snapshot(doc)?)?.outlined(doc, 1.)
}
pub fn export_note() -> varos_core::ExportNote {
    varos_core::ExportNote {
        kind: "text_outlines".into(),
        object_id: None,
        message: "text exported as outlines".into(),
    }
}

pub mod edit;

pub mod host_fonts;

pub fn font_hash(hash: [u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Export diagnostics for area overflow, alongside the mandatory outlines note.
pub fn export_notes(doc: &Document) -> Result<Vec<varos_core::ExportNote>, String> {
    if doc.text_boxes.is_empty() {
        return Ok(vec![]);
    }
    let mut notes = vec![export_note()];
    let mut engine = TextLayout::new(font_export::snapshot(doc)?)?;
    for text in &doc.text_boxes {
        if engine.compose_document(doc, text, 1.)?.overset {
            notes.push(varos_core::ExportNote {
                kind: "text_overset".into(),
                object_id: Some(text.id),
                message: "Area text has overset source; only fitting lines exported".into(),
            });
        }
    }
    Ok(notes)
}

mod tracking;

// ---- Lane H ----
pub mod flow;
mod frame_split;
pub mod path_mapping;

pub mod font_export;
