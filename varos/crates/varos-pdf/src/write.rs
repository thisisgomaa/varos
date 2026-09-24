//! The WRITE side: Document → PDF bytes. One page loop serves two outputs:
//! - the native `.vrs` container (`write_pdf`): pages + the embedded editable model (the `.ai` pattern);
//! - the pure export (`crate::export`): the same pages, and NOTHING else — no model, no names.
//!
//! The page loop below was moved here from `lib.rs` (S6-A) unchanged, then gained the MASKS_PLAN
//! Stage 5 clip (one `q <mask rings> W* n … Q` per run of clip-group members). `write_pdf`'s bytes are
//! pinned by `tests/export_pdf.rs::native_write_is_byte_identical_to_fixture`.

use std::sync::atomic::{AtomicBool, Ordering};

use pdf_writer::types::{AssociationKind, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use varos_core::file::{doc_to_blob, VRS_VERSION};
use varos_core::flatten::{control_bbox, Rect as WRect};
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

/// One path as the page loop draws it: resolved paints, drawability, its live unit transform, its
/// padded WORLD control box and the clip group it paints under.
#[derive(Clone, Copy)]
pub(crate) struct Drawn<'a> {
    pub(crate) p: &'a Path,
    xf: Xform,
    fill: Option<Rgba>,
    stroke: Option<Rgba>,
    fillable: bool,
    strokable: bool,
    pub(crate) pad: f32,
    /// Conservative WORLD box (anchors + handles of all rings, through the unit transform) grown by
    /// `pad` — it contains every point the path can paint (round caps/joins: nothing lies beyond w/2).
    bbox: WRect,
    /// The NEAREST clip group (canvas `clip_group_of`: single level) whose mask clips this path, if any.
    pub(crate) clip: Option<u32>,
}

/// Every path the page loop draws on `page`, in paint order.
fn drawn_on<'a>(doc: &'a Document, page: &PageSpec) -> impl Iterator<Item = Drawn<'a>> + 'a {
    // artwork: the paintable content in document order (paint_list, LAYERS_VISION §5 — a mask
    // source must never reach the page); conservative bbox cull per page
    let page_box = page_rect(page);
    doc.paint_list().filter_map(move |(pi, p)| drawable(doc, pi, p).filter(|d| overlaps(d.bbox, page_box)))
}

/// A path's draw recipe, or `None` when it draws nothing anywhere (hidden, or no paint to show).
pub(crate) fn drawable<'a>(doc: &Document, pi: usize, p: &'a Path) -> Option<Drawn<'a>> {
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
    let (x0, y0, x1, y1) = control_bbox(doc, pi); // A7: the WORLD (rotated) extent, so nothing clips away
    let bbox = (x0 - pad, y0 - pad, x1 + pad, y1 + pad);
    Some(Drawn { p, xf, fill, stroke, fillable, strokable, pad, bbox, clip: doc.clip_group_of(p.id) })
}

/// The mask of clip group `c`: every path under its `mask_child` that has a ring to emit, with its own
/// live transform and its WORLD control box. Hidden mask paths count — the canvas ignores the mask's
/// eye too (`scene.rs` `mask_rings_of`). Empty ⇒ the clip hides its members entirely.
pub(crate) fn mask_paths(doc: &Document, c: u32) -> Vec<(&Path, Xform, WRect)> {
    doc.node_mask_child(c)
        .map(|mc| doc.node_paths(mc))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|id| doc.pidx(id))
        .filter(|&i| {
            let mp = &doc.paths[i];
            mp.anchors.len() >= 2 || mp.holes.iter().any(|h| h.len() >= 2)
        })
        .map(|i| (&doc.paths[i], doc.unit_xform(doc.paths[i].id), control_bbox(doc, i)))
        .collect()
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

        // MASKS_PLAN Stage 5 / §2.4: a clip group's members form ONE contiguous run in paint order (as on
        // the canvas, `scene.rs` Group::Clip), so each run is written as ONE `q <mask rings> W* n … Q`.
        let items: Vec<Drawn> = drawn_on(doc, ab).collect();
        let page_box = page_rect(ab);
        let mut i = 0;
        while i < items.len() {
            let clip = items[i].clip;
            let run_len = items[i..].iter().take_while(|d| d.clip == clip).count();
            let run = &items[i..i + run_len];
            i += run_len;
            let Some(cg) = clip else {
                for d in run {
                    paint(&mut c, &mut gss, &mut knocks, &mut ids, d, &t);
                }
                continue;
            };
            let mask = mask_paths(doc, cg);
            // A member paints only where (its box ∩ the page) meets some mask ring's box; one that
            // doesn't is wholly clipped out on this page and is not written at all.
            let members: Vec<&Drawn> = run
                .iter()
                .filter(|d| intersect(d.bbox, page_box).is_some_and(|v| mask.iter().any(|m| overlaps(v, m.2))))
                .collect();
            let Some(reach) = members.iter().filter_map(|d| intersect(d.bbox, page_box)).reduce(union) else {
                continue;
            };
            // Only rings whose box meets where the run can paint are written. Exact, not a heuristic: under
            // even-odd a ring whose box misses a point cannot contain it, so the clip is unchanged wherever
            // a member paints — and a mask path far away (e.g. on a hidden board) never reaches the file.
            c.save_state();
            for (mp, mxf, _) in mask.iter().filter(|m| overlaps(m.2, reach)) {
                emit_rings(&mut c, mp, mxf, &t);
            }
            c.clip_even_odd().end_path();
            for d in members {
                paint(&mut c, &mut gss, &mut knocks, &mut ids, d, &t);
            }
            c.restore_state();
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

/// One path's paint ops (knockout XObject or in-place fill/stroke), appended to the page content.
fn paint(
    c: &mut Content,
    gss: &mut Vec<Gs>,
    knocks: &mut Vec<Knock>,
    ids: &mut Alloc,
    d: &Drawn,
    t: &impl Fn([f32; 2]) -> (f32, f32),
) {
    let Drawn { p, xf, fill, stroke, fillable, strokable, bbox, .. } = *d;
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
        emit_rings(&mut ic, p, &xf, t);
        ic.fill_even_odd().restore_state();
        ic.save_state()
            .set_parameters(Name(b"Gk"))
            .set_stroke_rgb(stroke[0], stroke[1], stroke[2])
            .set_line_width(p.stroke_width);
        emit_rings(&mut ic, p, &xf, t);
        ic.stroke().restore_state();
        let xr = ids.next();
        let bb = page_bbox(bbox, t);
        // paint site: object opacity applied ONCE to the whole unit
        let n = gs_name(gss, ids, p.opacity, p.opacity);
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
        let n = gs_name(gss, ids, fa, sa);
        c.set_parameters(Name(n.as_bytes()));
        if let (true, Some(f)) = (fillable, fill) {
            c.set_fill_rgb(f[0], f[1], f[2]);
        }
        if let (true, Some(s)) = (strokable, stroke) {
            c.set_stroke_rgb(s[0], s[1], s[2]);
            c.set_line_width(p.stroke_width);
        }
        emit_rings(c, p, &xf, t);
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

/// A page's WORLD rect as (x0, y0, x1, y1).
fn page_rect(page: &PageSpec) -> WRect {
    let [ax, ay, aw, ah] = page.rect;
    (ax, ay, ax + aw, ay + ah)
}
/// Do two WORLD boxes touch (edges inclusive, as the page cull always was)?
fn overlaps(a: WRect, b: WRect) -> bool {
    a.0 <= b.2 && a.2 >= b.0 && a.1 <= b.3 && a.3 >= b.1
}
fn intersect(a: WRect, b: WRect) -> Option<WRect> {
    overlaps(a, b).then(|| (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3)))
}
fn union(a: WRect, b: WRect) -> WRect {
    (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3))
}
/// A WORLD box in page space (for the Form XObject /BBox), corners ordered lower-left/upper-right.
fn page_bbox((x0, y0, x1, y1): WRect, t: &impl Fn([f32; 2]) -> (f32, f32)) -> [f32; 4] {
    let (ax0, ay0) = t([x0, y0]);
    let (ax1, ay1) = t([x1, y1]);
    [ax0.min(ax1), ay0.min(ay1), ax0.max(ax1), ay0.max(ay1)]
}
