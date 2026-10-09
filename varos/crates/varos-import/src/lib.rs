//! Foreign-format firewall: SVG normalization never participates in `.vrs` decoding.
use std::io::Read;
use varos_core::model::{Anchor, Artboard, Document, GroupRole, Node, NodeKind, Path, Xform};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ImportReport {
    pub paths: usize,
    pub loss_notes: Vec<String>,
}
impl ImportReport {
    fn loss(&mut self, note: &str) {
        if !self.loss_notes.iter().any(|n| n == note) {
            self.loss_notes.push(note.into());
        }
    }
}
/// SVG or gzip-compressed SVG bytes. External resources are never fetched or opened.
pub fn import_svg(bytes: &[u8]) -> Result<(Document, ImportReport), String> {
    if bytes.len() > MAX_BYTES {
        return Err("SVG exceeds 16 MiB limit".into());
    }
    let decoded;
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        if out.len() > MAX_BYTES {
            return Err("SVGZ exceeds decompressed limit".into());
        }
        decoded = out;
        decoded.as_slice()
    } else {
        bytes
    };
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    let xml = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    if xml.descendants().any(|n| n.ancestors().take(130).count() > 128) {
        return Err("SVG nesting exceeds 128 levels".into());
    }
    let root = xml.root_element();
    if root.tag_name().name() != "svg" {
        return Err("expected SVG root".into());
    }
    if xml.descendants().count() > 100_000 {
        return Err("SVG exceeds element limit".into());
    }
    let mut report = ImportReport::default();
    for n in xml.descendants().filter(|n| n.is_element()) {
        match n.tag_name().name() {
            "linearGradient" | "radialGradient" | "pattern" => {
                report.loss("Gradient/pattern paints omitted; solid paints only")
            }
            "text" | "tspan" => report.loss("Text omitted"),
            "image" => report.loss("Images omitted; external resources disabled"),
            "filter" => report.loss("Filters omitted"),
            "clipPath" | "mask" => report.loss("SVG clipping/masks omitted"),
            "animate" | "animateTransform" | "set" => report.loss("Animation omitted; static artwork imported"),
            "foreignObject" => report.loss("Foreign objects omitted"),
            _ => {}
        }
    }
    // Normalize the viewport to viewBox units: the imported Untitled page is viewBox-sized.
    let mut normalized = text.to_owned();
    if let Some(v) = root.attribute("viewBox") {
        let nums: Vec<f32> = v
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|_| "invalid viewBox")?;
        if nums.len() != 4 || nums.iter().any(|v| !v.is_finite()) || nums[2] <= 0.0 || nums[3] <= 0.0 {
            return Err("invalid viewBox".into());
        }
        let mut edits: Vec<_> = ["width", "height", "preserveAspectRatio"]
            .iter()
            .filter_map(|name| root.attribute_node(*name).map(|a| a.range()))
            .collect();
        edits.sort_by_key(|r| std::cmp::Reverse(r.start));
        for range in edits {
            normalized.replace_range(range, "");
        }
        let start = root.range().start + 1;
        let end =
            normalized[start..].find(|c: char| c.is_whitespace() || c == '/' || c == '>').ok_or("invalid SVG root")?;
        let start = start + end;
        normalized
            .insert_str(start, &format!(" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"", nums[2], nums[3]));
    }
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&normalized, &options).map_err(|e| e.to_string())?;
    let mut doc = Document::default();
    let id = doc.nid();
    doc.artboards.push(Artboard {
        id,
        w: tree.size().width(),
        h: tree.size().height(),
        page_color: None,
        ..Default::default()
    });
    walk(tree.root(), doc.active_layer, 1.0, &mut doc, &mut report)?;
    let limits = varos_core::format::Limits::DEFAULT;
    varos_core::format::check_structure(&doc, &limits).map_err(|e| e.to_string())?;
    doc.sync_tree();
    varos_core::format::validate(&doc, &limits).map_err(|e| e.to_string())?;
    report.paths = doc.paths.len();
    Ok((doc, report))
}
fn node(doc: &mut Document, kind: NodeKind, parent: u32, name: &str) -> u32 {
    let id = doc.nid();
    doc.nodes.push(Node {
        id,
        kind,
        name: name.into(),
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
    if let Some(p) = doc.nodes.iter_mut().find(|n| n.id == parent) {
        p.children.insert(0, id);
    }
    id
}
fn solid(p: &usvg::Paint, alpha: f32, report: &mut ImportReport) -> Option<[f32; 4]> {
    if let usvg::Paint::Color(c) = p {
        Some([c.red as f32 / 255., c.green as f32 / 255., c.blue as f32 / 255., alpha])
    } else {
        report.loss("Gradient/pattern paints omitted; solid paints only");
        None
    }
}
fn walk(
    group: &usvg::Group,
    parent: u32,
    opacity: f32,
    doc: &mut Document,
    report: &mut ImportReport,
) -> Result<(), String> {
    let opacity = opacity * group.opacity().get();
    if group.blend_mode() != usvg::BlendMode::Normal || group.isolate() {
        report.loss("Blend/isolation compositing omitted");
    }
    if group.opacity().get() != 1.0 {
        report.loss("Group opacity flattened onto paths; overlapping children may differ");
    }
    if group.clip_path().is_some() || group.mask().is_some() {
        report.loss("SVG clipping/masks omitted");
    }
    if !group.filters().is_empty() {
        report.loss("Filters omitted");
    }
    for child in group.children() {
        match child {
            usvg::Node::Group(g) => {
                let id = node(doc, NodeKind::Group, parent, g.id());
                walk(g, id, opacity, doc, report)?;
            }
            usvg::Node::Path(p) => {
                if p.paint_order() == usvg::PaintOrder::StrokeAndFill {
                    report.loss("Stroke-before-fill order converted to fill-before-stroke");
                }
                let t = p.abs_transform();
                let map =
                    |p: usvg::tiny_skia_path::Point| [t.sx * p.x + t.kx * p.y + t.tx, t.ky * p.x + t.sy * p.y + t.ty];
                let mut rings: Vec<(Vec<Anchor>, bool)> = vec![];
                let mut ring: Vec<Anchor> = vec![];
                for seg in p.data().segments() {
                    use usvg::tiny_skia_path::PathSegment::*;
                    match seg {
                        MoveTo(p) => {
                            if !ring.is_empty() {
                                rings.push((std::mem::take(&mut ring), false));
                            }
                            ring.push(anchor(doc, map(p)));
                        }
                        LineTo(p) => ring.push(anchor(doc, map(p))),
                        QuadTo(c, p) => {
                            let end = map(p);
                            let c = map(c);
                            if let Some(last) = ring.last_mut() {
                                let a = last.p;
                                last.hout = Some([a[0] + (c[0] - a[0]) * 2. / 3., a[1] + (c[1] - a[1]) * 2. / 3.]);
                            }
                            let mut a = anchor(doc, end);
                            a.hin = Some([end[0] + (c[0] - end[0]) * 2. / 3., end[1] + (c[1] - end[1]) * 2. / 3.]);
                            ring.push(a);
                        }
                        CubicTo(a, b, p) => {
                            if let Some(last) = ring.last_mut() {
                                last.hout = Some(map(a));
                            }
                            let mut a = anchor(doc, map(p));
                            a.hin = Some(map(b));
                            ring.push(a);
                        }
                        Close => {
                            if ring.len() > 1 && ring.first().map(|a| a.p) == ring.last().map(|a| a.p) {
                                if let Some(last) = ring.pop() {
                                    ring[0].hin = last.hin;
                                }
                            }
                            rings.push((std::mem::take(&mut ring), true));
                        }
                    }
                }
                if !ring.is_empty() {
                    rings.push((ring, false));
                }
                let fill = p.fill().and_then(|f| solid(f.paint(), f.opacity().get(), report));
                let stroke = p.stroke().and_then(|s| solid(s.paint(), s.opacity().get(), report));
                let width = p.stroke().map_or(0., |s| s.width().get());
                if let Some(s) = p.stroke() {
                    if s.dasharray().is_some()
                        || s.linecap() != usvg::LineCap::Round
                        || s.linejoin() != usvg::LineJoin::Round
                    {
                        report.loss("Stroke dash/cap/join options converted to undashed round caps/joins");
                    }
                }
                let sx = t.sx.hypot(t.ky);
                let sy = t.kx.hypot(t.sy);
                if stroke.is_some() && ((sx - sy).abs() > 0.0001 || (t.sx * t.kx + t.ky * t.sy).abs() > 0.0001) {
                    report.loss("Nonuniform stroke transform approximated by scalar width");
                }
                if rings.len() > 1 && p.fill().is_some_and(|f| f.rule() == usvg::FillRule::NonZero) {
                    report.loss("Nonzero compound fill converted to even-odd; overlapping contours may differ");
                }
                for (outer, closed, holes) in contours(rings)? {
                    let id = doc.nid();
                    let mut path = Path::new(id, outer, closed, fill, stroke, width * sx);
                    path.opacity = opacity;
                    path.hidden = !p.is_visible();
                    path.name = (!p.id().is_empty()).then(|| p.id().into());
                    path.holes = holes;
                    if path.anchors.iter().chain(path.holes.iter().flatten()).any(|a| {
                        a.p.iter().chain(a.hin.iter().flatten()).chain(a.hout.iter().flatten()).any(|v| !v.is_finite())
                    }) {
                        return Err("nonfinite SVG geometry".into());
                    }
                    doc.paths.push(path);
                    node(doc, NodeKind::Path(id), parent, "");
                }
            }
            usvg::Node::Image(_) => report.loss("Images omitted; external resources disabled"),
            usvg::Node::Text(_) => report.loss("Text omitted"),
        }
    }
    Ok(())
}
fn anchor(doc: &mut Document, p: [f32; 2]) -> Anchor {
    Anchor { id: doc.nid(), p, hin: None, hout: None, smooth: false }
}

// Split disjoint subpaths and nested islands; inner rings stay editable holes.
type Contour = (Vec<Anchor>, bool, Vec<Vec<Anchor>>);
fn contours(rings: Vec<(Vec<Anchor>, bool)>) -> Result<Vec<Contour>, String> {
    if rings.len() > 1000 {
        return Err("SVG path exceeds 1000 contour limit".into());
    }
    let polys: Vec<_> = rings.iter().map(|(a, c)| Document::ring(a, *c, 24)).collect();
    // Reject crossing/touching boundaries before assigning nesting depth. A first point
    // alone cannot establish containment (both starts can lie inside intersecting rings).
    let boxes: Vec<_> = polys
        .iter()
        .map(|p| {
            p.iter().fold([f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY], |b, p| {
                [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]
            })
        })
        .collect();
    let mut containers = vec![Vec::new(); rings.len()];
    for i in 0..rings.len() {
        for j in 0..i {
            if !rings[i].1 || !rings[j].1 {
                continue;
            }
            let a = boxes[i];
            let b = boxes[j];
            if a[0] > b[2] || b[0] > a[2] || a[1] > b[3] || b[1] > a[3] {
                continue;
            }
            if polys[i].windows(2).any(|a| polys[j].windows(2).any(|b| segments_touch(a, b))) {
                return Err("Unsupported SVG topology: intersecting or touching compound contours".into());
            }
            if !polys[i].is_empty() && polys[i].iter().all(|p| varos_core::geom::point_in_poly(&polys[j], *p)) {
                containers[i].push(j);
            } else if !polys[j].is_empty() && polys[j].iter().all(|p| varos_core::geom::point_in_poly(&polys[i], *p)) {
                containers[j].push(i);
            }
        }
    }
    let mut result = Vec::new();
    for (i, (a, c)) in rings.iter().enumerate() {
        if !containers[i].len().is_multiple_of(2) {
            continue;
        }
        let holes = rings
            .iter()
            .enumerate()
            .filter(|(j, _)| containers[*j].len() == containers[i].len() + 1 && containers[*j].contains(&i))
            .map(|(_, r)| r.0.clone())
            .collect();
        result.push((a.clone(), *c, holes));
    }
    Ok(result)
}

// Inclusive segment intersection, including collinear overlap and boundary contact.
// Use f64 intermediates so finite SVG coordinates cannot overflow the cross product.
fn segments_touch(a: &[[f32; 2]], b: &[[f32; 2]]) -> bool {
    fn side(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f64 {
        (b[0] as f64 - a[0] as f64) * (p[1] as f64 - a[1] as f64)
            - (b[1] as f64 - a[1] as f64) * (p[0] as f64 - a[0] as f64)
    }
    if (0..2).any(|k| a[0][k].max(a[1][k]) < b[0][k].min(b[1][k]) || b[0][k].max(b[1][k]) < a[0][k].min(a[1][k])) {
        return false;
    }
    let opposite = |x: f64, y: f64| (x <= 0.0 && y >= 0.0) || (x >= 0.0 && y <= 0.0);
    opposite(side(a[0], a[1], b[0]), side(a[0], a[1], b[1])) && opposite(side(b[0], b[1], a[0]), side(b[0], b[1], a[1]))
}
