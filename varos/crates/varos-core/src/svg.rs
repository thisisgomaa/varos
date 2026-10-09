//! Dependency-free SVG 1.1 export: plan first, then return one standalone file per page.
//! No disk I/O. Coordinates remain world-space; viewBox equals artboard bounds. WholeBoard is
//! the headless `--all` equivalent. Paint order/transforms/nearest clips mirror PDF; opacity
//! follows scene::Group (isolation before knockout). SVG knockout uses a luminance mask.
mod stroke;
use crate::flatten::{control_bbox, Rect};
use crate::format::{check_structure, validate::authored, Limits};
use crate::model::{Anchor, Document, NodeKind, Path, Xform};
use crate::Rgba;
use std::collections::HashSet;
use std::fmt::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportScope {
    AllVisibleArtboards,
    ActiveArtboard,
    /// Visible artwork including free floaters, even when artboards exist.
    WholeBoard,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PageSpec {
    /// World x, y, width, height in Varos points (1 pt = 1 px at 72 ppi).
    /// SVG dimensions are unitless, preserving the same numeric viewport size.
    pub rect: [f32; 4],
    pub background: Option<Rgba>,
    pub artboard: Option<usize>,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPlan {
    pub scope: ExportScope,
    pub pages: Vec<PageSpec>,
}
/// Destination naming and atomic writes are the caller's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub struct SvgFile {
    pub page: PageSpec,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    Cancelled,
    NeedsArtboards,
    NoVisibleArtboards,
    ActiveArtboardHidden,
    NothingToExport,
    InvalidDocument(String),
    LimitExceeded,
    InvalidPage,
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded => f.write_str("limit_exceeded: stroke geometry budget"),
            Self::Cancelled => f.write_str("The export was cancelled."),
            Self::NeedsArtboards => f.write_str("This board has no artboards. Export the whole board instead."),
            Self::NoVisibleArtboards => f.write_str("Every artboard is hidden."),
            Self::ActiveArtboardHidden => f.write_str("The active artboard is hidden."),
            Self::NothingToExport => f.write_str("There is no visible artwork to export."),
            Self::InvalidDocument(e) => write!(f, "Cannot export this board: {e}"),
            Self::InvalidPage => f.write_str("The export page has invalid bounds or colour."),
        }
    }
}
impl std::error::Error for ExportError {}
pub fn default_scope(doc: &Document) -> ExportScope {
    if doc.artboards.is_empty() {
        ExportScope::WholeBoard
    } else {
        ExportScope::AllVisibleArtboards
    }
}
fn check_document(doc: &Document) -> Result<(), ExportError> {
    // Check before any tree traversal; tree-less paths are allowed, as in PDF export.
    check_structure(doc, &Limits::DEFAULT).map_err(|e| ExportError::InvalidDocument(e.to_string()))?;
    authored(doc).map_err(|e| ExportError::InvalidDocument(e.to_string()))?;
    for (pi, p) in doc.paths.iter().enumerate() {
        let xf = doc.unit_xform(p.id);
        if p.anchors.iter().chain(p.holes.iter().flatten()).any(|a| {
            [Some(a.p), a.hin, a.hout].into_iter().flatten().any(|q| xf.apply(q).iter().any(|v| !v.is_finite()))
        }) {
            return Err(ExportError::InvalidDocument("Transformed coordinates exceed the supported range.".into()));
        }
        if p.anchors.is_empty() && p.holes.iter().all(Vec::is_empty) {
            continue;
        }
        let b = control_bbox(doc, pi);
        let pad = if p.stroke.solid().is_some() { crate::geom::painted_padding(p) } else { 0.0 };
        let extent = [b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad];
        if extent.iter().any(|v| !v.is_finite())
            || !(extent[2] - extent[0]).is_finite()
            || !(extent[3] - extent[1]).is_finite()
        {
            return Err(ExportError::InvalidDocument("Paint bounds exceed the supported range.".into()));
        }
    }
    Ok(())
}
pub fn plan_svg_export(doc: &Document, scope: ExportScope) -> Result<ExportPlan, ExportError> {
    check_document(doc)?;
    let board = |i: usize| {
        let a = &doc.artboards[i];
        PageSpec { rect: [a.x, a.y, a.w, a.h], background: a.page_color, artboard: Some(i), name: a.name.clone() }
    };
    let pages = match scope {
        ExportScope::AllVisibleArtboards => {
            if doc.artboards.is_empty() {
                return Err(ExportError::NeedsArtboards);
            }
            let pages: Vec<_> =
                doc.artboards.iter().enumerate().filter(|(_, a)| !a.hidden).map(|(i, _)| board(i)).collect();
            if pages.is_empty() {
                return Err(ExportError::NoVisibleArtboards);
            }
            pages
        }
        ExportScope::ActiveArtboard => {
            let a = doc.active_artboard().ok_or(ExportError::NeedsArtboards)?;
            if a.hidden {
                return Err(ExportError::ActiveArtboardHidden);
            }
            vec![board(doc.active.min(doc.artboards.len() - 1))]
        }
        ExportScope::WholeBoard => vec![artwork_bounds(doc).ok_or(ExportError::NothingToExport)?],
    };
    Ok(ExportPlan { scope, pages })
}
/// Plan selection bounds and narrow its immutable export snapshot. Clip mask geometry is retained.
pub fn plan_selection_svg_export(
    doc: &Document,
    selected: &HashSet<u32>,
) -> Result<(Document, ExportPlan), ExportError> {
    check_document(doc)?;
    if selected.is_empty() {
        return Err(ExportError::NothingToExport);
    }
    let masks: HashSet<u32> = doc.paths.iter().filter(|p| doc.is_mask_source(p.id)).map(|p| p.id).collect();
    let mut narrowed = doc.clone();
    for path in &mut narrowed.paths {
        if !selected.contains(&path.id) && !masks.contains(&path.id) {
            path.hidden = true;
        }
    }
    let page = artwork_bounds(&narrowed).ok_or(ExportError::NothingToExport)?;
    Ok((narrowed, ExportPlan { scope: ExportScope::WholeBoard, pages: vec![page] }))
}

/// Deterministic output. Cancellation never returns a partial set of files. Snapshot and plan
/// should belong together; custom rectangles are allowed and checked before writing.
pub fn export_svg_files(doc: &Document, plan: &ExportPlan, cancel: &AtomicBool) -> Result<Vec<SvgFile>, ExportError> {
    export_svg_files_with_report(doc, plan, cancel).map(|(output, _)| output)
}

/// Export bytes together with explicit diagnostics.
pub fn export_svg_files_with_report(
    doc: &Document,
    plan: &ExportPlan,
    cancel: &AtomicBool,
) -> Result<(Vec<SvgFile>, crate::ExportReport), ExportError> {
    cancelled(cancel)?;
    check_document(doc)?;
    let resolved = crate::live_corners::document(doc);
    let doc = &resolved;
    if plan.pages.is_empty() {
        return Err(ExportError::NothingToExport);
    }
    let mut report = crate::ExportReport::default();
    let mut stroke_budget = crate::stroke::evaluate::StrokeBudget::default();
    for p in &doc.paths {
        if !p.stroke_style.is_default() {
            let coverage =
                crate::stroke::evaluate(p, 0.01, &|| cancel.load(Ordering::Relaxed)).map_err(stroke_error)?;
            for _ in &plan.pages {
                stroke_budget.charge(&coverage).map_err(stroke_error)?;
            }
            report.notes.extend(coverage.report.notes);
            if !stroke::native(p) {
                report.notes.push(crate::ExportNote {
                    kind: "stroke_baked".into(),
                    object_id: Some(p.id),
                    message: "SVG: aligned, fitted, dotted, degenerate or arrowed coverage".into(),
                });
            }
        }
    }
    let mut files = Vec::with_capacity(plan.pages.len());
    for page in &plan.pages {
        cancelled(cancel)?;
        if !page.rect.iter().all(|n| n.is_finite())
            || page.artboard.is_some_and(|i| i >= doc.artboards.len())
            || page.rect[2] < 0.001
            || page.rect[3] < 0.001
            || !(page.rect[0] + page.rect[2]).is_finite()
            || !(page.rect[1] + page.rect[3]).is_finite()
            || page.background.is_some_and(|c| c.iter().any(|v| !v.is_finite() || !(0.0..=1.0).contains(v)))
        {
            return Err(ExportError::InvalidPage);
        }
        files.push(SvgFile { page: page.clone(), bytes: write_page(doc, page, cancel)?.into_bytes() });
    }
    Ok((files, report))
}
fn stroke_error(e: crate::stroke::StrokeError) -> ExportError {
    match e {
        crate::stroke::StrokeError::LimitExceeded => ExportError::LimitExceeded,
        crate::stroke::StrokeError::Cancelled => ExportError::Cancelled,
        _ => ExportError::InvalidDocument(e.to_string()),
    }
}

fn cancelled(c: &AtomicBool) -> Result<(), ExportError> {
    if c.load(Ordering::Relaxed) {
        Err(ExportError::Cancelled)
    } else {
        Ok(())
    }
}
struct Drawn<'a> {
    p: &'a Path,
    xf: Xform,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    bbox: Rect,
    pad: f32,
    clip: Option<u32>,
}
fn drawable<'a>(doc: &Document, pi: usize, p: &'a Path) -> Option<Drawn<'a>> {
    if doc.eff_hidden(p.id) {
        return None;
    }
    let fill = p.fill.solid().filter(|_| p.anchors.len() >= 3);
    let stroke = p.stroke.solid().filter(|_| {
        (p.anchors.len() >= 2 || (!p.stroke_style.is_default() && !p.anchors.is_empty())) && p.stroke_width > 0.0
    });
    if fill.is_none() && stroke.is_none() {
        return None;
    }
    let pad = if stroke.is_some() { crate::geom::painted_padding(p) } else { 0.0 };
    let b = control_bbox(doc, pi);
    Some(Drawn {
        p,
        xf: doc.unit_xform(p.id),
        fill,
        stroke,
        bbox: (b.0 - pad, b.1 - pad, b.2 + pad, b.3 + pad),
        pad,
        clip: doc.clip_group_of(p.id),
    })
}
fn mask_paths(doc: &Document, c: u32) -> Vec<(&Path, Xform, Rect)> {
    doc.node_mask_child(c)
        .map(|m| doc.node_paths(m))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| doc.pidx(id))
        .filter(|&i| doc.paths[i].anchors.len() >= 2 || doc.paths[i].holes.iter().any(|h| h.len() >= 2))
        .map(|i| (&doc.paths[i], doc.unit_xform(doc.paths[i].id), control_bbox(doc, i)))
        .collect()
}
fn intersection(a: Rect, b: Rect) -> Option<Rect> {
    (a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1).then_some((
        a.0.max(b.0),
        a.1.max(b.1),
        a.2.min(b.2),
        a.3.min(b.3),
    ))
}
fn union(a: Rect, b: Rect) -> Rect {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}
fn artwork_bounds(doc: &Document) -> Option<PageSpec> {
    let mut bounds = None;
    for (pi, p) in doc.paint_list() {
        let Some(d) = drawable(doc, pi, p) else { continue };
        let b = doc.outline_bbox(pi);
        let mut b = (b.0 - d.pad, b.1 - d.pad, b.2 + d.pad, b.3 + d.pad);
        if let Some(c) = d.clip {
            let Some(mask) = mask_paths(doc, c)
                .iter()
                .filter_map(|(p, _, _)| doc.pidx(p.id))
                .map(|i| doc.outline_bbox(i))
                .reduce(union)
            else {
                continue;
            };
            let Some(clipped) = intersection(b, mask) else { continue };
            b = clipped;
        }
        bounds = Some(bounds.map_or(b, |old| union(old, b)));
    }
    bounds.map(|b| PageSpec {
        rect: [b.0, b.1, (b.2 - b.0).max(1.0), (b.3 - b.1).max(1.0)],
        background: None,
        artboard: None,
        name: doc.name.clone(),
    })
}
fn write_page(doc: &Document, page: &PageSpec, cancel: &AtomicBool) -> Result<String, ExportError> {
    let [x, y, w, h] = page.rect;
    let page_box = (x, y, x + w, y + h);
    let root = page.artboard.map_or_else(|| "board".into(), |i| id("artboard", i + 1, &page.name));
    let mut out=format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" id=\"{root}\" width=\"{}\" height=\"{}\" viewBox=\"{}\" overflow=\"hidden\">\n",num(w),num(h),numbers(&page.rect));
    title(&mut out, &page.name);
    if let Some(bg) = page.background {
        writeln!(
            out,
            "<rect id=\"background\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
            num(x),
            num(y),
            num(w),
            num(h),
            color(bg),
            num(bg[3])
        )
        .unwrap();
    }
    let items: Vec<_> = doc
        .paint_list()
        .filter_map(|(i, p)| drawable(doc, i, p))
        .filter(|d| intersection(d.bbox, page_box).is_some())
        .collect();
    let mut used_nodes = HashSet::new();
    let mut i = 0;
    while i < items.len() {
        cancelled(cancel)?;
        let clip = items[i].clip;
        let n = items[i..].iter().take_while(|d| d.clip == clip).count();
        let run = &items[i..i + n];
        i += n;
        let mut members: Vec<_> = run.iter().collect();
        if let Some(c) = clip {
            let masks = mask_paths(doc, c);
            members.retain(|d| {
                intersection(d.bbox, page_box).is_some_and(|b| masks.iter().any(|m| intersection(b, m.2).is_some()))
            });
            let Some(reach) = members.iter().filter_map(|d| intersection(d.bbox, page_box)).reduce(union) else {
                continue;
            };
            let mut data = String::new();
            for (p, xf, _) in masks.iter().filter(|m| intersection(m.2, reach).is_some()) {
                data.push_str(&path_data(p, xf));
            }
            // One compound path: SVG clipPath children union, while the model's rings XOR.
            writeln!(out,"<defs><clipPath id=\"clip-{c}-{i}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{data}\" clip-rule=\"evenodd\"/></clipPath></defs>\n<g clip-path=\"url(#clip-{c}-{i})\">").unwrap();
        }
        let mut open = Vec::new();
        for d in members {
            cancelled(cancel)?;
            let mut ancestors = Vec::new();
            let mut cur = doc.node_of_path(d.p.id).and_then(|n| doc.node(n)).and_then(|n| n.parent);
            while let Some(nid) = cur {
                let Some(node) = doc.node(nid) else { break };
                if !matches!(node.kind, NodeKind::Path(_)) {
                    ancestors.push(nid);
                }
                cur = node.parent;
            }
            ancestors.reverse();
            let common = open.iter().zip(&ancestors).take_while(|(a, b)| a == b).count();
            for _ in common..open.len() {
                out.push_str("</g>\n");
            }
            for &nid in &ancestors[common..] {
                let node = doc.node(nid).unwrap();
                if used_nodes.insert(nid) {
                    writeln!(out, "<g id=\"{}\">", id("node", nid as usize, &node.name)).unwrap();
                    title(&mut out, &node.name);
                } else {
                    out.push_str("<g>\n");
                }
            }
            open = ancestors;
            if d.p.stroke_style.is_default() {
                paint(&mut out, d);
            } else {
                stroke::paint(&mut out, d)?;
            }
        }
        for _ in open {
            out.push_str("</g>\n");
        }
        if clip.is_some() {
            out.push_str("</g>\n");
        }
    }
    out.push_str("</svg>\n");
    Ok(out)
}
fn paint(out: &mut String, d: &Drawn<'_>) {
    let p = d.p;
    let data = path_data(p, &d.xf);
    writeln!(
        out,
        "<g id=\"{}\" opacity=\"{}\">",
        id("path", p.id as usize, p.name.as_deref().unwrap_or("")),
        num(p.opacity)
    )
    .unwrap();
    if let Some(name) = &p.name {
        title(out, name);
    }
    let knockout = p.opacity >= 0.999 && d.fill.is_some() && d.stroke.is_some_and(|c| c[3] < 0.999);
    if knockout {
        let b = d.bbox;
        writeln!(out,"<defs><mask id=\"knockout-{}\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/><path d=\"{data}\" fill=\"none\" stroke=\"black\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></mask></defs>",p.id,num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1),num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1),num(p.stroke_width)).unwrap();
        let c = d.fill.unwrap();
        writeln!(
            out,
            "<path d=\"{data}\" fill=\"{}\" fill-opacity=\"{}\" fill-rule=\"evenodd\" mask=\"url(#knockout-{})\"/>",
            color(c),
            num(c[3]),
            p.id
        )
        .unwrap();
        stroke(out, &data, d.stroke.unwrap(), p.stroke_width);
    } else {
        let fill = d.fill.map_or_else(|| "none".into(), color);
        let stroke = d.stroke.map_or_else(|| "none".into(), color);
        writeln!(out,"<path d=\"{data}\" fill=\"{fill}\" fill-opacity=\"{}\" fill-rule=\"evenodd\" stroke=\"{stroke}\" stroke-opacity=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",num(d.fill.map_or(1.0,|c| c[3])),num(d.stroke.map_or(1.0,|c| c[3])),num(p.stroke_width)).unwrap();
    }
    out.push_str("</g>\n");
}
fn stroke(out: &mut String, data: &str, c: Rgba, width: f32) {
    writeln!(out,"<path d=\"{data}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",color(c),num(c[3]),num(width)).unwrap();
}
fn path_data(p: &Path, xf: &Xform) -> String {
    let mut data = String::new();
    ring(&mut data, &p.anchors, p.closed, xf);
    for h in &p.holes {
        ring(&mut data, h, true, xf);
    }
    data
}
fn ring(out: &mut String, anchors: &[Anchor], closed: bool, xf: &Xform) {
    if anchors.len() < 2 {
        return;
    }
    write!(out, "M{} ", numbers(&xf.apply(anchors[0].p))).unwrap();
    let n = if closed { anchors.len() } else { anchors.len() - 1 };
    for i in 0..n {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % anchors.len()];
        if a.hout.is_none() && b.hin.is_none() {
            write!(out, "L{} ", numbers(&xf.apply(b.p))).unwrap();
        } else {
            write!(
                out,
                "C{} {} {} ",
                numbers(&xf.apply(a.hout.unwrap_or(a.p))),
                numbers(&xf.apply(b.hin.unwrap_or(b.p))),
                numbers(&xf.apply(b.p))
            )
            .unwrap();
        }
    }
    if closed {
        out.push_str("Z ");
    }
}
fn num(v: f32) -> String {
    // Quantize before formatting: platform sin/cos differences of one ulp near a
    // three-decimal midpoint must not change rotated-coordinate golden output.
    let rounded = ((v as f64) * 10_000.0).round() / 10_000.0;
    if rounded.abs() < 0.0005 {
        "0.000".into()
    } else {
        format!("{rounded:.3}")
    }
}
fn numbers(v: &[f32]) -> String {
    v.iter().map(|v| num(*v)).collect::<Vec<_>>().join(" ")
}
fn color(c: Rgba) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8
    )
}
fn id(kind: &str, value: usize, name: &str) -> String {
    let mut out = format!("{kind}-{value}");
    if !name.is_empty() {
        out.push('-');
        for c in name.chars() {
            // Encoding underscores too makes this injective and XML-name safe.
            if literal_name_char(c) {
                out.push(c);
            } else {
                write!(out, "_x{:02X}_", c as u32).unwrap();
            }
        }
    }
    out
}
// Keep Unicode letters/digits only where XML 1.0 (5th edition) permits a
// NameChar. The ASCII kind prefix supplies NameStartChar. Escaping underscore
// reserves the _xHH_ delimiter, so literal names cannot mimic encoded names.
fn literal_name_char(c: char) -> bool {
    c.is_alphanumeric()
        && matches!(c, '0'..='9' | 'A'..='Z' | 'a'..='z'
            | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}'
            | '\u{300}'..='\u{37d}' | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}' | '\u{203f}'..='\u{2040}'
            | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
}
fn title(out: &mut String, name: &str) {
    if name.is_empty() {
        return;
    }
    out.push_str("<title>");
    for c in name.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '\t' | '\n' => out.push(c),
            c if c >= ' ' && c != '\u{fffe}' && c != '\u{ffff}' => out.push(c),
            _ => out.push('\u{fffd}'),
        }
    }
    out.push_str("</title>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_names_are_injective_and_xml_name_safe() {
        assert_eq!(id("path", 2, "اسم١٢"), "path-2-اسم١٢");
        assert_eq!(id("path", 2, "A /!?_"), "path-2-A_x20__x2F__x21__x3F__x5F_");
        assert_ne!(id("path", 2, " "), id("path", 2, "_x20_"));
        // Superscript two is alphanumeric but outside XML NameChar ranges.
        assert_eq!(id("path", 2, "²"), "path-2-_xB2_");
    }

    #[test]
    fn neighboring_floats_at_decimal_midpoints_format_identically() {
        for value in [12.3455_f32, -12.3455_f32, 0.0005_f32, -0.0005_f32] {
            let bits = value.to_bits();
            assert_eq!(num(f32::from_bits(bits - 1)), num(value));
            assert_eq!(num(f32::from_bits(bits + 1)), num(value));
        }
        assert_eq!(num(-0.00001), "0.000");
    }
}

// ---- Lane C ----
pub mod options;
