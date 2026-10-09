//! Lane H: real PDF glyph text with sparse font subsets and logical-cluster ToUnicode.
use pdf_writer::{
    types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap},
    Content, Filter, Name, Pdf, Rect, Ref, Str,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use varos_core::{
    model::{Document, Xform},
    ExportNote, ExportReport,
};
use varos_text_layout::TextLayout;
struct Glyph {
    cid: u16,
    font: usize,
    size: f32,
    p: [f32; 2],
    fill: [f32; 4],
    bounds: [f32; 4],
}
struct Auxiliary {
    commands: Vec<varos_text_layout::font_export::Command>,
    fill: [f32; 4],
    origin: [f32; 2],
    bounds: [f32; 4],
}
struct Text {
    glyphs: Vec<Glyph>,
    auxiliaries: Vec<Auxiliary>,
}
#[derive(Default)]
pub(crate) struct Pool {
    texts: Vec<Text>,
    paths: BTreeMap<u32, usize>,
    pub fonts: Vec<(String, Ref)>,
    painted: BTreeSet<usize>,
}
fn next(ids: &mut i32) -> Ref {
    *ids += 1;
    Ref::new(*ids)
}
fn compress(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(bytes).map_err(|e| e.to_string())?;
    z.finish().map_err(|e| e.to_string())
}
pub(crate) fn prepare(
    original: &Document,
    outlined: &Document,
    pdf: &mut Pdf,
    ids: &mut i32,
    report: &mut ExportReport,
) -> Result<Pool, String> {
    let mut pool = Pool::default();
    if original.text_boxes.is_empty() {
        return Ok(pool);
    }
    let mut engine = TextLayout::new(varos_text_layout::font_export::snapshot(original)?)?;
    for source in &original.text_boxes {
        let root = varos_core::typography::story_root(original, source.id)?;
        let story = original.text_boxes.iter().find(|t| t.id == root).ok_or("missing story")?;
        let resolved = original.typography.resolved(story)?;
        let result = engine.compose_document(original, source, 1.)?;
        let fallback = if original
            .typography
            .frames
            .get(&source.id)
            .is_some_and(|f| matches!(f.binding, Some(varos_core::typography::Binding::Path { .. })))
        {
            Some("curved PDF text placement uses outlines".to_string())
        } else if resolved.runs.iter().any(|r| r.style.fill[3] != 1.) {
            Some("translucent text uses outlines".to_string())
        } else {
            None
        };
        let rectangle = if original.typography.frames.get(&source.id).is_none_or(|f| f.binding.is_none()) {
            match source.box_kind {
                varos_core::text::TextBoxKind::Area(r) => Some(r),
                _ => None,
            }
        } else {
            None
        };
        let mut glyphs: Vec<_> = result
            .layout
            .lines
            .iter()
            .filter(|line| rectangle.is_none_or(|r| line.baseline + result.origin[1] + line.descent <= r[1] + r[3]))
            .flat_map(|l| l.glyphs.iter())
            .collect();
        // PDF copy order follows logical source, while matrices retain shaped visual placement.
        glyphs.sort_by_key(|g| (g.cluster.start, g.cluster.end, g.advance == 0.));
        let mut faces: BTreeMap<usize, Vec<(u16, String)>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut requests = Vec::new();
        let mut auxiliaries = Vec::new();
        for g in glyphs {
            let outline = varos_text_layout::font_export::glyph_outline(engine.fonts(), g)?;
            let bounds = command_bounds(&outline.commands, result.origin).unwrap_or([
                g.x + result.origin[0],
                g.y + result.origin[1] - g.size,
                g.x + result.origin[0] + g.advance,
                g.y + result.origin[1],
            ]);
            if !seen.insert((g.cluster.start, g.cluster.end)) {
                let mut at = 0;
                let fill = resolved
                    .runs
                    .iter()
                    .find_map(|r| {
                        at += r.text.len();
                        (g.cluster.start < at).then_some(r.style.fill)
                    })
                    .unwrap_or([0., 0., 0., 1.]);
                auxiliaries.push(Auxiliary { commands: outline.commands, fill, origin: result.origin, bounds });
                continue;
            }
            let unicode =
                result.layout.source.get(g.cluster.clone()).ok_or("invalid shaped source cluster")?.to_string();
            let entries = faces.entry(g.face.0).or_default();
            let cid = u16::try_from(entries.len() + 1).map_err(|_| "PDF text CID limit")?;
            entries.push((g.id, unicode));
            let mut at = 0;
            let fill = resolved
                .runs
                .iter()
                .find_map(|r| {
                    at += r.text.len();
                    (g.cluster.start < at).then_some(r.style.fill)
                })
                .unwrap_or([0., 0., 0., 1.]);
            requests.push((
                g.face.0,
                Glyph {
                    cid,
                    font: 0,
                    size: g.size,
                    p: [g.x + g.offset[0] + result.origin[0], g.y + g.offset[1] + result.origin[1]],
                    fill,
                    bounds,
                },
            ));
        }
        let mut subsets = BTreeMap::new();
        let mut fallback = fallback;
        if fallback.is_none() {
            for (&face, entries) in &faces {
                let f = engine.fonts().face(varos_text_layout::font_export::FaceId(face)).ok_or("missing font face")?;
                let cff = crate::font_subset::tables(&f.bytes)?.contains_key(b"CFF ");
                let subset = if cff {
                    crate::font_subset::embedding_allowed(&f.bytes, true).and_then(|()| {
                        crate::cff_subset::subset(
                            engine.fonts(),
                            varos_text_layout::font_export::FaceId(face),
                            &entries.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
                            &subset_name(pool.fonts.len() + subsets.len()),
                        )
                    })
                } else {
                    crate::font_subset::subset(&f.bytes, &entries.iter().map(|(g, _)| *g).collect())
                };
                match subset {
                    Ok(bytes) => {
                        subsets.insert(face, (bytes, cff));
                    }
                    Err(e) => {
                        fallback = Some(e);
                        break;
                    }
                }
            }
        }
        if let Some(reason) = fallback {
            report.notes.push(ExportNote {
                kind: "text_outlines".into(),
                object_id: Some(source.id),
                message: format!("Text outlined: {reason}"),
            });
            continue;
        }
        let mut font_indices = BTreeMap::new();
        for (face, entries) in faces {
            let f = engine.fonts().face(varos_text_layout::font_export::FaceId(face)).ok_or("missing font face")?;
            let (bytes, cff) = subsets.get(&face).ok_or("missing font subset")?;
            let index = pool.fonts.len();
            let resource = format!("VT{index}");
            let r = write_font(pdf, ids, &f.bytes, bytes, &entries, index, *cff)?;
            pool.fonts.push((resource, r));
            font_indices.insert(face, index);
        }
        let glyphs = requests
            .into_iter()
            .map(|(face, mut g)| {
                g.font = font_indices[&face];
                g
            })
            .collect();
        let index = pool.texts.len();
        pool.texts.push(Text { glyphs, auxiliaries });
        if let Some(node) = varos_core::text::node_id(original, source.id) {
            for p in outlined.node_paths(node) {
                pool.paths.insert(p, index);
            }
        }
        report.notes.push(ExportNote {
            kind: "text_embedded".into(),
            object_id: Some(source.id),
            message: "Text embedded with a glyph subset and logical Unicode map".into(),
        });
        if result.overset {
            report.notes.push(ExportNote {
                kind: "text_overset".into(),
                object_id: Some(source.id),
                message: "Only fitting text was exported; overset source remains editable".into(),
            });
        }
    }
    Ok(pool)
}
impl Pool {
    pub fn page(&mut self) {
        self.painted.clear();
    }
    pub fn paint(
        &mut self,
        path: u32,
        xf: Xform,
        c: &mut Content,
        t: &impl Fn([f32; 2]) -> (f32, f32),
        eligible: &impl Fn((f32, f32, f32, f32)) -> bool,
    ) -> bool {
        let Some(&index) = self.paths.get(&path) else {
            return false;
        };
        if !self.painted.insert(index) {
            return true;
        }
        c.save_state().begin_text();
        let (sin, cos) = xf.rot.sin_cos();
        for g in &self.texts[index].glyphs {
            if !eligible(world_bounds(g.bounds, xf)) {
                continue;
            }
            let (x, y) = t(xf.apply(g.p));
            c.set_fill_rgb(g.fill[0], g.fill[1], g.fill[2]);
            c.set_font(Name(self.fonts[g.font].0.as_bytes()), g.size);
            c.set_text_matrix([cos, -sin, sin, cos, x, y]);
            c.show(Str(&g.cid.to_be_bytes()));
        }
        c.end_text();
        for Auxiliary { commands, fill, origin, bounds } in &self.texts[index].auxiliaries {
            if !eligible(world_bounds(*bounds, xf)) {
                continue;
            }
            c.set_fill_rgb(fill[0], fill[1], fill[2]);
            let point = |p: [f32; 2]| t(xf.apply([p[0] + origin[0], p[1] + origin[1]]));
            for command in commands {
                use varos_text_layout::font_export::Command;
                match *command {
                    Command::Move(p) => {
                        let (x, y) = point(p);
                        c.move_to(x, y);
                    }
                    Command::Line(p) => {
                        let (x, y) = point(p);
                        c.line_to(x, y);
                    }
                    Command::Cubic(a, b, p) => {
                        let (ax, ay) = point(a);
                        let (bx, by) = point(b);
                        let (x, y) = point(p);
                        c.cubic_to(ax, ay, bx, by, x, y);
                    }
                    Command::Close => {
                        c.close_path();
                    }
                }
            }
            c.fill_nonzero();
        }
        c.restore_state();
        true
    }
}
fn write_font(
    pdf: &mut Pdf,
    ids: &mut i32,
    original: &[u8],
    subset: &[u8],
    entries: &[(u16, String)],
    index: usize,
    cff: bool,
) -> Result<Ref, String> {
    let font = ttf_parser::Face::parse(original, 0).map_err(|_| "invalid font metrics")?;
    let scale = 1000. / f32::from(font.units_per_em());
    let bbox = font.global_bounding_box();
    let type0 = next(ids);
    let cid = next(ids);
    let desc = next(ids);
    let file = next(ids);
    let unicode = next(ids);
    let mapping = next(ids);
    let name = subset_name(index);
    let info = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };
    pdf.type0_font(type0)
        .base_font(Name(name.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid)
        .to_unicode(unicode);
    {
        let mut f = pdf.cid_font(cid);
        f.subtype(if cff { CidFontType::Type0 } else { CidFontType::Type2 })
            .base_font(Name(name.as_bytes()))
            .system_info(info)
            .font_descriptor(desc);
        if !cff {
            f.cid_to_gid_map_stream(mapping);
        }
        f.widths().consecutive(
            1,
            entries
                .iter()
                .map(|(gid, _)| f32::from(font.glyph_hor_advance(ttf_parser::GlyphId(*gid)).unwrap_or(0)) * scale),
        );
    }
    let mut descriptor = pdf.font_descriptor(desc);
    descriptor
        .name(Name(name.as_bytes()))
        .flags(FontFlags::SYMBOLIC)
        .bbox(Rect::new(
            f32::from(bbox.x_min) * scale,
            f32::from(bbox.y_min) * scale,
            f32::from(bbox.x_max) * scale,
            f32::from(bbox.y_max) * scale,
        ))
        .italic_angle(font.italic_angle())
        .ascent(f32::from(font.ascender()) * scale)
        .descent(f32::from(font.descender()) * scale)
        .cap_height(f32::from(font.capital_height().unwrap_or(font.ascender())) * scale)
        .stem_v(80.);
    if cff {
        descriptor.font_file3(file);
    } else {
        descriptor.font_file2(file);
    }
    drop(descriptor);
    let zipped = compress(subset)?;
    let mut stream = pdf.stream(file, &zipped);
    stream.filter(Filter::FlateDecode);
    if cff {
        stream.pair(Name(b"Subtype"), Name(b"CIDFontType0C"));
    } else {
        stream.pair(Name(b"Length1"), subset.len() as i32);
    }
    drop(stream);
    let mut cmap = UnicodeCmap::new(Name(b"VarosUnicode"), info);
    let mut map = vec![0, 0];
    for (i, (gid, text)) in entries.iter().enumerate() {
        map.extend_from_slice(&gid.to_be_bytes());
        cmap.pair_with_multiple((i + 1) as u16, text.chars());
    }
    pdf.stream(mapping, &map);
    pdf.stream(unicode, &cmap.finish());
    Ok(type0)
}

fn subset_name(index: usize) -> String {
    let prefix: String = (0..6).rev().map(|n| char::from(b'A' + ((index / 26usize.pow(n)) % 26) as u8)).collect();
    format!("{prefix}+Varos")
}

fn command_bounds(commands: &[varos_text_layout::font_export::Command], origin: [f32; 2]) -> Option<[f32; 4]> {
    use varos_text_layout::font_export::Command;
    commands
        .iter()
        .flat_map(|c| match *c {
            Command::Move(p) | Command::Line(p) => vec![p],
            Command::Cubic(a, b, p) => vec![a, b, p],
            Command::Close => vec![],
        })
        .map(|p| [p[0] + origin[0], p[1] + origin[1]])
        .fold(None, |b, p| {
            Some(match b {
                None => [p[0], p[1], p[0], p[1]],
                Some([x, y, r, b]) => [x.min(p[0]), y.min(p[1]), r.max(p[0]), b.max(p[1])],
            })
        })
}
fn world_bounds([x, y, r, b]: [f32; 4], xf: Xform) -> (f32, f32, f32, f32) {
    [[x, y], [r, y], [r, b], [x, b]]
        .into_iter()
        .map(|p| xf.apply(p))
        .fold((f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY), |(x, y, r, b), p| {
            (x.min(p[0]), y.min(p[1]), r.max(p[0]), b.max(p[1]))
        })
}
