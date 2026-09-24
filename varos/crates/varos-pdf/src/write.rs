//! The WRITE side: Document → PDF bytes. One page loop serves two outputs:
//! - the native `.vrs` container (`write_pdf`): pages + the embedded editable model (the `.ai` pattern);
//! - the pure export (`crate::export`): the same pages, and NOTHING else — no model, no names.
//!
//! The page loop below was moved here verbatim from `lib.rs` (S6-A); `write_pdf`'s bytes are pinned
//! by `tests/export_pdf.rs::native_write_is_byte_identical_to_fixture`.

use std::sync::atomic::{AtomicBool, Ordering};

use pdf_writer::types::{AssociationKind, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use varos_core::file::{doc_to_blob, VRS_VERSION};
use varos_core::model::{Anchor, Artboard, Document, Path, Xform};
use varos_core::Rgba;

use crate::export::{ExportError, PageSpec};

const MODEL_NAME: &[u8] = b"model.varos.json";

/// Monotone Ref allocator (pdf-writer ids are ours to manage).
struct Alloc(i32);
impl Alloc {
    fn next(&mut self) -> Ref {
        self.0 += 1;
        Ref::new(self.0)
    }
}

/// A pooled ExtGState (quantized alphas → one object per distinct pair per page).
struct Gs {
    ca: f32,
    cap: f32,
    r: Ref,
}
/// A knockout Form XObject queued for writing after its page.
struct Knock {
    r: Ref,
    content: Vec<u8>,
    bbox: [f32; 4],
    gs_fill: (Ref, f32),
    gs_stroke: (Ref, f32),
}

/// The native `.vrs` container: one page per visible board + the embedded editable model.
pub fn write_pdf(doc: &Document) -> Result<Vec<u8>, String> {
    let blob = doc_to_blob(doc)?;
    let never = AtomicBool::new(false);
    write_pages(doc, &native_pages(doc), Some(&blob), &never).map_err(|e| e.to_string())
}

/// The native container's page list. One page per VISIBLE board — a hidden board (board eye OFF)
/// exports NO page, matching the canvas (Ahmed 07-06 export gap). An artboard-less doc still saves on
/// its default frame; and if EVERY board is hidden we keep the first as a single frame so the container
/// never degrades to a zero-page (invalid) PDF. (Right for the native file; the pure export plans its
/// own pages and never uses these fallbacks — see `crate::export::plan_pdf_export`.)
fn native_pages(doc: &Document) -> Vec<PageSpec> {
    let boards: Vec<Artboard> = if doc.artboards.is_empty() {
        vec![Artboard::default()]
    } else {
        let vis: Vec<Artboard> = doc.artboards.iter().filter(|a| !a.hidden).cloned().collect();
        if vis.is_empty() {
            vec![doc.artboards[0].clone()]
        } else {
            vis
        }
    };
    boards.iter().map(PageSpec::of_board).collect()
}

/// One path as the page loop draws it: resolved paints, drawability, its live unit transform and the
/// clipping mask it paints under.
pub(crate) struct Drawn<'a> {
    pub(crate) p: &'a Path,
    pub(crate) xf: Xform,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    fillable: bool,
    strokable: bool,
    pub(crate) pad: f32,
    /// `Some(mask)` = a clip-group member: the mask paths (each with its OWN live transform) whose
    /// silhouette clips it, even-odd, exactly as the canvas's `Group::Clip`. Only mask paths with a
    /// drawable ring (≥ 2 anchors) are kept, so `Some` is never empty.
    pub(crate) clip: Option<Vec<(&'a Path, Xform)>>,
}

/// Every path the page loop draws on `page`, in paint order.
fn drawn_on<'a>(doc: &'a Document, page: &'a PageSpec) -> impl Iterator<Item = Drawn<'a>> + 'a {
    // artwork: the paintable content in document order (paint_list, LAYERS_VISION §5 — a mask
    // source must never reach the page); conservative bbox cull per page
    doc.paint_list().filter_map(move |(_, p)| {
        let d = drawable(doc, p)?;
        bbox_hits(p, &d.xf, d.pad, page).then_some(d)
    })
}

/// A path's draw recipe, or `None` when it draws nothing anywhere (hidden, no paint to show, or a clip
/// member that its mask clips away entirely).
pub(crate) fn drawable<'a>(doc: &'a Document, p: &'a Path) -> Option<Drawn<'a>> {
    // WYSIWYG with the canvas: skip anything EFFECTIVELY hidden — the path's own eye, a hidden
    // parent group/layer (node cascade), OR art whose every member board is hidden (board eye).
    // Raw `p.hidden` missed the last two, so hidden groups and hidden pages still bled out.
    if doc.eff_hidden(p.id) {
        return None;
    }
    // A7 (Stage 6): the path's live per-object transform — rotation is applied to every control
    // point before the world→page map, so the exported PDF matches the rotated canvas exactly
    // (cubics are affine-invariant → mapping control points is exact). Identity ⇒ today's output.
    let xf = doc.unit_xform(p.id);
    // resolve each paint to its drawable solid ONCE (Paint::None — and future gradients — ⇒ None)
    let (fill, stroke) = (p.fill.solid(), p.stroke.solid());
    // WYSIWYG with the canvas: an OPEN path still FILLS (implied straight close between endpoints,
    // A32) — the exact rule `scene::fill_prims` draws by. The old `p.closed` guard dropped the fill
    // of any shape a deleted anchor had opened, so it filled on screen but vanished in the PDF (FB1).
    let fillable = p.anchors.len() >= 3 && fill.is_some();
    let strokable = stroke.is_some() && p.anchors.len() >= 2 && p.stroke_width > 0.0;
    if !fillable && !strokable {
        return None;
    }
    let pad = if strokable { p.stroke_width * 0.5 } else { 0.0 };
    // MASKS_PLAN Stage 5: a clip-group member paints only inside its mask. Single level, like the
    // canvas (`scene.rs` keys the clip on the NEAREST clip group, `clip_group_of`). A mask with no
    // drawable ring clips its members to nothing, and a member whose box misses the mask's box is
    // wholly clipped out — neither is emitted (so nothing the mask hides reaches the file).
    let clip = match doc.clip_group_of(p.id) {
        None => None,
        Some(c) => {
            let mask: Vec<(&Path, Xform)> = doc
                .node_mask_child(c)
                .map(|mc| doc.node_paths(mc))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|mp| doc.pidx(mp).map(|i| &doc.paths[i]))
                .filter(|mp| mp.anchors.len() >= 2 || mp.holes.iter().any(|h| h.len() >= 2))
                .map(|mp| (mp, doc.unit_xform(mp.id)))
                .collect();
            let (x0, y0, x1, y1) = world_bbox(p, &xf, pad);
            let hits = mask.iter().any(|(mp, mxf)| {
                let (m0, n0, m1, n1) = world_bbox(mp, mxf, 0.0);
                x0 <= m1 && x1 >= m0 && y0 <= n1 && y1 >= n0
            });
            if !hits {
                return None;
            }
            Some(mask)
        }
    };
    Some(Drawn { p, xf, fill, stroke, fillable, strokable, pad, clip })
}

/// Write `pages` (world rects, in order) as a PDF. `model = Some(blob)` embeds the editable model
/// (native `.vrs`); `None` writes the pages and a bare catalog only — no FileSpec, no /AF, no /Names,
/// no /VAROS_* keys, no Info dictionary (the pure export). `cancel` is checked before every page.
pub(crate) fn write_pages(
    doc: &Document,
    pages: &[PageSpec],
    model: Option<&str>,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, ExportError> {
    let mut ids = Alloc(0);
    let cat_id = ids.next();
    let tree_id = ids.next();
    // the model's refs are allocated only when it is embedded (and in the native file's historic order)
    let model_ids = model.map(|_| (ids.next(), ids.next()));

    let mut pdf = Pdf::new();
    let mut page_ids = Vec::new();

    for ab in pages {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExportError::Cancelled);
        }
        let [ab_x, ab_y, ab_w, ab_h] = ab.rect;
        // world → page (Y-flip, artboard-local, identity CTM)
        let t = |p: [f32; 2]| -> (f32, f32) { (p[0] - ab_x, (ab_y + ab_h) - p[1]) };

        let mut c = Content::new();
        c.set_line_cap(LineCapStyle::RoundCap).set_line_join(LineJoinStyle::RoundJoin); // screen parity
        let mut gss: Vec<Gs> = Vec::new();
        let mut knocks: Vec<Knock> = Vec::new();

        // page background (transparent pages emit nothing — viewers show their own backdrop, like .ai)
        if let Some(bg) = ab.background {
            c.save_state();
            if bg[3] < 0.999 {
                let n = gs_name(&mut gss, &mut ids, bg[3], bg[3]);
                c.set_parameters(Name(n.as_bytes()));
            }
            c.set_fill_rgb(bg[0], bg[1], bg[2]);
            c.rect(0.0, 0.0, ab_w, ab_h);
            c.fill_even_odd();
            c.restore_state();
        }

        for Drawn { p, xf, fill, stroke, fillable, strokable, pad, clip } in drawn_on(doc, ab) {
            // MASKS_PLAN Stage 5: `q <mask rings> W* n <the member's usual paint> Q` — the PDF-native clip,
            // even-odd over every mask ring like the canvas. Knockout XObjects paint inside it too.
            if let Some(mask) = &clip {
                c.save_state();
                for (mp, mxf) in mask {
                    emit_rings(&mut c, mp, mxf, &t);
                }
                c.clip_even_odd().end_path();
            }
            let fa = fill.map_or(0.0, |f| f[3]) * p.opacity;
            let sa = stroke.map_or(0.0, |s| s[3]) * p.opacity;

            if fillable && strokable && sa < 0.999 {
                // Varos knockout: fill+stroke composite as an isolated unit, the stroke band REPLACES
                // the fill beneath it, then the unit fades once → /I /K transparency group.
                let (fill, stroke) = (fill.unwrap(), stroke.unwrap());
                let mut ic = Content::new();
                ic.set_line_cap(LineCapStyle::RoundCap).set_line_join(LineJoinStyle::RoundJoin);
                let gf = ids.next();
                let gk = ids.next();
                ic.save_state().set_parameters(Name(b"Gf")).set_fill_rgb(fill[0], fill[1], fill[2]);
                emit_rings(&mut ic, p, &xf, &t);
                ic.fill_even_odd().restore_state();
                ic.save_state()
                    .set_parameters(Name(b"Gk"))
                    .set_stroke_rgb(stroke[0], stroke[1], stroke[2])
                    .set_line_width(p.stroke_width);
                emit_rings(&mut ic, p, &xf, &t);
                ic.stroke().restore_state();
                let xr = ids.next();
                let bb = page_bbox(p, &xf, pad, &t);
                // paint site: object opacity applied ONCE to the whole unit
                let n = gs_name(&mut gss, &mut ids, p.opacity, p.opacity);
                c.save_state().set_parameters(Name(n.as_bytes()));
                c.x_object(Name(format!("Fx{}", knocks.len()).as_bytes()));
                c.restore_state();
                knocks.push(Knock {
                    r: xr,
                    content: ic.finish().to_vec(),
                    bbox: bb,
                    gs_fill: (gf, fill[3]),
                    gs_stroke: (gk, stroke[3]),
                });
            } else {
                c.save_state();
                let n = gs_name(&mut gss, &mut ids, fa, sa);
                c.set_parameters(Name(n.as_bytes()));
                if let (true, Some(f)) = (fillable, fill) {
                    c.set_fill_rgb(f[0], f[1], f[2]);
                }
                if let (true, Some(s)) = (strokable, stroke) {
                    c.set_stroke_rgb(s[0], s[1], s[2]);
                    c.set_line_width(p.stroke_width);
                }
                emit_rings(&mut c, p, &xf, &t);
                match (fillable, strokable) {
                    (true, true) => {
                        c.fill_even_odd_and_stroke();
                    } // B* — one gs carries /ca + /CA
                    (true, false) => {
                        c.fill_even_odd();
                    }
                    _ => {
                        c.stroke();
                    }
                }
                c.restore_state();
            }
            if clip.is_some() {
                c.restore_state();
            }
        }

        // page objects: content stream → page (+ resources) → gs objects → knockout xobjects
        let cont_id = ids.next();
        pdf.stream(cont_id, &c.finish());
        let page_id = ids.next();
        let mut page = pdf.page(page_id);
        page.parent(tree_id).media_box(Rect::new(0.0, 0.0, ab_w, ab_h)).contents(cont_id);
        {
            let mut res = page.resources();
            if !gss.is_empty() {
                let mut d = res.ext_g_states();
                for (i, g) in gss.iter().enumerate() {
                    d.pair(Name(format!("GS{i}").as_bytes()), g.r);
                }
            }
            if !knocks.is_empty() {
                let mut d = res.x_objects();
                for (i, k) in knocks.iter().enumerate() {
                    d.pair(Name(format!("Fx{i}").as_bytes()), k.r);
                }
            }
        }
        page.finish();
        for g in &gss {
            pdf.ext_graphics(g.r).non_stroking_alpha(g.ca).stroking_alpha(g.cap);
        }
        for k in &knocks {
            let mut x = pdf.form_xobject(k.r, &k.content);
            x.bbox(Rect::new(k.bbox[0], k.bbox[1], k.bbox[2], k.bbox[3]));
            {
                let mut g = x.group();
                g.transparency().isolated(true).knockout(true);
            }
            x.resources().ext_g_states().pair(Name(b"Gf"), k.gs_fill.0).pair(Name(b"Gk"), k.gs_stroke.0);
            x.finish();
            pdf.ext_graphics(k.gs_fill.0).non_stroking_alpha(k.gs_fill.1);
            pdf.ext_graphics(k.gs_stroke.0).stroking_alpha(k.gs_stroke.1);
        }
        page_ids.push(page_id);
    }

    pdf.pages(tree_id).kids(page_ids.iter().copied()).count(page_ids.len() as i32);

    match (model, model_ids) {
        (Some(blob), Some((emb_id, fs_id))) => {
            // the embedded editable model (.ai pattern): EmbeddedFile + FileSpec(/AF Source) + name tree + private keys
            pdf.embedded_file(emb_id, blob.as_bytes()).subtype(Name(b"application/json"));
            let mut fs = pdf.file_spec(fs_id);
            fs.path(Str(MODEL_NAME))
                .unic_file(TextStr("model.varos.json"))
                .embedded_file(emb_id)
                .association_kind(AssociationKind::Source)
                .description(TextStr("Varos editable model (source of truth)"));
            fs.finish();

            let mut cat = pdf.catalog(cat_id);
            cat.pages(tree_id);
            cat.names().embedded_files().names().insert(Str(MODEL_NAME), fs_id);
            cat.insert(Name(b"AF")).array().item(fs_id);
            cat.pair(Name(b"VAROS_Model"), emb_id);
            cat.pair(Name(b"VAROS_SchemaVersion"), VRS_VERSION as i32);
            cat.finish();
        }
        _ => {
            // the pure export: the page tree and nothing else
            pdf.catalog(cat_id).pages(tree_id);
        }
    }

    Ok(pdf.finish())
}

/// Pooled ExtGState name for an (/ca, /CA) pair — one object per distinct (quantized) pair per page.
fn gs_name(gss: &mut Vec<Gs>, ids: &mut Alloc, ca: f32, cap: f32) -> String {
    let q = |v: f32| (v * 1000.0).round() / 1000.0;
    let (ca, cap) = (q(ca), q(cap));
    let i = gss.iter().position(|g| g.ca == ca && g.cap == cap).unwrap_or_else(|| {
        gss.push(Gs { ca, cap, r: ids.next() });
        gss.len() - 1
    });
    format!("GS{i}")
}

/// Emit the path's outer ring + hole rings as subpaths of ONE path object. Closed rings emit the
/// wrap-around cubic EXPLICITLY before `h` — `h` alone closes with a straight line and would silently
/// flatten the closing curve (the classic exporter bug).
fn emit_rings(c: &mut Content, p: &Path, xf: &Xform, t: &impl Fn([f32; 2]) -> (f32, f32)) {
    emit_ring(c, &p.anchors, p.closed, xf, t);
    for hole in &p.holes {
        emit_ring(c, hole, true, xf, t);
    }
}
fn emit_ring(c: &mut Content, anchors: &[Anchor], closed: bool, xf: &Xform, t: &impl Fn([f32; 2]) -> (f32, f32)) {
    let n = anchors.len();
    if n < 2 {
        return;
    }
    // A7: local anchor → WORLD (unit transform) → page. Rotation is affine ⇒ mapping control points is exact.
    let tw = |q: [f32; 2]| t(xf.apply(q));
    let p0 = tw(anchors[0].p);
    c.move_to(p0.0, p0.1);
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % n];
        let c1 = tw(a.hout.unwrap_or(a.p));
        let c2 = tw(b.hin.unwrap_or(b.p));
        let p3 = tw(b.p);
        c.cubic_to(c1.0, c1.1, c2.0, c2.1, p3.0, p3.1);
    }
    if closed {
        c.close_path();
    }
}

/// Conservative world bbox (anchors + handles of all rings) inflated by `pad`; hits the page rect?
fn bbox_hits(p: &Path, xf: &Xform, pad: f32, page: &PageSpec) -> bool {
    let [ax, ay, aw, ah] = page.rect;
    let (x0, y0, x1, y1) = world_bbox(p, xf, pad);
    x0 <= ax + aw && x1 >= ax && y0 <= ay + ah && y1 >= ay
}
fn world_bbox(p: &Path, xf: &Xform, pad: f32) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
        for q in [Some(a.p), a.hin, a.hout].into_iter().flatten() {
            let q = xf.apply(q); // A7: the cull box is the WORLD (rotated) extent, so nothing clips away
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
    }
    (x0 - pad, y0 - pad, x1 + pad, y1 + pad)
}
/// The same bbox in page space (for the Form XObject /BBox), corners ordered lower-left/upper-right.
fn page_bbox(p: &Path, xf: &Xform, pad: f32, t: &impl Fn([f32; 2]) -> (f32, f32)) -> [f32; 4] {
    let (x0, y0, x1, y1) = world_bbox(p, xf, pad);
    let (ax0, ay0) = t([x0, y0]);
    let (ax1, ay1) = t([x1, y1]);
    [ax0.min(ax1), ay0.min(ay1), ax0.max(ax1), ay0.max(ay1)]
}
