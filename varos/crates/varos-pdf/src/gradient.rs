//! Lane B: axial/radial shadings with sampled midpoint/spread functions and luminosity alpha masks.
use super::write::{self, Alloc, Drawn};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use varos_core::{
    gradient::{Gradient, GradientKind, Spread},
    model::{Document, Paint},
};
pub(super) struct Shading {
    gradient: Gradient,
    bounds: [f32; 4],
    opacity: f32,
    pub colour: Option<Ref>,
    pub form: Option<Ref>,
    pub state: Option<Ref>,
}
pub(super) type Pool = Vec<Shading>;
fn shading(pdf: &mut Pdf, ids: &mut Alloc, g: &Gradient, domain: [f32; 2], alpha: bool) -> Ref {
    let r = ids.next();
    let f = ids.next();
    let count = 4096;
    let mut bytes = Vec::new();
    for i in 0..count {
        let t = domain[0] + (domain[1] - domain[0]) * i as f32 / (count - 1) as f32;
        let c = g.sample(t);
        let channels: Vec<_> = if alpha { vec![c[3]] } else { c[..3].to_vec() };
        for v in channels {
            bytes.extend(((v * 65535.).round().clamp(0., 65535.) as u16).to_be_bytes());
        }
    }
    let range = if alpha { vec![0., 1.] } else { vec![0., 1., 0., 1., 0., 1.] };
    pdf.sampled_function(f, &bytes)
        .domain([0., 1.])
        .range(range.clone())
        .size([count])
        .bits_per_sample(16)
        .encode([0., (count - 1) as f32])
        .decode(range);
    let mut d = pdf.indirect(r).dict();
    d.pair(Name(b"ShadingType"), if g.kind == GradientKind::Linear { 2 } else { 3 })
        .pair(Name(b"ColorSpace"), Name(if alpha { b"DeviceGray" } else { b"DeviceRGB" }))
        .pair(Name(b"Function"), f);
    d.insert(Name(b"Domain")).array().items([0., 1.]);
    d.insert(Name(b"Extend")).array().items([true, true]);
    let coords = if g.kind == GradientKind::Linear {
        vec![domain[0], 0., domain[1], 0.]
    } else {
        vec![g.focal[0], g.focal[1], 0., g.focal[0] * (1. - domain[1]), g.focal[1] * (1. - domain[1]), domain[1]]
    };
    d.insert(Name(b"Coords")).array().items(coords);
    r
}
fn register(
    pdf: &mut Pdf,
    ids: &mut Alloc,
    pool: &mut Pool,
    g: Gradient,
    bounds: [f32; 4],
    opacity: f32,
    t: &impl Fn([f32; 2]) -> (f32, f32),
) -> usize {
    if let Some(i) = pool.iter().position(|s| s.gradient == g && s.bounds == bounds && s.opacity == opacity) {
        return i;
    }
    let mut domain = [0., 1.];
    if g.spread != Spread::Pad {
        let vals = [[bounds[0], bounds[1]], [bounds[2], bounds[1]], [bounds[2], bounds[3]], [bounds[0], bounds[3]]]
            .map(|p| g.parameter(p));
        domain[0] = if g.kind == GradientKind::Radial { 0. } else { vals.into_iter().fold(0., f32::min).floor() };
        domain[1] = vals.into_iter().fold(1., f32::max).ceil();
    }
    let colour = shading(pdf, ids, &g, domain, false);
    let alpha = g.stops.iter().any(|s| s.colour[3] * s.opacity < 1.);
    let [a, b, c, d, e, f] = g.placement;
    let origin = t([e, f]);
    let x = t([e + a, f + b]);
    let y = t([e + c, f + d]);
    let matrix = [x.0 - origin.0, x.1 - origin.1, y.0 - origin.0, y.1 - origin.1, origin.0, origin.1];
    let state = if alpha || opacity < 1. {
        let state = ids.next();
        if alpha {
            let ar = shading(pdf, ids, &g, domain, true);
            let form = ids.next();
            let mut content = Content::new();
            content.transform(matrix).shading(Name(b"A"));
            let bbox = write::page_bbox((bounds[0], bounds[1], bounds[2], bounds[3]), t);
            let data = content.finish();
            let mut x = pdf.form_xobject(form, &data);
            x.bbox(Rect::new(bbox[0], bbox[1], bbox[2], bbox[3]));
            x.group().transparency().isolated(true).color_space().device_gray();
            x.resources().shadings().pair(Name(b"A"), ar);
            x.finish();
            let mut gs = pdf.ext_graphics(state);
            gs.non_stroking_alpha(opacity);
            gs.soft_mask().subtype(pdf_writer::types::MaskType::Luminosity).group(form);
        } else {
            pdf.ext_graphics(state).non_stroking_alpha(opacity);
        }
        Some(state)
    } else {
        None
    };
    pool.push(Shading { gradient: g, bounds, opacity, colour: Some(colour), state, form: None });
    pool.len() - 1
}
#[allow(clippy::too_many_arguments)]
pub(super) fn paint(
    doc: &Document,
    d: &Drawn<'_>,
    c: &mut Content,
    pdf: &mut Pdf,
    ids: &mut Alloc,
    pool: &mut Pool,
    // ---- w3-cmyk ----
    colours: &crate::colour_management::Resources,
    t: &impl Fn([f32; 2]) -> (f32, f32),
) -> bool {
    let paints = [d.p.appearance().fill().resolved(doc), d.p.appearance().stroke().resolved(doc)];
    if !paints.iter().any(|p| matches!(p, Paint::Gradient(_))) {
        return false;
    }
    let mut local = Content::new();
    let mut used = std::collections::BTreeSet::new();
    for (slot, paint) in paints.into_iter().enumerate() {
        if !paint.is_painted() || (slot == 0 && d.p.anchors.len() < 3) || (slot == 1 && d.p.stroke_width <= 0.) {
            continue;
        }
        let c = &mut local;
        c.save_state();
        if slot == 0 {
            write::emit_rings(c, d.p, &d.xf, t);
        } else {
            write::emit_coverage(c, d.p, &d.xf, t);
        }
        match paint {
            Paint::Gradient(g) => {
                c.clip_even_odd().end_path();
                let g = g.transformed(d.xf);
                let i = register(pdf, ids, pool, g.clone(), [d.bbox.0, d.bbox.1, d.bbox.2, d.bbox.3], 1.0, t);
                used.insert(i);
                if pool[i].state.is_some() {
                    c.set_parameters(Name(format!("GrGS{i}").as_bytes()));
                }
                let [a, b, cc, dd, e, f] = g.placement;
                let o = t([e, f]);
                let x = t([e + a, f + b]);
                let y = t([e + cc, f + dd]);
                c.transform([x.0 - o.0, x.1 - o.1, y.0 - o.0, y.1 - o.1, o.0, o.1])
                    .shading(Name(format!("Gr{i}").as_bytes()));
            }
            // ---- w3-cmyk ----
            source @ (Paint::Solid(_) | Paint::Managed(_)) => {
                let Some(s) = source.representative() else { continue };
                let gs = ids.next();
                pdf.ext_graphics(gs).non_stroking_alpha(s[3]);
                let i = pool.len();
                pool.push(Shading {
                    gradient: Gradient::default(),
                    bounds: [f32::NAN; 4],
                    opacity: 0.,
                    colour: None,
                    form: None,
                    state: Some(gs),
                });
                used.insert(i);
                c.set_parameters(Name(format!("GrGS{i}").as_bytes()));
                // ---- w3-cmyk ----
                crate::colour_management::set(c, doc, &source, s, false);
                c.fill_even_odd();
            }
            _ => {}
        }
        c.restore_state();
    }
    // Isolated knockout preserves the established appearance semantics: the stroke replaces
    // its own fill, and object opacity is applied exactly once to the combined artwork.
    let form = ids.next();
    let bbox = write::page_bbox(d.bbox, t);
    let data = local.finish();
    // ---- w3-cmyk ----
    let mut x = pdf.form_xobject(form, &data);
    x.bbox(Rect::new(bbox[0], bbox[1], bbox[2], bbox[3]));
    x.group().transparency().isolated(true).knockout(true).color_space().device_rgb();
    {
        let mut res = x.resources();
        // ---- w3-cmyk ----
        if !colours.spaces.is_empty() {
            let mut spaces = res.color_spaces();
            for (name, r) in &colours.spaces {
                spaces.pair(Name(name.as_bytes()), *r);
            }
        }
        {
            let mut sh = res.shadings();
            // Include cached shadings too: register can reuse an earlier slot.
            for &i in &used {
                let entry = &pool[i];
                if let Some(colour) = entry.colour {
                    sh.pair(Name(format!("Gr{i}").as_bytes()), colour);
                }
            }
        }
        let mut gs = res.ext_g_states();
        for &i in &used {
            let entry = &pool[i];
            if let Some(state) = entry.state {
                gs.pair(Name(format!("GrGS{i}").as_bytes()), state);
            }
        }
    }
    x.finish();
    let state = ids.next();
    pdf.ext_graphics(state).non_stroking_alpha(d.p.opacity);
    let i = pool.len();
    pool.push(Shading {
        gradient: Gradient::default(),
        bounds: [f32::NAN; 4],
        opacity: 0.,
        colour: None,
        form: Some(form),
        state: Some(state),
    });
    c.save_state()
        .set_parameters(Name(format!("GrGS{i}").as_bytes()))
        .x_object(Name(format!("GrForm{i}").as_bytes()))
        .restore_state();
    true
}

/// Bound sampled streams before allocating PDF objects. The conservative estimate also covers
/// masks and per-object forms, and never depends on a reader discovering an oversized saved file.
pub(super) fn check_budget(doc: &Document, pages: usize) -> Result<(), crate::ExportError> {
    let count = doc
        .paths
        .iter()
        .flat_map(|p| [p.appearance().fill(), p.appearance().stroke()])
        .filter(|p| matches!(p.resolved(doc), Paint::Gradient(_)))
        .count();
    let bytes = count.saturating_mul(pages).saturating_mul(40 * 1024);
    if bytes > varos_core::format::Limits::DEFAULT.max_decoded_stream_bytes {
        Err(crate::ExportError::LimitExceeded)
    } else {
        Ok(())
    }
}
