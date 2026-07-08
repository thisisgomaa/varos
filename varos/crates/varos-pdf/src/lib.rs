//! `.vrs` = a VALID PDF container (one page per artboard, artwork rendered by any viewer) with the
//! editable varos-core model embedded as an associated file — the `.ai` pattern (SAVE_EXPORT_PLAN §1).
//! Write = `pdf-writer`, read-back = `lopdf`. PDF semantics live ONLY in this crate (§0 hedge: the
//! container can be swapped with zero model rewrite).
//!
//! Mapping decisions (design-reviewed 2026-07-02):
//! - World→page flips CPU-side (X = wx−ab.x, Y = ab.y+ab.h−wy); identity CTM, MediaBox [0 0 w h].
//!   Cubics are affine-invariant → control points use the same map.
//! - Closed paths emit the WRAP-AROUND cubic explicitly before `h` (h alone would straighten it).
//! - Fills are `f*` (even-odd) with holes as subpaths of the SAME path; plain fill+stroke is one `B*`.
//! - Alphas: /ca = fill.a·opacity, /CA = stroke.a·opacity via pooled ExtGStates.
//! - Varos knockout semantics (translucent stroke must not blend over its own fill) map to a Form
//!   XObject transparency group /I true /K true — emitted ONLY when fill+stroke exist and the
//!   effective stroke alpha < 1. Known gap: pdf.js (Firefox) ignores /K → slightly darker band there.
//! - The model blob ({"varos":1,…}) embeds via EmbeddedFiles name tree + /AF (AFRelationship=Source)
//!   + catalog /VAROS_Model + /VAROS_SchemaVersion; read prefers /VAROS_Model, falls back to the tree.

use std::path::Path as FsPath;

use pdf_writer::types::{AssociationKind, LineCapStyle, LineJoinStyle};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use varos_core::file::{doc_from_blob, doc_to_blob, write_atomic, VRS_VERSION};
use varos_core::model::{Anchor, Artboard, Document, Path};

const MODEL_NAME: &[u8] = b"model.varos.json";

// ───────────────────────────── public API (drop-in for varos_core::file) ─────────────────────────────

/// Save the document as a `.vrs` PDF container, atomically.
pub fn save_vrs(doc: &Document, path: &FsPath) -> Result<(), String> {
    write_atomic(path, &write_pdf(doc)?)
}

/// Load a `.vrs`: a PDF container (the model blob is recovered from inside), or a legacy raw-JSON
/// `.vrs` from the first slice (sniffed by the missing `%PDF-` header).
pub fn load_vrs(path: &FsPath) -> Result<Document, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read failed: {e}"))?;
    if bytes.starts_with(b"%PDF-") {
        doc_from_blob(&extract_model(&bytes)?)
    } else {
        let s = String::from_utf8(bytes).map_err(|_| "not a valid .vrs".to_string())?;
        doc_from_blob(&s)
    }
}

// ───────────────────────────── write: Document → PDF bytes ─────────────────────────────

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

pub fn write_pdf(doc: &Document) -> Result<Vec<u8>, String> {
    let blob = doc_to_blob(doc)?;
    let mut ids = Alloc(0);
    let cat_id = ids.next();
    let tree_id = ids.next();
    let emb_id = ids.next();
    let fs_id = ids.next();

    // One page per VISIBLE board — a hidden board (board eye OFF) exports NO page, matching the canvas
    // (Ahmed 07-06 export gap). An artboard-less doc still saves on its default frame; and if EVERY
    // board is hidden we keep the first as a single frame so the container never degrades to a
    // zero-page (invalid) PDF.
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

    let mut pdf = Pdf::new();
    let mut page_ids = Vec::new();

    for ab in &boards {
        // world → page (Y-flip, artboard-local, identity CTM)
        let t = |p: [f32; 2]| -> (f32, f32) { (p[0] - ab.x, (ab.y + ab.h) - p[1]) };

        let mut c = Content::new();
        c.set_line_cap(LineCapStyle::RoundCap).set_line_join(LineJoinStyle::RoundJoin); // screen parity
        let mut gss: Vec<Gs> = Vec::new();
        let mut knocks: Vec<Knock> = Vec::new();

        // page background (transparent pages emit nothing — viewers show their own backdrop, like .ai)
        if let Some(bg) = ab.page_color {
            c.save_state();
            if bg[3] < 0.999 {
                let n = gs_name(&mut gss, &mut ids, bg[3], bg[3]);
                c.set_parameters(Name(n.as_bytes()));
            }
            c.set_fill_rgb(bg[0], bg[1], bg[2]);
            c.rect(0.0, 0.0, ab.w, ab.h);
            c.fill_even_odd();
            c.restore_state();
        }

        // artwork: the paintable content in document order (paint_list, LAYERS_VISION §5 — a mask
        // source must never reach the page); conservative bbox cull per page
        for (_, p) in doc.paint_list() {
            // WYSIWYG with the canvas: skip anything EFFECTIVELY hidden — the path's own eye, a hidden
            // parent group/layer (node cascade), OR art whose every member board is hidden (board eye).
            // Raw `p.hidden` missed the last two, so hidden groups and hidden pages still bled out.
            if doc.eff_hidden(p.id) {
                continue;
            }
            // resolve each paint to its drawable solid ONCE (Paint::None — and future gradients — ⇒ None)
            let (fill, stroke) = (p.fill.solid(), p.stroke.solid());
            // WYSIWYG with the canvas: an OPEN path still FILLS (implied straight close between endpoints,
            // A32) — the exact rule `scene::fill_prims` draws by. The old `p.closed` guard dropped the fill
            // of any shape a deleted anchor had opened, so it filled on screen but vanished in the PDF (FB1).
            let fillable = p.anchors.len() >= 3 && fill.is_some();
            let strokable = stroke.is_some() && p.anchors.len() >= 2 && p.stroke_width > 0.0;
            if !fillable && !strokable {
                continue;
            }
            let pad = if strokable { p.stroke_width * 0.5 } else { 0.0 };
            if !bbox_hits(p, pad, ab) {
                continue;
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
                emit_rings(&mut ic, p, &t);
                ic.fill_even_odd().restore_state();
                ic.save_state()
                    .set_parameters(Name(b"Gk"))
                    .set_stroke_rgb(stroke[0], stroke[1], stroke[2])
                    .set_line_width(p.stroke_width);
                emit_rings(&mut ic, p, &t);
                ic.stroke().restore_state();
                let xr = ids.next();
                let bb = page_bbox(p, pad, &t);
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
                emit_rings(&mut c, p, &t);
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

        // page objects: content stream → page (+ resources) → gs objects → knockout xobjects
        let cont_id = ids.next();
        pdf.stream(cont_id, &c.finish());
        let page_id = ids.next();
        let mut page = pdf.page(page_id);
        page.parent(tree_id).media_box(Rect::new(0.0, 0.0, ab.w, ab.h)).contents(cont_id);
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
fn emit_rings(c: &mut Content, p: &Path, t: &impl Fn([f32; 2]) -> (f32, f32)) {
    emit_ring(c, &p.anchors, p.closed, t);
    for hole in &p.holes {
        emit_ring(c, hole, true, t);
    }
}
fn emit_ring(c: &mut Content, anchors: &[Anchor], closed: bool, t: &impl Fn([f32; 2]) -> (f32, f32)) {
    let n = anchors.len();
    if n < 2 {
        return;
    }
    let p0 = t(anchors[0].p);
    c.move_to(p0.0, p0.1);
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let a = &anchors[i];
        let b = &anchors[(i + 1) % n];
        let c1 = t(a.hout.unwrap_or(a.p));
        let c2 = t(b.hin.unwrap_or(b.p));
        let p3 = t(b.p);
        c.cubic_to(c1.0, c1.1, c2.0, c2.1, p3.0, p3.1);
    }
    if closed {
        c.close_path();
    }
}

/// Conservative world bbox (anchors + handles of all rings) inflated by `pad`; hits the artboard?
fn bbox_hits(p: &Path, pad: f32, ab: &Artboard) -> bool {
    let (x0, y0, x1, y1) = world_bbox(p, pad);
    x0 <= ab.x + ab.w && x1 >= ab.x && y0 <= ab.y + ab.h && y1 >= ab.y
}
fn world_bbox(p: &Path, pad: f32) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for a in p.anchors.iter().chain(p.holes.iter().flatten()) {
        for q in [Some(a.p), a.hin, a.hout].into_iter().flatten() {
            x0 = x0.min(q[0]);
            y0 = y0.min(q[1]);
            x1 = x1.max(q[0]);
            y1 = y1.max(q[1]);
        }
    }
    (x0 - pad, y0 - pad, x1 + pad, y1 + pad)
}
/// The same bbox in page space (for the Form XObject /BBox), corners ordered lower-left/upper-right.
fn page_bbox(p: &Path, pad: f32, t: &impl Fn([f32; 2]) -> (f32, f32)) -> [f32; 4] {
    let (x0, y0, x1, y1) = world_bbox(p, pad);
    let (ax0, ay0) = t([x0, y0]);
    let (ax1, ay1) = t([x1, y1]);
    [ax0.min(ax1), ay0.min(ay1), ax0.max(ax1), ay0.max(ay1)]
}

// ───────────────────────────── read: PDF bytes → model blob ─────────────────────────────

fn extract_model(bytes: &[u8]) -> Result<String, String> {
    let doc = lopdf::Document::load_mem(bytes).map_err(|e| format!("not a readable PDF: {e}"))?;
    let catalog = doc.catalog().map_err(|e| format!("no PDF catalog: {e}"))?;

    let stream_bytes = |obj: &lopdf::Object| -> Result<Vec<u8>, String> {
        let (_, o) = doc.dereference(obj).map_err(|e| e.to_string())?;
        let s = o.as_stream().map_err(|e| e.to_string())?;
        Ok(s.decompressed_content().unwrap_or_else(|_| s.content.clone()))
    };

    // fast path: the private catalog key Varos writes
    if let Ok(obj) = catalog.get(b"VAROS_Model") {
        let data = stream_bytes(obj)?;
        return String::from_utf8(data).map_err(|_| "embedded model is not UTF-8".into());
    }
    // fallback: /Names → /EmbeddedFiles name tree → FileSpec /EF /F (survives third-party re-saves better)
    fn collect(doc: &lopdf::Document, node: &lopdf::Dictionary, out: &mut Vec<lopdf::Object>) {
        if let Ok(pairs) = node.get(b"Names") {
            if let Ok((_, o)) = doc.dereference(pairs) {
                if let Ok(arr) = o.as_array() {
                    for kv in arr.chunks(2) {
                        if let [_k, v] = kv {
                            out.push(v.clone());
                        }
                    }
                }
            }
        } else if let Ok(kids) = node.get(b"Kids") {
            if let Ok((_, o)) = doc.dereference(kids) {
                if let Ok(arr) = o.as_array() {
                    for kid in arr {
                        if let Ok((_, kd)) = doc.dereference(kid) {
                            if let Ok(d) = kd.as_dict() {
                                collect(doc, d, out);
                            }
                        }
                    }
                }
            }
        }
    }
    let names = catalog
        .get_deref(b"Names", &doc)
        .and_then(|o| o.as_dict())
        .map_err(|_| "no embedded Varos model in this PDF".to_string())?;
    let root = names
        .get_deref(b"EmbeddedFiles", &doc)
        .and_then(|o| o.as_dict())
        .map_err(|_| "no embedded Varos model in this PDF".to_string())?;
    let mut specs = Vec::new();
    collect(&doc, root, &mut specs);
    for spec in specs {
        let Ok((_, so)) = doc.dereference(&spec) else { continue };
        let Ok(sd) = so.as_dict() else { continue };
        let Ok(ef) = sd.get_deref(b"EF", &doc).and_then(|o| o.as_dict()) else { continue };
        if let Ok(f) = ef.get(b"F").or_else(|_| ef.get(b"UF")) {
            if let Ok(data) = stream_bytes(f) {
                if let Ok(s) = String::from_utf8(data) {
                    return Ok(s);
                }
            }
        }
    }
    Err("no embedded Varos model in this PDF".into())
}
