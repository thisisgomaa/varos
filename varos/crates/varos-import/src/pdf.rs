//! PDF-compatible AI/static vector subset via lopdf (MIT); no interpreter or font substitution.
use crate::{ImportOptions, ImportReport, MAX_BYTES};
use lopdf::{content::Content, Object};
use std::io::Read;
use varos_core::{
    model::{Anchor, Artboard, Document, GroupRole, NodeKind, Path},
    stroke::{StrokeCap, StrokeJoin, StrokeStyle},
};

pub fn import_pdf(bytes: &[u8]) -> Result<(Document, ImportReport), String> {
    crate::import_file(bytes, crate::Format::Pdf, ImportOptions::default())
}
#[derive(Clone)]
struct State {
    matrix: [f32; 6],
    fill: [f32; 4],
    stroke: [f32; 4],
    width: f32,
    style: StrokeStyle,
    parent: u32,
}
impl Default for State {
    fn default() -> Self {
        let style = StrokeStyle { cap: StrokeCap::Butt, join: StrokeJoin::Miter, ..Default::default() };
        Self {
            matrix: [1., 0., 0., 1., 0., 0.],
            fill: [0., 0., 0., 1.],
            stroke: [0., 0., 0., 1.],
            width: 1.,
            style,
            parent: 0,
        }
    }
}
fn number(o: &Object) -> Result<f32, String> {
    let n = match o {
        Object::Integer(v) => *v as f32,
        Object::Real(v) => *v,
        _ => return Err("PDF number expected".into()),
    };
    if !n.is_finite() || n.abs() > 1e8 {
        return Err("PDF number exceeds geometry limits".into());
    }
    Ok(n)
}
fn inherited<'a>(pdf: &'a lopdf::Document, mut id: lopdf::ObjectId, key: &[u8]) -> Result<&'a Object, String> {
    for _ in 0..64 {
        let d = pdf.get_dictionary(id).map_err(|e| e.to_string())?;
        if let Ok(v) = d.get(key) {
            return pdf.dereference(v).map(|(_, v)| v).map_err(|e| e.to_string());
        }
        id = d.get(b"Parent").and_then(Object::as_reference).map_err(|_| "Missing PDF page box")?;
    }
    Err("PDF page inheritance exceeds depth limit".into())
}
fn clip(doc: &mut Document, parent: u32, rings: Vec<(Vec<Anchor>, bool)>) -> Result<u32, String> {
    let mut contours = crate::contours(rings)?;
    if contours.len() != 1 {
        return Err("Disjoint PDF clipping paths unsupported".into());
    }
    let (anchors, _, holes) = contours.pop().ok_or("Empty PDF clip")?;
    if anchors.len() < 3 {
        return Err("PDF clip requires area".into());
    }
    let group = crate::node(doc, NodeKind::Group, parent, "PDF clipping");
    let id = doc.nid();
    let mut path = Path::new(id, anchors, true, Some([0., 0., 0., 1.]), None, 0.);
    path.holes = holes;
    doc.paths.push(path);
    let mask = crate::node(doc, NodeKind::Path(id), group, "");
    if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == group) {
        n.role = GroupRole::Clip;
        n.mask_child = Some(mask);
    }
    Ok(group)
}
pub(crate) fn read(bytes: &[u8], options: ImportOptions) -> Result<(Document, ImportReport), String> {
    if bytes.len() > MAX_BYTES || !bytes.starts_with(b"%PDF-") {
        return Err("Expected bounded PDF-compatible AI/PDF; EPS/PostScript AI unsupported".into());
    }
    // Conservative profile: disallow object/xref streams before lopdf can inflate them.
    let source = String::from_utf8_lossy(bytes);
    for feature in ["/ObjStm", "/XRef", "/Encrypt", "/Prev", "#"] {
        if source.contains(feature) {
            return Err(format!("Unsupported PDF profile: {feature}"));
        }
    }
    if source.matches(" obj").count() > 10_000 || source.matches("<<").count() > 10_000 {
        return Err("PDF object budget exceeded".into());
    }
    let mut arrays = 0usize;
    for byte in source.bytes() {
        if byte == b'[' {
            arrays += 1;
            if arrays > 64 {
                return Err("PDF array nesting budget exceeded".into());
            }
        } else if byte == b']' {
            arrays = arrays.saturating_sub(1);
        }
    }
    let mut depth = 0usize;
    for token in source.as_bytes().windows(2) {
        if token == b"<<" {
            depth += 1;
            if depth > 64 {
                return Err("PDF nesting budget exceeded".into());
            }
        }
        if token == b">>" {
            depth = depth.saturating_sub(1);
        }
    }
    crate::control::checkpoint()?;
    let pdf = lopdf::Document::load_mem_with_options(bytes, lopdf::LoadOptions { strict: true, ..Default::default() })
        .map_err(|e| e.to_string())?;
    if pdf.objects.len() > 10_000 || pdf.is_encrypted() {
        return Err("Encrypted/oversized PDF refused".into());
    }
    let catalog = pdf.catalog().map_err(|e| e.to_string())?;
    if catalog.has(b"VAROS_Model") || catalog.has(b"VAROS_SchemaVersion") || source.contains("model.varos.json") {
        return Err("Varos PDF must use the native reader; foreign reconstruction is forbidden".into());
    }
    if catalog.has(b"OCProperties") {
        return Err("PDF optional-content layers unsupported".into());
    }
    let pages = pdf.get_pages();
    if pages.is_empty() || pages.len() > 100 {
        return Err("PDF page limit exceeded or no pages".into());
    }
    if pages.len() > 1 && options.page.is_none() {
        return Err("Multi-page PDF requires explicit one-based page selection".into());
    }
    let id = *pages.get(&options.page.unwrap_or(1)).ok_or("PDF page selection out of range")?;
    let d = pdf.get_dictionary(id).map_err(|e| e.to_string())?;
    let box_obj = inherited(&pdf, id, b"CropBox").or_else(|_| inherited(&pdf, id, b"MediaBox"))?;
    let b = box_obj.as_array().map_err(|e| e.to_string())?.iter().map(number).collect::<Result<Vec<_>, _>>()?;
    if b.len() != 4 || b[2] <= b[0] || b[3] <= b[1] {
        return Err("Invalid PDF page box".into());
    }
    let rotation = inherited(&pdf, id, b"Rotate").ok().map(number).transpose()?.unwrap_or(0.);
    if rotation != 0. {
        return Err("Rotated PDF pages unsupported; export without page rotation".into());
    }
    if d.has(b"UserUnit") {
        return Err("PDF UserUnit unsupported".into());
    }
    let mut report = ImportReport::default();
    if d.has(b"Annots") || catalog.has(b"AcroForm") {
        report.loss("PDF annotations/forms/interactivity omitted; static page only");
    }
    let mut data = Vec::new();
    for stream_id in pdf.get_page_contents(id) {
        let stream = pdf.get_object(stream_id).and_then(Object::as_stream).map_err(|e| e.to_string())?;
        let mut chunk = Vec::new();
        match stream.dict.get(b"Filter") {
            Err(_) => chunk.extend_from_slice(&stream.content),
            Ok(Object::Name(n)) if n == b"FlateDecode" && !stream.dict.has(b"DecodeParms") => {
                flate2::read::ZlibDecoder::new(stream.content.as_slice())
                    .take((MAX_BYTES + 1) as u64)
                    .read_to_end(&mut chunk)
                    .map_err(|e| e.to_string())?;
            }
            _ => return Err("Unsupported PDF content encoding".into()),
        }
        if data.len() + chunk.len() + 1 > MAX_BYTES {
            return Err("PDF expanded content limit exceeded".into());
        }
        data.extend(chunk);
        data.push(b'\n');
    }
    let mut depth = 0usize;
    for byte in &data {
        if *byte == b'[' {
            depth += 1;
            if depth > 64 {
                return Err("PDF content array nesting budget exceeded".into());
            }
        } else if *byte == b']' {
            depth = depth.saturating_sub(1);
        }
    }
    let content = Content::decode(&data).map_err(|e| e.to_string())?;
    if content.operations.len() > 100_000 {
        return Err("PDF operation limit exceeded".into());
    }
    let mut doc = Document::default();
    let aid = doc.nid();
    doc.artboards.push(Artboard { id: aid, w: b[2] - b[0], h: b[3] - b[1], page_color: None, ..Default::default() });
    let mut state = State::default();
    let page_ring = [[0., 0.], [b[2] - b[0], 0.], [b[2] - b[0], b[3] - b[1]], [0., b[3] - b[1]]]
        .map(|p| crate::anchor(&mut doc, p))
        .to_vec();
    let parent = doc.active_layer;
    state.parent = clip(&mut doc, parent, vec![(page_ring, true)])?;
    let mut pending_clip: Option<bool> = None;
    let mut stack = Vec::new();
    let mut rings = Vec::new();
    let mut ring = Vec::new();
    for op in content.operations {
        crate::control::checkpoint()?;
        let nums = || op.operands.iter().map(number).collect::<Result<Vec<_>, _>>();
        let map = |x: f32, y: f32| {
            let m = state.matrix;
            [m[0] * x + m[2] * y + m[4] - b[0], b[3] - (m[1] * x + m[3] * y + m[5])]
        };
        match op.operator.as_str() {
            "q" => {
                if stack.len() >= 64 {
                    return Err("PDF graphics stack limit exceeded".into());
                }
                stack.push(state.clone());
            }
            "Q" => state = stack.pop().ok_or("Unbalanced PDF graphics state")?,
            "cm" => {
                let n = nums()?;
                if n.len() != 6 {
                    return Err("Invalid PDF matrix".into());
                }
                let m = state.matrix;
                state.matrix = [
                    m[0] * n[0] + m[2] * n[1],
                    m[1] * n[0] + m[3] * n[1],
                    m[0] * n[2] + m[2] * n[3],
                    m[1] * n[2] + m[3] * n[3],
                    m[0] * n[4] + m[2] * n[5] + m[4],
                    m[1] * n[4] + m[3] * n[5] + m[5],
                ];
            }
            "rg" | "RG" | "g" | "G" => {
                let n = nums()?;
                let rgb = if n.len() == 1 {
                    [n[0], n[0], n[0], 1.]
                } else if n.len() == 3 {
                    [n[0], n[1], n[2], 1.]
                } else {
                    return Err("Invalid PDF colour".into());
                };
                if rgb.iter().any(|v| !(0.0..=1.0).contains(v)) {
                    return Err("PDF colour out of range".into());
                }
                if op.operator == "rg" || op.operator == "g" {
                    state.fill = rgb;
                } else {
                    state.stroke = rgb;
                }
            }
            "w" | "J" | "j" | "M" => {
                let n = nums()?;
                if n.len() != 1 {
                    return Err("Invalid PDF stroke state".into());
                }
                match op.operator.as_str() {
                    "w" => {
                        if n[0] <= 0. {
                            return Err("PDF hairline strokes unsupported".into());
                        }
                        state.width = n[0];
                    }
                    "J" => {
                        state.style.cap = match n[0] {
                            0. => StrokeCap::Butt,
                            1. => StrokeCap::Round,
                            2. => StrokeCap::Square,
                            _ => return Err("Invalid PDF cap".into()),
                        }
                    }
                    "j" => {
                        state.style.join = match n[0] {
                            0. => StrokeJoin::Miter,
                            1. => StrokeJoin::Round,
                            2. => StrokeJoin::Bevel,
                            _ => return Err("Invalid PDF join".into()),
                        }
                    }
                    _ => state.style.miter_limit = n[0],
                }
            }
            "m" | "l" => {
                let n = nums()?;
                if n.len() != 2 {
                    return Err("Invalid PDF path point".into());
                }
                if op.operator == "m" && !ring.is_empty() {
                    rings.push((std::mem::take(&mut ring), false));
                }
                if op.operator == "l" && ring.is_empty() {
                    return Err("PDF line without move".into());
                }
                ring.push(crate::anchor(&mut doc, map(n[0], n[1])));
            }
            "c" | "v" | "y" => {
                let n = nums()?;
                let expected = if op.operator == "c" { 6 } else { 4 };
                if n.len() != expected {
                    return Err("Invalid PDF curve".into());
                }
                let last = ring.last_mut().ok_or("PDF curve without move")?;
                let (a, c, end) = match op.operator.as_str() {
                    "c" => (map(n[0], n[1]), map(n[2], n[3]), map(n[4], n[5])),
                    "v" => (last.p, map(n[0], n[1]), map(n[2], n[3])),
                    _ => (map(n[0], n[1]), map(n[2], n[3]), map(n[2], n[3])),
                };
                last.hout = Some(a);
                let mut end = crate::anchor(&mut doc, end);
                end.hin = Some(c);
                ring.push(end);
            }
            "re" => {
                let n = nums()?;
                if n.len() != 4 {
                    return Err("Invalid PDF rectangle".into());
                }
                if !ring.is_empty() {
                    rings.push((std::mem::take(&mut ring), false));
                }
                let a = [[n[0], n[1]], [n[0] + n[2], n[1]], [n[0] + n[2], n[1] + n[3]], [n[0], n[1] + n[3]]]
                    .map(|p| crate::anchor(&mut doc, map(p[0], p[1])));
                rings.push((a.to_vec(), true));
            }
            "h" => {
                if ring.is_empty() {
                    return Err("PDF close without move".into());
                }
                rings.push((std::mem::take(&mut ring), true));
            }
            "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                if !ring.is_empty() {
                    rings.push((std::mem::take(&mut ring), matches!(op.operator.as_str(), "s" | "b" | "b*")));
                }
                let fill = !matches!(op.operator.as_str(), "S" | "s" | "n");
                let stroke = matches!(op.operator.as_str(), "S" | "s" | "B" | "B*" | "b" | "b*");
                let clipping = if let Some(evenodd) = pending_clip.take() {
                    if !evenodd && rings.len() > 1 {
                        return Err("Nonzero compound PDF clip unsupported".into());
                    }
                    Some(rings.clone())
                } else {
                    None
                };
                if op.operator != "n" {
                    if fill && rings.len() > 1 && !op.operator.ends_with('*') {
                        return Err("Nonzero compound PDF fills unsupported; export even-odd outlines".into());
                    }
                    let m = state.matrix;
                    let sx = m[0].hypot(m[1]);
                    let sy = m[2].hypot(m[3]);
                    if stroke && ((sx - sy).abs() > 0.0001 || (m[0] * m[2] + m[1] * m[3]).abs() > 0.0001) {
                        return Err("Nonuniform PDF stroke transform unsupported".into());
                    }
                    for (a, closed, holes) in crate::contours(std::mem::take(&mut rings))? {
                        if a.len() < 2 {
                            continue;
                        }
                        let id = doc.nid();
                        let mut p = Path::new(
                            id,
                            a,
                            closed,
                            fill.then_some(state.fill),
                            stroke.then_some(state.stroke),
                            state.width * sx,
                        );
                        p.holes = holes;
                        p.stroke_style = state.style.clone();
                        doc.paths.push(p);
                        crate::node(&mut doc, NodeKind::Path(id), state.parent, "");
                    }
                } else {
                    rings.clear();
                }
                if let Some(rings) = clipping {
                    state.parent = clip(&mut doc, state.parent, rings)?;
                }
            }
            "BT" | "ET" | "Tf" | "Tm" | "Td" | "TD" | "T*" | "Tc" | "Tw" | "Tz" | "TL" | "Ts" | "Tj" | "TJ" | "'"
            | "\"" => report.loss("PDF text omitted: lopdf exposes no verified embedded glyph outline conversion"),
            "Tr" => {
                let n = nums()?;
                if n != [0.] {
                    return Err("PDF text clipping/render mode unsupported".into());
                }
            }
            "Do" => {
                let name = op.operands.first().and_then(|o| o.as_name().ok()).ok_or("PDF XObject requires name")?;
                let resources = inherited(&pdf, id, b"Resources")?.as_dict().map_err(|e| e.to_string())?;
                let object = resources.get(b"XObject").map_err(|e| e.to_string())?;
                let objects =
                    pdf.dereference(object).map_err(|e| e.to_string())?.1.as_dict().map_err(|e| e.to_string())?;
                let object = objects.get(name).map_err(|e| e.to_string())?;
                let stream =
                    pdf.dereference(object).map_err(|e| e.to_string())?.1.as_stream().map_err(|e| e.to_string())?;
                if stream.dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|s| s == b"Image") {
                    report.loss("PDF images omitted: native image/blob node prerequisite absent");
                } else {
                    return Err("PDF vector Form XObjects unsupported; export expanded page paths".into());
                }
            }
            "W" | "W*" => {
                if pending_clip.is_some() {
                    return Err("Duplicate PDF clip operator".into());
                }
                pending_clip = Some(op.operator == "W*");
            }
            "d" => {
                if op.operands.len() != 2 {
                    return Err("Invalid PDF dash state".into());
                }
                state.style.dash = op.operands[0]
                    .as_array()
                    .map_err(|e| e.to_string())?
                    .iter()
                    .map(number)
                    .collect::<Result<_, _>>()?;
                state.style.dash_phase = number(&op.operands[1])?;
                state.style.validate(0).map_err(|e| e.to_string())?;
            }
            _ => return Err(format!("Unsupported PDF operator {}; no silent appearance fallback", op.operator)),
        }
        if doc.paths.len() > 50_000 || doc.ids > 500_000 {
            return Err("PDF geometry budget exceeded".into());
        }
    }
    if pending_clip.is_some() || !stack.is_empty() || !ring.is_empty() || !rings.is_empty() {
        return Err("Unbalanced/unpainted PDF content".into());
    }
    if doc.paths.len() <= 1 {
        return Err(format!("PDF has no supported vector artwork: {}", report.loss_notes.join("; ")));
    }
    doc.sync_tree();
    report.paths = doc.paths.len();
    Ok((doc, report))
}
